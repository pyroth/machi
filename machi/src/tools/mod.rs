//! Tool system for agent capabilities.
//!
//! This module provides the tool abstraction that allows agents to interact
//! with wallets, blockchains, and external services.
//!
//! # Architecture
//!
//! - [`Tool`] trait: Define executable tools
//! - [`ToolDefinition`]: Schema for LLM function calling
//! - [`ToolRegistry`]: Manage and execute tools
//! - [`wallet`]: Built-in wallet operation tools

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;


pub mod wallet;

/// A tool parameter definition for LLM function calling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    /// Parameter name.
    pub name: String,
    /// Parameter type (e.g., "string", "number", "boolean").
    pub r#type: String,
    /// Parameter description.
    pub description: String,
    /// Whether this parameter is required.
    pub required: bool,
}

/// A tool definition that describes a tool's interface for LLMs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name (should be snake_case).
    pub name: String,
    /// Tool description for the LLM.
    pub description: String,
    /// Tool parameters.
    pub parameters: Vec<ToolParameter>,
}

impl ToolDefinition {
    /// Create a new tool definition.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters: Vec::new(),
        }
    }

    /// Add a required parameter.
    pub fn param(
        mut self,
        name: impl Into<String>,
        r#type: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        self.parameters.push(ToolParameter {
            name: name.into(),
            r#type: r#type.into(),
            description: description.into(),
            required: true,
        });
        self
    }

    /// Add an optional parameter.
    pub fn optional_param(
        mut self,
        name: impl Into<String>,
        r#type: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        self.parameters.push(ToolParameter {
            name: name.into(),
            r#type: r#type.into(),
            description: description.into(),
            required: false,
        });
        self
    }

    /// Convert to JSON schema format for LLM function calling.
    pub fn to_json_schema(&self) -> Value {
        let properties: serde_json::Map<String, Value> = self
            .parameters
            .iter()
            .map(|p| {
                (
                    p.name.clone(),
                    serde_json::json!({
                        "type": p.r#type,
                        "description": p.description
                    }),
                )
            })
            .collect();

        let required: Vec<&str> = self
            .parameters
            .iter()
            .filter(|p| p.required)
            .map(|p| p.name.as_str())
            .collect();

        serde_json::json!({
            "type": "object",
            "properties": properties,
            "required": required
        })
    }
}

/// The result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Whether the tool execution was successful.
    pub success: bool,
    /// The result data (on success) or error message (on failure).
    pub data: Value,
}

impl ToolResult {
    /// Create a successful result.
    pub fn ok(data: impl Serialize) -> Self {
        Self {
            success: true,
            data: serde_json::to_value(data).unwrap_or(Value::Null),
        }
    }

    /// Create a failed result.
    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: Value::String(message.into()),
        }
    }
}

/// Context passed to tools during execution.
pub struct ToolContext<'a, C> {
    /// Reference to the wallet for signing operations.
    pub wallet: &'a crate::wallet::AgentWallet,
    /// Reference to the chain adapter.
    pub chain: &'a C,
}

/// Trait for executable tools.
///
/// Tools are async functions that can be called by agents to perform actions.
pub trait Tool<C>: Send + Sync {
    /// Get the tool definition for LLM function calling.
    fn definition(&self) -> ToolDefinition;

    /// Execute the tool with the given arguments.
    fn execute<'a>(
        &'a self,
        ctx: &'a ToolContext<'a, C>,
        args: Value,
    ) -> Pin<Box<dyn Future<Output = ToolResult> + Send + 'a>>;
}

/// A registry that manages and executes tools.
pub struct ToolRegistry<C> {
    tools: HashMap<String, Arc<dyn Tool<C>>>,
}

impl<C> Default for ToolRegistry<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C> ToolRegistry<C> {
    /// Create a new empty tool registry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool.
    pub fn register(&mut self, tool: impl Tool<C> + 'static) {
        let name = tool.definition().name.clone();
        self.tools.insert(name, Arc::new(tool));
    }

    /// Get a tool by name.
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool<C>>> {
        self.tools.get(name)
    }

    /// Get all tool definitions.
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    /// Execute a tool by name.
    pub async fn execute(&self, ctx: &ToolContext<'_, C>, name: &str, args: Value) -> ToolResult {
        match self.tools.get(name) {
            Some(tool) => tool.execute(ctx, args).await,
            None => ToolResult::err(format!("Tool not found: {name}")),
        }
    }
}

/// Helper macro to extract a required string argument.
#[macro_export]
macro_rules! get_string_arg {
    ($args:expr, $name:literal) => {
        match $args.get($name).and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => return ToolResult::err(concat!("Missing required argument: ", $name)),
        }
    };
}

/// Helper macro to extract an optional string argument.
#[macro_export]
macro_rules! get_optional_string_arg {
    ($args:expr, $name:literal) => {
        $args.get($name).and_then(|v| v.as_str()).map(String::from)
    };
}

/// Helper macro to extract an optional number argument.
#[macro_export]
macro_rules! get_optional_u32_arg {
    ($args:expr, $name:literal, $default:expr) => {
        $args
            .get($name)
            .and_then(|v| v.as_u64())
            .map(|n| n as u32)
            .unwrap_or($default)
    };
}
