//! Ollama Chat Completions API implementation.
//!
//! Implements the [`Model`] trait for Ollama's Chat API,
//! supporting both synchronous and streaming generation.

use super::client::OllamaClient;
use super::streaming::StreamingResponse;
use crate::error::AgentError;
use crate::message::{ChatMessage, ChatMessageToolCall, MessageRole};
use crate::providers::common::{
    ApiClient, GenerateOptions, Model, ModelResponse, ModelStream, TokenUsage, saturating_u32,
};
use async_trait::async_trait;
use serde_json::Value;
use tracing::{debug, instrument};

/// Ollama Chat Completion model.
///
/// Implements the [`Model`] trait for Ollama's Chat API.
#[derive(Clone)]
pub struct CompletionModel {
    client: OllamaClient,
    model_id: String,
    /// Default number of tokens to predict.
    pub num_predict: Option<u32>,
    /// Keep model loaded in memory.
    pub keep_alive: Option<String>,
}

impl std::fmt::Debug for CompletionModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompletionModel")
            .field("model_id", &self.model_id)
            .field("num_predict", &self.num_predict)
            .field("keep_alive", &self.keep_alive)
            .finish_non_exhaustive()
    }
}

impl CompletionModel {
    /// Create a new completion model.
    pub(crate) fn new(client: OllamaClient, model_id: impl Into<String>) -> Self {
        Self {
            client,
            model_id: model_id.into(),
            num_predict: None,
            keep_alive: None,
        }
    }

    /// Set the number of tokens to predict.
    #[must_use]
    pub const fn with_num_predict(mut self, num_predict: u32) -> Self {
        self.num_predict = Some(num_predict);
        self
    }

    /// Set `keep_alive` duration (e.g., "5m", "1h", "-1" for indefinite).
    #[must_use]
    pub fn with_keep_alive(mut self, keep_alive: impl Into<String>) -> Self {
        self.keep_alive = Some(keep_alive.into());
        self
    }

    /// Extract base64 image data from MessageContent for Ollama's images field.
    fn extract_base64_image(content: &crate::message::MessageContent) -> Option<String> {
        use crate::message::MessageContent;
        match content {
            MessageContent::Image { image, .. } => Some(image.clone()),
            MessageContent::ImageUrl { image_url } => {
                // Extract base64 from data URL: data:image/...;base64,<data>
                let url = &image_url.url;
                url.find(";base64,").map(|pos| url[pos + 8..].to_string())
            }
            _ => None,
        }
    }

    /// Build the request body for the API.
    fn build_request_body(&self, messages: &[ChatMessage], options: &GenerateOptions) -> Value {
        let api_messages: Vec<Value> = messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    MessageRole::System => "system",
                    MessageRole::User => "user",
                    MessageRole::Assistant | MessageRole::ToolCall => "assistant",
                    MessageRole::ToolResponse => "tool",
                };

                let mut obj = serde_json::json!({ "role": role });

                // Content (optional for tool call messages)
                if let Some(content) = msg.text_content() {
                    obj["content"] = serde_json::json!(content);
                }

                // Extract images from content for Ollama's format
                if let Some(contents) = &msg.content {
                    let images: Vec<String> = contents
                        .iter()
                        .filter_map(Self::extract_base64_image)
                        .collect();
                    if !images.is_empty() {
                        obj["images"] = serde_json::json!(images);
                    }
                }

                // Tool calls - Ollama format requires type and index
                if let Some(tool_calls) = &msg.tool_calls {
                    let tc_json: Vec<Value> = tool_calls
                        .iter()
                        .enumerate()
                        .map(|(i, tc)| {
                            serde_json::json!({
                                "type": "function",
                                "function": {
                                    "index": i,
                                    "name": tc.function.name,
                                    "arguments": tc.function.arguments
                                }
                            })
                        })
                        .collect();
                    obj["tool_calls"] = serde_json::json!(tc_json);
                }

                // Tool response requires tool_name field
                if msg.role == MessageRole::ToolResponse
                    && let Some(tool_call_id) = &msg.tool_call_id
                {
                    obj["tool_name"] = serde_json::json!(tool_call_id);
                }

