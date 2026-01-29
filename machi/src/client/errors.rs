//! Error types for the client module.

use crate::http;
use thiserror::Error;

/// Errors that can occur during client building operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ClientBuilderError {
    #[error("reqwest error: {0}")]
    HttpError(
        #[from]
        #[source]
        reqwest::Error,
    ),

    #[error("invalid property: {0}")]
    InvalidProperty(&'static str),
}

/// Errors that can occur during client verification operations.
#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("invalid authentication")]
    InvalidAuthentication,

    #[error("provider error: {0}")]
    ProviderError(String),

    #[error("http error: {0}")]
    HttpError(#[from] http::Error),
}
