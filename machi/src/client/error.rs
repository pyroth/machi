//! Error types for the client module.

use crate::http;
use thiserror::Error;

/// Errors that can occur during client building operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ClientBuilderError {
    /// HTTP client error.
    #[error("reqwest error: {0}")]
    HttpError(
        #[from]
        #[source]
        reqwest::Error,
    ),

    /// Invalid property configuration.
    #[error("invalid property: {0}")]
    InvalidProperty(&'static str),
}

/// Errors that can occur during client verification operations.
#[derive(Debug, Error)]
pub enum VerifyError {
    /// Invalid or missing authentication credentials.
    #[error("invalid authentication")]
    InvalidAuthentication,

    /// Provider-specific error.
    #[error("provider error: {0}")]
    ProviderError(String),

    /// HTTP transport error.
    #[error("http error: {0}")]
    HttpError(#[from] http::Error),
}
