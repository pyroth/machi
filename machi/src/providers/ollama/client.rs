//! Ollama API client implementation.
//!
//! Provides a client for interacting with Ollama's local LLM server,
//! supporting models like Llama, Qwen, Mistral, DeepSeek, and more.

use super::completion::CompletionModel;
use crate::providers::ApiClient;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use std::sync::Arc;

/// Default Ollama API base URL (local server).
pub const OLLAMA_API_BASE_URL: &str = "http://localhost:11434";

/// Ollama API client for creating completion models.
///
/// Ollama runs locally and doesn't require an API key by default.
/// Supports a wide variety of open-source models.
///
/// # Example
///
/// ```rust,ignore
/// use machi::providers::ollama::OllamaClient;
///
/// // Connect to default local server
/// let client = OllamaClient::new();
///
/// // Connect to custom host
/// let client = OllamaClient::builder()
///     .base_url("http://192.168.1.100:11434")
///     .build();
///
/// let model = client.completion_model("llama3.3");
/// ```
#[derive(Clone)]
pub struct OllamaClient {
    http_client: reqwest::Client,
    base_url: Arc<str>,
}

impl std::fmt::Debug for OllamaClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OllamaClient")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

impl Default for OllamaClient {
    fn default() -> Self {
        Self::new()
    }
}

impl OllamaClient {
    /// Create a new Ollama client with default settings.
    ///
    /// Connects to `http://localhost:11434` by default.
    #[must_use]
    pub fn new() -> Self {
        Self::builder().build()
    }

    /// Create a new client builder.
    #[must_use]
    pub fn builder() -> OllamaClientBuilder {
        OllamaClientBuilder::default()
    }

    /// Create a completion model with the specified model ID.
    ///
    /// # Arguments
    ///
    /// * `model_id` - The model identifier (e.g., "llama3.3", "qwen2.5", "mistral")
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let client = OllamaClient::new();
    /// let llama = client.completion_model("llama3.3");
    /// let qwen = client.completion_model("qwen2.5");
    /// ```
    #[must_use]
    pub fn completion_model(&self, model_id: impl Into<String>) -> CompletionModel {
        CompletionModel::new(self.clone(), model_id)
    }

    /// Check if the Ollama server is running and accessible.
    ///
    /// # Errors
    ///
    /// Returns an error if the server is not reachable.
    pub async fn health_check(&self) -> Result<bool, reqwest::Error> {
        let response = self
            .http_client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    /// List available models on the Ollama server.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn list_models(&self) -> Result<Vec<String>, reqwest::Error> {
        let response = self
            .http_client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        let models = response["models"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["name"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Ok(models)
    }
}

impl ApiClient for OllamaClient {
    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }

    fn auth_headers(&self) -> HeaderMap {
        // Ollama doesn't require authentication
        let mut headers = HeaderMap::with_capacity(1);
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers
    }
}

/// Builder for [`OllamaClient`].
///
/// Provides a fluent API for configuring the client with custom settings.
#[derive(Debug, Default)]
pub struct OllamaClientBuilder {
    base_url: Option<String>,
    timeout_secs: Option<u64>,
}

impl OllamaClientBuilder {
    /// Set a custom base URL.
    ///
    /// Useful for connecting to remote Ollama servers.
    #[must_use]
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    /// Set the request timeout in seconds.
    ///
    /// Default is no timeout (Ollama inference can be slow).
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
    /// Panics if the HTTP client fails to build.
    #[must_use]
    pub fn build(self) -> OllamaClient {
        let base_url = self
            .base_url
            .unwrap_or_else(|| OLLAMA_API_BASE_URL.to_string());
        let http_client = Self::build_http_client(self.timeout_secs);

        OllamaClient {
            http_client,
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
