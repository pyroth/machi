//! Core types for the managed agent system.

use std::{collections::HashMap, fmt::Write as _};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::Result;

/// Trait for types that can act as managed agents.
///
/// This trait allows any agent-like type to be used as a managed agent
/// within another agent's workflow.
#[async_trait]
pub trait ManagedAgent: Send + Sync {
    /// Get the unique name of this agent.
    fn name(&self) -> &str;

    /// Get a description of what this agent does.
    fn description(&self) -> &str;

    /// Execute a task and return the result.
    ///
    /// # Arguments
    ///
    /// * `task` - The task description to execute
    /// * `additional_args` - Optional additional context/arguments
    ///
    /// # Returns
    ///
    /// A string containing the agent's response/report.
    async fn call(
        &self,
        task: &str,
        additional_args: Option<HashMap<String, Value>>,
    ) -> Result<String>;

    /// Get metadata about this managed agent for prompt generation.
    fn info(&self) -> ManagedAgentInfo {
        ManagedAgentInfo::new(self.name(), self.description())
    }

    /// Whether to provide a run summary in the response.
    fn provide_run_summary(&self) -> bool {
        false
    }
}

/// A boxed dynamic managed agent.
pub type BoxedManagedAgent = Box<dyn ManagedAgent>;

/// Metadata describing a managed agent for prompt generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedAgentInfo {
    /// Unique name of the managed agent.
    pub name: String,
    /// Description of what the agent does.
    pub description: String,
    /// Input specifications.
    pub inputs: ManagedAgentInputs,
    /// Output type (always "string" for managed agents).
    pub output_type: String,
}

impl ManagedAgentInfo {
    /// Create new managed agent info.
    #[must_use]
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            inputs: ManagedAgentInputs::default(),
            output_type: "string".to_string(),
        }
    }

    /// Generate a tool-calling prompt representation for this agent.
    #[must_use]
    pub fn to_tool_calling_prompt(&self) -> String {
        let inputs_json = serde_json::to_string(&self.inputs).unwrap_or_default();
        let mut result = String::with_capacity(
            self.name.len() + self.description.len() + inputs_json.len() + 64,
        );
        let _ = write!(
            result,
            "{}: {}\n    Takes inputs: {}\n    Returns an output of type: {}",
            self.name, self.description, inputs_json, self.output_type
        );
        result
    }
}

/// Configuration for a managed agent's inputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedAgentInputs {
    /// Task input specification.
    pub task: ManagedAgentInput,
    /// Additional arguments specification.
    pub additional_args: ManagedAgentInput,
}

impl Default for ManagedAgentInputs {
    fn default() -> Self {
        Self {
            task: ManagedAgentInput {
                input_type: "string".to_string(),
                description: "Long detailed description of the task.".to_string(),
                nullable: false,
            },
            additional_args: ManagedAgentInput {
                input_type: "object".to_string(),
                description: "Dictionary of extra inputs to pass to the managed agent, e.g. images, dataframes, or any other contextual data it may need.".to_string(),
                nullable: true,
            },
        }
    }
}

/// A single input specification for managed agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedAgentInput {
    /// The type of the input (e.g., "string", "object").
    #[serde(rename = "type")]
    pub input_type: String,
    /// Description of the input.
    pub description: String,
    /// Whether the input is nullable.
    #[serde(default)]
    pub nullable: bool,
}

/// Arguments for calling a managed agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedAgentArgs {
    /// The task to delegate to the managed agent.
    pub task: String,
    /// Optional additional arguments/context.
    #[serde(default)]
    pub additional_args: Option<HashMap<String, Value>>,
}
