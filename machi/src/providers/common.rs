//! Common types and traits for all providers.
//!
//! This module defines the core abstractions that all provider implementations
//! must satisfy, ensuring a consistent interface across different LLM APIs.
//!
//! # Architecture
//!
//! The provider system is built on several key abstractions:
//!
//! - [`Model`] - The core trait for language model implementations
//! - [`ApiClient`] - Base trait for API client configurations
//! - [`StreamParser`] - Unified interface for parsing streaming responses
//! - [`TokenUsage`] - Token counting and usage tracking
//!
//! # Example
//!
//! ```rust,ignore
//! use machi::providers::{Model, GenerateOptions};
//! use machi::message::ChatMessage;
//!
//! async fn chat(model: &impl Model) -> Result<String, AgentError> {
//!     let messages = vec![ChatMessage::user("Hello!")];
//!     let response = model.generate(messages, GenerateOptions::default()).await?;
//!     Ok(response.text().unwrap_or_default())
//! }
//! ```

use crate::error::AgentError;
use crate::message::{ChatMessage, ChatMessageStreamDelta, ChatMessageToolCall};
use crate::tool::ToolDefinition;
use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::task::{Context, Poll};

// ============================================================================
// Token Usage
// ============================================================================

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

// ============================================================================
// Utility Functions
// ============================================================================

/// Safely convert u64 to u32, saturating at `u32::MAX` if overflow.
#[inline]
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub const fn saturating_u32(value: u64) -> u32 {
    if value > u32::MAX as u64 {
        u32::MAX
    } else {
        value as u32
    }
}

// ============================================================================
// Model Response
// ============================================================================

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

// ============================================================================
// Model Stream
// ============================================================================

/// Stream of model response deltas for streaming generation.
#[cfg(not(target_arch = "wasm32"))]
pub type ModelStream =
    Pin<Box<dyn Stream<Item = Result<ChatMessageStreamDelta, AgentError>> + Send>>;

/// Stream of model response deltas for streaming generation (WASM version, not Send).
#[cfg(target_arch = "wasm32")]
pub type ModelStream = Pin<Box<dyn Stream<Item = Result<ChatMessageStreamDelta, AgentError>>>>;

// ============================================================================
// Tool Choice
// ============================================================================

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

// ============================================================================
// Generate Options
// ============================================================================

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

// ============================================================================
// Model Trait
// ============================================================================

/// The core trait for language model implementations.
///
/// This trait defines the interface that all LLM providers must implement.
/// It supports both synchronous and streaming generation, as well as
/// tool/function calling capabilities.
///
/// # Implementing a Custom Provider
///
/// To implement a custom provider, you need to:
/// 1. Implement the [`model_id`](Model::model_id) method
/// 2. Implement the [`generate`](Model::generate) method
/// 3. Optionally override [`generate_stream`](Model::generate_stream) for streaming
/// 4. Override capability methods as needed
///
/// # Example
///
/// ```rust,ignore
/// use machi::providers::{Model, GenerateOptions};
///
/// async fn generate_response(model: &impl Model) {
///     let messages = vec![ChatMessage::user("Hello!")];
///     let response = model.generate(messages, GenerateOptions::new()).await?;
///     println!("{}", response.text().unwrap_or_default());
/// }
/// ```
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait Model: Send + Sync {
    /// Get the model identifier (e.g., "gpt-4o", "claude-3-5-sonnet-latest").
    fn model_id(&self) -> &str;

    /// Generate a response for the given messages.
    ///
    /// # Arguments
    ///
    /// * `messages` - The conversation history
    /// * `options` - Generation options (temperature, tools, etc.)
    ///
    /// # Errors
    ///
    /// Returns an error if the API call fails or the response cannot be parsed.
    async fn generate(
        &self,
        messages: Vec<ChatMessage>,
        options: GenerateOptions,
    ) -> Result<ModelResponse, AgentError>;

    /// Generate a streaming response.
    ///
    /// Default implementation falls back to non-streaming generate.
    ///
    /// # Arguments
    ///
    /// * `messages` - The conversation history
    /// * `options` - Generation options
    ///
    /// # Errors
    ///
    /// Returns an error if the API call fails.
    async fn generate_stream(
        &self,
        messages: Vec<ChatMessage>,
        options: GenerateOptions,
    ) -> Result<ModelStream, AgentError> {
        let response = self.generate(messages, options).await?;
        let delta = ChatMessageStreamDelta {
            content: response.message.text_content(),
            tool_calls: None,
            token_usage: response.token_usage,
        };
        Ok(Box::pin(futures::stream::once(async move { Ok(delta) })))
    }

    /// Check if the model supports the stop parameter.
    ///
    /// Some models (like `OpenAI`'s o3, o4, gpt-5 series) don't support stop sequences.
    fn supports_stop_parameter(&self) -> bool {
        true
    }

    /// Check if the model supports streaming responses.
    fn supports_streaming(&self) -> bool {
        false
    }

    /// Check if the model supports tool/function calling.
    fn supports_tool_calling(&self) -> bool {
        true
    }

    /// Get the provider name (e.g., "openai", "anthropic", "ollama").
    fn provider(&self) -> &'static str {
        "unknown"
    }
}

