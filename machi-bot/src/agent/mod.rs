//! Agent loop for processing messages.
//!
//! Agent module for LLM-powered message processing.
//!
//! This module provides the core agent loop that processes inbound messages
//! using LLM models and tools, managing conversation context and sessions.

mod context;
mod loop_runner;

pub use context::{ContextBuilder, MessageRole};
pub use loop_runner::{AgentLoop, AgentLoopBuilder, AgentLoopConfig};
