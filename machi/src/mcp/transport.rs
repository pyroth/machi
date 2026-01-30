//! Transport configuration for MCP connections.

/// Transport configuration for MCP client connections.
#[derive(Debug, Clone)]
pub enum TransportConfig {
    /// HTTP transport for remote MCP servers.
    Http {
        /// Server URL (e.g., "http://localhost:8080")
        url: String,
    },

    /// Stdio transport for local MCP server processes.
    Stdio {
        /// Command to execute (e.g., "python", "node")
        command: String,
        /// Command arguments (e.g., ["server.py"])
        args: Vec<String>,
        /// Optional working directory
        cwd: Option<String>,
        /// Optional environment variables
        env: Option<Vec<(String, String)>>,
    },
}

impl TransportConfig {
    /// Creates an HTTP transport configuration.
    #[must_use]
    pub fn http(url: impl Into<String>) -> Self {
        Self::Http { url: url.into() }
    }

    /// Creates a stdio transport configuration.
    #[must_use]
    pub fn stdio(command: impl Into<String>, args: &[&str]) -> Self {
        Self::Stdio {
            command: command.into(),
            args: args.iter().map(|s| (*s).to_string()).collect(),
            cwd: None,
            env: None,
        }
    }

    /// Creates a stdio transport with working directory.
    #[must_use]
    pub fn stdio_with_cwd(
        command: impl Into<String>,
        args: &[&str],
        cwd: impl Into<String>,
    ) -> Self {
        Self::Stdio {
            command: command.into(),
            args: args.iter().map(|s| (*s).to_string()).collect(),
            cwd: Some(cwd.into()),
            env: None,
        }
    }
}
