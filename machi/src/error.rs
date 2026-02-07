//! Unified error types for the machi framework.
//!
//! This module provides a comprehensive error hierarchy covering:
//! - LLM provider errors (authentication, rate limiting, etc.)
//! - Tool execution errors
//! - Agent runtime errors

use std::fmt;

/// Result type alias for machi operations.
pub type Result<T> = std::result::Result<T, Error>;

/// The main error type for the machi framework.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// LLM provider error.
    #[error("LLM error: {0}")]
    Llm(#[from] LlmError),

    /// Tool execution error.
    #[error("Tool error: {0}")]
    Tool(#[from] ToolError),

    /// Memory/session error.
    #[error("Memory error: {0}")]
    Memory(#[from] crate::memory::MemoryError),

    /// Agent runtime error.
    #[error("Agent error: {0}")]
    Agent(String),

    /// Maximum steps reached during agent execution.
    #[error("Maximum steps ({max_steps}) reached without final answer")]
    MaxSteps {
        /// The maximum number of steps configured.
        max_steps: usize,
    },

    /// Input guardrail tripwire was triggered.
    #[error("Input guardrail '{name}' tripwire triggered")]
    InputGuardrailTriggered {
        /// Name of the guardrail that triggered.
        name: String,
        /// Diagnostic information from the guardrail.
        info: serde_json::Value,
    },

    /// Output guardrail tripwire was triggered.
    #[error("Output guardrail '{name}' tripwire triggered")]
    OutputGuardrailTriggered {
        /// Name of the guardrail that triggered.
        name: String,
        /// Diagnostic information from the guardrail.
        info: serde_json::Value,
    },

    /// Agent execution was interrupted.
    #[error("Agent execution was interrupted")]
    Interrupted,

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// HTTP request error.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

impl Error {
    /// Create an agent error with a message.
    #[must_use]
    pub fn agent(msg: impl Into<String>) -> Self {
        Self::Agent(msg.into())
    }

    /// Create a max steps error.
    #[must_use]
    pub const fn max_steps(max_steps: usize) -> Self {
        Self::MaxSteps { max_steps }
    }

    /// Create an input guardrail triggered error.
    #[must_use]
    pub fn input_guardrail_triggered(name: impl Into<String>, info: serde_json::Value) -> Self {
        Self::InputGuardrailTriggered {
            name: name.into(),
            info,
        }
    }

    /// Create an output guardrail triggered error.
    #[must_use]
    pub fn output_guardrail_triggered(name: impl Into<String>, info: serde_json::Value) -> Self {
        Self::OutputGuardrailTriggered {
            name: name.into(),
            info,
        }
    }
}

/// Error type for LLM provider operations.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct LlmError {
    /// The error kind.
    pub kind: LlmErrorKind,
    /// The provider name (e.g., "openai", "ollama").
    pub provider: Option<String>,
    /// Additional error message.
    pub message: String,
    /// Optional error code from the provider.
    pub code: Option<String>,
}

/// Categories of LLM errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LlmErrorKind {
    /// Authentication or authorization failure.
    Auth,
    /// Rate limit exceeded.
    RateLimited,
    /// Context length exceeded.
    ContextExceeded,
    /// Invalid request parameters.
    InvalidRequest,
    /// Response format error.
    ResponseFormat,
    /// Network or connection error.
    Network,
    /// Streaming error.
    Stream,
    /// HTTP status error.
    HttpStatus,
    /// Provider-specific error.
    Provider,
    /// Internal error.
    Internal,
    /// Feature not supported.
    NotSupported,
}

impl LlmError {
    /// Create an authentication error.
    #[must_use]
    pub fn auth(provider: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: LlmErrorKind::Auth,
            provider: Some(provider.into()),
            message: message.into(),
            code: None,
        }
    }

    /// Create a rate limit error.
    #[must_use]
    pub fn rate_limited(provider: impl Into<String>) -> Self {
        Self {
            kind: LlmErrorKind::RateLimited,
            provider: Some(provider.into()),
            message: "Rate limit exceeded. Please retry after some time.".into(),
            code: None,
        }
    }

    /// Create a context exceeded error.
    #[must_use]
    pub fn context_exceeded(used: usize, max: usize) -> Self {
        Self {
            kind: LlmErrorKind::ContextExceeded,
            provider: None,
            message: format!("Context length exceeded: used {used}, max {max}"),
            code: None,
        }
    }

    /// Create a response format error.
    #[must_use]
    pub fn response_format(expected: impl Into<String>, got: impl Into<String>) -> Self {
        Self {
            kind: LlmErrorKind::ResponseFormat,
            provider: None,
            message: format!("Expected {}, got {}", expected.into(), got.into()),
            code: None,
        }
    }

    /// Create a network error.
    #[must_use]
    pub fn network(message: impl Into<String>) -> Self {
        Self {
            kind: LlmErrorKind::Network,
            provider: None,
            message: message.into(),
            code: None,
        }
    }

    /// Create a streaming error.
    #[must_use]
    pub fn stream(message: impl Into<String>) -> Self {
        Self {
            kind: LlmErrorKind::Stream,
            provider: None,
            message: message.into(),
            code: None,
        }
    }

    /// Create an HTTP status error.
    #[must_use]
    pub fn http_status(status: u16, body: impl Into<String>) -> Self {
        Self {
            kind: LlmErrorKind::HttpStatus,
            provider: None,
            message: format!("HTTP {status}: {}", body.into()),
            code: Some(status.to_string()),
        }
    }

    /// Create a provider-specific error.
    #[must_use]
    pub fn provider(provider: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: LlmErrorKind::Provider,
            provider: Some(provider.into()),
            message: message.into(),
            code: None,
        }
    }

    /// Create a provider error with an error code.
    #[must_use]
    pub fn provider_code(
        provider: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind: LlmErrorKind::Provider,
            provider: Some(provider.into()),
            message: message.into(),
            code: Some(code.into()),
        }
    }

    /// Create an internal error.
    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            kind: LlmErrorKind::Internal,
            provider: None,
            message: message.into(),
            code: None,
        }
    }

    /// Create a not supported error.
    #[must_use]
    pub fn not_supported(feature: impl Into<String>) -> Self {
        Self {
            kind: LlmErrorKind::NotSupported,
            provider: None,
            message: format!("Feature not supported: {}", feature.into()),
            code: None,
        }
    }

    /// Check if this is a retryable error.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(self.kind, LlmErrorKind::RateLimited | LlmErrorKind::Network)
    }
}

