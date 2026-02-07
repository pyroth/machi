//! MCP (Model Context Protocol) integration for machi agents.
//!
//! This module bridges [MCP servers](https://modelcontextprotocol.io/) with the
//! machi tool system, allowing agents to use tools exposed by any MCP-compatible
//! server.
//!
//! # Architecture
//!
//! ```text
//! McpServer (manages rmcp client connection)
//!   ├── stdio(command, args) → StdioBuilder → connect()
//!   ├── http(url)            → HttpBuilder  → connect()
//!   ├── list_tools()         → discover available tools
//!   ├── call_tool(name, args)→ invoke a remote tool
//!   └── tools()              → Vec<BoxedTool> for Agent integration
//!
//! McpTool (bridges a single MCP tool → machi DynTool)
//! ```
//!
//! # Examples
//!
//! ```rust,ignore
//! use machi::mcp::McpServer;
//!
//! // Simple — just command + args
//! let server = McpServer::stdio("npx", ["-y", "@anthropic/mcp-server-filesystem"])
//!     .connect().await?;
//!
//! // With environment variables and working directory
//! let server = McpServer::stdio("uvx", ["mcp-server-github"])
//!     .env("GITHUB_TOKEN", "ghp_xxx")
//!     .working_dir("/projects/myrepo")
//!     .name("github")
//!     .connect().await?;
//!
//! // HTTP with auth
//! let server = McpServer::http("https://mcp.example.com/v1")
//!     .bearer_auth("sk-xxx")
//!     .name("remote-tools")
//!     .connect().await?;
//!
//! // Get all tools as BoxedTool for use with Agent
//! let tools = server.tools().await?;
//! let agent = Agent::new("assistant")
//!     .tools(tools)
//!     .provider(provider);
//! ```

use async_trait::async_trait;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult, Content, Tool as McpToolDef};
use rmcp::service::{Peer, RoleClient, RunningService};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::child_process::TokioChildProcess;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use serde_json::Value;
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::error::ToolError;
use crate::tool::{BoxedTool, DynTool, ToolDefinition};

/// Builder for connecting to an MCP server via a child process (stdio).
///
/// Created by [`McpServer::stdio`]. Use method chaining to configure the
/// subprocess, then call [`connect`](Self::connect) to establish the connection.
///
/// # Example
///
/// ```rust,ignore
/// let server = McpServer::stdio("uvx", ["mcp-server-github"])
///     .env("GITHUB_TOKEN", "ghp_xxx")
///     .envs([("FOO", "bar"), ("BAZ", "qux")])
///     .working_dir("/home/user/project")
///     .name("github")
///     .connect()
///     .await?;
/// ```
#[derive(Debug)]
pub struct StdioBuilder {
    command: String,
    args: Vec<String>,
    envs: HashMap<String, String>,
    working_dir: Option<PathBuf>,
    name: Option<String>,
}

impl StdioBuilder {
    /// Add a single environment variable for the child process.
    #[must_use]
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.envs.insert(key.into(), value.into());
        self
    }

    /// Add multiple environment variables for the child process.
    #[must_use]
    pub fn envs(
        mut self,
        vars: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        for (k, v) in vars {
            self.envs.insert(k.into(), v.into());
        }
        self
    }

    /// Set the working directory for the child process.
    #[must_use]
    pub fn working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Override the default connection name (defaults to `"stdio:{command}"`).
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Establish the connection to the MCP server.
    pub async fn connect(self) -> crate::Result<McpServer> {
        info!(
            command = %self.command,
            args = ?self.args,
            envs = ?self.envs.keys().collect::<Vec<_>>(),
            "Connecting to MCP server via stdio",
        );

        let mut cmd = tokio::process::Command::new(&self.command);
        cmd.args(&self.args);

        if !self.envs.is_empty() {
            cmd.envs(&self.envs);
        }
        if let Some(ref dir) = self.working_dir {
            cmd.current_dir(dir);
        }

        let transport = TokioChildProcess::new(cmd).map_err(|e| {
            crate::Error::agent(format!(
                "Failed to spawn MCP server process '{}': {e}",
                self.command,
            ))
        })?;

        let service = ().serve(transport).await.map_err(|e| {
            crate::Error::agent(format!(
                "Failed to initialize MCP connection to '{}': {e}",
                self.command,
            ))
        })?;

        let name = self
            .name
            .unwrap_or_else(|| format!("stdio:{}", self.command));
        info!(name = %name, "MCP server connected");

        Ok(McpServer {
            service: Arc::new(RwLock::new(service)),
            cached_tools: Arc::new(RwLock::new(None)),
            name,
        })
    }
}

