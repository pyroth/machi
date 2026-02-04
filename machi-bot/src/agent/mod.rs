//! Agent loop for processing messages.
//!
//! Agent module for LLM-powered message processing.
//!
//! This module provides the core agent loop that processes inbound messages
//! using LLM models and tools, managing conversation context and sessions.

pub mod confirmation;
mod context;
mod loop_runner;

pub use confirmation::{
    AutoApproveHandler, CliConfirmationHandler, ConfirmationHandler, ConfirmationManager,
    ConfirmationRequest, ConfirmationResponse, TelegramConfirmationHandler,
};
pub use context::{ContextBuilder, MessageRole};
pub use loop_runner::{AgentLoop, AgentLoopBuilder, AgentLoopConfig};
