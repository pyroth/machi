//! `OpenAI` API client implementation.
//!
//! Provides a client for interacting with OpenAI's Chat Completions API,
//! supporting GPT-4, GPT-5, O-series, and compatible APIs.

use super::completion::CompletionModel;
use crate::providers::common::{ApiClient, FromEnv};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use std::sync::Arc;

/// Default `OpenAI` API base URL.
pub const OPENAI_API_BASE_URL: &str = "https://api.openai.com/v1";

/// `OpenAI` API client for creating completion models.
///
/// Supports OpenAI's official API as well as compatible APIs like Azure OpenAI,
/// local proxies, and third-party providers.
///
/// # Example
///
/// ```rust,ignore
/// use machi::providers::openai::OpenAIClient;
///
/// // From environment variable OPENAI_API_KEY
/// let client = OpenAIClient::from_env();
///
/// // With explicit API key
/// let client = OpenAIClient::new("sk-...");
///
/// // With custom base URL (for Azure, local models, etc.)
/// let client = OpenAIClient::builder()
///     .api_key("sk-...")
///     .base_url("https://my-openai-proxy.com/v1")
///     .build();
/// ```
#[derive(Clone)]
pub struct OpenAIClient {
    http_client: reqwest::Client,
    api_key: Arc<str>,
    base_url: Arc<str>,
}

impl std::fmt::Debug for OpenAIClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAIClient")
            .field("base_url", &self.base_url)
            .field("api_key", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl OpenAIClient {
    /// Create a new `OpenAI` client with the given API key.
    ///
    /// Uses the default `OpenAI` API base URL.
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::builder().api_key(api_key).build()
    }

    /// Create a new client builder.
    #[must_use]
    pub fn builder() -> OpenAIClientBuilder {
        OpenAIClientBuilder::default()
    }

    /// Create a completion model with the specified model ID.
    ///
    /// # Arguments
    ///
    /// * `model_id` - The model identifier (e.g., "gpt-4o", "gpt-5", "o3")
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let client = OpenAIClient::from_env();
    /// let gpt4 = client.completion_model("gpt-4o");
    /// let gpt5 = client.completion_model("gpt-5");
    /// ```
    #[must_use]
    pub fn completion_model(&self, model_id: impl Into<String>) -> CompletionModel {
        CompletionModel::new(self.clone(), model_id)
    }
}

impl ApiClient for OpenAIClient {
    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }

    fn auth_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::with_capacity(2);

        // Authorization header
        if let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", self.api_key)) {
            headers.insert(AUTHORIZATION, value);
        }

        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers
    }
}

impl FromEnv for OpenAIClient {
    /// Create a new `OpenAI` client from environment variables.
    ///
    /// # Environment Variables
    ///
    /// - `OPENAI_API_KEY` (required): The API key
    /// - `OPENAI_BASE_URL` (optional): Custom base URL
    ///
    /// # Panics
    ///
    /// Panics if `OPENAI_API_KEY` is not set.
    fn from_env() -> Self {
        let api_key =
            std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY environment variable not set");

        let mut builder = Self::builder().api_key(api_key);

        if let Ok(base_url) = std::env::var("OPENAI_BASE_URL") {
            builder = builder.base_url(base_url);
        }

        builder.build()
    }
}

/// Builder for [`OpenAIClient`].
///
/// Provides a fluent API for configuring the client with custom settings.
#[derive(Debug, Default)]
pub struct OpenAIClientBuilder {
    api_key: Option<String>,
    base_url: Option<String>,
    timeout_secs: Option<u64>,
}

impl OpenAIClientBuilder {
    /// Set the API key.
    #[must_use]
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set a custom base URL.
    ///
    /// Useful for Azure `OpenAI`, local models, or proxies.
    #[must_use]
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    /// Set the request timeout in seconds.
    ///
    /// Default is no timeout. Note: timeout is not supported on WASM.
    #[must_use]
    pub const fn timeout_secs(mut self, timeout: u64) -> Self {
        self.timeout_secs = Some(timeout);
        self
    }

    /// Build the client.
    ///
    /// # Panics
    ///
    /// Panics if the API key is not set or if the HTTP client fails to build.
    #[must_use]
    pub fn build(self) -> OpenAIClient {
        let api_key = self.api_key.expect("API key is required");
        let base_url = self
            .base_url
            .unwrap_or_else(|| OPENAI_API_BASE_URL.to_string());
        let http_client = Self::build_http_client(self.timeout_secs);

        OpenAIClient {
            http_client,
            api_key: api_key.into(),
            base_url: base_url.into(),
        }
    }

    /// Build the HTTP client with configured settings.
    fn build_http_client(timeout_secs: Option<u64>) -> reqwest::Client {
        #[allow(unused_mut)]
        let mut builder = reqwest::Client::builder();

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(timeout) = timeout_secs {
            builder = builder.timeout(std::time::Duration::from_secs(timeout));
        }

        builder.build().expect("Failed to build HTTP client")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_builder() {
        let client = OpenAIClient::builder()
            .api_key("test-key")
            .base_url("https://custom.api.com/v1")
            .timeout_secs(30)
            .build();

        assert_eq!(client.base_url(), "https://custom.api.com/v1");
    }

    #[test]
    fn test_default_base_url() {
        let client = OpenAIClient::new("test-key");
        assert_eq!(client.base_url(), OPENAI_API_BASE_URL);
    }
}
