//! MCP (Model Context Protocol) tool integration.
//!
//! This module provides integration with MCP servers through the `rmcp` crate.

use std::borrow::Cow;

use rmcp::model::RawContent;

use crate::completion::ToolDefinition;
use crate::core::wasm_compat::WasmBoxedFuture;

use super::errors::ToolError;
use super::traits::ToolDyn;

/// A tool that wraps an MCP server tool.
#[derive(Clone)]
pub struct McpTool {
    definition: rmcp::model::Tool,
    client: rmcp::service::ServerSink,
}

impl McpTool {
    pub fn from_mcp_server(
        definition: rmcp::model::Tool,
        client: rmcp::service::ServerSink,
    ) -> Self {
        Self { definition, client }
    }
}

impl From<&rmcp::model::Tool> for ToolDefinition {
    fn from(val: &rmcp::model::Tool) -> Self {
        Self {
            name: val.name.to_string(),
            description: val.description.clone().unwrap_or(Cow::from("")).to_string(),
            parameters: val.schema_as_json_value(),
        }
    }
}

impl From<rmcp::model::Tool> for ToolDefinition {
    fn from(val: rmcp::model::Tool) -> Self {
        Self {
            name: val.name.to_string(),
            description: val.description.clone().unwrap_or(Cow::from("")).to_string(),
            parameters: val.schema_as_json_value(),
        }
    }
}

/// Error type for MCP tool operations.
#[derive(Debug, thiserror::Error)]
#[error("MCP tool error: {0}")]
pub struct McpToolError(String);

impl From<McpToolError> for ToolError {
    fn from(e: McpToolError) -> Self {
        ToolError::ToolCallError(Box::new(e))
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
                .call_tool(rmcp::model::CallToolRequestParam {
                    name,
                    arguments,
                    task: None,
                })
                .await
                .map_err(|e| McpToolError(format!("Tool returned an error: {e}")))?;

            if let Some(true) = result.is_error {
                let error_msg = result
                    .content
                    .into_iter()
                    .map(|x| x.raw.as_text().map(|y| y.to_owned()))
                    .map(|x| x.map(|x| x.clone().text))
                    .collect::<Option<Vec<String>>>();

                let error_message = error_msg.map(|x| x.join("\n"));
                if let Some(error_message) = error_message {
                    return Err(McpToolError(error_message).into());
                } else {
                    return Err(McpToolError("No message returned".to_string()).into());
                }
            };

            Ok(result
                .content
                .into_iter()
                .map(|c| match c.raw {
                    rmcp::model::RawContent::Text(raw) => raw.text,
                    rmcp::model::RawContent::Image(raw) => {
                        format!("data:{};base64,{}", raw.mime_type, raw.data)
                    }
                    rmcp::model::RawContent::Resource(raw) => match raw.resource {
                        rmcp::model::ResourceContents::TextResourceContents {
                            uri,
                            mime_type,
                            text,
                            ..
                        } => {
                            format!(
                                "{mime_type}{uri}:{text}",
                                mime_type = mime_type
                                    .map(|m| format!("data:{m};"))
                                    .unwrap_or_default(),
                            )
                        }
                        rmcp::model::ResourceContents::BlobResourceContents {
                            uri,
                            mime_type,
                            blob,
                            ..
                        } => format!(
                            "{mime_type}{uri}:{blob}",
                            mime_type = mime_type
                                .map(|m| format!("data:{m};"))
                                .unwrap_or_default(),
                        ),
                    },
                    RawContent::Audio(_) => {
                        panic!("Support for audio results from an MCP tool is currently unimplemented. Come back later!")
                    }
                    thing => {
                        panic!("Unsupported type found: {thing:?}")
                    }
                })
                .collect::<String>())
        })
    }
}
