//! Core types for model providers.
//!
//! This module contains the fundamental data structures used across
//! all provider implementations.

use crate::message::{ChatMessage, ChatMessageToolCall};
use crate::tool::ToolDefinition;
use serde::{Deserialize, Serialize};

/// Token usage information from a model response.
///
/// Tracks both input (prompt) and output (completion) tokens for cost
/// estimation and context window management.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenUsage {
    /// Number of tokens in the input/prompt.
    pub input_tokens: u32,
    /// Number of tokens in the output/completion.
    pub output_tokens: u32,
}

impl TokenUsage {
    /// Create new token usage with specified counts.
    #[must_use]
    pub const fn new(input_tokens: u32, output_tokens: u32) -> Self {
        Self {
            input_tokens,
            output_tokens,
        }
    }

    /// Get total token count.
    #[must_use]
    pub const fn total(&self) -> u32 {
        self.input_tokens + self.output_tokens
    }

    /// Check if usage is empty (no tokens recorded).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.input_tokens == 0 && self.output_tokens == 0
    }
}

impl std::ops::Add for TokenUsage {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            input_tokens: self.input_tokens.saturating_add(rhs.input_tokens),
            output_tokens: self.output_tokens.saturating_add(rhs.output_tokens),
        }
    }
}

impl std::ops::AddAssign for TokenUsage {
    fn add_assign(&mut self, rhs: Self) {
        self.input_tokens = self.input_tokens.saturating_add(rhs.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(rhs.output_tokens);
    }
}

impl std::iter::Sum for TokenUsage {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), |acc, x| acc + x)
    }
}

/// Response from a model generation call.
///
/// Contains the generated message, token usage statistics, and optionally
/// the raw API response for debugging or advanced use cases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    /// The generated message.
    pub message: ChatMessage,
    /// Token usage information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<TokenUsage>,
    /// Raw response from the API (provider-specific).
    #[serde(skip)]
    pub raw: Option<serde_json::Value>,
}

impl ModelResponse {
    /// Create a new model response.
    #[must_use]
    pub const fn new(message: ChatMessage) -> Self {
        Self {
            message,
            token_usage: None,
            raw: None,
        }
    }

    /// Set token usage.
    #[must_use]
    pub const fn with_token_usage(mut self, usage: TokenUsage) -> Self {
        self.token_usage = Some(usage);
        self
    }

    /// Set raw response.
    #[must_use]
    pub fn with_raw(mut self, raw: serde_json::Value) -> Self {
        self.raw = Some(raw);
        self
    }

    /// Get the text content of the response.
    #[must_use]
    pub fn text(&self) -> Option<String> {
        self.message.text_content()
    }

    /// Get tool calls from the response.
    #[must_use]
    pub fn tool_calls(&self) -> Option<&[ChatMessageToolCall]> {
        self.message.tool_calls.as_deref()
    }

    /// Check if the response contains tool calls.
    #[must_use]
    pub fn has_tool_calls(&self) -> bool {
        self.message
            .tool_calls
            .as_ref()
            .is_some_and(|tc| !tc.is_empty())
    }

    /// Check if the response has text content.
    #[must_use]
    pub fn has_text(&self) -> bool {
        self.message.text_content().is_some_and(|t| !t.is_empty())
    }
}

/// Tool choice mode for function calling.
///
/// Controls how the model decides whether to use tools during generation.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ToolChoice {
    /// Model decides whether to call tools based on context.
    #[default]
    Auto,
    /// Model must call at least one tool.
    Required,
    /// Model should not call any tools, even if available.
    None,
}

impl ToolChoice {
    /// Convert to OpenAI API format string.
    #[must_use]
    pub const fn as_openai_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Required => "required",
            Self::None => "none",
        }
    }
}

/// Options for model generation requests.
///
/// Provides fine-grained control over the generation process, including
/// temperature, token limits, tool availability, and response format.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GenerateOptions {
    /// Stop sequences to end generation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    /// Available tools for function calling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    /// Tool choice mode - controls whether model must/can/cannot call tools.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    /// Temperature for sampling (0.0 to 2.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Maximum tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Top-p (nucleus) sampling parameter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Response format specification (e.g., JSON mode).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<serde_json::Value>,
}

impl GenerateOptions {
    /// Create new default generate options.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set stop sequences.
    #[must_use]
    pub fn with_stop_sequences(mut self, sequences: Vec<String>) -> Self {
        self.stop_sequences = Some(sequences);
        self
    }

    /// Set available tools for function calling.
    #[must_use]
    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Set tool choice mode.
    #[must_use]
    pub const fn with_tool_choice(mut self, choice: ToolChoice) -> Self {
        self.tool_choice = Some(choice);
        self
    }

    /// Set temperature.
    #[must_use]
    pub const fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    /// Set max tokens.
    #[must_use]
    pub const fn with_max_tokens(mut self, max: u32) -> Self {
        self.max_tokens = Some(max);
        self
    }

    /// Set top-p sampling.
    #[must_use]
    pub const fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    /// Set response format.
    #[must_use]
    pub fn with_response_format(mut self, format: serde_json::Value) -> Self {
        self.response_format = Some(format);
        self
    }

