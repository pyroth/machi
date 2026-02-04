//! Audio transcription module for voice message support.
//!
//! This module provides transcription capabilities using various providers,
//! with Groq Whisper being the primary supported provider.

mod groq;
mod provider;

pub use groq::GroqTranscriber;
pub use provider::{TranscribeResult, Transcriber, TranscriberError};
