//! Completion client trait.

use crate::agent::AgentBuilder;
use crate::completion::CompletionModel;
use crate::extract::ExtractorBuilder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A provider client with completion capabilities.
pub trait CompletionClient {
    /// The completion model type used by this client.
    type CompletionModel: CompletionModel<Client = Self>;

    /// Creates a completion model with the given model identifier.
    ///
    /// # Example
    /// ```rust,ignore
    /// use machi::prelude::*;
    /// use machi::providers::openai::{Client, self};
    ///
    /// let openai = Client::new("your-api-key");
    /// let gpt4 = openai.completion_model(openai::GPT4);
    /// ```
    fn completion_model(&self, model: impl Into<String>) -> Self::CompletionModel {
        Self::CompletionModel::make(self, model)
    }

    /// Creates an agent builder with the given completion model.
    ///
    /// # Example
    /// ```rust,ignore
    /// use machi::prelude::*;
    /// use machi::providers::openai::{Client, self};
    ///
    /// let openai = Client::new("your-api-key");
    /// let agent = openai.agent(openai::GPT_4)
    ///    .preamble("You are a comedian AI.")
    ///    .temperature(0.9)
    ///    .build();
    /// ```
    fn agent(&self, model: impl Into<String>) -> AgentBuilder<Self::CompletionModel> {
        AgentBuilder::new(self.completion_model(model))
    }

    /// Creates an extractor builder with the given completion model.
    fn extractor<T>(&self, model: impl Into<String>) -> ExtractorBuilder<Self::CompletionModel, T>
    where
        T: JsonSchema + for<'a> Deserialize<'a> + Serialize + Send + Sync,
    {
        ExtractorBuilder::new(self.completion_model(model))
    }
}
