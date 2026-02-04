//! Session storage backends.
//!
//! Provides different storage implementations for session persistence.

use crate::error::StorageResult;
use crate::util::timestamp_ms;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::RwLock;
use tracing::debug;

/// A single message in conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryMessage {
    /// Message role: "user", "assistant", or "system".
    pub role: String,
    /// Message content.
    pub content: String,
    /// Timestamp (Unix milliseconds).
    pub timestamp: u64,
}

impl HistoryMessage {
    /// Create a new history message.
    #[must_use]
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            timestamp: timestamp_ms(),
        }
    }

    /// Create a user message.
    #[must_use]
    pub fn user(content: impl Into<String>) -> Self {
        Self::new("user", content)
    }

    /// Create an assistant message.
    #[must_use]
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new("assistant", content)
    }

    /// Create a system message.
    #[must_use]
    pub fn system(content: impl Into<String>) -> Self {
        Self::new("system", content)
    }
}

/// Session data stored in storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    /// Session key (e.g., "telegram:123456").
    pub key: String,
    /// Conversation history.
    pub history: Vec<HistoryMessage>,
    /// Session creation timestamp.
    pub created_at: u64,
    /// Last activity timestamp.
    pub updated_at: u64,
    /// Custom metadata.
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl SessionData {
    /// Create a new empty session.
    #[must_use]
    pub fn new(key: impl Into<String>) -> Self {
        let now = timestamp_ms();
        Self {
            key: key.into(),
            history: Vec::new(),
            created_at: now,
            updated_at: now,
            metadata: serde_json::Value::Null,
        }
    }
}

/// Trait for session storage backends.
#[async_trait]
pub trait SessionStorage: Send + Sync {
    /// Load a session by key.
    async fn load(&self, key: &str) -> StorageResult<Option<SessionData>>;

    /// Save a session.
    async fn save(&self, session: &SessionData) -> StorageResult<()>;

    /// Delete a session.
    async fn delete(&self, key: &str) -> StorageResult<()>;

    /// List all session keys.
    async fn list_keys(&self) -> StorageResult<Vec<String>>;

    /// Check if a session exists.
    async fn exists(&self, key: &str) -> StorageResult<bool> {
        Ok(self.load(key).await?.is_some())
    }
}

/// In-memory session storage.
///
/// Fast but not persistent across restarts.
#[derive(Debug, Default)]
pub struct MemoryStorage {
    sessions: RwLock<HashMap<String, SessionData>>,
}

impl MemoryStorage {
    /// Create a new memory storage.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SessionStorage for MemoryStorage {
    async fn load(&self, key: &str) -> StorageResult<Option<SessionData>> {
        Ok(self.sessions.read().await.get(key).cloned())
    }

    async fn save(&self, session: &SessionData) -> StorageResult<()> {
        self.sessions
            .write()
            .await
            .insert(session.key.clone(), session.clone());
        Ok(())
    }

    async fn delete(&self, key: &str) -> StorageResult<()> {
        self.sessions.write().await.remove(key);
        Ok(())
    }

    async fn list_keys(&self) -> StorageResult<Vec<String>> {
        Ok(self.sessions.read().await.keys().cloned().collect())
    }
}

/// File-based session storage.
///
/// Persists sessions as JSON files in a directory.
#[derive(Debug)]
pub struct FileStorage {
    base_path: PathBuf,
}

impl FileStorage {
    /// Create a new file storage with the given base path.
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    /// Create file storage in the default location (~/.machi-bot/sessions).
    #[must_use]
    pub fn default_path() -> Self {
        let path = dirs_next::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".machi-bot")
            .join("sessions");
        Self::new(path)
    }

    /// Get the file path for a session key.
    fn session_path(&self, key: &str) -> PathBuf {
        // Sanitize key for filename
        let safe_key = key.replace([':', '/', '\\'], "_");
        self.base_path.join(format!("{safe_key}.json"))
    }

    /// Ensure the storage directory exists.
    async fn ensure_dir(&self) -> StorageResult<()> {
        tokio::fs::create_dir_all(&self.base_path).await?;
        Ok(())
    }
}

#[async_trait]
impl SessionStorage for FileStorage {
    async fn load(&self, key: &str) -> StorageResult<Option<SessionData>> {
        let path = self.session_path(key);

        if !path.exists() {
            return Ok(None);
        }

        let content = tokio::fs::read_to_string(&path).await?;
        let session: SessionData = serde_json::from_str(&content)?;
        debug!(key = %key, "loaded session from file");
        Ok(Some(session))
    }

    async fn save(&self, session: &SessionData) -> StorageResult<()> {
        self.ensure_dir().await?;

        let path = self.session_path(&session.key);
        let content = serde_json::to_string_pretty(session)?;
        tokio::fs::write(&path, content).await?;
        debug!(key = %session.key, "saved session to file");
        Ok(())
    }

    async fn delete(&self, key: &str) -> StorageResult<()> {
        let path = self.session_path(key);

        if path.exists() {
            tokio::fs::remove_file(&path).await?;
            debug!(key = %key, "deleted session file");
        }
        Ok(())
    }

    async fn list_keys(&self) -> StorageResult<Vec<String>> {
        self.ensure_dir().await?;

        let mut keys = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.base_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json")
                && let Some(stem) = path.file_stem()
            {
                // Restore original key format
                let key = stem.to_string_lossy().replace('_', ":");
                keys.push(key);
            }
        }

        Ok(keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_storage() {
        let storage = MemoryStorage::new();

        // Save
        let mut session = SessionData::new("test:123");
        session.history.push(HistoryMessage::user("Hello"));
        storage.save(&session).await.unwrap();

        // Load
        let loaded = storage.load("test:123").await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().history.len(), 1);

        // List
        let keys = storage.list_keys().await.unwrap();
        assert_eq!(keys.len(), 1);

        // Delete
        storage.delete("test:123").await.unwrap();
        assert!(storage.load("test:123").await.unwrap().is_none());
    }

    #[test]
    fn test_history_message() {
        let msg = HistoryMessage::user("Hello");
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "Hello");
        assert!(msg.timestamp > 0);
    }
}
