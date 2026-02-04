//! Session manager for handling conversation state.

use super::storage::{HistoryMessage, SessionData, SessionStorage};
use crate::error::StorageResult;
use crate::util::timestamp_ms;
use std::sync::Arc;
use tracing::{debug, info};

/// Configuration for session management.
#[derive(Debug, Clone, Copy)]
pub struct SessionConfig {
    /// Maximum number of messages to keep in history.
    pub max_history_length: usize,
    /// Whether to auto-save after each message.
    pub auto_save: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            max_history_length: 50,
            auto_save: true,
        }
    }
}

/// A conversation session.
#[derive(Debug)]
pub struct Session {
    data: SessionData,
    config: SessionConfig,
    modified: bool,
}

impl Session {
    /// Create a new session with the given key.
    fn new(key: impl Into<String>, config: SessionConfig) -> Self {
        Self {
            data: SessionData::new(key),
            config,
            modified: false,
        }
    }

    /// Create a session from existing data.
    const fn from_data(data: SessionData, config: SessionConfig) -> Self {
        Self {
            data,
            config,
            modified: false,
        }
    }

    /// Get the session key.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.data.key
    }

    /// Get the conversation history.
    #[must_use]
    pub fn history(&self) -> &[HistoryMessage] {
        &self.data.history
    }

    /// Get history formatted for LLM (as Vec of role/content maps).
    #[must_use]
    pub fn get_history(&self) -> Vec<serde_json::Value> {
        self.data
            .history
            .iter()
            .map(|msg| {
                serde_json::json!({
                    "role": msg.role,
                    "content": msg.content
                })
            })
            .collect()
    }

    /// Add a message to the history.
    pub fn add_message(&mut self, role: impl Into<String>, content: impl Into<String>) {
        let msg = HistoryMessage::new(role, content);
        self.data.history.push(msg);
        self.data.updated_at = timestamp_ms();
        self.modified = true;

        // Trim history if too long
        if self.data.history.len() > self.config.max_history_length {
            let trim_count = self.data.history.len() - self.config.max_history_length;
            self.data.history.drain(0..trim_count);
        }
    }

    /// Add a user message.
    pub fn add_user_message(&mut self, content: impl Into<String>) {
        self.add_message("user", content);
    }

    /// Add an assistant message.
    pub fn add_assistant_message(&mut self, content: impl Into<String>) {
        self.add_message("assistant", content);
    }

    /// Clear the conversation history.
    pub fn clear_history(&mut self) {
        self.data.history.clear();
        self.data.updated_at = timestamp_ms();
        self.modified = true;
    }

    /// Check if the session has been modified since last save.
    #[must_use]
    pub const fn is_modified(&self) -> bool {
        self.modified
    }

    /// Mark the session as saved.
    const fn mark_saved(&mut self) {
        self.modified = false;
    }

    /// Get the underlying data for storage.
    const fn data(&self) -> &SessionData {
        &self.data
    }

    /// Get session metadata.
    #[must_use]
    pub const fn metadata(&self) -> &serde_json::Value {
        &self.data.metadata
    }

    /// Set session metadata.
    pub fn set_metadata(&mut self, metadata: serde_json::Value) {
        self.data.metadata = metadata;
        self.modified = true;
    }
}

/// Session manager for creating and managing sessions.
pub struct SessionManager {
    storage: Arc<dyn SessionStorage>,
    config: SessionConfig,
}

impl std::fmt::Debug for SessionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionManager")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl SessionManager {
    /// Create a new session manager with the given storage backend.
    pub fn new(storage: impl SessionStorage + 'static) -> Self {
        Self {
            storage: Arc::new(storage),
            config: SessionConfig::default(),
        }
    }

    /// Create a session manager with custom config.
    pub fn with_config(storage: impl SessionStorage + 'static, config: SessionConfig) -> Self {
        Self {
            storage: Arc::new(storage),
            config,
        }
    }

    /// Get or create a session by key.
    pub async fn get_or_create(&self, key: &str) -> StorageResult<Session> {
        if let Some(data) = self.storage.load(key).await? {
            debug!(key = %key, "loaded existing session");
            Ok(Session::from_data(data, self.config))
        } else {
            debug!(key = %key, "created new session");
            Ok(Session::new(key, self.config))
        }
    }

    /// Save a session.
    pub async fn save(&self, session: &mut Session) -> StorageResult<()> {
        self.storage.save(session.data()).await?;
        session.mark_saved();
        debug!(key = %session.key(), "session saved");
        Ok(())
    }

    /// Delete a session.
    pub async fn delete(&self, key: &str) -> StorageResult<()> {
        self.storage.delete(key).await?;
        info!(key = %key, "session deleted");
        Ok(())
    }

    /// List all session keys.
    pub async fn list(&self) -> StorageResult<Vec<String>> {
        self.storage.list_keys().await
    }

    /// Get the session configuration.
    #[must_use]
    pub const fn config(&self) -> &SessionConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::super::storage::MemoryStorage;
    use super::*;

    #[tokio::test]
    async fn test_session_manager() {
        let manager = SessionManager::new(MemoryStorage::new());

        // Get or create
        let mut session = manager.get_or_create("test:123").await.unwrap();
        assert_eq!(session.key(), "test:123");
        assert!(session.history().is_empty());

        // Add messages
        session.add_user_message("Hello");
        session.add_assistant_message("Hi there!");
        assert_eq!(session.history().len(), 2);
        assert!(session.is_modified());

        // Save
        manager.save(&mut session).await.unwrap();
        assert!(!session.is_modified());

        // Reload
        let reloaded = manager.get_or_create("test:123").await.unwrap();
        assert_eq!(reloaded.history().len(), 2);
    }

    #[test]
    fn test_session_history_trim() {
        let config = SessionConfig {
            max_history_length: 3,
            auto_save: false,
        };
        let mut session = Session::new("test", config);

        session.add_message("user", "1");
        session.add_message("assistant", "2");
        session.add_message("user", "3");
        session.add_message("assistant", "4");

        assert_eq!(session.history().len(), 3);
        assert_eq!(session.history()[0].content, "2");
    }
}
