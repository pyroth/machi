//! Gateway service for running the complete bot.
//!
//! The gateway is the unified entry point that orchestrates:
//! - Message bus
//! - Channel manager (Telegram, CLI, etc.)
//! - Agent loop
//! - Session management

use crate::agent::{AgentLoop, AgentLoopConfig};
use crate::bus::MessageBus;
use crate::channel::ChannelManager;
use crate::channels::CliChannel;
use crate::config::{BotConfig, load_config};
use crate::error::{BotError, Result};

#[cfg(feature = "telegram")]
use crate::channels::{TelegramChannel, telegram::TelegramChannelConfig};

use machi::prelude::Model;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// Gateway configuration.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    /// Bot configuration.
    pub bot_config: BotConfig,
    /// Workspace directory.
    pub workspace: PathBuf,
    /// Whether to enable CLI channel.
    pub enable_cli: bool,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            bot_config: BotConfig::default(),
            workspace: default_workspace(),
            enable_cli: true,
        }
    }
}

fn default_workspace() -> PathBuf {
    dirs_next::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".machi-bot")
        .join("workspace")
}

/// Gateway service that runs the complete bot.
pub struct Gateway<M: Model + Clone + Send + Sync + 'static> {
    config: GatewayConfig,
    bus: MessageBus,
    channel_manager: ChannelManager,
    model: M,
    running: Arc<RwLock<bool>>,
}

impl<M: Model + Clone + Send + Sync + 'static> std::fmt::Debug for Gateway<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gateway")
            .field("config", &self.config)
            .field("bus", &self.bus)
            .finish_non_exhaustive()
    }
}

