//! MCP Server example - run this in a separate terminal.
//!
//! Run with: `cargo run -p machi --example mcp_server --features rmcp`
//!
//! This starts an MCP server on localhost:8080 with a `sum` tool.

use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
    service::TowerToHyperService,
};
use rmcp::{
    RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, Content, ErrorData, Implementation, InitializeRequestParams,
        InitializeResult, ListResourceTemplatesResult, ListResourcesResult, PaginatedRequestParams,
        ProtocolVersion, ReadResourceRequestParams, ReadResourceResult, ServerCapabilities,
        ServerInfo,
    },
    schemars,
    service::RequestContext,
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpService, session::local::LocalSessionManager,
    },
};

const SERVER_ADDR: &str = "localhost:8080";

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SumRequest {
    pub a: i32,
    pub b: i32,
}

#[derive(Clone)]
pub struct McpServer {
    tool_router: ToolRouter<McpServer>,
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl McpServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Calculate the sum of two numbers")]
    fn sum(
        &self,
        Parameters(SumRequest { a, b }): Parameters<SumRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![Content::text(
            (a + b).to_string(),
        )]))
    }
}

#[tool_handler]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("MCP server with sum tool.".to_string()),
        }
    }

    async fn list_resources(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(ListResourcesResult {
            resources: vec![],
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        _: ReadResourceRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        Err(ErrorData::resource_not_found("not_found", None))
    }

    async fn list_resource_templates(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        Ok(ListResourceTemplatesResult {
            next_cursor: None,
            resource_templates: Vec::new(),
            meta: None,
        })
    }

    async fn initialize(
        &self,
        _: InitializeRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, ErrorData> {
        Ok(self.get_info())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let service = TowerToHyperService::new(StreamableHttpService::new(
        || Ok(McpServer::new()),
        LocalSessionManager::default().into(),
        Default::default(),
    ));

    let listener = tokio::net::TcpListener::bind(SERVER_ADDR).await?;
    println!("MCP server listening on http://{SERVER_ADDR}");
    println!("Press Ctrl+C to stop");

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("\nShutting down...");
                break;
            }
            accept = listener.accept() => {
                if let Ok((stream, addr)) = accept {
                    println!("Client connected: {addr}");
                    let io = TokioIo::new(stream);
                    let service = service.clone();
                    tokio::spawn(async move {
                        let _ = Builder::new(TokioExecutor::default())
                            .serve_connection(io, service)
                            .await;
                    });
                }
            }
        }
    }

    Ok(())
}
