//! Cron job definitions and types.

use crate::util::generate_job_id;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

/// Unique identifier for a cron job.
pub type CronJobId = String;

/// Schedule specification for a cron job.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CronSchedule {
    /// Run at fixed intervals.
    Every {
        /// Interval in seconds.
        seconds: u64,
    },
    /// Run using cron expression (e.g., "0 9 * * *" for 9 AM daily).
    Cron {
        /// Cron expression string.
        expression: String,
    },
    /// Run once at a specific time.
    Once {
        /// Target execution time.
        at: SystemTime,
    },
}

impl CronSchedule {
    /// Create an interval schedule.
    #[must_use]
    pub const fn every(seconds: u64) -> Self {
        Self::Every { seconds }
    }

    /// Create a cron expression schedule.
    #[must_use]
    pub fn cron(expression: impl Into<String>) -> Self {
        Self::Cron {
            expression: expression.into(),
        }
    }

    /// Create a one-time schedule.
    #[must_use]
    pub const fn once(at: SystemTime) -> Self {
        Self::Once { at }
    }

    /// Calculate the next run time from a given time.
    #[must_use]
    pub fn next_run(&self, from: SystemTime) -> Option<SystemTime> {
        match self {
            Self::Every { seconds } => Some(from + Duration::from_secs(*seconds)),
            Self::Cron { expression } => parse_cron_next(expression, from),
            Self::Once { at } => {
                if *at > from {
                    Some(*at)
                } else {
                    None
                }
            }
        }
    }
}

/// Status of a cron job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum JobStatus {
    /// Job is active and will run on schedule.
    #[default]
    Active,
    /// Job is paused and will not run.
    Paused,
    /// Job has completed (for one-time jobs).
    Completed,
    /// Job encountered an error.
    Failed,
}

/// A scheduled cron job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    /// Unique job identifier.
    pub id: CronJobId,
    /// Human-readable name.
    pub name: String,
    /// Message or command to execute.
    pub message: String,
    /// Schedule specification.
    pub schedule: CronSchedule,
    /// Target channel for the message.
    pub channel: String,
    /// Target chat ID.
    pub chat_id: String,
    /// Current job status.
    pub status: JobStatus,
    /// When the job was created.
    pub created_at: SystemTime,
    /// When the job last ran.
    pub last_run: Option<SystemTime>,
    /// When the job will next run.
    pub next_run: Option<SystemTime>,
    /// Number of times the job has run.
    pub run_count: u64,
}

impl CronJob {
    /// Create a new cron job builder.
    #[must_use]
    pub fn builder() -> CronJobBuilder {
        CronJobBuilder::new()
    }

    /// Check if the job should run now.
    #[must_use]
    pub fn should_run(&self, now: SystemTime) -> bool {
        if self.status != JobStatus::Active {
            return false;
        }
        self.next_run.is_some_and(|next| next <= now)
    }

    /// Mark the job as having run and calculate next run time.
    pub fn mark_run(&mut self) {
        let now = SystemTime::now();
        self.last_run = Some(now);
        self.run_count += 1;

        // Calculate next run time
        self.next_run = self.schedule.next_run(now);

        // Mark one-time jobs as completed
        if matches!(self.schedule, CronSchedule::Once { .. }) {
            self.status = JobStatus::Completed;
        }
    }

    /// Pause the job.
    pub const fn pause(&mut self) {
        self.status = JobStatus::Paused;
    }

    /// Resume the job.
    pub fn resume(&mut self) {
        if self.status == JobStatus::Paused {
            self.status = JobStatus::Active;
            // Recalculate next run time
            self.next_run = self.schedule.next_run(SystemTime::now());
        }
    }
}

/// Builder for creating cron jobs.
#[derive(Debug, Default)]
pub struct CronJobBuilder {
    name: Option<String>,
    message: Option<String>,
    schedule: Option<CronSchedule>,
    channel: Option<String>,
    chat_id: Option<String>,
}

impl CronJobBuilder {
    /// Create a new builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the job name.
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the message to execute.
    #[must_use]
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Set the schedule.
    #[must_use]
    pub fn schedule(mut self, schedule: CronSchedule) -> Self {
        self.schedule = Some(schedule);
        self
    }

    /// Set to run every N seconds.
    #[must_use]
    pub fn every(self, seconds: u64) -> Self {
        self.schedule(CronSchedule::every(seconds))
    }

    /// Set cron expression.
    #[must_use]
    pub fn cron(self, expression: impl Into<String>) -> Self {
        self.schedule(CronSchedule::cron(expression))
    }

    /// Set target channel.
    #[must_use]
    pub fn channel(mut self, channel: impl Into<String>) -> Self {
        self.channel = Some(channel.into());
        self
    }

    /// Set target chat ID.
    #[must_use]
    pub fn chat_id(mut self, chat_id: impl Into<String>) -> Self {
        self.chat_id = Some(chat_id.into());
        self
    }

    /// Build the cron job.
    ///
    /// # Panics
    ///
    /// Panics if required fields are not set.
    #[must_use]
    pub fn build(self) -> CronJob {
        let now = SystemTime::now();
        let schedule = self.schedule.expect("schedule is required");
        let next_run = schedule.next_run(now);

        CronJob {
            id: generate_job_id(),
            name: self.name.unwrap_or_else(|| "Unnamed Job".to_string()),
            message: self.message.expect("message is required"),
            schedule,
            channel: self.channel.unwrap_or_else(|| "cli".to_string()),
            chat_id: self.chat_id.unwrap_or_else(|| "default".to_string()),
            status: JobStatus::Active,
            created_at: now,
            last_run: None,
            next_run,
            run_count: 0,
        }
    }
}

/// Parse a cron expression and calculate next run time.
///
/// Supports basic cron format: minute hour day month weekday
/// Examples:
/// - "0 9 * * *" - Every day at 9:00 AM
/// - "*/15 * * * *" - Every 15 minutes
/// - "0 0 * * 0" - Every Sunday at midnight
fn parse_cron_next(expression: &str, from: SystemTime) -> Option<SystemTime> {
    // Simplified cron parsing - in production use a cron parsing library
    if expression.split_whitespace().count() < 5 {
        return None;
    }

    // For now, just schedule 1 minute ahead as a placeholder
    // A full implementation would parse the cron expression properly
    Some(from + Duration::from_secs(60))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_builder() {
        let job = CronJob::builder()
            .name("Test Job")
            .message("Hello!")
            .every(3600)
            .channel("telegram")
            .chat_id("12345")
            .build();

        assert_eq!(job.name, "Test Job");
        assert_eq!(job.message, "Hello!");
        assert_eq!(job.channel, "telegram");
        assert_eq!(job.status, JobStatus::Active);
    }

    #[test]
    fn test_schedule_every() {
        let schedule = CronSchedule::every(60);
        let now = SystemTime::now();
        let next = schedule.next_run(now).unwrap();
        assert!(next > now);
    }

    #[test]
    fn test_job_should_run() {
        let mut job = CronJob::builder()
            .name("Test")
            .message("Test")
            .every(1)
            .build();

        // Manually set next_run to past
        job.next_run = Some(SystemTime::now() - Duration::from_secs(1));
        assert!(job.should_run(SystemTime::now()));

        // Paused job should not run
        job.pause();
        assert!(!job.should_run(SystemTime::now()));
    }
}
