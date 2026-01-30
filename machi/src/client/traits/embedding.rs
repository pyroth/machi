//! Embeddings client trait.

use crate::embedding::{Embed, EmbeddingModel, EmbeddingsBuilder};

/// A provider client with embedding capabilities.
pub trait EmbeddingsClient {
    /// The embedding model type used by this client.
    type EmbeddingModel: EmbeddingModel;

    /// Creates an embedding model with the given model identifier.
    fn embedding_model(&self, model: impl Into<String>) -> Self::EmbeddingModel;

    /// Creates an embedding model with the given model identifier and dimensions.
    fn embedding_model_with_ndims(
        &self,
        model: impl Into<String>,
        ndims: usize,
    ) -> Self::EmbeddingModel;

    /// Creates an embedding builder with the given model.
    fn embeddings<D: Embed>(
        &self,
        model: impl Into<String>,
    ) -> EmbeddingsBuilder<Self::EmbeddingModel, D> {
        EmbeddingsBuilder::new(self.embedding_model(model))
    }

    /// Creates an embedding builder with the given model and dimensions.
    fn embeddings_with_ndims<D: Embed>(
        &self,
        model: &str,
        ndims: usize,
    ) -> EmbeddingsBuilder<Self::EmbeddingModel, D> {
        EmbeddingsBuilder::new(self.embedding_model_with_ndims(model, ndims))
    }
}
