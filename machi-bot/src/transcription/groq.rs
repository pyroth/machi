//! Groq Whisper transcription provider.
//!
//! Uses Groq's hosted Whisper API for fast and accurate transcription.

use super::provider::{
    AudioFormat, TranscribeResult, Transcriber, TranscriberError, TranscriptionResult,
};
use async_trait::async_trait;
use std::path::Path;
use tracing::{debug, info};

/// Groq Whisper API endpoint.
const GROQ_WHISPER_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";

/// Groq transcription provider using Whisper.
#[derive(Debug, Clone)]
pub struct GroqTranscriber {
    api_key: Option<String>,
    model: String,
}

impl GroqTranscriber {
    /// Create a new Groq transcriber with an API key.
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: Some(api_key.into()),
            model: "whisper-large-v3-turbo".to_string(),
        }
    }

    /// Create a transcriber from environment variable `GROQ_API_KEY`.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            api_key: std::env::var("GROQ_API_KEY").ok(),
            model: "whisper-large-v3-turbo".to_string(),
        }
    }

    /// Set the model to use.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Get the API key if configured.
    fn get_api_key(&self) -> TranscribeResult<&str> {
        self.api_key
            .as_deref()
            .ok_or(TranscriberError::MissingApiKey)
    }
}

impl Default for GroqTranscriber {
    fn default() -> Self {
        Self::from_env()
    }
}

#[async_trait]
impl Transcriber for GroqTranscriber {
    fn name(&self) -> &'static str {
        "groq-whisper"
    }

    fn is_available(&self) -> bool {
        self.api_key.is_some()
    }

    async fn transcribe(&self, path: &Path) -> TranscribeResult<TranscriptionResult> {
        let api_key = self.get_api_key()?;

        if !path.exists() {
            return Err(TranscriberError::FileNotFound(path.display().to_string()));
        }

        // Detect format
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("ogg");

        let format = AudioFormat::from_extension(extension)
            .ok_or_else(|| TranscriberError::UnsupportedFormat(extension.to_string()))?;

        // Read file
        let data = tokio::fs::read(path).await?;
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio.ogg");

        debug!(path = %path.display(), format = ?format, "transcribing audio file");

        self.transcribe_bytes_internal(&data, filename, format, api_key)
            .await
    }

    async fn transcribe_bytes(
        &self,
        data: &[u8],
        filename: &str,
    ) -> TranscribeResult<TranscriptionResult> {
        let api_key = self.get_api_key()?;

        let extension = filename.rsplit('.').next().unwrap_or("ogg");

        let format = AudioFormat::from_extension(extension)
            .ok_or_else(|| TranscriberError::UnsupportedFormat(extension.to_string()))?;

        self.transcribe_bytes_internal(data, filename, format, api_key)
            .await
    }
}

impl GroqTranscriber {
    async fn transcribe_bytes_internal(
        &self,
        data: &[u8],
        filename: &str,
        format: AudioFormat,
        api_key: &str,
    ) -> TranscribeResult<TranscriptionResult> {
        use reqwest::multipart::{Form, Part};

        let client = reqwest::Client::new();

        // Create multipart form
        let file_part = Part::bytes(data.to_vec())
            .file_name(filename.to_string())
            .mime_str(format.mime_type())
            .map_err(|e| TranscriberError::Request(e.to_string()))?;

        let form = Form::new()
            .part("file", file_part)
            .text("model", self.model.clone())
            .text("response_format", "verbose_json");

        // Send request
        let response = client
            .post(GROQ_WHISPER_URL)
            .header("Authorization", format!("Bearer {api_key}"))
            .multipart(form)
            .send()
            .await
            .map_err(|e| TranscriberError::Request(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(TranscriberError::Api(format!("HTTP {status}: {body}")));
        }

        // Parse response
        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| TranscriberError::Request(e.to_string()))?;

        let text = json["text"].as_str().unwrap_or("").to_string();

        let duration = json["duration"].as_f64();
        let language = json["language"].as_str().map(String::from);

        info!(
            text_len = text.len(),
            duration = ?duration,
            language = ?language,
            "transcription complete"
        );

        Ok(TranscriptionResult {
            text,
            duration,
            language,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_groq_transcriber_creation() {
        let transcriber = GroqTranscriber::new("test-key");
        assert!(transcriber.is_available());
        assert_eq!(transcriber.name(), "groq-whisper");
    }

    #[test]
    fn test_audio_format_detection() {
        assert_eq!(AudioFormat::from_extension("ogg"), Some(AudioFormat::Ogg));
        assert_eq!(AudioFormat::from_extension("mp3"), Some(AudioFormat::Mp3));
        assert_eq!(AudioFormat::from_extension("wav"), Some(AudioFormat::Wav));
        assert_eq!(AudioFormat::from_extension("xyz"), None);
    }
}
