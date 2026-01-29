#[cfg(feature = "image")]
mod image {
    use crate::modalities::image::generation::ImageGenerationModel;

    /// A provider client with image generation capabilities.
    pub trait ImageGenerationClient {
        /// The ImageGenerationModel used by the Client
        type ImageGenerationModel: ImageGenerationModel<Client = Self>;

        /// Create an image generation model with the given name.
        fn image_generation_model(&self, model: impl Into<String>) -> Self::ImageGenerationModel;

        /// Create a custom image generation model with the given name.
        fn custom_image_generation_model(
            &self,
            model: impl Into<String>,
        ) -> Self::ImageGenerationModel {
            Self::ImageGenerationModel::make(self, model)
        }
    }
}

#[cfg(feature = "image")]
pub use image::*;
