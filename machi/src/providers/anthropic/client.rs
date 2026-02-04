//! Anthropic API client implementation.
//!
//! Provides a client for interacting with Anthropic's Messages API,
//! supporting Claude models like Claude 4, Claude 3.5, and more.

use super::ANTHROPIC_VERSION_LATEST;
use super::completion::CompletionModel;
use crate::providers::{ApiClient, FromEnv};
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use std::sync::Arc;

/// Default Anthropic API base URL.
pub const ANTHROPIC_API_BASE_URL: &str = "https://api.anthropic.com";

/// Anthropic API client for creating completion models.
///
/// Supports Claude models with features like tool use, vision, and extended thinking.
///
/// # Example
///
/// ```rust,ignore
/// use machi::providers::anthropic::AnthropicClient;
///
/// // From environment variable ANTHROPIC_API_KEY
/// let client = AnthropicClient::from_env();
///
/// // With explicit API key
/// let client = AnthropicClient::new("sk-ant-...");
///
/// // With custom configuration
/// let client = AnthropicClient::builder()
///     .api_key("sk-ant-...")
///     .anthropic_version("2023-06-01")
///     .anthropic_beta("prompt-caching-2024-07-31")
///     .build();
/// ```
#[derive(Clone)]
pub struct AnthropicClient {
    http_client: reqwest::Client,
    api_key: Arc<str>,
    base_url: Arc<str>,
    anthropic_version: Arc<str>,
    anthropic_betas: Vec<String>,
}

impl std::fmt::Debug for AnthropicClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnthropicClient")
            .field("base_url", &self.base_url)
            .field("anthropic_version", &self.anthropic_version)
            .field("api_key", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl AnthropicClient {
    /// Create a new Anthropic client with the given API key.
    ///
    /// Uses the default Anthropic API base URL and latest API version.
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::builder().api_key(api_key).build()
    }

    /// Create a new client builder.
    #[must_use]
    pub fn builder() -> AnthropicClientBuilder {
        AnthropicClientBuilder::default()
    }

    /// Create a completion model with the specified model ID.
    ///
    /// # Arguments
    ///
    /// * `model_id` - The model identifier (e.g., "claude-3-5-sonnet-latest")
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let client = AnthropicClient::from_env();
    /// let sonnet = client.completion_model("claude-sonnet-4-5-latest");
    /// let opus = client.completion_model("claude-opus-4-5-latest");
    /// ```
    #[must_use]
    pub fn completion_model(&self, model_id: impl Into<String>) -> CompletionModel {
        CompletionModel::new(self.clone(), model_id)
    }

    /// Get the Anthropic API version being used.
    #[must_use]
    pub fn anthropic_version(&self) -> &str {
        &self.anthropic_version
    }
}

impl ApiClient for AnthropicClient {
    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }

    fn auth_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::with_capacity(4);

        // API key header
        if let Ok(value) = HeaderValue::from_str(&self.api_key) {
            headers.insert("x-api-key", value);
        }

        // Version header
        if let Ok(value) = HeaderValue::from_str(&self.anthropic_version) {
            headers.insert("anthropic-version", value);
        }

        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        // Beta headers if any
        if !self.anthropic_betas.is_empty()
            && let Ok(value) = HeaderValue::from_str(&self.anthropic_betas.join(","))
        {
            headers.insert("anthropic-beta", value);
        }

        headers
    }
}

impl FromEnv for AnthropicClient {
    /// Create a new Anthropic client from environment variables.
    ///
    /// Uses `ANTHROPIC_API_KEY` for the API key and optionally
    /// `ANTHROPIC_BASE_URL` for a custom base URL.
    ///
    /// # Panics
    ///
    /// Panics if `ANTHROPIC_API_KEY` is not set.
    fn from_env() -> Self {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .expect("ANTHROPIC_API_KEY environment variable not set");

        let mut builder = Self::builder().api_key(api_key);

        if let Ok(base_url) = std::env::var("ANTHROPIC_BASE_URL") {
            builder = builder.base_url(base_url);
        }

        builder.build()
    }
}

/// Builder for [`AnthropicClient`].
///
/// Provides a fluent API for configuring the client with custom settings.
#[derive(Debug)]
pub struct AnthropicClientBuilder {
    api_key: Option<String>,
    base_url: Option<String>,
    anthropic_version: String,
    anthropic_betas: Vec<String>,
    timeout_secs: Option<u64>,
}

impl Default for AnthropicClientBuilder {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: None,
            anthropic_version: ANTHROPIC_VERSION_LATEST.to_string(),
            anthropic_betas: Vec::new(),
            timeout_secs: None,
        }
    }
}

impl AnthropicClientBuilder {
    /// Set the API key.
    #[must_use]
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set a custom base URL.
    #[must_use]
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    /// Set the Anthropic API version.
    #[must_use]
    pub fn anthropic_version(mut self, version: impl Into<String>) -> Self {
        self.anthropic_version = version.into();
        self
    }

    /// Add a beta feature flag.
    #[must_use]
    pub fn anthropic_beta(mut self, beta: impl Into<String>) -> Self {
        self.anthropic_betas.push(beta.into());
        self
    }

    /// Add multiple beta feature flags.
    #[must_use]
    pub fn anthropic_betas(mut self, betas: &[&str]) -> Self {
        self.anthropic_betas
            .extend(betas.iter().map(|s| (*s).to_string()));
        self
    }

    /// Set the request timeout in seconds.
    ///
    /// Note: timeout is not supported on WASM.
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
    pub fn build(self) -> AnthropicClient {
        let api_key = self.api_key.expect("API key is required");
        let base_url = self
            .base_url
            .unwrap_or_else(|| ANTHROPIC_API_BASE_URL.to_string());
        let http_client = Self::build_http_client(self.timeout_secs);

        AnthropicClient {
            http_client,
            api_key: api_key.into(),
            base_url: base_url.into(),
            anthropic_version: self.anthropic_version.into(),
            anthropic_betas: self.anthropic_betas,
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
        let client = AnthropicClient::builder()
            .api_key("test-key")
            .base_url("https://custom.api.com")
            .anthropic_version("2023-06-01")
            .anthropic_beta("prompt-caching-2024-07-31")
            .timeout_secs(30)
            .build();

        assert_eq!(client.base_url(), "https://custom.api.com");
    }

    #[test]
    fn test_default_base_url() {
        let client = AnthropicClient::new("test-key");
        assert_eq!(client.base_url(), ANTHROPIC_API_BASE_URL);
    }
}
