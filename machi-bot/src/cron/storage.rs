//! Storage backends for cron jobs.

use super::job::{CronJob, CronJobId};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Error type for cron storage operations.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("job not found: {0}")]
    NotFound(CronJobId),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Result type for cron storage operations.
pub type StorageResult<T> = Result<T, StorageError>;

/// Trait for cron job storage backends.
#[async_trait]
pub trait CronStorage: Send + Sync {
    /// List all jobs.
    async fn list(&self) -> StorageResult<Vec<CronJob>>;

    /// Get a job by ID.
    async fn get(&self, id: &CronJobId) -> StorageResult<Option<CronJob>>;

    /// Save a job (insert or update).
    async fn save(&self, job: &CronJob) -> StorageResult<()>;

    /// Delete a job by ID.
    async fn delete(&self, id: &CronJobId) -> StorageResult<()>;
}

/// In-memory cron job storage.
#[derive(Debug, Default)]
pub struct MemoryCronStorage {
    jobs: Arc<RwLock<HashMap<CronJobId, CronJob>>>,
}

impl MemoryCronStorage {
    /// Create a new memory storage.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl CronStorage for MemoryCronStorage {
    async fn list(&self) -> StorageResult<Vec<CronJob>> {
        let jobs = self.jobs.read().await;
        Ok(jobs.values().cloned().collect())
    }

    async fn get(&self, id: &CronJobId) -> StorageResult<Option<CronJob>> {
        let jobs = self.jobs.read().await;
        Ok(jobs.get(id).cloned())
    }

    async fn save(&self, job: &CronJob) -> StorageResult<()> {
        let mut jobs = self.jobs.write().await;
        jobs.insert(job.id.clone(), job.clone());
        Ok(())
    }

    async fn delete(&self, id: &CronJobId) -> StorageResult<()> {
        let mut jobs = self.jobs.write().await;
        jobs.remove(id);
        Ok(())
    }
}

/// File-based cron job storage.
#[derive(Debug)]
pub struct FileCronStorage {
    path: PathBuf,
    cache: Arc<RwLock<HashMap<CronJobId, CronJob>>>,
}

impl FileCronStorage {
    /// Create a new file storage at the given path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Load jobs from file into cache.
    async fn load(&self) -> StorageResult<()> {
        if !self.path.exists() {
            return Ok(());
        }

        let content = tokio::fs::read_to_string(&self.path).await?;
        let jobs: Vec<CronJob> = serde_json::from_str(&content)?;

        let mut cache = self.cache.write().await;
        cache.clear();
        for job in jobs {
            cache.insert(job.id.clone(), job);
        }

        Ok(())
    }

    /// Save cache to file.
    async fn persist(&self) -> StorageResult<()> {
        let cache = self.cache.read().await;
        let jobs: Vec<&CronJob> = cache.values().collect();
        let content = serde_json::to_string_pretty(&jobs)?;

        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&self.path, content).await?;
        Ok(())
    }

    /// Initialize storage by loading from file.
    pub async fn init(&self) -> StorageResult<()> {
        self.load().await
    }
}

#[async_trait]
impl CronStorage for FileCronStorage {
    async fn list(&self) -> StorageResult<Vec<CronJob>> {
        let cache = self.cache.read().await;
        Ok(cache.values().cloned().collect())
    }

    async fn get(&self, id: &CronJobId) -> StorageResult<Option<CronJob>> {
        let cache = self.cache.read().await;
        Ok(cache.get(id).cloned())
    }

    async fn save(&self, job: &CronJob) -> StorageResult<()> {
        {
            let mut cache = self.cache.write().await;
            cache.insert(job.id.clone(), job.clone());
        }
        self.persist().await
    }

    async fn delete(&self, id: &CronJobId) -> StorageResult<()> {
        {
            let mut cache = self.cache.write().await;
            cache.remove(id);
        }
        self.persist().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cron::job::CronSchedule;

    #[tokio::test]
    async fn test_memory_storage() {
        let storage = MemoryCronStorage::new();

        let job = CronJob::builder()
            .name("Test")
            .message("Hello")
            .schedule(CronSchedule::every(60))
            .build();

        // Save
        storage.save(&job).await.unwrap();

        // Get
        let retrieved = storage.get(&job.id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Test");

        // List
        let jobs = storage.list().await.unwrap();
        assert_eq!(jobs.len(), 1);

        // Delete
        storage.delete(&job.id).await.unwrap();
        let jobs = storage.list().await.unwrap();
        assert!(jobs.is_empty());
    }
}
