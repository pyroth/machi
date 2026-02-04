//! Memory system for tracking agent steps and state.
//!
//! This module provides the memory infrastructure for agents, allowing them
//! to track their execution history, tool calls, and observations.
//!
//! # Architecture
//!
//! The memory system follows the smolagents pattern with these components:
//!
//! - **`AgentMemory`** - Container for all steps in an agent run
//! - **`MemoryStep`** - Trait for all step types
//! - **Step Types** - `TaskStep`, `ActionStep`, `PlanningStep`, `FinalAnswerStep`
//!
//! # Example
//!
//! ```rust,ignore
//! use machi::memory::{AgentMemory, TaskStep, ActionStep};
//!
//! let mut memory = AgentMemory::new("You are a helpful assistant.");
//! memory.add_step(TaskStep::new("What is 2 + 2?"));
//! ```

mod steps;
mod timing;
mod types;

pub use steps::{ActionStep, FinalAnswerStep, PlanningStep, SystemPromptStep, TaskStep};
pub use timing::Timing;
pub use types::{MemoryStep, ToolCall};

pub use crate::providers::common::TokenUsage;

use crate::message::ChatMessage;
use serde_json::Value;

/// Agent memory containing system prompt and all steps.
///
/// This is the main container for an agent's execution history. It stores
/// the system prompt and a chronological list of all steps taken.
#[derive(Debug)]
pub struct AgentMemory {
    /// System prompt step.
    pub system_prompt: SystemPromptStep,
    /// List of steps taken by the agent.
    pub steps: Vec<Box<dyn MemoryStep>>,
}

impl AgentMemory {
    /// Create a new agent memory with the given system prompt.
    #[must_use]
    pub fn new(system_prompt: impl Into<String>) -> Self {
        Self {
            system_prompt: SystemPromptStep {
                system_prompt: system_prompt.into(),
            },
            steps: Vec::new(),
        }
    }

    /// Reset the memory, clearing all steps.
    pub fn reset(&mut self) {
        self.steps.clear();
    }

    /// Add a step to memory.
    pub fn add_step<S: MemoryStep + 'static>(&mut self, step: S) {
        self.steps.push(Box::new(step));
    }

    /// Get the number of steps.
    #[must_use]
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Check if memory is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Convert memory to messages for the model.
    #[must_use]
    pub fn to_messages(&self, summary_mode: bool) -> Vec<ChatMessage> {
        let mut messages = self.system_prompt.to_messages(summary_mode);
        for step in &self.steps {
            messages.extend(step.to_messages(summary_mode));
        }
        messages
    }

    /// Get all steps as values.
    #[must_use]
    pub fn get_steps(&self) -> Vec<Value> {
        self.steps.iter().map(|s| s.to_value()).collect()
    }

    /// Get total token usage across all steps.
    #[must_use]
    pub fn total_token_usage(&self) -> TokenUsage {
        let mut total = TokenUsage::default();
        for step in &self.steps {
            if let Some(action) = step.as_any().downcast_ref::<ActionStep>() {
                if let Some(usage) = action.token_usage {
                    total += usage;
                }
            } else if let Some(planning) = step.as_any().downcast_ref::<PlanningStep>()
                && let Some(usage) = planning.token_usage
            {
                total += usage;
            }
        }
        total
    }

    /// Get all code actions concatenated (for code agents).
    #[must_use]
    pub fn return_full_code(&self) -> String {
        self.steps
            .iter()
            .filter_map(|s| {
                s.as_any()
                    .downcast_ref::<ActionStep>()
                    .and_then(|a| a.code_action.clone())
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

impl Default for AgentMemory {
    fn default() -> Self {
        Self::new("")
    }
}
