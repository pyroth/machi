//! Unified error types for machi-bot.
//!
//! This module provides a comprehensive error hierarchy for the bot framework.
//! All module-specific errors can be converted into the main `BotError` type.

use std::fmt;

// ============================================================================
// Main Error Type
// ============================================================================

/// The main error type for machi-bot operations.
///
/// This enum consolidates all error types from various modules into a single
/// type that can be used throughout the application.
#[derive(Debug, thiserror::Error)]
pub enum BotError {
    /// Message bus error.
    #[error("bus: {0}")]
    Bus(#[from] BusError),

    /// Channel error.
    #[error("channel: {0}")]
    Channel(#[from] ChannelError),

    /// Agent/LLM error.
    #[error("agent: {0}")]
    Agent(#[from] AgentError),

    /// Configuration error.
    #[error("config: {0}")]
    Config(#[from] ConfigError),

    /// Session/storage error.
    #[error("storage: {0}")]
    Storage(#[from] StorageError),

    /// IO error.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    /// Task join error.
    #[error("task: {0}")]
    Task(String),

    /// Generic internal error.
    #[error("{0}")]
    Internal(String),
}

impl BotError {
    /// Create an agent error from a string.
    #[inline]
    pub fn agent(msg: impl Into<String>) -> Self {
        Self::Agent(AgentError::Execution(msg.into()))
    }

    /// Create a config error from a string.
    #[inline]
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(ConfigError::Invalid(msg.into()))
    }

    /// Create an internal error.
    #[inline]
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}

impl From<tokio::task::JoinError> for BotError {
    fn from(err: tokio::task::JoinError) -> Self {
        Self::Task(err.to_string())
    }
}

/// Result type alias for machi-bot operations.
pub type Result<T> = std::result::Result<T, BotError>;

// ============================================================================
// Message Bus Errors
// ============================================================================

/// Error type for message bus operations.
#[derive(Debug, thiserror::Error)]
pub enum BusError {
    /// Failed to send inbound message.
    #[error("inbound channel closed")]
    InboundClosed,

    /// Failed to send outbound message.
    #[error("outbound channel closed")]
    OutboundClosed,

    /// Failed to receive message.
    #[error("receive failed: {0}")]
    ReceiveFailed(String),

    /// Channel not found.
    #[error("channel not found: {0}")]
    ChannelNotFound(String),
}

/// Result type for message bus operations.
pub type BusResult<T> = std::result::Result<T, BusError>;

// ============================================================================
// Channel Errors
// ============================================================================

/// Error type for channel operations.
#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    /// Failed to start the channel.
    #[error("start failed: {0}")]
    StartFailed(String),

    /// Failed to stop the channel.
    #[error("stop failed: {0}")]
    StopFailed(String),

    /// Failed to send message.
    #[error("send failed: {0}")]
    SendFailed(String),

    /// Configuration error.
    #[error("config: {0}")]
    Config(String),

    /// Authentication failed.
    #[error("auth failed: {0}")]
    AuthFailed(String),

    /// Rate limited.
    #[error("rate limited: retry after {0}s")]
    RateLimited(u64),

    /// Channel is not connected.
    #[error("not connected")]
    NotConnected,

    /// Internal error.
    #[error("{0}")]
    Internal(String),
}

impl ChannelError {
    /// Create a start failed error.
    #[inline]
    pub fn start(msg: impl Into<String>) -> Self {
        Self::StartFailed(msg.into())
    }

    /// Create a send failed error.
    #[inline]
    pub fn send(msg: impl Into<String>) -> Self {
        Self::SendFailed(msg.into())
    }

    /// Create an internal error.
    #[inline]
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}

/// Result type for channel operations.
pub type ChannelResult<T> = std::result::Result<T, ChannelError>;

// ============================================================================
// Agent Errors
// ============================================================================

/// Error type for agent/LLM operations.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// Model/API error.
    #[error("model: {0}")]
    Model(String),

    /// Tool execution error.
    #[error("tool: {0}")]
    Tool(String),

    /// Execution error.
    #[error("{0}")]
    Execution(String),

    /// Timeout.
    #[error("timeout after {0}s")]
    Timeout(u64),

    /// Max iterations reached.
    #[error("max iterations ({0}) reached")]
    MaxIterations(usize),
}

impl AgentError {
    /// Create a model error.
    #[inline]
    pub fn model(msg: impl Into<String>) -> Self {
        Self::Model(msg.into())
    }

    /// Create a tool error.
    #[inline]
    pub fn tool(msg: impl Into<String>) -> Self {
        Self::Tool(msg.into())
    }
}

/// Result type for agent operations.
pub type AgentResult<T> = std::result::Result<T, AgentError>;

// ============================================================================
// Configuration Errors
// ============================================================================

/// Error type for configuration operations.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// IO error.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// JSON parsing error.
    #[error("parse: {0}")]
    Parse(#[from] serde_json::Error),

    /// Missing required field.
    #[error("missing: {0}")]
    Missing(String),

    /// Invalid value.
    #[error("invalid: {0}")]
    Invalid(String),
}

impl ConfigError {
    /// Create a missing field error.
    #[inline]
    pub fn missing(field: impl Into<String>) -> Self {
        Self::Missing(field.into())
    }

    /// Create an invalid value error.
    #[inline]
    pub fn invalid(msg: impl Into<String>) -> Self {
        Self::Invalid(msg.into())
    }
}

/// Result type for configuration operations.
pub type ConfigResult<T> = std::result::Result<T, ConfigError>;

// ============================================================================
// Storage Errors
// ============================================================================

/// Error type for storage operations (sessions, cron jobs, etc.).
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// IO error.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    /// Item not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// Item already exists.
    #[error("already exists: {0}")]
    AlreadyExists(String),
}

impl StorageError {
    /// Create a not found error.
    #[inline]
    pub fn not_found(key: impl Into<String>) -> Self {
        Self::NotFound(key.into())
    }
}

/// Result type for storage operations.
pub type StorageResult<T> = std::result::Result<T, StorageError>;

// ============================================================================
// Error Context Extension
// ============================================================================

/// Extension trait for adding context to errors.
pub trait ErrorContext<T> {
    /// Add context to an error.
    fn context(self, msg: impl Into<String>) -> Result<T>;

