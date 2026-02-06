//! Core session trait for persistent conversation memory.
//!
//! The [`Session`] trait defines an async interface for storing and retrieving
//! conversation history — the primary abstraction for agent memory.
//!
//! # Design
//!
//! - **Stateless agents** — history lives in the session, not the agent.
//! - **Messages as ground truth** — no separate metadata layer.
//! - **Backend-agnostic** — implement [`Session`] for any storage engine.

use async_trait::async_trait;

use crate::error::Result;
use crate::message::Message;

/// Async trait for session-based conversation memory.
///
/// Stores a chronological sequence of [`Message`]s identified by a unique
/// session ID. Implementations may be backed by in-memory storage, SQLite,
/// or any other persistent store.
///
/// All implementations must be `Send + Sync` for use across async tasks.
#[async_trait]
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement the `Session` trait",
    label = "this type cannot be used as a conversation session",
    note = "implement `Session` to provide conversation memory for agents"
)]
pub trait Session: Send + Sync {
    /// Returns the unique session identifier.
    fn id(&self) -> &str;

    /// Retrieves conversation history.
    ///
    /// - `limit: Some(n)` — returns the **latest** `n` messages in chronological order.
    /// - `limit: None` — returns all messages.
    async fn get_messages(&self, limit: Option<usize>) -> Result<Vec<Message>>;

    /// Appends messages to the conversation history in order.
    async fn add_messages(&self, messages: &[Message]) -> Result<()>;

    /// Removes and returns the most recent message.
    ///
    /// Returns `Ok(None)` if the session is empty. Useful for undo/correction workflows.
    async fn pop_message(&self) -> Result<Option<Message>>;

    /// Removes all messages from this session.
    async fn clear(&self) -> Result<()>;

    /// Returns the number of stored messages.
    async fn len(&self) -> Result<usize>;

    /// Returns `true` if this session contains no messages.
    async fn is_empty(&self) -> Result<bool> {
        Ok(self.len().await? == 0)
    }
}

/// A heap-allocated, type-erased session for dynamic dispatch.
pub type BoxedSession = Box<dyn Session>;

/// A shared, reference-counted session for use across tasks.
pub type SharedSession = std::sync::Arc<dyn Session>;
