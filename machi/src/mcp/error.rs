//! Error types for MCP operations.

use std::io;

/// Error type for MCP client operations.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    /// Failed to connect to an HTTP MCP server.
    #[error("Failed to connect to MCP server at {url}: {message}")]
    HttpConnectionFailed {
        /// The URL that failed.
        url: String,
        /// Error message.
        message: String,
    },

    /// Failed to spawn a local MCP server process.
    #[error("Failed to spawn MCP process '{command}': {message}")]
    ProcessSpawnFailed {
        /// The command that failed.
        command: String,
        /// Error message.
        message: String,
    },

    /// Failed to list tools from the server.
    #[error("Failed to list tools: {0}")]
    ListToolsFailed(String),

    /// Tool execution error.
    #[error("Tool execution failed: {0}")]
    ToolExecutionFailed(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Server returned an error.
    #[error("Server error: {0}")]
    ServerError(String),
}
