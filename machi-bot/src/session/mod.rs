//! Session management for conversation state.
//!
//! This module provides session tracking and persistence for multi-turn
//! conversations across different channels.

mod manager;
mod storage;

pub use manager::{Session, SessionManager};
pub use storage::{FileStorage, MemoryStorage, SessionStorage};
