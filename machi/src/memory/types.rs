//! Core types for the memory system.

use crate::message::ChatMessage;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::any::Any;

/// Base trait for memory steps.
///
/// All step types (TaskStep, ActionStep, etc.) implement this trait,
/// allowing them to be stored polymorphically in AgentMemory.
pub trait MemoryStep: Send + Sync + std::fmt::Debug {
    /// Convert the step to messages for the model.
    fn to_messages(&self, summary_mode: bool) -> Vec<ChatMessage>;

    /// Get the step as a serializable value.
    fn to_value(&self) -> Value;

    /// Downcast to Any for type checking.
    fn as_any(&self) -> &dyn Any;
}

/// A tool call made during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for the tool call.
    pub id: String,
    /// Name of the tool.
    pub name: String,
    /// Arguments passed to the tool.
    pub arguments: Value,
}

impl ToolCall {
    /// Create a new tool call.
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>, arguments: Value) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            arguments,
        }
    }
}
