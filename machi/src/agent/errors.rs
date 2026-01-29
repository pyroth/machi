//! Error types for the agent module.

use crate::completion::{CompletionError, PromptError};
use crate::tool::ToolSetError;
use thiserror::Error;

/// Errors that can occur during streaming agent operations.
#[derive(Debug, Error)]
pub enum StreamingError {
    #[error("CompletionError: {0}")]
    Completion(#[from] CompletionError),

    #[error("PromptError: {0}")]
    Prompt(#[from] Box<PromptError>),

    #[error("ToolSetError: {0}")]
    Tool(#[from] ToolSetError),
}
