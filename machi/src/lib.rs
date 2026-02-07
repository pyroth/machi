//! Machi - A Rust framework for building AI agents
//!
//! This crate provides a lightweight, ergonomic framework for building AI agents
//! that can use tools and interact with language models.

#[cfg(feature = "a2a")]
pub mod a2a;
pub mod agent;
pub mod audio;
pub mod callback;
pub mod chat;
pub mod embedding;
pub mod error;
pub mod guardrail;
pub mod llms;
#[cfg(feature = "mcp")]
pub mod mcp;
pub mod memory;
pub mod message;
pub mod prelude;
pub mod stream;
pub mod tool;
#[cfg(feature = "toolkit")]
pub mod tools;
pub mod usage;
#[cfg(feature = "wallet")]
pub mod wallet;

pub use error::{Error, Result};

// Re-export submodule error types at crate root for convenience.
pub use agent::AgentError;
pub use llms::{LlmError, LlmErrorKind};
pub use memory::MemoryError;
pub use tool::ToolError;
#[cfg(feature = "wallet")]
pub use wallet::WalletError;

#[cfg(feature = "derive")]
pub use machi_derive::tool;
