//! Channel trait and base functionality for chat integrations.
//!
//! This module defines the core abstraction for chat channels, enabling
//! different messaging platforms (Telegram, WhatsApp, CLI, etc.) to
//! integrate with the bot framework.

use crate::bus::MessageBus;
use crate::error::ChannelResult;
use crate::events::OutboundMessage;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

/// Channel state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChannelState {
    /// Channel is not started.
    #[default]
    Stopped,
    /// Channel is starting up.
    Starting,
    /// Channel is running and connected.
    Running,
    /// Channel is stopping.
    Stopping,
    /// Channel encountered an error.
    Error,
}

/// Channel status information.
#[derive(Debug, Clone)]
pub struct ChannelStatus {
    /// Channel name.
    pub name: String,
    /// Current state.
    pub state: ChannelState,
    /// Number of messages received.
    pub messages_received: u64,
    /// Number of messages sent.
    pub messages_sent: u64,
    /// Last error message, if any.
    pub last_error: Option<String>,
    /// Whether the channel is healthy.
    pub healthy: bool,
}

/// Trait for implementing chat channels.
#[async_trait]
pub trait Channel: Send + Sync {
    /// Get the unique name of this channel.
    fn name(&self) -> &str;

    /// Start the channel and begin processing messages.
    ///
    /// The channel should:
    /// 1. Connect to the messaging platform
    /// 2. Subscribe to `bus.subscribe_channel(self.name())` for outbound messages
    /// 3. Spawn background tasks for message handling
    async fn start(&self, bus: &MessageBus) -> ChannelResult<()>;

    /// Stop the channel and cleanup resources.
    async fn stop(&self) -> ChannelResult<()>;

    /// Send an outbound message through this channel.
    async fn send(&self, msg: &OutboundMessage) -> ChannelResult<()>;

    /// Get the current channel status.
    async fn status(&self) -> ChannelStatus;

    /// Check if the channel is currently running.
    async fn is_running(&self) -> bool {
        self.status().await.state == ChannelState::Running
    }
}

/// Type alias for a boxed channel.
pub type BoxedChannel = Box<dyn Channel>;

/// Manager for multiple channels.
///
/// The channel manager handles the lifecycle of multiple channels and
/// provides a unified interface for starting, stopping, and monitoring them.
pub struct ChannelManager {
    channels: RwLock<Vec<Arc<dyn Channel>>>,
    bus: MessageBus,
}

impl std::fmt::Debug for ChannelManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelManager")
            .field("bus", &self.bus)
            .finish_non_exhaustive()
    }
}

impl ChannelManager {
    /// Create a new channel manager with the given message bus.
    #[must_use]
    pub fn new(bus: MessageBus) -> Self {
        Self {
            channels: RwLock::new(Vec::new()),
            bus,
        }
    }

    /// Register a channel with the manager.
    pub async fn register(&self, channel: impl Channel + 'static) {
        let channel: Arc<dyn Channel> = Arc::new(channel);
        self.channels.write().await.push(Arc::clone(&channel));
        info!(channel = %channel.name(), "channel registered");
    }

    /// Register a boxed channel.
    pub async fn register_boxed(&self, channel: Arc<dyn Channel>) {
        self.channels.write().await.push(Arc::clone(&channel));
        info!(channel = %channel.name(), "channel registered");
    }

    /// Start all registered channels.
    pub async fn start_all(&self) -> Vec<ChannelResult<()>> {
        let channels = self.channels.read().await;
        let mut results = Vec::with_capacity(channels.len());

        for channel in channels.iter() {
            info!(channel = %channel.name(), "starting channel");
            let result = channel.start(&self.bus).await;
            if let Err(ref e) = result {
                error!(channel = %channel.name(), error = %e, "failed to start channel");
            }
            results.push(result);
        }

        results
    }

    /// Stop all registered channels.
    pub async fn stop_all(&self) -> Vec<ChannelResult<()>> {
        let channels = self.channels.read().await;
        let mut results = Vec::with_capacity(channels.len());

        for channel in channels.iter() {
            info!(channel = %channel.name(), "stopping channel");
            let result = channel.stop().await;
            if let Err(ref e) = result {
                error!(channel = %channel.name(), error = %e, "failed to stop channel");
            }
            results.push(result);
        }

        results
    }

    /// Get status of all channels.
    pub async fn status_all(&self) -> Vec<ChannelStatus> {
        let channels = self.channels.read().await;
        let mut statuses = Vec::with_capacity(channels.len());

        for channel in channels.iter() {
            statuses.push(channel.status().await);
        }

        statuses
    }

    /// Get a reference to the message bus.
    #[must_use]
    pub const fn bus(&self) -> &MessageBus {
        &self.bus
    }

