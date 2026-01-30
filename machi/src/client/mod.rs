//! Provider client infrastructure.
//!
//! This module provides the core abstractions for interacting with LLM providers.
//! It defines a unified [`Client`] type that can be configured for different
//! providers (`OpenAI`, Anthropic, etc.) while maintaining a consistent API.
//!
//! # Architecture
//!
//! - **[`Client`]**: The main client structure, generic over provider extensions
//! - **[`ClientBuilder`]**: Fluent builder for constructing clients
//! - **[`traits`]**: Provider capability traits (completion, embeddings, etc.)
//!
//! # Example
//!
//! ```rust,ignore
//! use machi::providers::openai;
//!
//! // Create a client from environment
//! let client = openai::Client::from_env();
//!
//! // Or with explicit API key
//! let client = openai::Client::new("your-api-key")?;
//!
//! // Create models
//! let completion_model = client.completion_model("gpt-4o");
//! let embedding_model = client.embedding_model("text-embedding-ada-002");
//! ```

mod auth;
mod builder;
mod core;
pub mod error;
mod response;
pub mod traits;

// Core types
pub use builder::{ClientBuilder, NeedsApiKey};
pub use core::{Capable, Client, Transport};

// Auth types
pub use auth::{ApiKey, BearerAuth, Nothing};

// Error types
pub use error::{ClientBuilderError, VerifyError};

// Response types
pub use response::FinalCompletionResponse;

// Trait re-exports for convenience
pub use traits::provider::ProviderClient;
pub use traits::{
    Capabilities, Capability, CompletionClient, DebugExt, EmbeddingsClient, Provider,
    ProviderBuilder, VerifyClient,
};

#[cfg(feature = "image")]
pub use traits::ImageGenerationClient;

#[cfg(feature = "audio")]
pub use traits::AudioGenerationClient;

// Transcription client
pub use traits::TranscriptionClient;

// Feature-gated imports for trait implementations
#[cfg(feature = "image")]
use crate::image_generation::ImageGenerationModel;

#[cfg(feature = "audio")]
use crate::audio_generation::AudioGenerationModel;

use crate::{
    completion::CompletionModel, core::wasm_compat::WasmCompatSend, embedding::EmbeddingModel,
    modalities::audio::transcription::TranscriptionModel,
};

// Trait implementations for Client

impl<M, Ext, H> CompletionClient for Client<Ext, H>
where
    Ext: Capabilities<H, Completion = Capable<M>>,
    M: CompletionModel<Client = Self>,
{
    type CompletionModel = M;

    fn completion_model(&self, model: impl Into<String>) -> Self::CompletionModel {
        M::make(self, model)
    }
}

impl<M, Ext, H> EmbeddingsClient for Client<Ext, H>
where
    Ext: Capabilities<H, Embeddings = Capable<M>>,
    M: EmbeddingModel<Client = Self>,
{
    type EmbeddingModel = M;

    fn embedding_model(&self, model: impl Into<String>) -> Self::EmbeddingModel {
        M::make(self, model, None)
    }

    fn embedding_model_with_ndims(
        &self,
        model: impl Into<String>,
        ndims: usize,
    ) -> Self::EmbeddingModel {
        M::make(self, model, Some(ndims))
    }
}

impl<M, Ext, H> TranscriptionClient for Client<Ext, H>
where
    Ext: Capabilities<H, Transcription = Capable<M>>,
    M: TranscriptionModel<Client = Self> + WasmCompatSend,
{
    type TranscriptionModel = M;

    fn transcription_model(&self, model: impl Into<String>) -> Self::TranscriptionModel {
        M::make(self, model)
    }
}

#[cfg(feature = "image")]
impl<M, Ext, H> ImageGenerationClient for Client<Ext, H>
where
    Ext: Capabilities<H, ImageGeneration = Capable<M>>,
    M: ImageGenerationModel<Client = Self>,
{
    type ImageGenerationModel = M;

    fn image_generation_model(&self, model: impl Into<String>) -> Self::ImageGenerationModel {
        M::make(self, model)
    }
}

#[cfg(feature = "audio")]
impl<M, Ext, H> AudioGenerationClient for Client<Ext, H>
where
    Ext: Capabilities<H, AudioGeneration = Capable<M>>,
    M: AudioGenerationModel<Client = Self>,
{
    type AudioGenerationModel = M;

    fn audio_generation_model(&self, model: impl Into<String>) -> Self::AudioGenerationModel {
        M::make(self, model)
    }
}