/// Builder for connecting to an MCP server via Streamable HTTP.
///
/// Created by [`McpServer::http`]. Use method chaining to configure the
/// HTTP connection, then call [`connect`](Self::connect).
///
/// # Example
///
/// ```rust,ignore
/// let server = McpServer::http("https://mcp.example.com/v1")
///     .bearer_auth("sk-xxx")
///     .name("remote-tools")
///     .connect()
///     .await?;
/// ```
#[derive(Debug)]
pub struct HttpBuilder {
    url: String,
    bearer_token: Option<String>,
    name: Option<String>,
}

impl HttpBuilder {
    /// Set a Bearer authentication token.
    ///
    /// The token is sent as `Authorization: Bearer <token>` on every request.
    #[must_use]
    pub fn bearer_auth(mut self, token: impl Into<String>) -> Self {
        self.bearer_token = Some(token.into());
        self
    }

    /// Override the default connection name (defaults to `"http:{url}"`).
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Establish the connection to the MCP server.
    pub async fn connect(self) -> crate::Result<McpServer> {
        info!(url = %self.url, "Connecting to MCP server via HTTP");

        let mut config = StreamableHttpClientTransportConfig::with_uri(self.url.clone());
        if let Some(token) = self.bearer_token {
            config = config.auth_header(token);
        }

        let transport = StreamableHttpClientTransport::from_config(config);

        let service: RunningService<RoleClient, ()> = ().serve(transport).await.map_err(|e| {
            crate::Error::agent(format!(
                "Failed to initialize MCP connection to '{}': {e}",
                self.url,
            ))
        })?;

        let name = self.name.unwrap_or_else(|| format!("http:{}", self.url));
        info!(name = %name, "MCP server connected");

        Ok(McpServer {
            service: Arc::new(RwLock::new(service)),
            cached_tools: Arc::new(RwLock::new(None)),
            name,
        })
    }
}

/// A connection to an MCP server.
///
/// `McpServer` manages the lifecycle of the connection and provides methods
/// to discover and invoke tools exposed by the server. Tools can be extracted
/// as [`BoxedTool`] instances for direct use with [`Agent`](crate::agent::Agent).
///
/// # Connection methods
///
/// | Method | Transport | Use case |
/// |--------|-----------|----------|
/// | [`stdio`](Self::stdio) | Child process (stdin/stdout) | Local MCP servers |
/// | [`http`](Self::http) | Streamable HTTP | Remote MCP servers |
///
/// Both return a builder that allows fine-grained configuration before
/// calling `.connect().await`.
///
/// # Examples
///
/// ```rust,ignore
/// // Minimal
/// let server = McpServer::stdio("npx", ["-y", "mcp-server-fs"]).connect().await?;
///
/// // With env vars (e.g. API key) and working directory
/// let server = McpServer::stdio("uvx", ["mcp-server-github"])
///     .env("GITHUB_TOKEN", std::env::var("GITHUB_TOKEN").unwrap())
///     .working_dir("/projects/myrepo")
///     .name("github")
///     .connect().await?;
///
/// // HTTP with bearer auth
/// let server = McpServer::http("https://mcp.acme.com/v1")
///     .bearer_auth("sk-xxx")
///     .connect().await?;
///
/// // Use tools directly
/// let result = server.call_tool("git_log", serde_json::json!({"count": 5})).await?;
///
/// // Or extract as agent tools
/// let agent = Agent::new("dev")
///     .tools(server.tools().await?)
///     .provider(provider);
/// ```
pub struct McpServer {
    /// The running rmcp client service.
    service: Arc<RwLock<RunningService<RoleClient, ()>>>,
    /// Cached tool definitions from the server.
    cached_tools: Arc<RwLock<Option<Vec<McpToolDef>>>>,
    /// Human-readable name for this server connection.
    name: String,
}

