//! Completion module for LLM text generation.
//!
//! This module provides the core abstractions for working with completion models:
//!
//! - **Traits**: [`Prompt`], [`Chat`], [`Completion`], [`CompletionModel`]
//! - **Messages**: [`Message`], [`AssistantContent`], [`UserContent`]
//! - **Requests**: [`CompletionRequest`], [`CompletionRequestBuilder`]
//! - **Streaming**: [`StreamingPrompt`], [`StreamingChat`], [`StreamingCompletion`]
//!
//! # Example
//!
//! ```rust,ignore
//! use machi::completion::{Prompt, CompletionModel};
//!
//! // Using a completion model directly
//! let response = model.completion_request("Hello!")
//!     .preamble("You are a helpful assistant.")
//!     .temperature(0.7)
//!     .send()
//!     .await?;
//! ```

pub mod error;
pub mod message;
pub mod request;
pub mod streaming;
pub mod traits;

pub use error::{CompletionError, MessageError, PromptError};
pub use message::{AssistantContent, Message};
pub use request::*;
pub use streaming::*;
pub use traits::{Chat, Completion, CompletionModel, GetTokenUsage, Prompt};