    /// Get the number of registered channels.
    pub async fn channel_count(&self) -> usize {
        self.channels.read().await.len()
    }
}

/// Base implementation helpers for channels.
///
/// Provides common functionality that most channel implementations need.
pub struct ChannelBase {
    name: String,
    state: RwLock<ChannelState>,
    stats: RwLock<ChannelStats>,
}

impl std::fmt::Debug for ChannelBase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelBase")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Default)]
struct ChannelStats {
    messages_received: u64,
    messages_sent: u64,
    last_error: Option<String>,
}

impl ChannelBase {
    /// Create a new channel base.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            state: RwLock::new(ChannelState::default()),
            stats: RwLock::new(ChannelStats::default()),
        }
    }

    /// Get the channel name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the current state.
    pub async fn state(&self) -> ChannelState {
        *self.state.read().await
    }

    /// Set the channel state.
    pub async fn set_state(&self, state: ChannelState) {
        *self.state.write().await = state;
        debug!(channel = %self.name, ?state, "channel state changed");
    }

    /// Record a received message.
    pub async fn record_received(&self) {
        self.stats.write().await.messages_received += 1;
    }

    /// Record a sent message.
    pub async fn record_sent(&self) {
        self.stats.write().await.messages_sent += 1;
    }

    /// Record an error.
    pub async fn record_error(&self, error: impl Into<String>) {
        let error = error.into();
        error!(channel = %self.name, %error, "channel error");
        self.stats.write().await.last_error = Some(error);
    }

    /// Build status from current state and stats.
    pub async fn build_status(&self) -> ChannelStatus {
        let state = *self.state.read().await;
        let stats = self.stats.read().await;

        ChannelStatus {
            name: self.name.clone(),
            state,
            messages_received: stats.messages_received,
            messages_sent: stats.messages_sent,
            last_error: stats.last_error.clone(),
            healthy: state == ChannelState::Running && stats.last_error.is_none(),
        }
    }
}

/// Configuration for channel allowlists.
#[derive(Debug, Clone, Default)]
pub struct AllowlistConfig {
    /// Allowed sender IDs. Empty means allow all.
    pub allowed_senders: Vec<String>,
    /// Allowed chat IDs. Empty means allow all.
    pub allowed_chats: Vec<String>,
}

impl AllowlistConfig {
    /// Create a new allowlist config.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an allowed sender.
    #[must_use]
    pub fn allow_sender(mut self, sender: impl Into<String>) -> Self {
        self.allowed_senders.push(sender.into());
        self
    }

    /// Add an allowed chat.
    #[must_use]
    pub fn allow_chat(mut self, chat: impl Into<String>) -> Self {
        self.allowed_chats.push(chat.into());
        self
    }

    /// Check if a sender is allowed.
    #[must_use]
    pub fn is_sender_allowed(&self, sender: &str) -> bool {
        self.allowed_senders.is_empty() || self.allowed_senders.iter().any(|s| s == sender)
    }

    /// Check if a chat is allowed.
    #[must_use]
    pub fn is_chat_allowed(&self, chat: &str) -> bool {
        self.allowed_chats.is_empty() || self.allowed_chats.iter().any(|c| c == chat)
    }

    /// Check if a message from the given sender in the given chat is allowed.
    #[must_use]
    pub fn is_allowed(&self, sender: &str, chat: &str) -> bool {
        self.is_sender_allowed(sender) && self.is_chat_allowed(chat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allowlist_empty() {
        let config = AllowlistConfig::new();
        assert!(config.is_allowed("anyone", "anywhere"));
    }

    #[test]
    fn test_allowlist_sender() {
        let config = AllowlistConfig::new()
            .allow_sender("user1")
            .allow_sender("user2");

        assert!(config.is_sender_allowed("user1"));
        assert!(config.is_sender_allowed("user2"));
        assert!(!config.is_sender_allowed("user3"));
    }

    #[test]
    fn test_allowlist_chat() {
        let config = AllowlistConfig::new().allow_chat("chat1");

        assert!(config.is_chat_allowed("chat1"));
        assert!(!config.is_chat_allowed("chat2"));
    }

    #[tokio::test]
    async fn test_channel_base() {
        let base = ChannelBase::new("test");
        assert_eq!(base.name(), "test");
        assert_eq!(base.state().await, ChannelState::Stopped);

        base.set_state(ChannelState::Running).await;
        assert_eq!(base.state().await, ChannelState::Running);

        base.record_received().await;
        base.record_sent().await;

        let status = base.build_status().await;
        assert_eq!(status.messages_received, 1);
        assert_eq!(status.messages_sent, 1);
        assert!(status.healthy);
    }
}
