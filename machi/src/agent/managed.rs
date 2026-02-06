//! Managed agent tool wrapper.
//!
//! This module provides [`ManagedAgentTool`], which wraps an [`Agent`] as a
//! [`DynTool`] so that parent agents can dispatch tasks to sub-agents through
//! the standard tool-calling interface.
//!
//! When the parent agent's LLM invokes a managed agent tool, the Runner
//! spawns a child run for the sub-agent. Because managed agent tools
//! implement [`DynTool`], they participate in parallel execution alongside
//! regular tools via `tokio::JoinSet`.
//!
//! # Architecture
//!
//! ```text
//! Parent Agent (driven by Runner)
//!   ├─ LLM response: tool_calls = [search_tool, agent_researcher, agent_writer]
//!   │
//!   └─ Runner executes all three in parallel:
//!       ├─ search_tool.call_json(args)           → normal tool
//!       ├─ ManagedAgentTool("researcher").call_json(args) → child Runner.run()
//!       └─ ManagedAgentTool("writer").call_json(args)     → child Runner.run()
//! ```

use async_trait::async_trait;
use serde_json::Value;

use crate::error::ToolError;
use crate::tool::{DynTool, ToolDefinition};

use super::config::Agent;
use super::result::RunConfig;

/// Wraps an [`Agent`] as a [`DynTool`] for use as a managed sub-agent.
///
/// When called, this tool spawns a child [`Runner`](super::Runner) execution
/// for the wrapped agent with the provided task string as input.
///
/// The tool's name and description are derived from the wrapped agent's
/// `name` and `description` fields respectively.
pub struct ManagedAgentTool {
    /// The wrapped sub-agent configuration (includes its own provider).
    agent: Agent,
    /// Optional run config overrides for the child run.
    config: Option<RunConfig>,
}

impl ManagedAgentTool {
    /// Create a new managed agent tool.
    ///
    /// The wrapped agent must have a provider configured via
    /// [`Agent::provider`] before it can be called.
    #[must_use]
    pub const fn new(agent: Agent) -> Self {
        Self {
            agent,
            config: None,
        }
    }

    /// Set an optional run config for child runs.
    #[must_use]
    pub fn with_config(mut self, config: RunConfig) -> Self {
        self.config = Some(config);
        self
    }
}

impl std::fmt::Debug for ManagedAgentTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedAgentTool")
            .field("agent_name", &self.agent.name)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl DynTool for ManagedAgentTool {
    fn name(&self) -> &str {
        &self.agent.name
    }

    fn description(&self) -> String {
        self.agent.description.clone()
    }

    fn definition(&self) -> ToolDefinition {
        self.agent.tool_definition()
    }

    async fn call_json(&self, args: Value) -> Result<Value, ToolError> {
        let task = args.get("task").and_then(Value::as_str).unwrap_or_default();

        if task.is_empty() {
            return Err(ToolError::invalid_args(
                "Managed agent requires a non-empty 'task' argument",
            ));
        }

        // Spawn a child runner for the sub-agent (uses agent's own provider).
        let config = self.config.clone().unwrap_or_default();
        let result = super::Runner::run(&self.agent, task, config)
            .await
            .map_err(|e| {
                ToolError::execution(format!("Managed agent '{}' failed: {e}", self.agent.name))
            })?;

        Ok(result.output)
    }
}
