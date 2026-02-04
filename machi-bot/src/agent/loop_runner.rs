//! Agent loop runner - the core message processing engine.

use super::confirmation::{
    CliConfirmationHandler, ConfirmationHandler, ConfirmationManager, ConfirmationRequest,
    ConfirmationResponse, TelegramConfirmationHandler,
};
use super::context::ContextBuilder;
use crate::bus::MessageBus;
use crate::config::{ExecConfig, ToolPoliciesConfig, ToolPolicy};
use crate::error::{BotError, Result};
use crate::events::{InboundMessage, OutboundMessage};
use crate::session::{MemoryStorage, SessionManager};

use async_trait::async_trait;
use machi::prelude::*;
use machi::tool::{
    ConfirmationHandler as MachiConfirmationHandler, ToolConfirmationRequest,
    ToolConfirmationResponse, ToolExecutionPolicy,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

/// Configuration for the agent loop.
#[derive(Debug, Clone)]
pub struct AgentLoopConfig {
    /// Maximum iterations per message.
    pub max_iterations: usize,
    /// Timeout for processing a single message.
    pub message_timeout: Duration,
    /// Workspace directory.
    pub workspace: PathBuf,
    /// Exec tool configuration.
    pub exec_config: ExecConfig,
    /// Tool execution policies.
    pub tool_policies: ToolPoliciesConfig,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: 20,
            message_timeout: Duration::from_secs(300),
            workspace: default_workspace(),
            exec_config: ExecConfig::default(),
            tool_policies: ToolPoliciesConfig::default(),
        }
    }
}

fn default_workspace() -> PathBuf {
    dirs_next::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".machi-bot")
        .join("workspace")
}

/// The agent loop processes messages from the bus using machi agents.
pub struct AgentLoop<M: Model + Clone + Send + Sync + 'static> {
    bus: MessageBus,
    model: M,
    config: AgentLoopConfig,
    sessions: SessionManager,
    #[allow(dead_code)] // Will be used for advanced context building
    context: ContextBuilder,
    running: Arc<RwLock<bool>>,
    confirmation_manager: Arc<ConfirmationManager>,
}

impl<M: Model + Clone + Send + Sync + 'static> std::fmt::Debug for AgentLoop<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentLoop")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl<M: Model + Clone + Send + Sync + 'static> AgentLoop<M> {
    /// Create a new agent loop with the given model.
    pub fn new(bus: MessageBus, model: M) -> Self {
        let config = AgentLoopConfig::default();
        Self {
            bus,
            model,
            sessions: SessionManager::new(MemoryStorage::new()),
            context: ContextBuilder::new(&config.workspace),
            config,
            running: Arc::new(RwLock::new(false)),
            confirmation_manager: Arc::new(ConfirmationManager::new()),
        }
    }

    /// Create an agent loop with custom configuration.
    pub fn with_config(bus: MessageBus, model: M, config: AgentLoopConfig) -> Self {
        Self {
            sessions: SessionManager::new(MemoryStorage::new()),
            context: ContextBuilder::new(&config.workspace),
            bus,
            model,
            config,
            running: Arc::new(RwLock::new(false)),
            confirmation_manager: Arc::new(ConfirmationManager::new()),
        }
    }

    /// Create an agent loop with a shared confirmation manager.
    pub fn with_confirmation_manager(
        bus: MessageBus,
        model: M,
        config: AgentLoopConfig,
        manager: Arc<ConfirmationManager>,
    ) -> Self {
        Self {
            sessions: SessionManager::new(MemoryStorage::new()),
            context: ContextBuilder::new(&config.workspace),
            bus,
            model,
            config,
            running: Arc::new(RwLock::new(false)),
            confirmation_manager: manager,
        }
    }

    /// Get the confirmation manager.
    #[must_use]
    pub fn confirmation_manager(&self) -> &Arc<ConfirmationManager> {
        &self.confirmation_manager
    }

