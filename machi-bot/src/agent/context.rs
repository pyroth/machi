//! Context building for agent prompts.
//!
//! This module provides utilities for constructing LLM message context,
//! including system prompts, conversation history, and tool interactions.

use serde_json::Value;
use std::path::Path;

/// Role for a message in the conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    /// System instruction message.
    System,
    /// User input message.
    User,
    /// Assistant response message.
    Assistant,
    /// Tool result message.
    Tool,
}

impl MessageRole {
    /// Get the string representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

/// Builder for constructing LLM message context.
#[derive(Debug, Clone)]
pub struct ContextBuilder {
    workspace: String,
    system_prompt: Option<String>,
}

impl ContextBuilder {
    /// Create a new context builder with the given workspace path.
    pub fn new(workspace: impl AsRef<Path>) -> Self {
        Self {
            workspace: workspace.as_ref().to_string_lossy().to_string(),
            system_prompt: None,
        }
    }

    /// Set a custom system prompt.
    #[must_use]
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Get the default system prompt.
    #[must_use]
    pub fn default_system_prompt(&self) -> String {
        format!(
            "# Machi Bot

You are a helpful AI assistant with access to various tools.

## Capabilities
- You can read and write files in the workspace
- You can execute shell commands
- You can search the web and fetch web pages
- You can send messages to the user

## Workspace
Your workspace is at: {workspace}

## Guidelines
1. Be helpful, concise, and accurate
2. Use tools when needed to accomplish tasks
3. Always confirm before making significant changes
4. If you're unsure, ask for clarification

## Response Style
- Be conversational but professional
- Use markdown for formatting when appropriate
- Keep responses focused and relevant",
            workspace = self.workspace
        )
    }

    /// Build the initial messages array for LLM.
    #[must_use]
    pub fn build_messages(
        &self,
        history: &[Value],
        current_message: &str,
        media: Option<&[Value]>,
    ) -> Vec<Value> {
        let mut messages = Vec::new();

        // System prompt
        let system_prompt = self
            .system_prompt
            .clone()
            .unwrap_or_else(|| self.default_system_prompt());

        messages.push(serde_json::json!({
            "role": "system",
            "content": system_prompt
        }));

        // History
        messages.extend(history.iter().cloned());

        // Current user message
        // TODO: Handle media attachments when media is Some
        let _ = media; // Acknowledge unused parameter for now
        let user_message = serde_json::json!({
            "role": "user",
            "content": current_message
        });
        messages.push(user_message);

        messages
    }

    /// Add an assistant message with optional tool calls.
    #[must_use]
    pub fn add_assistant_message(
        &self,
        messages: Vec<Value>,
        content: Option<&str>,
        tool_calls: Option<Vec<Value>>,
    ) -> Vec<Value> {
        let mut messages = messages;

        let mut msg = serde_json::json!({
            "role": "assistant",
            "content": content.unwrap_or("")
        });

        if let Some(calls) = tool_calls {
            msg["tool_calls"] = Value::Array(calls);
        }

        messages.push(msg);
        messages
    }

    /// Add a tool result message.
    #[must_use]
    pub fn add_tool_result(
        &self,
        messages: Vec<Value>,
        tool_call_id: &str,
        tool_name: &str,
        result: &str,
    ) -> Vec<Value> {
        let mut messages = messages;

        messages.push(serde_json::json!({
            "role": "tool",
            "tool_call_id": tool_call_id,
            "name": tool_name,
            "content": result
        }));

        messages
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_messages() {
        let builder = ContextBuilder::new("/workspace");
        let history = vec![
            serde_json::json!({"role": "user", "content": "Hello"}),
            serde_json::json!({"role": "assistant", "content": "Hi!"}),
        ];

        let messages = builder.build_messages(&history, "How are you?", None);

        assert_eq!(messages.len(), 4); // system + 2 history + current
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[3]["content"], "How are you?");
    }

    #[test]
    fn test_default_system_prompt() {
        let builder = ContextBuilder::new("/test/workspace");
        let prompt = builder.default_system_prompt();
        assert!(prompt.contains("/test/workspace"));
    }
}
