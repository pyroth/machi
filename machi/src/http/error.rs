//! Error types for the http module.

use http::{HeaderValue, StatusCode};
use thiserror::Error;

/// Errors that can occur during HTTP operations.
#[derive(Debug, Error)]
pub enum Error {
    #[error("Http error: {0}")]
    Protocol(#[from] http::Error),

    #[error("Invalid status code: {0}")]
    InvalidStatusCode(StatusCode),

    #[error("Invalid status code {0} with message: {1}")]
    InvalidStatusCodeWithMessage(StatusCode, String),

    #[error("Header value outside of legal range: {0}")]
    InvalidHeaderValue(#[from] http::header::InvalidHeaderValue),

    #[error("Request in error state, cannot access headers")]
    NoHeaders,

    #[error("Stream ended")]
    StreamEnded,

    #[error("Invalid content type was returned: {0:?}")]
    InvalidContentType(HeaderValue),

    #[cfg(not(target_family = "wasm"))]
    #[error("Http client error: {0}")]
    Instance(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),

    #[cfg(target_family = "wasm")]
    #[error("Http client error: {0}")]
    Instance(#[from] Box<dyn std::error::Error + 'static>),
}

/// Result type alias for HTTP operations.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(not(target_family = "wasm"))]
pub(crate) fn instance_error<E: std::error::Error + Send + Sync + 'static>(error: E) -> Error {
    Error::Instance(error.into())
}

#[cfg(target_family = "wasm")]
pub(crate) fn instance_error<E: std::error::Error + 'static>(error: E) -> Error {
    Error::Instance(error.into())
}
