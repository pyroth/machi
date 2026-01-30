//! Client traits for provider abstraction.
//!
//! This module defines the fundamental traits for provider clients,
//! enabling a unified interface across different LLM providers.

mod audio;
mod completion;
mod embedding;
mod image;
pub mod provider;
mod transcription;
mod verify;

// Provider traits
pub use provider::{Capabilities, Capability, DebugExt, Provider, ProviderBuilder};

// Client capability traits
pub use completion::CompletionClient;
pub use embedding::EmbeddingsClient;
pub use transcription::TranscriptionClient;
pub use verify::VerifyClient;

#[cfg(feature = "image")]
pub use image::ImageGenerationClient;

#[cfg(feature = "audio")]
pub use audio::AudioGenerationClient;

// Re-export Capable from core for convenience
pub use super::core::Capable;
