//! Image generation client trait.

#[cfg(feature = "image")]
mod inner {
    use crate::modalities::image::generation::ImageGenerationModel;

    /// A provider client with image generation capabilities.
    pub trait ImageGenerationClient {
        /// The image generation model type used by this client.
        type ImageGenerationModel: ImageGenerationModel<Client = Self>;

        /// Creates an image generation model with the given model identifier.
        fn image_generation_model(&self, model: impl Into<String>) -> Self::ImageGenerationModel;

        /// Creates a custom image generation model with the given model identifier.
        fn custom_image_generation_model(
            &self,
            model: impl Into<String>,
        ) -> Self::ImageGenerationModel {
            Self::ImageGenerationModel::make(self, model)
        }
    }
}

#[cfg(feature = "image")]
pub use inner::*;
