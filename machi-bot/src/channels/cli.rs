//! Command-line interface channel implementation.
//!
//! The CLI channel provides a simple text-based interface for interacting
//! with the bot through standard input/output.

use crate::bus::MessageBus;
use crate::channel::{Channel, ChannelBase, ChannelState, ChannelStatus};
use crate::error::{ChannelError, ChannelResult};
use crate::events::{InboundMessage, OutboundMessage};
use async_trait::async_trait;
use std::io::{self, BufRead, Write};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, info};

/// CLI channel configuration.
#[derive(Debug, Clone)]
pub struct CliChannelConfig {
    /// Prompt string to display before user input.
    pub prompt: String,
    /// Whether to echo user input.
    pub echo_input: bool,
    /// Session identifier for this CLI session.
    pub session_id: String,
}

impl Default for CliChannelConfig {
    fn default() -> Self {
        Self {
            prompt: "> ".to_string(),
            echo_input: false,
            session_id: "cli".to_string(),
        }
    }
}

impl CliChannelConfig {
    /// Create a new CLI channel config.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the prompt string.
    #[must_use]
    pub fn prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = prompt.into();
        self
    }

    /// Set whether to echo user input.
    #[must_use]
    pub const fn echo_input(mut self, echo: bool) -> Self {
        self.echo_input = echo;
        self
    }

    /// Set the session ID.
    #[must_use]
    pub fn session_id(mut self, id: impl Into<String>) -> Self {
        self.session_id = id.into();
        self
    }
}

/// Command-line interface channel.
///
/// This channel reads from stdin and writes to stdout, providing a simple
/// way to interact with the bot from the terminal.
///
/// # Example
///
/// ```rust,ignore
/// use machi_bot::channels::CliChannel;
/// use machi_bot::bus::MessageBus;
///
/// let bus = MessageBus::new();
/// let cli = CliChannel::new();
/// cli.start(&bus).await?;
/// ```
#[derive(Debug)]
pub struct CliChannel {
    base: ChannelBase,
    #[allow(dead_code)] // Config will be used for advanced features
    config: CliChannelConfig,
    shutdown_tx: RwLock<Option<mpsc::Sender<()>>>,
}

impl CliChannel {
    /// Create a new CLI channel with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(CliChannelConfig::default())
    }

    /// Create a new CLI channel with the given configuration.
    #[must_use]
    pub fn with_config(config: CliChannelConfig) -> Self {
        Self {
            base: ChannelBase::new("cli"),
            config,
            shutdown_tx: RwLock::new(None),
        }
    }

    /// Process a single line of input and publish to the bus.
    #[allow(dead_code)] // Will be used when implementing full stdin handling
    async fn process_input(&self, bus: &MessageBus, input: &str) -> ChannelResult<()> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        let msg = InboundMessage::new("cli", "user", &self.config.session_id, trimmed);

        self.base.record_received().await;
        bus.publish_inbound(msg)
            .await
            .map_err(|e| ChannelError::Internal(e.to_string()))?;

        Ok(())
    }

    /// Format and print an outbound message.
    #[allow(clippy::print_stdout)] // CLI channel intentionally prints to stdout
    fn print_message(msg: &OutboundMessage) {
        // All formats currently output as-is; HTML stripping could be added later
        let content = &msg.content;
        println!("\n{content}\n");
    }
}

impl Default for CliChannel {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Channel for CliChannel {
    fn name(&self) -> &str {
        self.base.name()
    }

    async fn start(&self, bus: &MessageBus) -> ChannelResult<()> {
        self.base.set_state(ChannelState::Starting).await;

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        *self.shutdown_tx.write().await = Some(shutdown_tx);

        // Subscribe to outbound messages
        let mut outbound_rx = bus.subscribe_channel("cli").await;

        // Clone what we need for the output task
        // Spawn output handler
        #[allow(clippy::print_stdout)] // CLI channel intentionally prints to stdout
        let _output_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(msg) = outbound_rx.recv() => {
                        let content = &msg.content;
                        println!("\n{content}\n");
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("CLI output handler shutting down");
                        break;
                    }
                }
            }
        });

        self.base.set_state(ChannelState::Running).await;
        info!("CLI channel started");

        Ok(())
    }

    async fn stop(&self) -> ChannelResult<()> {
        self.base.set_state(ChannelState::Stopping).await;

        // Send shutdown signal
        let guard = self.shutdown_tx.write().await;
        if let Some(tx) = &*guard {
            let _ = tx.send(()).await;
        }
        drop(guard);

        self.base.set_state(ChannelState::Stopped).await;
        info!("CLI channel stopped");

        Ok(())
    }

    async fn send(&self, msg: &OutboundMessage) -> ChannelResult<()> {
        Self::print_message(msg);
        self.base.record_sent().await;
        Ok(())
    }

    async fn status(&self) -> ChannelStatus {
        self.base.build_status().await
    }
}

/// Run an interactive CLI session.
///
/// This function blocks and runs an interactive session, reading from stdin
/// and waiting for responses.
///
/// # Arguments
///
/// * `bus` - The message bus to publish messages to
/// * `config` - CLI configuration
///
/// # Example
///
/// ```rust,ignore
/// use machi_bot::channels::cli::{run_interactive, CliChannelConfig};
/// use machi_bot::bus::MessageBus;
///
/// let bus = MessageBus::new();
/// run_interactive(&bus, CliChannelConfig::default()).await;
/// ```
#[allow(clippy::print_stdout)] // CLI intentionally prints to stdout
pub async fn run_interactive(bus: &MessageBus, config: CliChannelConfig) -> ChannelResult<()> {
    let prompt = config.prompt.clone();
    let session_id = config.session_id.clone();

    // Subscribe to outbound messages for this session
    let mut outbound_rx = bus.subscribe_channel("cli").await;

    // Spawn output handler
    let output_handle = tokio::spawn(async move {
        while let Some(msg) = outbound_rx.recv().await {
            println!("\n{}\n", msg.content);
            print!("{prompt}");
            let _ = io::stdout().flush();
        }
    });

    // Read input from stdin
    let stdin = io::stdin();
    let reader = stdin.lock();

    print!("{}", config.prompt);
    let _ = io::stdout().flush();

    for line in reader.lines() {
        let line = line.map_err(|e| ChannelError::Internal(e.to_string()))?;
        let trimmed = line.trim();

        // Handle exit commands
        if trimmed == "exit" || trimmed == "quit" || trimmed == "/quit" {
            break;
        }

        if trimmed.is_empty() {
            print!("{}", config.prompt);
            let _ = io::stdout().flush();
            continue;
        }

        // Publish message
        let msg = InboundMessage::new("cli", "user", &session_id, trimmed);
        bus.publish_inbound(msg)
            .await
            .map_err(|e| ChannelError::Internal(e.to_string()))?;
    }

    output_handle.abort();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cli_channel_lifecycle() {
        let channel = CliChannel::new();
        let bus = MessageBus::new();

        // Start
        channel.start(&bus).await.unwrap();
        assert!(channel.is_running().await);

        // Stop
        channel.stop().await.unwrap();
        let status = channel.status().await;
        assert_eq!(status.state, ChannelState::Stopped);
    }

    #[test]
    fn test_config_builder() {
        let config = CliChannelConfig::new()
            .prompt(">> ")
            .echo_input(true)
            .session_id("test");

        assert_eq!(config.prompt, ">> ");
        assert!(config.echo_input);
        assert_eq!(config.session_id, "test");
    }
}