    /// Add context using a closure (lazy evaluation).
    fn with_context<F: FnOnce() -> String>(self, f: F) -> Result<T>;
}

impl<T, E: Into<BotError>> ErrorContext<T> for std::result::Result<T, E> {
    fn context(self, msg: impl Into<String>) -> Result<T> {
        self.map_err(|e| {
            let err = e.into();
            BotError::Internal(format!("{}: {}", msg.into(), err))
        })
    }

    fn with_context<F: FnOnce() -> String>(self, f: F) -> Result<T> {
        self.map_err(|e| {
            let err = e.into();
            BotError::Internal(format!("{}: {}", f(), err))
        })
    }
}

// ============================================================================
// Display Helpers
// ============================================================================

/// A wrapper that displays errors in a user-friendly format.
#[derive(Debug)]
pub struct DisplayError<'a>(pub &'a BotError);

impl fmt::Display for DisplayError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            BotError::Agent(e) => write!(f, "Agent error: {e}"),
            BotError::Config(e) => write!(f, "Configuration error: {e}"),
            BotError::Channel(e) => write!(f, "Channel error: {e}"),
            BotError::Bus(e) => write!(f, "Message bus error: {e}"),
            BotError::Storage(e) => write!(f, "Storage error: {e}"),
            BotError::Io(e) => write!(f, "IO error: {e}"),
            BotError::Json(e) => write!(f, "JSON error: {e}"),
            BotError::Task(e) => write!(f, "Task error: {e}"),
            BotError::Internal(e) => write!(f, "Internal error: {e}"),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_conversions() {
        let bus_err = BusError::InboundClosed;
        let bot_err: BotError = bus_err.into();
        assert!(matches!(bot_err, BotError::Bus(_)));

        let channel_err = ChannelError::NotConnected;
        let bot_err: BotError = channel_err.into();
        assert!(matches!(bot_err, BotError::Channel(_)));
    }

    #[test]
    fn test_error_helpers() {
        let err = BotError::agent("test error");
        assert!(matches!(err, BotError::Agent(_)));

        let err = BotError::config("invalid value");
        assert!(matches!(err, BotError::Config(_)));
    }

    #[test]
    fn test_channel_error_helpers() {
        let err = ChannelError::send("failed");
        assert!(matches!(err, ChannelError::SendFailed(_)));
    }
}
