//! MCP tool wrapper for agent integration.

use std::borrow::Cow;

use rmcp::model::RawContent;

use crate::completion::ToolDefinition;
use crate::core::wasm_compat::WasmBoxedFuture;
use crate::tool::{ToolDyn, ToolError};

use super::error::McpError;

/// A tool that wraps an MCP server tool.
#[derive(Clone)]
pub struct McpTool {
    definition: rmcp::model::Tool,
    client: rmcp::service::ServerSink,
}

impl McpTool {
    /// Creates a new MCP tool from a server tool definition and client sink.
    #[must_use]
    pub const fn new(definition: rmcp::model::Tool, client: rmcp::service::ServerSink) -> Self {
        Self { definition, client }
    }
}

impl From<McpError> for ToolError {
    fn from(e: McpError) -> Self {
        Self::ToolCallError(Box::new(e))
    }
}

impl ToolDyn for McpTool {
    fn name(&self) -> String {
        self.definition.name.to_string()
    }

    fn definition(&self, _prompt: String) -> WasmBoxedFuture<'_, ToolDefinition> {
        Box::pin(async move {
            ToolDefinition {
                name: self.definition.name.to_string(),
                description: self
                    .definition
                    .description
                    .clone()
                    .unwrap_or(Cow::from(""))
                    .to_string(),
                parameters: serde_json::to_value(&self.definition.input_schema).unwrap_or_default(),
            }
        })
    }

    fn call(&self, args: String) -> WasmBoxedFuture<'_, Result<String, ToolError>> {
        let name = self.definition.name.clone();
        let arguments = serde_json::from_str(&args).unwrap_or_default();

        Box::pin(async move {
            let result = self
                .client
                .call_tool(rmcp::model::CallToolRequestParams {
                    meta: None,
                    name,
                    arguments,
                    task: None,
                })
                .await
                .map_err(|e| McpError::ToolExecutionFailed(format!("Tool call error: {e}")))?;

            if result.is_error == Some(true) {
                let error_msg = result
                    .content
                    .into_iter()
                    .map(|x| x.raw.as_text().map(std::borrow::ToOwned::to_owned))
                    .map(|x| x.map(|x| x.text))
                    .collect::<Option<Vec<String>>>();

                let error_message = error_msg.map(|x| x.join("\n"));
                if let Some(error_message) = error_message {
                    return Err(McpError::ToolExecutionFailed(error_message).into());
                }
                return Err(
                    McpError::ToolExecutionFailed("No message returned".to_string()).into(),
                );
            }

            Ok(result
                .content
                .into_iter()
                .map(|c| match c.raw {
                    RawContent::Text(raw) => raw.text,
                    RawContent::Image(raw) => {
                        format!("data:{};base64,{}", raw.mime_type, raw.data)
                    }
                    RawContent::Resource(raw) => match raw.resource {
                        rmcp::model::ResourceContents::TextResourceContents {
                            uri,
                            mime_type,
                            text,
                            ..
                        } => {
                            format!(
                                "{mime_type}{uri}:{text}",
                                mime_type =
                                    mime_type.map(|m| format!("data:{m};")).unwrap_or_default(),
                            )
                        }
                        rmcp::model::ResourceContents::BlobResourceContents {
                            uri,
                            mime_type,
                            blob,
                            ..
                        } => format!(
                            "{mime_type}{uri}:{blob}",
                            mime_type = mime_type.map(|m| format!("data:{m};")).unwrap_or_default(),
                        ),
                    },
                    RawContent::Audio(_) => "[Audio content not supported]".to_string(),
                    RawContent::ResourceLink(link) => {
                        format!("[ResourceLink: {:?}]", link)
                    }
                })
                .collect::<String>())
        })
    }
}