// ============================================================================
// FromEnv Trait
// ============================================================================

/// Trait for providers that can be created from environment variables.
pub trait FromEnv: Sized {
    /// Create a new client from environment variables.
    ///
    /// # Panics
    ///
    /// Panics if required environment variables are not set.
    fn from_env() -> Self;
}

// ============================================================================
// API Client Infrastructure
// ============================================================================

/// Base configuration for API clients.
///
/// Provides common functionality for HTTP-based API clients.
pub trait ApiClient: Clone + Send + Sync {
    /// Get the base URL for API requests.
    fn base_url(&self) -> &str;

    /// Get the HTTP client instance.
    fn http_client(&self) -> &reqwest::Client;

    /// Build authentication headers for API requests.
    fn auth_headers(&self) -> HeaderMap;
}

/// Shared HTTP client configuration.
#[derive(Debug, Clone)]
pub struct HttpClientConfig {
    /// Request timeout in seconds.
    pub timeout_secs: Option<u64>,
    /// Maximum number of retries.
    pub max_retries: u32,
    /// User agent string.
    pub user_agent: Option<String>,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            timeout_secs: Some(120),
            max_retries: 3,
            user_agent: None,
        }
    }
}

impl HttpClientConfig {
    /// Build a reqwest client with this configuration.
    ///
    /// # Panics
    ///
    /// Panics if the client cannot be built.
    #[must_use]
    pub fn build_client(&self) -> reqwest::Client {
        #[allow(unused_mut)]
        let mut builder = reqwest::Client::builder();

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(timeout) = self.timeout_secs {
            builder = builder.timeout(std::time::Duration::from_secs(timeout));
        }

        if let Some(ref user_agent) = self.user_agent {
            builder = builder.user_agent(user_agent);
        }

        builder.build().expect("Failed to build HTTP client")
    }
}

// ============================================================================
// Streaming Infrastructure
// ============================================================================

/// A generic streaming response parser for SSE (Server-Sent Events) format.
///
/// Handles buffering and line parsing for streaming API responses.
#[derive(Debug)]
pub struct SseStreamParser<S> {
    inner: S,
    buffer: String,
}

impl<S> SseStreamParser<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    /// Create a new SSE stream parser.
    pub const fn new(stream: S) -> Self {
        Self {
            inner: stream,
            buffer: String::new(),
        }
    }

    /// Try to extract the next complete line from the buffer.
    fn next_line(&mut self) -> Option<String> {
        self.buffer.find('\n').map(|pos| {
            let line = self.buffer[..pos].to_string();
            self.buffer = self.buffer[pos + 1..].to_string();
            line
        })
    }

    /// Parse an SSE data line, stripping the "data: " prefix.
    #[must_use]
    pub fn parse_sse_data(line: &str) -> Option<&str> {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(':') {
            return None;
        }
        trimmed.strip_prefix("data: ")
    }

    /// Check if the data indicates stream completion.
    #[must_use]
    pub fn is_done_marker(data: &str) -> bool {
        data.trim() == "[DONE]"
    }
}

impl<S> Stream for SseStreamParser<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    type Item = Result<String, AgentError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            // Try to get a complete line from buffer
            if let Some(line) = self.next_line() {
                if let Some(data) = Self::parse_sse_data(&line)
                    && !Self::is_done_marker(data)
                {
                    return Poll::Ready(Some(Ok(data.to_string())));
                }
                continue;
            }

            // Need more data from the inner stream
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    if let Ok(text) = std::str::from_utf8(&bytes) {
                        self.buffer.push_str(text);
                    }
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(AgentError::from(e))));
                }
                Poll::Ready(None) => {
                    // Process remaining buffer
                    if !self.buffer.is_empty() {
                        let remaining = std::mem::take(&mut self.buffer);
                        for line in remaining.lines() {
                            if let Some(data) = Self::parse_sse_data(line)
                                && !Self::is_done_marker(data)
                            {
                                return Poll::Ready(Some(Ok(data.to_string())));
                            }
                        }
                    }
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

/// A generic streaming response parser for NDJSON (Newline-Delimited JSON) format.
///
/// Used by Ollama and similar providers.
#[derive(Debug)]
pub struct NdjsonStreamParser<S> {
    inner: S,
    buffer: String,
}

impl<S> NdjsonStreamParser<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    /// Create a new NDJSON stream parser.
    pub const fn new(stream: S) -> Self {
        Self {
            inner: stream,
            buffer: String::new(),
        }
    }

    /// Try to extract the next complete line from the buffer.
    fn next_line(&mut self) -> Option<String> {
        self.buffer.find('\n').map(|pos| {
            let line = self.buffer[..pos].to_string();
            self.buffer = self.buffer[pos + 1..].to_string();
            line
        })
    }
}

