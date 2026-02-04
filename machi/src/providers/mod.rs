//! LLM Provider implementations for various model APIs.
//!
//! This module provides a unified interface for interacting with different LLM providers.
//! Each provider implements the [`Model`] trait, enabling seamless switching between providers.
//!
//! # Supported Providers
//!
//! - **`OpenAI`**: GPT-5, GPT-4.1, O3/O4 series, and compatible APIs
//! - **Anthropic**: Claude 4.5, Claude 4, Claude 3.5, and other Claude models
//! - **Ollama**: Local LLM inference (Llama, Qwen, Mistral, `DeepSeek`, etc.)
//!
//! # Example
//!
//! ```rust,ignore
//! use machi::providers::openai::OpenAIClient;
//! use machi::providers::anthropic::AnthropicClient;
//! use machi::providers::ollama::OllamaClient;
//!
//! // Create an OpenAI client
//! let openai = OpenAIClient::from_env();
//! let gpt5 = openai.completion_model("gpt-5");
//!
//! // Create an Anthropic client
//! let anthropic = AnthropicClient::from_env();
//! let claude = anthropic.completion_model("claude-sonnet-4-5-latest");
//!
//! // Create an Ollama client (local, no API key needed)
//! let ollama = OllamaClient::new();
//! let llama = ollama.completion_model("llama3.3");
//! ```

mod config;
mod streaming;
mod types;

pub mod anthropic;
pub mod mock;
pub mod ollama;
pub mod openai;

// Re-export config types
pub use config::{HttpClientConfig, RetryConfig};

// Re-export streaming types
pub use streaming::{ModelStream, NdjsonStreamParser, SseStreamParser};

// Re-export core types
pub use types::{GenerateOptions, ModelResponse, TokenUsage, ToolChoice};

// Re-export mock model
pub use mock::MockModel;

// Re-export main client types for convenience
pub use anthropic::AnthropicClient;
pub use ollama::OllamaClient;
pub use openai::OpenAIClient;

use crate::error::AgentError;
use crate::message::{ChatMessage, ChatMessageStreamDelta};
use async_trait::async_trait;
use reqwest::header::HeaderMap;

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

/// Trait for providers that can be created from environment variables.
pub trait FromEnv: Sized {
    /// Create a new client from environment variables.
    ///
    /// # Panics
    ///
    /// Panics if required environment variables are not set.
    fn from_env() -> Self;
}

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

    #[test]
    fn test_saturating_u32() {
        assert_eq!(saturating_u32(0), 0);
        assert_eq!(saturating_u32(100), 100);
        assert_eq!(saturating_u32(u32::MAX as u64), u32::MAX);
        assert_eq!(saturating_u32(u64::MAX), u32::MAX);
        assert_eq!(saturating_u32(u32::MAX as u64 + 1), u32::MAX);
    }

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
}