impl<M: Model + Clone + Send + Sync + 'static> Gateway<M> {
    /// Create a new gateway with the given model.
    pub fn new(model: M) -> Self {
        let bus = MessageBus::new();
        Self {
            config: GatewayConfig::default(),
            channel_manager: ChannelManager::new(bus.clone()),
            bus,
            model,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Create a gateway with custom configuration.
    pub fn with_config(model: M, config: GatewayConfig) -> Self {
        let bus = MessageBus::new();
        Self {
            channel_manager: ChannelManager::new(bus.clone()),
            bus,
            model,
            config,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Get a reference to the message bus.
    #[must_use]
    pub const fn bus(&self) -> &MessageBus {
        &self.bus
    }

    /// Get a reference to the channel manager.
    #[must_use]
    pub const fn channel_manager(&self) -> &ChannelManager {
        &self.channel_manager
    }

    /// Register channels based on configuration.
    async fn setup_channels(&self) -> Result<()> {
        // CLI channel
        if self.config.enable_cli {
            let cli = CliChannel::new();
            self.channel_manager.register(cli).await;
            info!("CLI channel enabled");
        }

        // Telegram channel
        #[cfg(feature = "telegram")]
        if self.config.bot_config.channels.telegram.enabled {
            if let Some(ref token) = self.config.bot_config.channels.telegram.token {
                let mut tg_config = TelegramChannelConfig::new(token);

                // Add allowed users
                for user_id_str in &self.config.bot_config.channels.telegram.allow_from {
                    if let Ok(user_id) = user_id_str.parse::<i64>() {
                        tg_config = tg_config.allow_user(user_id);
                    }
                }

                let telegram = TelegramChannel::new(tg_config);
                self.channel_manager.register(telegram).await;
                info!("Telegram channel enabled");
            } else {
                error!("Telegram enabled but no token configured");
            }
        }

        Ok(())
    }

    /// Run the gateway.
    ///
    /// This starts all channels and the agent loop, then waits for shutdown.
    pub async fn run(&self) -> Result<()> {
        *self.running.write().await = true;
        info!("Gateway starting...");

        // Setup channels
        self.setup_channels().await?;

        // Start all channels
        let channel_results = self.channel_manager.start_all().await;
        for result in &channel_results {
            if let Err(e) = result {
                error!(error = %e, "failed to start channel");
            }
        }

        // Create and run agent loop
        let agent_config = AgentLoopConfig {
            max_iterations: self.config.bot_config.agents.defaults.max_iterations,
            workspace: self.config.workspace.clone(),
            ..Default::default()
        };

        let agent_loop = AgentLoop::with_config(self.bus.clone(), self.model.clone(), agent_config);

        info!("Gateway started. Press Ctrl+C to stop.");

        // Run agent loop (this blocks until stopped)
        let result = agent_loop.run().await;

        // Cleanup
        info!("Gateway stopping...");
        self.channel_manager.stop_all().await;
        *self.running.write().await = false;

        info!("Gateway stopped");
        result
    }

    /// Check if the gateway is running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Get channel statuses.
    pub async fn status(&self) -> GatewayStatus {
        let channel_statuses = self.channel_manager.status_all().await;
        let bus_stats = self.bus.stats().await;

        GatewayStatus {
            running: *self.running.read().await,
            channels: channel_statuses
                .into_iter()
                .map(|s| ChannelStatusInfo {
                    name: s.name,
                    state: format!("{:?}", s.state),
                    messages_received: s.messages_received,
                    messages_sent: s.messages_sent,
                    healthy: s.healthy,
                })
                .collect(),
            total_inbound: bus_stats.inbound_count,
            total_outbound: bus_stats.outbound_count,
        }
    }
}

/// Gateway status information.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GatewayStatus {
    /// Whether the gateway is running.
    pub running: bool,
    /// Channel statuses.
    pub channels: Vec<ChannelStatusInfo>,
    /// Total inbound messages processed.
    pub total_inbound: u64,
    /// Total outbound messages processed.
    pub total_outbound: u64,
}

/// Channel status info for gateway status.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ChannelStatusInfo {
    /// Channel name.
    pub name: String,
    /// Channel state.
    pub state: String,
    /// Messages received.
    pub messages_received: u64,
    /// Messages sent.
    pub messages_sent: u64,
    /// Whether the channel is healthy.
    pub healthy: bool,
}

/// Builder for creating a Gateway.
pub struct GatewayBuilder<M: Model + Clone + Send + Sync + 'static> {
    model: Option<M>,
    config: GatewayConfig,
}

impl<M: Model + Clone + Send + Sync + 'static> std::fmt::Debug for GatewayBuilder<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl<M: Model + Clone + Send + Sync + 'static> Default for GatewayBuilder<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: Model + Clone + Send + Sync + 'static> GatewayBuilder<M> {
    /// Create a new builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            model: None,
            config: GatewayConfig::default(),
        }
    }

    /// Set the model.
    #[must_use]
    pub fn model(mut self, model: M) -> Self {
        self.model = Some(model);
        self
    }

    /// Set the bot configuration.
    #[must_use]
    pub fn bot_config(mut self, config: BotConfig) -> Self {
        self.config.bot_config = config;
        self
    }

    /// Set the workspace directory.
    #[must_use]
    pub fn workspace(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.workspace = path.into();
        self
    }

    /// Enable or disable CLI channel.
    #[must_use]
    pub const fn enable_cli(mut self, enable: bool) -> Self {
        self.config.enable_cli = enable;
        self
    }

    /// Load configuration from file.
    pub async fn load_config(mut self) -> Result<Self> {
        self.config.bot_config = load_config()
            .await
            .map_err(|e| BotError::config(e.to_string()))?;
        Ok(self)
    }

    /// Build the gateway.
    ///
    /// # Panics
    ///
    /// Panics if model is not set.
    #[must_use]
    pub fn build(self) -> Gateway<M> {
        let model = self.model.expect("model is required");
        Gateway::with_config(model, self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GatewayConfig::default();
        assert!(config.enable_cli);
        assert!(config.workspace.ends_with("workspace"));
    }
}
