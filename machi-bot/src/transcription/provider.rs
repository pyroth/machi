//! Transcription provider trait and common types.

use async_trait::async_trait;
use std::path::Path;

/// Error type for transcription operations.
#[derive(Debug, thiserror::Error)]
pub enum TranscriberError {
    /// API key not configured.
    #[error("API key not configured")]
    MissingApiKey,
    /// File not found.
    #[error("file not found: {0}")]
    FileNotFound(String),
    /// Unsupported format.
    #[error("unsupported audio format: {0}")]
    UnsupportedFormat(String),
    /// API error.
    #[error("API error: {0}")]
    Api(String),
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Request error.
    #[error("request error: {0}")]
    Request(String),
}

/// Result type for transcription operations.
pub type TranscribeResult<T> = Result<T, TranscriberError>;

/// Transcription result with metadata.
#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    /// The transcribed text.
    pub text: String,
    /// Duration of the audio in seconds.
    pub duration: Option<f64>,
    /// Detected language.
    pub language: Option<String>,
}

/// Trait for audio transcription providers.
#[async_trait]
pub trait Transcriber: Send + Sync {
    /// Get the provider name.
    fn name(&self) -> &str;

    /// Check if the provider is configured and ready.
    fn is_available(&self) -> bool;

    /// Transcribe an audio file.
    async fn transcribe(&self, path: &Path) -> TranscribeResult<TranscriptionResult>;

    /// Transcribe audio from bytes.
    async fn transcribe_bytes(
        &self,
        data: &[u8],
        filename: &str,
    ) -> TranscribeResult<TranscriptionResult>;
}

/// Supported audio formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    /// MP3 audio.
    Mp3,
    /// MP4/M4A audio.
    Mp4,
    /// MPEG audio.
    Mpeg,
    /// MPGA audio.
    Mpga,
    /// OGG audio (commonly used by Telegram voice messages).
    Ogg,
    /// WAV audio.
    Wav,
    /// WebM audio.
    Webm,
}

impl AudioFormat {
    /// Detect format from file extension.
    #[must_use]
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "mp3" => Some(Self::Mp3),
            "mp4" | "m4a" => Some(Self::Mp4),
            "mpeg" => Some(Self::Mpeg),
            "mpga" => Some(Self::Mpga),
            "ogg" | "oga" | "opus" => Some(Self::Ogg),
            "wav" => Some(Self::Wav),
            "webm" => Some(Self::Webm),
            _ => None,
        }
    }

    /// Get the MIME type for this format.
    #[must_use]
    pub const fn mime_type(self) -> &'static str {
        match self {
            Self::Mp4 => "audio/mp4",
            Self::Mp3 | Self::Mpeg | Self::Mpga => "audio/mpeg",
            Self::Ogg => "audio/ogg",
            Self::Wav => "audio/wav",
            Self::Webm => "audio/webm",
        }
    }
}
