//! Error types for the memory subsystem.
//!
//! [`MemoryError`] covers all session operation failure modes and integrates
//! into the global [`Error`](crate::Error) hierarchy via `Error::Memory`.

/// Error type for memory/session operations.
///
/// Each variant represents a distinct failure mode, enabling callers to
/// pattern-match on specific cases (e.g., retrying transient storage errors).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MemoryError {
    /// JSON serialization or deserialization of a [`Message`](crate::message::Message) failed.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// The storage backend encountered an error.
    #[error("[{backend}] {message}")]
    Storage {
        /// Backend identifier (e.g., `"sqlite"`, `"redis"`).
        backend: &'static str,
        /// Human-readable error description.
        message: String,
    },

    /// Failed to acquire a lock (e.g., `Mutex` poisoned by a panic).
    #[error("lock error: {0}")]
    Lock(String),

    /// An async task failed to join (`spawn_blocking` panicked or was cancelled).
    #[error("task error: {0}")]
    Task(String),
}

impl MemoryError {
    /// Creates a [`Storage`](Self::Storage) error for the given backend.
    #[must_use]
    pub fn storage(backend: &'static str, message: impl Into<String>) -> Self {
        Self::Storage {
            backend,
            message: message.into(),
        }
    }

    /// Returns `true` if this is a transient error that may succeed on retry.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(self, Self::Lock(_) | Self::Task(_))
    }
}

/// Enables `?` on [`rusqlite::Error`] inside closures returning [`MemoryError`].
///
/// `#[from]` cannot be used here because the conversion targets the [`Storage`](MemoryError::Storage)
/// variant which requires a hardcoded `backend` field.
#[cfg(feature = "memory-sqlite")]
impl From<rusqlite::Error> for MemoryError {
    fn from(e: rusqlite::Error) -> Self {
        Self::storage("sqlite", e.to_string())
    }
}

/// Convenience alias for memory-scoped results.
pub type MemoryResult<T> = Result<T, MemoryError>;