impl std::fmt::Debug for McpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpServer")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl McpServer {
    /// Create a builder for connecting to an MCP server via a child process.
    ///
    /// The returned [`StdioBuilder`] lets you configure environment variables,
    /// working directory, and other options before calling `.connect().await`.
    ///
    /// # Arguments
    ///
    /// * `command` — the executable to run (e.g. `"npx"`, `"uvx"`, `"python"`)
    /// * `args` — arguments to pass to the command
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let server = McpServer::stdio("npx", [
    ///     "-y", "@anthropic/mcp-server-filesystem", "/tmp"
    /// ])
    /// .env("HOME", "/home/user")
    /// .connect().await?;
    /// ```
    pub fn stdio(
        command: impl AsRef<str>,
        args: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> StdioBuilder {
        StdioBuilder {
            command: command.as_ref().to_owned(),
            args: args.into_iter().map(|a| a.as_ref().to_owned()).collect(),
            envs: HashMap::new(),
            working_dir: None,
            name: None,
        }
    }

    /// Create a builder for connecting to an MCP server via Streamable HTTP.
    ///
    /// The returned [`HttpBuilder`] lets you configure authentication and
    /// other options before calling `.connect().await`.
    ///
    /// # Arguments
    ///
    /// * `url` — the HTTP(S) endpoint of the MCP server
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let server = McpServer::http("https://mcp.example.com/v1")
    ///     .bearer_auth("sk-xxx")
    ///     .name("remote")
    ///     .connect().await?;
    /// ```
    pub fn http(url: impl Into<String>) -> HttpBuilder {
        HttpBuilder {
            url: url.into(),
            bearer_token: None,
            name: None,
        }
    }

    /// Get the name of this server connection.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get a reference to the underlying rmcp [`Peer`] for advanced usage.
    ///
    /// This allows direct access to the full MCP protocol (prompts, resources,
    /// etc.) beyond what the convenience methods expose.
    pub async fn peer(&self) -> Peer<RoleClient> {
        let svc = self.service.read().await;
        svc.peer().clone()
    }

    /// Discover all tools exposed by the MCP server.
    ///
    /// Results are cached after the first call. Use [`refresh_tools`](Self::refresh_tools)
    /// to force a re-fetch.
    pub async fn list_tools(&self) -> crate::Result<Vec<McpToolDef>> {
        // Return cached if available.
        {
            let cache = self.cached_tools.read().await;
            if let Some(ref tools) = *cache {
                return Ok(tools.clone());
            }
        }

        self.refresh_tools().await
    }

    /// Re-fetch the tool list from the server, updating the cache.
    pub async fn refresh_tools(&self) -> crate::Result<Vec<McpToolDef>> {
        let svc = self.service.read().await;
        let tools = svc.peer().list_all_tools().await.map_err(|e| {
            crate::Error::agent(format!(
                "Failed to list tools from MCP server '{}': {e}",
                self.name
            ))
        })?;

        debug!(
            server = %self.name,
            count = tools.len(),
            "Discovered MCP tools",
        );

        let mut cache = self.cached_tools.write().await;
        *cache = Some(tools.clone());

        Ok(tools)
    }

    /// Call a tool on the MCP server by name.
    ///
    /// # Arguments
    ///
    /// * `name` — tool name as exposed by the server
    /// * `arguments` — JSON arguments (must be a JSON object or `Value::Null`)
    pub async fn call_tool(
        &self,
        name: impl Into<Cow<'static, str>>,
        arguments: Value,
    ) -> Result<String, ToolError> {
        let tool_name = name.into();
        let args_obj = match arguments {
            Value::Object(map) => Some(map),
            Value::Null => None,
            other => {
                return Err(ToolError::invalid_args(format!(
                    "MCP tool arguments must be a JSON object, got: {other}"
                )));
            }
        };

        let svc = self.service.read().await;
        let result: CallToolResult = svc
            .peer()
            .call_tool(CallToolRequestParams {
                meta: None,
                name: tool_name.clone(),
                arguments: args_obj,
                task: None,
            })
            .await
            .map_err(|e| {
                ToolError::execution(format!("MCP tool '{tool_name}' call failed: {e}"))
            })?;

        // Check if the server reported an error.
        if result.is_error == Some(true) {
            let text = extract_text_from_contents(&result.content);
            return Err(ToolError::execution(format!(
                "MCP tool '{tool_name}' returned error: {text}"
            )));
        }

        Ok(extract_text_from_contents(&result.content))
    }

    /// Convert all MCP tools into [`BoxedTool`] instances for use with an agent.
    ///
    /// Each tool is wrapped in an [`McpTool`] that implements [`DynTool`],
    /// forwarding calls to this server.
    pub async fn tools(&self) -> crate::Result<Vec<BoxedTool>> {
        let mcp_tools = self.list_tools().await?;
        let server = Arc::new(self.clone_inner());

        Ok(mcp_tools
            .into_iter()
            .map(|t| -> BoxedTool {
                Box::new(McpTool {
                    server: Arc::clone(&server),
                    tool_def: t,
                })
            })
            .collect())
    }

    /// Close the MCP server connection gracefully.
    pub async fn close(&self) -> crate::Result<()> {
        let mut svc = self.service.write().await;
        svc.close().await.map_err(|e| {
            crate::Error::agent(format!("Failed to close MCP server '{}': {e}", self.name))
        })?;
        info!(server = %self.name, "MCP server connection closed");
        Ok(())
    }

    /// Create a shallow clone that shares the underlying connection.
    fn clone_inner(&self) -> Self {
        Self {
            service: Arc::clone(&self.service),
            cached_tools: Arc::clone(&self.cached_tools),
            name: self.name.clone(),
        }
    }
}

/// A single tool from an MCP server, implementing [`DynTool`] for use
/// with machi agents.
///
/// Created by [`McpServer::tools`]. You typically do not construct this
/// directly.
struct McpTool {
    server: Arc<McpServer>,
    tool_def: McpToolDef,
}

impl std::fmt::Debug for McpTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpTool")
            .field("name", &self.tool_def.name)
            .field("server", &self.server.name)
            .finish()
    }
}

#[async_trait]
impl DynTool for McpTool {
    fn name(&self) -> &str {
        &self.tool_def.name
    }

    fn description(&self) -> String {
        self.tool_def
            .description
            .as_deref()
            .unwrap_or("MCP tool")
            .to_owned()
    }

    fn definition(&self) -> ToolDefinition {
        let params = Value::Object(self.tool_def.input_schema.as_ref().clone());
        ToolDefinition::new(self.tool_def.name.as_ref(), self.description(), params)
    }

    async fn call_json(&self, args: Value) -> Result<Value, ToolError> {
        let text = self
            .server
            .call_tool(self.tool_def.name.clone(), args)
            .await?;
        Ok(Value::String(text))
    }
}

/// Extract text from MCP content blocks into a single string.
fn extract_text_from_contents(contents: &[Content]) -> String {
    let mut output = String::new();
    for content in contents {
        if let Some(text) = content.as_text() {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(&text.text);
        }
    }
    if output.is_empty() {
        // Fallback: serialize the whole content as JSON
        serde_json::to_string(contents).unwrap_or_default()
    } else {
        output
    }
}