                obj
            })
            .collect();

        let mut body = serde_json::json!({
            "model": self.model_id,
            "messages": api_messages,
            "stream": false
        });

        // Options
        let mut opts = serde_json::Map::new();

        if let Some(temp) = options.temperature {
            opts.insert("temperature".to_string(), serde_json::json!(temp));
        }

        if let Some(top_p) = options.top_p {
            opts.insert("top_p".to_string(), serde_json::json!(top_p));
        }

        if let Some(max_tokens) = options.max_tokens.or(self.num_predict) {
            opts.insert("num_predict".to_string(), serde_json::json!(max_tokens));
        }

        if let Some(stop) = &options.stop_sequences
            && !stop.is_empty()
        {
            opts.insert("stop".to_string(), serde_json::json!(stop));
        }

        if !opts.is_empty() {
            body["options"] = Value::Object(opts);
        }

        // Keep alive
        if let Some(keep_alive) = &self.keep_alive {
            body["keep_alive"] = serde_json::json!(keep_alive);
        }

        // Tools
        if let Some(tools) = &options.tools
            && !tools.is_empty()
        {
            let tool_defs: Vec<Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters
                        }
                    })
                })
                .collect();
            body["tools"] = serde_json::json!(tool_defs);
            // Enable thinking mode for models like qwen3 that need it for tool calling
            // When think=true, response has "thinking" field with reasoning and "tool_calls" with calls
            body["think"] = serde_json::json!(true);
            // Note: Ollama doesn't support tool_choice parameter yet, but we handle
            // native tool_calls in parse_response() which takes priority over text parsing
        }

        body
    }

    /// Parse the API response into a `ModelResponse`.
    #[allow(clippy::unused_self, clippy::unnecessary_wraps)]
    fn parse_response(&self, json: Value) -> Result<ModelResponse, AgentError> {
        let message_json = &json["message"];

        // Get content - in think mode, actual response may be in "thinking" field
        let content = message_json["content"].as_str().map(String::from);

        // If content is empty but thinking exists, log it for debugging
        // The tool_calls should still be present when think=true
        if content.as_ref().is_none_or(String::is_empty)
            && let Some(thinking) = message_json["thinking"].as_str()
        {
            debug!(
                thinking_len = thinking.len(),
                "Model returned thinking content"
            );
            // Don't use thinking as content - tool_calls should be parsed below
        }

        // Parse tool calls
        let tool_calls = if message_json["tool_calls"].is_array() {
            let tc_array = message_json["tool_calls"]
                .as_array()
                .expect("tool_calls should be array");
            let calls: Vec<ChatMessageToolCall> = tc_array
                .iter()
                .enumerate()
                .filter_map(|(i, tc)| {
                    let name = tc["function"]["name"].as_str()?.to_string();
                    let arguments = tc["function"]["arguments"].clone();
                    Some(ChatMessageToolCall::new(
                        format!("call_{i}"),
                        name,
                        arguments,
                    ))
                })
                .collect();
            if calls.is_empty() { None } else { Some(calls) }
        } else {
            None
        };

        let message = ChatMessage {
            role: MessageRole::Assistant,
            content: content.map(|c| vec![crate::message::MessageContent::text(c)]),
            tool_calls,
            tool_call_id: None,
        };

        // Parse token usage
        let token_usage = if json.get("prompt_eval_count").is_some() {
            Some(TokenUsage {
                input_tokens: saturating_u32(json["prompt_eval_count"].as_u64().unwrap_or(0)),
                output_tokens: saturating_u32(json["eval_count"].as_u64().unwrap_or(0)),
            })
        } else {
            None
        };

        Ok(ModelResponse {
            message,
            token_usage,
            raw: Some(json),
        })
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl Model for CompletionModel {
    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn supports_stop_parameter(&self) -> bool {
        true
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn supports_tool_calling(&self) -> bool {
        // Tool calling support varies by model
        // Models like llama3.1+, qwen2.5, mistral-nemo support it
        true
    }

    fn provider(&self) -> &'static str {
        "ollama"
    }

    #[instrument(skip(self, messages, options), fields(model = %self.model_id))]
    async fn generate(
        &self,
        messages: Vec<ChatMessage>,
        options: GenerateOptions,
    ) -> Result<ModelResponse, AgentError> {
        let body = self.build_request_body(&messages, &options);
        let url = format!("{}/api/chat", self.client.base_url());

        // Log tools being sent to help debug tool calling issues
        if let Some(tools) = body.get("tools") {
            debug!(tools = %tools, "Sending request with tools");
        } else {
            debug!("Sending request to Ollama API");
        }

        let response = self
            .client
            .http_client()
            .post(&url)
            .headers(self.client.auth_headers())
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AgentError::model(format!(
                "Ollama API error ({status}): {error_text}"
            )));
        }

        let json: Value = response.json().await?;
        debug!(response = %json, "Ollama API response");
        self.parse_response(json)
    }

    #[instrument(skip(self, messages, options), fields(model = %self.model_id))]
    async fn generate_stream(
        &self,
        messages: Vec<ChatMessage>,
        options: GenerateOptions,
    ) -> Result<ModelStream, AgentError> {
        let mut body = self.build_request_body(&messages, &options);
        body["stream"] = serde_json::json!(true);

        let url = format!("{}/api/chat", self.client.base_url());

        debug!("Sending streaming request to Ollama API");

        let response = self
            .client
            .http_client()
            .post(&url)
            .headers(self.client.auth_headers())
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AgentError::model(format!(
                "Ollama API error ({status}): {error_text}"
            )));
        }

        let stream = StreamingResponse::new(response.bytes_stream());
        Ok(Box::pin(stream))
    }
}