    /// Check if tools are configured.
    #[must_use]
    pub fn has_tools(&self) -> bool {
        self.tools.as_ref().is_some_and(|t| !t.is_empty())
    }

    /// Check if stop sequences are configured.
    #[must_use]
    pub fn has_stop_sequences(&self) -> bool {
        self.stop_sequences.as_ref().is_some_and(|s| !s.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_usage_new() {
        let usage = TokenUsage::new(100, 50);
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
    }

    #[test]
    fn test_token_usage_total() {
        let usage = TokenUsage::new(100, 50);
        assert_eq!(usage.total(), 150);
    }

    #[test]
    fn test_token_usage_is_empty() {
        assert!(TokenUsage::default().is_empty());
        assert!(TokenUsage::new(0, 0).is_empty());
        assert!(!TokenUsage::new(1, 0).is_empty());
        assert!(!TokenUsage::new(0, 1).is_empty());
    }

    #[test]
    fn test_token_usage_add() {
        let usage1 = TokenUsage::new(100, 50);
        let usage2 = TokenUsage::new(200, 100);
        let sum = usage1 + usage2;

        assert_eq!(sum.input_tokens, 300);
        assert_eq!(sum.output_tokens, 150);
        assert_eq!(sum.total(), 450);
    }

    #[test]
    fn test_token_usage_add_assign() {
        let mut usage = TokenUsage::new(100, 50);
        usage += TokenUsage::new(200, 100);

        assert_eq!(usage.input_tokens, 300);
        assert_eq!(usage.output_tokens, 150);
    }

    #[test]
    fn test_token_usage_saturating_add() {
        let usage1 = TokenUsage::new(u32::MAX, u32::MAX);
        let usage2 = TokenUsage::new(1, 1);
        let sum = usage1 + usage2;

        assert_eq!(sum.input_tokens, u32::MAX);
        assert_eq!(sum.output_tokens, u32::MAX);
    }

    #[test]
    fn test_token_usage_sum() {
        let usages = vec![
            TokenUsage::new(10, 5),
            TokenUsage::new(20, 10),
            TokenUsage::new(30, 15),
        ];
        let total: TokenUsage = usages.into_iter().sum();

        assert_eq!(total.input_tokens, 60);
        assert_eq!(total.output_tokens, 30);
    }

    #[test]
    fn test_tool_choice_default() {
        assert_eq!(ToolChoice::default(), ToolChoice::Auto);
    }

    #[test]
    fn test_tool_choice_as_openai_str() {
        assert_eq!(ToolChoice::Auto.as_openai_str(), "auto");
        assert_eq!(ToolChoice::Required.as_openai_str(), "required");
        assert_eq!(ToolChoice::None.as_openai_str(), "none");
    }

    #[test]
    fn test_generate_options_default() {
        let opts = GenerateOptions::default();
        assert!(opts.stop_sequences.is_none());
        assert!(opts.tools.is_none());
        assert!(opts.tool_choice.is_none());
        assert!(opts.temperature.is_none());
        assert!(opts.max_tokens.is_none());
        assert!(opts.top_p.is_none());
        assert!(opts.response_format.is_none());
    }

    #[test]
    fn test_generate_options_builder() {
        let opts = GenerateOptions::new()
            .with_temperature(0.7)
            .with_max_tokens(1000)
            .with_top_p(0.9)
            .with_tool_choice(ToolChoice::Required)
            .with_stop_sequences(vec!["END".to_string()]);

        assert_eq!(opts.temperature, Some(0.7));
        assert_eq!(opts.max_tokens, Some(1000));
        assert_eq!(opts.top_p, Some(0.9));
        assert_eq!(opts.tool_choice, Some(ToolChoice::Required));
        assert!(opts.has_stop_sequences());
    }

    #[test]
    fn test_generate_options_has_tools() {
        let opts = GenerateOptions::default();
        assert!(!opts.has_tools());

        let opts_empty = GenerateOptions::new().with_tools(vec![]);
        assert!(!opts_empty.has_tools());
    }

    #[test]
    fn test_model_response_new() {
        let msg = ChatMessage::assistant("Hello");
        let response = ModelResponse::new(msg);

        assert!(response.token_usage.is_none());
        assert!(response.raw.is_none());
        assert_eq!(response.text(), Some("Hello".to_string()));
    }

    #[test]
    fn test_model_response_with_token_usage() {
        let msg = ChatMessage::assistant("Hello");
        let usage = TokenUsage::new(10, 5);
        let response = ModelResponse::new(msg).with_token_usage(usage);

        assert_eq!(response.token_usage, Some(usage));
    }

    #[test]
    fn test_model_response_has_text() {
        let msg = ChatMessage::assistant("Hello");
        let response = ModelResponse::new(msg);
        assert!(response.has_text());

        let empty_msg = ChatMessage::assistant("");
        let empty_response = ModelResponse::new(empty_msg);
        assert!(!empty_response.has_text());
    }
}
