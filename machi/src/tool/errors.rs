//! Error types for the tool module.

use std::fmt;

/// Errors that can occur during tool execution.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[cfg(not(target_family = "wasm"))]
    /// Error returned by the tool
    ToolCallError(#[from] Box<dyn std::error::Error + Send + Sync>),

    #[cfg(target_family = "wasm")]
    /// Error returned by the tool
    ToolCallError(#[from] Box<dyn std::error::Error>),

    /// Error caused by a de/serialization fail
    JsonError(#[from] serde_json::Error),
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolError::ToolCallError(e) => {
                let error_str = e.to_string();
                // This is required due to being able to use agents as tools
                // which means it is possible to get recursive tool call errors
                if error_str.starts_with("ToolCallError: ") {
                    write!(f, "{}", error_str)
                } else {
                    write!(f, "ToolCallError: {}", error_str)
                }
            }
            ToolError::JsonError(e) => write!(f, "JsonError: {e}"),
        }
    }
}

/// Errors that can occur during toolset operations.
#[derive(Debug, thiserror::Error)]
pub enum ToolSetError {
    /// Error returned by the tool
    #[error("ToolCallError: {0}")]
    ToolCallError(#[from] ToolError),

    /// Could not find a tool
    #[error("ToolNotFoundError: {0}")]
    ToolNotFoundError(String),

    // TODO: Revisit this
    #[error("JsonError: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Tool call was interrupted. Primarily useful for agent multi-step/turn prompting.
    #[error("Tool call interrupted")]
    Interrupted,
}
