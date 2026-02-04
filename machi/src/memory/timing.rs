//! Timing information for agent steps.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Timing information for a step.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Timing {
    /// Start time of the step.
    pub start_time: DateTime<Utc>,
    /// End time of the step (if completed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<DateTime<Utc>>,
}

impl Timing {
    /// Create a new timing starting now.
    #[must_use]
    pub fn start_now() -> Self {
        Self {
            start_time: Utc::now(),
            end_time: None,
        }
    }

    /// Mark the timing as complete.
    pub fn complete(&mut self) {
        self.end_time = Some(Utc::now());
    }

    /// Get the duration in seconds.
    #[must_use]
    pub fn duration_secs(&self) -> Option<f64> {
        self.end_time
            .map(|end| (end - self.start_time).num_milliseconds() as f64 / 1000.0)
    }
}

impl Default for Timing {
    fn default() -> Self {
        Self::start_now()
    }
}
