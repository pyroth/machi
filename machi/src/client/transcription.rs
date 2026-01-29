use crate::modalities::audio::transcription::TranscriptionModel;

/// A provider client with transcription capabilities.
pub trait TranscriptionClient {
    /// The type of TranscriptionModel used by the Client
    type TranscriptionModel: TranscriptionModel;

    /// Create a transcription model with the given name.
    fn transcription_model(&self, model: impl Into<String>) -> Self::TranscriptionModel;
}
