//! Cron job scheduler.

use super::job::{CronJob, CronJobId, JobStatus};
use super::storage::{CronStorage, StorageResult};
use crate::bus::MessageBus;
use crate::events::InboundMessage;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info, warn};

/// Handle for controlling the scheduler.
#[derive(Debug, Clone)]
pub struct SchedulerHandle {
    shutdown_tx: mpsc::Sender<()>,
}

impl SchedulerHandle {
    /// Signal the scheduler to stop.
    pub async fn stop(&self) {
        let _ = self.shutdown_tx.send(()).await;
    }
}

/// Cron job scheduler that runs jobs on their schedules.
#[derive(Debug)]
pub struct CronScheduler<S: CronStorage> {
    storage: Arc<S>,
    bus: MessageBus,
    check_interval: Duration,
    running: Arc<RwLock<bool>>,
}

impl<S: CronStorage + 'static> CronScheduler<S> {
    /// Create a new scheduler.
    pub fn new(storage: S, bus: MessageBus) -> Self {
        Self {
            storage: Arc::new(storage),
            bus,
            check_interval: Duration::from_secs(10),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Set the interval between job checks.
    #[must_use]
    pub const fn with_check_interval(mut self, interval: Duration) -> Self {
        self.check_interval = interval;
        self
    }

    /// Add a new job.
    pub async fn add_job(&self, job: CronJob) -> StorageResult<CronJobId> {
        let id = job.id.clone();
        self.storage.save(&job).await?;
        info!(job_id = %id, name = %job.name, "added cron job");
        Ok(id)
    }

    /// Remove a job by ID.
    pub async fn remove_job(&self, id: &CronJobId) -> StorageResult<()> {
        self.storage.delete(id).await?;
        info!(job_id = %id, "removed cron job");
        Ok(())
    }

    /// List all jobs.
    pub async fn list_jobs(&self) -> StorageResult<Vec<CronJob>> {
        self.storage.list().await
    }

    /// Get a job by ID.
    pub async fn get_job(&self, id: &CronJobId) -> StorageResult<Option<CronJob>> {
        self.storage.get(id).await
    }

    /// Pause a job.
    pub async fn pause_job(&self, id: &CronJobId) -> StorageResult<()> {
        if let Some(mut job) = self.storage.get(id).await? {
            job.pause();
            self.storage.save(&job).await?;
            info!(job_id = %id, "paused cron job");
        }
        Ok(())
    }

    /// Resume a paused job.
    pub async fn resume_job(&self, id: &CronJobId) -> StorageResult<()> {
        if let Some(mut job) = self.storage.get(id).await? {
            job.resume();
            self.storage.save(&job).await?;
            info!(job_id = %id, "resumed cron job");
        }
        Ok(())
    }

    /// Start the scheduler loop.
    ///
    /// Returns a handle that can be used to stop the scheduler.
    #[allow(clippy::unused_async)] // async is part of the public API contract
    pub async fn start(self) -> SchedulerHandle {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        let handle = SchedulerHandle { shutdown_tx };

        let storage = Arc::clone(&self.storage);
        let bus = self.bus.clone();
        let interval = self.check_interval;
        let running = Arc::clone(&self.running);

        tokio::spawn(async move {
            *running.write().await = true;
            info!("Cron scheduler started");

            loop {
                tokio::select! {
                    () = tokio::time::sleep(interval) => {
                        if let Err(e) = Self::check_and_run_jobs(&storage, &bus).await {
                            error!(error = %e, "error checking cron jobs");
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Cron scheduler shutting down");
                        break;
                    }
                }
            }

            *running.write().await = false;
        });

        handle
    }

    /// Check for due jobs and run them.
    async fn check_and_run_jobs(storage: &Arc<S>, bus: &MessageBus) -> StorageResult<()> {
        let now = std::time::SystemTime::now();
        let jobs = storage.list().await?;

        for mut job in jobs {
            if job.should_run(now) {
                debug!(job_id = %job.id, name = %job.name, "running cron job");

                // Create an inbound message to trigger the agent
                let msg = InboundMessage::new(
                    &job.channel,
                    "cron",
                    &job.chat_id,
                    format!("[Scheduled Task: {}] {}", job.name, job.message),
                );

                // Publish to bus
                if let Err(e) = bus.publish_inbound(msg).await {
                    warn!(
                        job_id = %job.id,
                        error = %e,
                        "failed to publish cron job message"
                    );
                    job.status = JobStatus::Failed;
                } else {
                    job.mark_run();
                }

                // Save updated job state
                storage.save(&job).await?;
            }
        }

        Ok(())
    }

    /// Check if the scheduler is running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cron::storage::MemoryCronStorage;

    #[tokio::test]
    async fn test_scheduler_add_remove() {
        let storage = MemoryCronStorage::new();
        let bus = MessageBus::new();
        let scheduler = CronScheduler::new(storage, bus);

        let job = CronJob::builder()
            .name("Test Job")
            .message("Hello!")
            .every(3600)
            .build();

        // Add
        let id = scheduler.add_job(job).await.unwrap();

        // List
        let jobs = scheduler.list_jobs().await.unwrap();
        assert_eq!(jobs.len(), 1);

        // Remove
        scheduler.remove_job(&id).await.unwrap();
        let jobs = scheduler.list_jobs().await.unwrap();
        assert!(jobs.is_empty());
    }

    #[tokio::test]
    async fn test_scheduler_pause_resume() {
        let storage = MemoryCronStorage::new();
        let bus = MessageBus::new();
        let scheduler = CronScheduler::new(storage, bus);

        let job = CronJob::builder()
            .name("Test")
            .message("Hello")
            .every(60)
            .build();

        let id = scheduler.add_job(job).await.unwrap();

        // Pause
        scheduler.pause_job(&id).await.unwrap();
        let job = scheduler.get_job(&id).await.unwrap().unwrap();
        assert_eq!(job.status, JobStatus::Paused);

        // Resume
        scheduler.resume_job(&id).await.unwrap();
        let job = scheduler.get_job(&id).await.unwrap().unwrap();
        assert_eq!(job.status, JobStatus::Active);
    }
}
