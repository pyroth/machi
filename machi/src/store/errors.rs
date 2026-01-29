//! Error types for the store module.

use reqwest::StatusCode;

use crate::embedding::EmbeddingError;
use crate::store::request::FilterError;

/// Errors from vector store operations.
#[derive(Debug, thiserror::Error)]
pub enum VectorStoreError {
    #[error("Embedding error: {0}")]
    EmbeddingError(#[from] EmbeddingError),

    #[error("Json error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[cfg(not(target_family = "wasm"))]
    #[error("Datastore error: {0}")]
    DatastoreError(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("Filter error: {0}")]
    FilterError(#[from] FilterError),

    #[cfg(target_family = "wasm")]
    #[error("Datastore error: {0}")]
    DatastoreError(#[from] Box<dyn std::error::Error + 'static>),

    #[error("Missing Id: {0}")]
    MissingIdError(String),

    #[error("HTTP request error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("External call to API returned an error. Error code: {0} Message: {1}")]
    ExternalAPIError(StatusCode, String),

    #[error("Error while building VectorSearchRequest: {0}")]
    BuilderError(String),
}
