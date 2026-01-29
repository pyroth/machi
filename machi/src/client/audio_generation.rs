#[cfg(feature = "audio")]
mod audio {
    use crate::modalities::audio::generation::AudioGenerationModel;

    /// A provider client with audio generation capabilities.
    pub trait AudioGenerationClient {
        /// The `AudioGenerationModel` used by the Client
        type AudioGenerationModel: AudioGenerationModel<Client = Self>;

        /// Create an audio generation model with the given name.
        fn audio_generation_model(&self, model: impl Into<String>) -> Self::AudioGenerationModel {
            Self::AudioGenerationModel::make(self, model)
        }
    }
}

#[cfg(feature = "audio")]
pub use audio::*;
