//! MCP (Model Context Protocol) client integration.
//!
//! This module provides integration with MCP servers, allowing agents to use
//! tools exposed by MCP-compatible services.
//!
//! # Example - Stdio Transport
//!
//! ```rust,ignore
//! use machi::prelude::*;
//!
//! // Connect to an MCP server via stdio (child process)
//! let mcp = McpClient::stdio("npx", ["-y", "@anthropics/mcp-server-memory"])
//!     .await?;
//!
//! let mut agent = Agent::builder()
//!     .model(model)
//!     .tools(mcp.tools())
//!     .build();
//! ```
//!
//! # Example - HTTP Transport (Streamable HTTP)
//!
//! ```rust,ignore
//! use machi::prelude::*;
//!
//! // Connect to an MCP server via HTTP
//! let mcp = McpClient::http("http://localhost:8080/mcp").await?;
//!
//! let mut agent = Agent::builder()
//!     .model(model)
//!     .tools(mcp.tools())
//!     .build();
//! ```

use std::{process::Stdio, sync::Arc};

use async_trait::async_trait;
use rmcp::{
    RoleClient, ServiceExt,
    model::{CallToolRequestParams, RawContent, Tool as McpToolDef},
    service::RunningService,
    transport::TokioChildProcess,
};
use serde_json::Value;
use tokio::process::Command;

use crate::{
    error::{AgentError, Result},
    tool::{BoxedTool, DynTool, ToolDefinition, ToolError},
};

/// Type alias for the MCP service.
type McpService = RunningService<RoleClient, ()>;

/// MCP client for connecting to MCP servers.
///
/// Provides a bridge between MCP protocol and machi's tool system,
/// automatically converting MCP tools into machi-compatible tools.
pub struct McpClient {
    service: Arc<McpService>,
    tool_definitions: Vec<McpToolDef>,
}

impl std::fmt::Debug for McpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpClient")
            .field("tools_count", &self.tool_definitions.len())
            .finish_non_exhaustive()
    }
}

impl McpClient {
    /// Connect to an MCP server via stdio (child process).
    ///
    /// # Arguments
    ///
    /// * `program` - The program to execute
    /// * `args` - Arguments to pass to the program
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mcp = McpClient::stdio("npx", ["-y", "@anthropics/mcp-server-memory"]).await?;
    /// ```
    pub async fn stdio<I, S>(program: &str, args: I) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let args: Vec<_> = args.into_iter().collect();
        let program = program.to_string();

        let mut cmd = Command::new(&program);
        for arg in &args {
            cmd.arg(arg);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let transport = TokioChildProcess::new(cmd)
            .map_err(|e| AgentError::internal(format!("Failed to spawn MCP process: {e}")))?;

        Self::from_stdio_transport(transport).await
    }

    /// Connect to an MCP server using a custom command.
    pub async fn from_command(mut command: Command) -> Result<Self> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let transport = TokioChildProcess::new(command)
            .map_err(|e| AgentError::internal(format!("Failed to spawn MCP process: {e}")))?;

        Self::from_stdio_transport(transport).await
    }

    /// Connect to an MCP server via HTTP (Streamable HTTP transport).
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the MCP server endpoint
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mcp = McpClient::http("http://localhost:8080/mcp").await?;
    /// ```
    pub async fn http(url: &str) -> Result<Self> {
        use rmcp::transport::StreamableHttpClientTransport;

        let transport = StreamableHttpClientTransport::from_uri(url);
        let service: McpService = ()
            .serve(transport)
            .await
            .map_err(|e| AgentError::internal(format!("Failed to connect to MCP server: {e}")))?;

        Self::from_service(service).await
    }

    /// Create client from stdio transport.
    async fn from_stdio_transport(transport: TokioChildProcess) -> Result<Self> {
        let service: McpService = ()
            .serve(transport)
            .await
            .map_err(|e| AgentError::internal(format!("Failed to connect to MCP server: {e}")))?;

        Self::from_service(service).await
    }

    /// Initialize client from a running service.
    async fn from_service(service: McpService) -> Result<Self> {
        let service = Arc::new(service);

        // Fetch available tools
        let tools_result = service
            .list_tools(Option::default())
            .await
            .map_err(|e| AgentError::internal(format!("Failed to list MCP tools: {e}")))?;

        Ok(Self {
            service,
            tool_definitions: tools_result.tools,
        })
    }

    /// Get the tools as boxed trait objects for use with agents.
    #[must_use]
    pub fn tools(&self) -> Vec<BoxedTool> {
        self.tool_definitions
            .iter()
            .map(|def| -> BoxedTool {
                Box::new(McpTool::new(def.clone(), Arc::clone(&self.service)))
            })
            .collect()
    }

    /// Get the number of available tools.
    #[must_use]
    pub const fn tool_count(&self) -> usize {
        self.tool_definitions.len()
    }

    /// Get tool names.
    #[must_use]
    pub fn tool_names(&self) -> Vec<&str> {
        self.tool_definitions
            .iter()
            .map(|t| t.name.as_ref())
            .collect()
    }

    /// Disconnect from the MCP server.
    pub async fn disconnect(self) -> Result<()> {
        if let Ok(service) = Arc::try_unwrap(self.service) {
            service
                .cancel()
                .await
                .map_err(|e| AgentError::internal(format!("Failed to disconnect: {e}")))?;
        }
        Ok(())
    }
}

/// A machi tool wrapper for MCP tools.
struct McpTool {
    name: String,
    description: String,
    input_schema: Value,
    service: Arc<McpService>,
}

impl McpTool {
    fn new(def: McpToolDef, service: Arc<McpService>) -> Self {
        let name = def.name.to_string();
        let description = def.description.map(|s| s.to_string()).unwrap_or_default();

        // Convert input_schema Arc<Map> to Value
        let input_schema = Value::Object((*def.input_schema).clone());

        Self {
            name,
            description,
            input_schema,
            service,
        }
    }

    fn build_parameters_schema(&self) -> Value {
        // The input_schema from MCP is already in JSON Schema format
        self.input_schema.clone()
    }

    /// Extract text content from MCP call result.
    fn extract_result_text(content: &[rmcp::model::Content]) -> String {
        content
            .iter()
            .filter_map(|c| match &c.raw {
                RawContent::Text(text) => Some(text.text.as_ref()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl std::fmt::Debug for McpTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpTool")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl DynTool for McpTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> String {
        if self.description.is_empty() {
            format!("MCP tool: {}", self.name)
        } else {
            self.description.clone()
        }
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name.clone(),
            description: self.description(),
            parameters: self.build_parameters_schema(),
            output_type: Some("string".to_string()),
            output_schema: None,
        }
    }

    async fn call_json(&self, args: Value) -> std::result::Result<Value, ToolError> {
        let arguments = args.as_object().cloned();

        let params = CallToolRequestParams {
            meta: None,
            name: self.name.clone().into(),
            arguments,
            task: None,
        };

        let result = self
            .service
            .call_tool(params)
            .await
            .map_err(|e| ToolError::execution(format!("MCP call failed: {e}")))?;

        if result.is_error.unwrap_or(false) {
            let error_text = Self::extract_result_text(&result.content);
            return Err(ToolError::execution(error_text));
        }

        let text = Self::extract_result_text(&result.content);
        Ok(Value::String(text))
    }
}
