//! Audio generation client trait.

#[cfg(feature = "audio")]
mod inner {
    use crate::modalities::audio::generation::AudioGenerationModel;

    /// A provider client with audio generation capabilities.
    pub trait AudioGenerationClient {
        /// The audio generation model type used by this client.
        type AudioGenerationModel: AudioGenerationModel<Client = Self>;

        /// Creates an audio generation model with the given model identifier.
        fn audio_generation_model(&self, model: impl Into<String>) -> Self::AudioGenerationModel {
            Self::AudioGenerationModel::make(self, model)
        }
    }
}

#[cfg(feature = "audio")]
pub use inner::*;