impl fmt::Display for LlmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(provider) = &self.provider {
            write!(f, "[{provider}] ")?;
        }
        write!(f, "{}", self.message)?;
        if let Some(code) = &self.code {
            write!(f, " (code: {code})")?;
        }
        Ok(())
    }
}

impl std::error::Error for LlmError {}

impl From<reqwest::Error> for LlmError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            Self::network("Request timed out")
        } else if err.is_connect() {
            Self::network(format!("Connection failed: {err}"))
        } else {
            Self::network(err.to_string())
        }
    }
}

/// Error type for tool execution failures.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum ToolError {
    /// Error during tool execution.
    #[error("Execution error: {0}")]
    Execution(String),

    /// Invalid arguments provided to the tool.
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    /// Tool not found.
    #[error("Tool not found: {0}")]
    NotFound(String),

    /// Tool is not initialized.
    #[error("Tool not initialized")]
    NotInitialized,

    /// Tool execution is forbidden by policy.
    #[error("Tool '{0}' is forbidden by policy")]
    Forbidden(String),

    /// Tool execution was denied by human confirmation.
    #[error("Tool '{0}' execution denied by confirmation")]
    ConfirmationDenied(String),

    /// Generic error.
    #[error("Tool error: {0}")]
    Other(String),
}

impl ToolError {
    /// Create an execution error.
    #[must_use]
    pub fn execution(msg: impl Into<String>) -> Self {
        Self::Execution(msg.into())
    }

    /// Create an invalid arguments error.
    #[must_use]
    pub fn invalid_args(msg: impl Into<String>) -> Self {
        Self::InvalidArguments(msg.into())
    }

    /// Create a not found error.
    #[must_use]
    pub fn not_found(name: impl Into<String>) -> Self {
        Self::NotFound(name.into())
    }

    /// Create a forbidden error.
    #[must_use]
    pub fn forbidden(tool_name: impl Into<String>) -> Self {
        Self::Forbidden(tool_name.into())
    }

    /// Create a confirmation denied error.
    #[must_use]
    pub fn confirmation_denied(tool_name: impl Into<String>) -> Self {
        Self::ConfirmationDenied(tool_name.into())
    }
}

impl From<String> for ToolError {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

impl From<&str> for ToolError {
    fn from(s: &str) -> Self {
        Self::Other(s.to_string())
    }
}

impl From<serde_json::Error> for ToolError {
    fn from(err: serde_json::Error) -> Self {
        Self::InvalidArguments(err.to_string())
    }
}