impl<S> Stream for NdjsonStreamParser<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    type Item = Result<String, AgentError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            // Try to get a complete line from buffer
            if let Some(line) = self.next_line() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    return Poll::Ready(Some(Ok(trimmed.to_string())));
                }
                continue;
            }

            // Need more data from the inner stream
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    if let Ok(text) = std::str::from_utf8(&bytes) {
                        self.buffer.push_str(text);
                    }
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(AgentError::from(e))));
                }
                Poll::Ready(None) => {
                    // Process remaining buffer
                    if !self.buffer.is_empty() {
                        let remaining = std::mem::take(&mut self.buffer);
                        let trimmed = remaining.trim();
                        if !trimmed.is_empty() {
                            return Poll::Ready(Some(Ok(trimmed.to_string())));
                        }
                    }
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

// ============================================================================
// Retry Configuration
// ============================================================================

/// Configuration for retrying failed requests.
#[derive(Debug, Clone, Copy)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_attempts: u32,
    /// Initial delay between retries in milliseconds.
    pub initial_delay_ms: u64,
    /// Exponential backoff multiplier.
    pub backoff_multiplier: f64,
    /// Whether to add jitter to retry delays.
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 1000,
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryConfig {
    /// Calculate delay for a given attempt number (0-indexed).
    #[must_use]
    #[allow(
        clippy::cast_possible_wrap,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn delay_for_attempt(&self, attempt: u32) -> std::time::Duration {
        let base_delay =
            self.initial_delay_ms as f64 * self.backoff_multiplier.powi(attempt as i32);
        let delay_ms = if self.jitter {
            // Add up to 25% jitter
            let jitter = base_delay * 0.25 * rand_factor();
            base_delay + jitter
        } else {
            base_delay
        };
        std::time::Duration::from_millis(delay_ms as u64)
    }
}

/// Generate a pseudo-random factor between 0.0 and 1.0.
fn rand_factor() -> f64 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    f64::from(nanos % 1000) / 1000.0
}

/// Check if a model ID supports the stop parameter.
///
/// `OpenAI`'s o3, o4, and gpt-5 series don't support stop sequences.
#[must_use]
pub fn model_supports_stop_parameter(model_id: &str) -> bool {
    let model_name = model_id.split('/').next_back().unwrap_or(model_id);

    // o3-mini is an exception that does support stop
    if model_name == "o3-mini" {
        return true;
    }

    // o3*, o4*, gpt-5* don't support stop
    !(model_name.starts_with("o3")
        || model_name.starts_with("o4")
        || model_name.starts_with("gpt-5"))
}

