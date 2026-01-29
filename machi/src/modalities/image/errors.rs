//! Error types for the image module.

use crate::http;
use thiserror::Error;

/// Errors that can occur during image generation operations.
#[derive(Debug, Error)]
pub enum ImageGenerationError {
    /// Http error (e.g.: connection error, timeout, etc.)
    #[error("HttpError: {0}")]
    HttpError(#[from] http::Error),

    /// Json error (e.g.: serialization, deserialization)
    #[error("JsonError: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Error building the transcription request
    #[error("RequestError: {0}")]
    RequestError(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),

    /// Error parsing the transcription response
    #[error("ResponseError: {0}")]
    ResponseError(String),

    /// Error returned by the transcription model provider
    #[error("ProviderError: {0}")]
    ProviderError(String),
}