    /// Convert bot ToolPolicy to machi ToolExecutionPolicy.
    fn to_machi_policy(policy: &ToolPolicy) -> ToolExecutionPolicy {
        match policy {
            ToolPolicy::Auto => ToolExecutionPolicy::Auto,
            ToolPolicy::RequireConfirmation => ToolExecutionPolicy::RequireConfirmation,
            ToolPolicy::Forbidden => ToolExecutionPolicy::Forbidden,
        }
    }

    /// Create a confirmation handler for the given channel.
    fn create_confirmation_handler(
        &self,
        channel: &str,
    ) -> Box<dyn MachiConfirmationHandler> {
        match channel {
            "cli" => Box::new(ChannelConfirmationAdapter::new(
                CliConfirmationHandler,
                self.config.tool_policies.confirmation_timeout,
            )),
            "telegram" => Box::new(ChannelConfirmationAdapter::new(
                TelegramConfirmationHandler::new(Arc::clone(&self.confirmation_manager)),
                self.config.tool_policies.confirmation_timeout,
            )),
            _ => Box::new(ChannelConfirmationAdapter::new(
                CliConfirmationHandler,
                self.config.tool_policies.confirmation_timeout,
            )),
        }
    }

    /// Run the agent loop, processing messages from the bus.
    pub async fn run(&self) -> Result<()> {
        *self.running.write().await = true;
        info!("Agent loop started");

        while *self.running.read().await {
            // Wait for next message with timeout
            let Some(msg) = self
                .bus
                .consume_inbound_timeout(Duration::from_secs(1))
                .await
            else {
                continue;
            };

            // Process the message
            match self.process_message(&msg).await {
                Ok(response) => {
                    if let Err(e) = self.bus.publish_outbound(response).await {
                        error!(error = %e, "failed to publish response");
                    }
                }
                Err(e) => {
                    error!(error = %e, "failed to process message");
                    // Send error response
                    let error_response = OutboundMessage::reply_to(
                        &msg,
                        format!("Sorry, I encountered an error: {e}"),
                    );
                    let _ = self.bus.publish_outbound(error_response).await;
                }
            }
        }

        info!("Agent loop stopped");
        Ok(())
    }

    /// Stop the agent loop.
    pub async fn stop(&self) {
        *self.running.write().await = false;
    }

    /// Check if the loop is running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Process a single inbound message.
    async fn process_message(&self, msg: &InboundMessage) -> Result<OutboundMessage> {
        debug!(
            channel = %msg.channel,
            sender = %msg.sender_id,
            "processing message"
        );

        // Get or create session
        let mut session = self
            .sessions
            .get_or_create(&msg.session_key())
            .await
            .map_err(|e| BotError::agent(e.to_string()))?;

        // Build agent with tools and policies
        let mut builder = Agent::builder()
            .model(self.model.clone())
            .max_steps(self.config.max_iterations)
            .add_base_tools();

        // Apply tool execution policies from config
        let _default_policy = Self::to_machi_policy(&self.config.tool_policies.default_policy);
        // TODO: Apply default policy to all tools when machi supports it
        for (tool_name, policy) in &self.config.tool_policies.tools {
            builder = builder.tool_policy(tool_name.clone(), Self::to_machi_policy(policy));
        }

        // Set confirmation handler based on channel
        let confirmation_handler = self.create_confirmation_handler(&msg.channel);
        builder = builder.confirmation_handler(confirmation_handler);

        let mut agent = builder.build();

        // Run agent with the message
        let result = agent
            .run(&msg.content)
            .await
            .map_err(|e| BotError::agent(e.to_string()))?;

        // Extract response content from Value
        let response_content = match result {
            serde_json::Value::String(s) => s,
            other => other.to_string(),
        };

        // Update session
        session.add_user_message(&msg.content);
        session.add_assistant_message(&response_content);
        self.sessions
            .save(&mut session)
            .await
            .map_err(|e| BotError::agent(e.to_string()))?;

        Ok(OutboundMessage::reply_to(msg, response_content))
    }

