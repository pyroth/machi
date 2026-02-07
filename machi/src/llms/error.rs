//! Error types for LLM provider operations.
//!
//! [`LlmError`] and [`LlmErrorKind`] cover all failure modes when communicating
//! with language model backends (authentication, rate limiting, network issues, etc.).
//! They integrate into the global [`Error`](crate::Error) hierarchy via `Error::Llm`.

use std::fmt;

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