/// Check if a model requires `max_completion_tokens` instead of `max_tokens`.
///
/// `OpenAI`'s o-series and gpt-5 series require the new parameter name.
/// The `max_tokens` parameter is deprecated for these models.
#[must_use]
pub fn model_requires_max_completion_tokens(model_id: &str) -> bool {
    let model_name = model_id.split('/').next_back().unwrap_or(model_id);

    // o1*, o3*, o4*, gpt-5* require max_completion_tokens
    model_name.starts_with("o1")
        || model_name.starts_with("o3")
        || model_name.starts_with("o4")
        || model_name.starts_with("gpt-5")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // TokenUsage Tests
    // ========================================================================

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

    // ========================================================================
    // Utility Function Tests
    // ========================================================================

    #[test]
    fn test_saturating_u32() {
        assert_eq!(saturating_u32(0), 0);
        assert_eq!(saturating_u32(100), 100);
        assert_eq!(saturating_u32(u32::MAX as u64), u32::MAX);
        assert_eq!(saturating_u32(u64::MAX), u32::MAX);
        assert_eq!(saturating_u32(u32::MAX as u64 + 1), u32::MAX);
    }

    // ========================================================================
    // ToolChoice Tests
    // ========================================================================

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

    // ========================================================================
    // GenerateOptions Tests
    // ========================================================================

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

    // ========================================================================
    // ModelResponse Tests
    // ========================================================================

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

    // ========================================================================
    // RetryConfig Tests
    // ========================================================================

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.initial_delay_ms, 1000);
        assert_eq!(config.backoff_multiplier, 2.0);
        assert!(config.jitter);
    }

    #[test]
    fn test_retry_config_delay_without_jitter() {
        let config = RetryConfig {
            max_attempts: 3,
            initial_delay_ms: 1000,
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let delay0 = config.delay_for_attempt(0);
        let delay1 = config.delay_for_attempt(1);
        let delay2 = config.delay_for_attempt(2);

        assert_eq!(delay0.as_millis(), 1000);
        assert_eq!(delay1.as_millis(), 2000);
        assert_eq!(delay2.as_millis(), 4000);
    }

    // ========================================================================
    // Model Support Tests
    // ========================================================================

    #[test]
    fn test_model_supports_stop() {
        // Models that support stop
        assert!(model_supports_stop_parameter("gpt-4"));
        assert!(model_supports_stop_parameter("gpt-4o"));
        assert!(model_supports_stop_parameter("gpt-4-turbo"));
        assert!(model_supports_stop_parameter("gpt-4.1"));
        assert!(model_supports_stop_parameter("claude-3-5-sonnet"));
        assert!(model_supports_stop_parameter("llama3.3"));

        // o3-mini is an exception
        assert!(model_supports_stop_parameter("o3-mini"));

        // Models that don't support stop
        assert!(!model_supports_stop_parameter("o3"));
        assert!(!model_supports_stop_parameter("o3-pro"));
        assert!(!model_supports_stop_parameter("o4-mini"));
        assert!(!model_supports_stop_parameter("gpt-5"));
        assert!(!model_supports_stop_parameter("gpt-5-mini"));
    }

    #[test]
    fn test_model_requires_max_completion_tokens() {
        // Legacy models use max_tokens
        assert!(!model_requires_max_completion_tokens("gpt-4"));
        assert!(!model_requires_max_completion_tokens("gpt-4o"));
        assert!(!model_requires_max_completion_tokens("gpt-4-turbo"));
        assert!(!model_requires_max_completion_tokens("gpt-3.5-turbo"));
        assert!(!model_requires_max_completion_tokens("gpt-4.1"));
        assert!(!model_requires_max_completion_tokens("gpt-4.1-mini"));
        assert!(!model_requires_max_completion_tokens("claude-3-5-sonnet"));

        // o-series and gpt-5 models use max_completion_tokens
        assert!(model_requires_max_completion_tokens("o1"));
        assert!(model_requires_max_completion_tokens("o1-mini"));
        assert!(model_requires_max_completion_tokens("o1-pro"));
        assert!(model_requires_max_completion_tokens("o3"));
        assert!(model_requires_max_completion_tokens("o3-mini"));
        assert!(model_requires_max_completion_tokens("o3-pro"));
        assert!(model_requires_max_completion_tokens("o4-mini"));
        assert!(model_requires_max_completion_tokens("gpt-5"));
        assert!(model_requires_max_completion_tokens("gpt-5-mini"));
        assert!(model_requires_max_completion_tokens("gpt-5.1"));
    }

    #[test]
    fn test_model_with_provider_prefix() {
        // Models with provider prefix (router format)
        assert!(model_supports_stop_parameter("openai/gpt-4o"));
        assert!(!model_supports_stop_parameter("openai/o3"));
        assert!(model_requires_max_completion_tokens("openai/o3"));
    }

    // ========================================================================
    // SSE Stream Parser Tests
    // ========================================================================

    #[test]
    fn test_sse_parse_data() {
        assert_eq!(
            SseStreamParser::<futures::stream::Empty<_>>::parse_sse_data("data: hello"),
            Some("hello")
        );
        assert_eq!(
            SseStreamParser::<futures::stream::Empty<_>>::parse_sse_data("data: [DONE]"),
            Some("[DONE]")
        );
        assert_eq!(
            SseStreamParser::<futures::stream::Empty<_>>::parse_sse_data(""),
            None
        );
        assert_eq!(
            SseStreamParser::<futures::stream::Empty<_>>::parse_sse_data(": comment"),
            None
        );
        assert_eq!(
            SseStreamParser::<futures::stream::Empty<_>>::parse_sse_data("event: message"),
            None
        );
    }

    #[test]
    fn test_sse_is_done_marker() {
        assert!(SseStreamParser::<futures::stream::Empty<_>>::is_done_marker("[DONE]"));
        assert!(SseStreamParser::<futures::stream::Empty<_>>::is_done_marker("  [DONE]  "));
        assert!(!SseStreamParser::<futures::stream::Empty<_>>::is_done_marker("done"));
        assert!(!SseStreamParser::<futures::stream::Empty<_>>::is_done_marker("{}"));
    }

    // ========================================================================
    // HttpClientConfig Tests
    // ========================================================================

    #[test]
    fn test_http_client_config_default() {
        let config = HttpClientConfig::default();
        assert_eq!(config.timeout_secs, Some(120));
        assert_eq!(config.max_retries, 3);
        assert!(config.user_agent.is_none());
    }
}