    /// Process a message directly (for CLI usage).
    pub async fn process_direct(&self, content: &str) -> Result<String> {
        let msg = InboundMessage::cli(content);
        let response = self.process_message(&msg).await?;
        Ok(response.content)
    }
}

/// Builder for creating an AgentLoop.
pub struct AgentLoopBuilder<M: Model + Clone + Send + Sync + 'static> {
    bus: Option<MessageBus>,
    model: Option<M>,
    config: AgentLoopConfig,
}

impl<M: Model + Clone + Send + Sync + 'static> std::fmt::Debug for AgentLoopBuilder<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentLoopBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl<M: Model + Clone + Send + Sync + 'static> Default for AgentLoopBuilder<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: Model + Clone + Send + Sync + 'static> AgentLoopBuilder<M> {
    /// Create a new builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bus: None,
            model: None,
            config: AgentLoopConfig::default(),
        }
    }

    /// Set the message bus.
    #[must_use]
    pub fn bus(mut self, bus: MessageBus) -> Self {
        self.bus = Some(bus);
        self
    }

    /// Set the model.
    #[must_use]
    pub fn model(mut self, model: M) -> Self {
        self.model = Some(model);
        self
    }

    /// Set max iterations.
    #[must_use]
    pub const fn max_iterations(mut self, max: usize) -> Self {
        self.config.max_iterations = max;
        self
    }

    /// Set the workspace path.
    #[must_use]
    pub fn workspace(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.workspace = path.into();
        self
    }

    /// Set message timeout.
    #[must_use]
    pub const fn message_timeout(mut self, timeout: Duration) -> Self {
        self.config.message_timeout = timeout;
        self
    }

    /// Build the agent loop.
    ///
    /// # Panics
    ///
    /// Panics if bus or model is not set.
    #[must_use]
    pub fn build(self) -> AgentLoop<M> {
        let bus = self.bus.expect("bus is required");
        let model = self.model.expect("model is required");

        AgentLoop::with_config(bus, model, self.config)
    }
}

/// Adapter to convert machi-bot's ConfirmationHandler to machi's ConfirmationHandler trait.
///
/// This bridges the gap between the two confirmation systems, allowing
/// channel-specific handlers (CLI, Telegram) to be used with machi's agent.
struct ChannelConfirmationAdapter<H: ConfirmationHandler + Send + Sync> {
    handler: H,
    timeout: u64,
}

impl<H: ConfirmationHandler + Send + Sync> ChannelConfirmationAdapter<H> {
    /// Create a new adapter with the given handler and timeout.
    fn new(handler: H, timeout: u64) -> Self {
        Self { handler, timeout }
    }
}

#[async_trait]
impl<H: ConfirmationHandler + Send + Sync + 'static> MachiConfirmationHandler
    for ChannelConfirmationAdapter<H>
{
    async fn confirm(&self, request: &ToolConfirmationRequest) -> ToolConfirmationResponse {
        // Convert machi's request to our request format
        let our_request = ConfirmationRequest::new(
            request.name.clone(),
            request.arguments.clone(),
            "unknown", // Channel will be determined by the handler
            "unknown", // Session key
        )
        .with_description(&request.description);

        // Call our handler
        let timeout = Duration::from_secs(self.timeout);
        let response = self.handler.confirm(&our_request, timeout).await;

        // Convert our response back to machi's format
        match response {
            ConfirmationResponse::Approved => ToolConfirmationResponse::Approved,
            ConfirmationResponse::Denied => ToolConfirmationResponse::Denied,
            ConfirmationResponse::ApproveAll => ToolConfirmationResponse::ApproveAll,
            ConfirmationResponse::Timeout => ToolConfirmationResponse::Denied,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AgentLoopConfig::default();
        assert_eq!(config.max_iterations, 20);
        assert!(config.workspace.ends_with("workspace"));
    }
}
