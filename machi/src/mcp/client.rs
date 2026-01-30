//! MCP client for connecting to local and remote servers.

use rmcp::{
    ServiceExt,
    model::{ClientCapabilities, Implementation, InitializeRequestParams, Tool},
    service::ServerSink,
    transport::{StreamableHttpClientTransport, child_process::TokioChildProcess},
};

use super::error::McpError;
use super::transport::TransportConfig;

/// Configuration for MCP client.
#[derive(Debug, Clone)]
pub struct McpClientConfig {
    /// Client name for identification.
    pub name: String,
    /// Client version.
    pub version: String,
}

impl Default for McpClientConfig {
    fn default() -> Self {
        Self {
            name: "machi".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// A high-level MCP client supporting both local and remote servers.
///
/// This provides a unified API for connecting to MCP servers regardless
/// of the underlying transport (HTTP or stdio).
///
/// # Examples
///
/// ## HTTP (Remote Server)
///
/// ```rust,ignore
/// let client = McpClient::http("http://localhost:8080").await?;
/// ```
///
/// ## Stdio (Local Process)
///
/// ```rust,ignore
/// let client = McpClient::stdio("python", &["server.py"]).await?;
/// ```
pub struct McpClient {
    sink: ServerSink,
    tools: Vec<Tool>,
}

impl McpClient {
    /// Connects to an HTTP MCP server.
    ///
    /// # Arguments
    ///
    /// * `url` - The HTTP URL of the server (e.g., "http://localhost:8080")
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let client = McpClient::http("http://localhost:8080").await?;
    /// println!("Tools: {:?}", client.tool_names());
    /// ```
    pub async fn http(url: impl Into<String>) -> Result<Self, McpError> {
        Self::connect(TransportConfig::http(url)).await
    }

    /// Spawns and connects to a local MCP server process.
    ///
    /// # Arguments
    ///
    /// * `command` - The command to execute (e.g., "python", "node")
    /// * `args` - Command arguments (e.g., &["server.py"])
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let client = McpClient::stdio("python", &["my_mcp_server.py"]).await?;
    /// println!("Tools: {:?}", client.tool_names());
    /// ```
    pub async fn stdio(command: impl Into<String>, args: &[&str]) -> Result<Self, McpError> {
        Self::connect(TransportConfig::stdio(command, args)).await
    }

    /// Connects using a transport configuration.
    pub async fn connect(config: TransportConfig) -> Result<Self, McpError> {
        Self::connect_with_client_config(config, McpClientConfig::default()).await
    }

    /// Connects with custom client configuration.
    pub async fn connect_with_client_config(
        transport_config: TransportConfig,
        client_config: McpClientConfig,
    ) -> Result<Self, McpError> {
        let client_info = InitializeRequestParams {
            meta: None,
            protocol_version: Default::default(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: client_config.name,
                version: client_config.version,
                ..Default::default()
            },
        };

        match transport_config {
            TransportConfig::Http { url } => {
                let transport = StreamableHttpClientTransport::from_uri(url.as_str());

                let service = client_info.serve(transport).await.map_err(|e| {
                    McpError::HttpConnectionFailed {
                        url: url.clone(),
                        message: e.to_string(),
                    }
                })?;

                let sink = service.peer().clone();
                let tools = service
                    .peer()
                    .list_tools(Default::default())
                    .await
                    .map_err(|e| McpError::ListToolsFailed(e.to_string()))?
                    .tools;

                Ok(Self { sink, tools })
            }

            TransportConfig::Stdio {
                command,
                args,
                cwd,
                env,
            } => {
                let mut cmd = tokio::process::Command::new(&command);
                cmd.args(&args);

                if let Some(dir) = cwd {
                    cmd.current_dir(dir);
                }

                if let Some(env_vars) = env {
                    for (key, value) in env_vars {
                        cmd.env(key, value);
                    }
                }

                let transport =
                    TokioChildProcess::new(cmd).map_err(|e| McpError::ProcessSpawnFailed {
                        command: command.clone(),
                        message: e.to_string(),
                    })?;

                let service = client_info.serve(transport).await.map_err(|e| {
                    McpError::ProcessSpawnFailed {
                        command: command.clone(),
                        message: e.to_string(),
                    }
                })?;

                let sink = service.peer().clone();
                let tools = service
                    .peer()
                    .list_tools(Default::default())
                    .await
                    .map_err(|e| McpError::ListToolsFailed(e.to_string()))?
                    .tools;

                Ok(Self { sink, tools })
            }
        }
    }

    /// Returns a reference to the cached tools.
    #[must_use]
    pub fn tools(&self) -> &[Tool] {
        &self.tools
    }

    /// Returns the tool names.
    #[must_use]
    pub fn tool_names(&self) -> Vec<&str> {
        self.tools.iter().map(|t| t.name.as_ref()).collect()
    }

    /// Returns the server sink for tool execution.
    #[must_use]
    pub fn sink(&self) -> &ServerSink {
        &self.sink
    }

    /// Consumes the client and returns the tools and sink.
    #[must_use]
    pub fn into_parts(self) -> (Vec<Tool>, ServerSink) {
        (self.tools, self.sink)
    }
}

/// Builder for connecting to multiple MCP servers.
///
/// Supports both HTTP and stdio transports with a declarative API
/// similar to Python agent frameworks.
///
/// # Example
///
/// ```rust,ignore
/// let servers = McpServers::new()
///     .http("calculator", "http://localhost:8080")
///     .stdio("local_tools", "python", &["tools.py"])
///     .connect_all()
///     .await?;
/// ```
#[derive(Default)]
pub struct McpServers {
    configs: Vec<(String, TransportConfig)>,
    client_config: McpClientConfig,
}

impl McpServers {
    /// Creates a new empty server collection.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an HTTP server.
    #[must_use]
    pub fn http(mut self, name: impl Into<String>, url: impl Into<String>) -> Self {
        self.configs.push((name.into(), TransportConfig::http(url)));
        self
    }

    /// Adds a stdio (local process) server.
    #[must_use]
    pub fn stdio(
        mut self,
        name: impl Into<String>,
        command: impl Into<String>,
        args: &[&str],
    ) -> Self {
        self.configs
            .push((name.into(), TransportConfig::stdio(command, args)));
        self
    }

    /// Sets custom client configuration.
    #[must_use]
    pub fn client_config(mut self, config: McpClientConfig) -> Self {
        self.client_config = config;
        self
    }

    /// Connects to all configured servers.
    pub async fn connect_all(self) -> Result<Vec<(String, McpClient)>, McpError> {
        let mut clients = Vec::with_capacity(self.configs.len());

        for (name, config) in self.configs {
            let client =
                McpClient::connect_with_client_config(config, self.client_config.clone()).await?;
            clients.push((name, client));
        }

        Ok(clients)
    }

    /// Connects to all servers and collects all tools into a single client-like result.
    pub async fn connect_and_merge(self) -> Result<(Vec<Tool>, Vec<ServerSink>), McpError> {
        let clients = self.connect_all().await?;

        let mut all_tools = Vec::new();
        let mut all_sinks = Vec::new();

        for (_, client) in clients {
            let (tools, sink) = client.into_parts();
            all_tools.extend(tools);
            all_sinks.push(sink);
        }

        Ok((all_tools, all_sinks))
    }
}
