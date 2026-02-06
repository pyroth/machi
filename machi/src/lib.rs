//! Machi - A Rust framework for building AI agents
//!
//! This crate provides a lightweight, ergonomic framework for building AI agents
//! that can use tools and interact with language models.

pub mod agent;
pub mod audio;
pub mod callback;
pub mod chat;
pub mod embedding;
pub mod error;
pub mod llms;
pub mod memory;
pub mod message;
pub mod prelude;
pub mod stream;
pub mod tool;
pub mod usage;

pub use error::{Error, LlmError, Result, ToolError};

#[cfg(feature = "derive")]
pub use machi_derive::tool;
