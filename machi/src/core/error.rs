//! Error types for the core module.

use thiserror::Error;

/// Error type for when trying to create a `OneOrMany` object with an empty vector.
#[derive(Debug, Error)]
#[error("Cannot create OneOrMany with an empty vector.")]
pub struct EmptyListError;
