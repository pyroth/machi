//! Transcription client trait.

use crate::modalities::audio::transcription::TranscriptionModel;

/// A provider client with transcription capabilities.
pub trait TranscriptionClient {
    /// The transcription model type used by this client.
    type TranscriptionModel: TranscriptionModel;

    /// Creates a transcription model with the given model identifier.
    fn transcription_model(&self, model: impl Into<String>) -> Self::TranscriptionModel;
}
