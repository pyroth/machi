//! Callback context for passing agent state to handlers.

use std::sync::Arc;

/// Context passed to callbacks during agent execution.
///
/// Provides read-only access to agent state and metadata.
#[derive(Debug, Clone)]
pub struct CallbackContext {
    /// Name of the agent (if set).
    pub agent_name: Option<String>,
    /// Current step number.
    pub step_number: usize,
    /// Total maximum steps allowed.
    pub max_steps: usize,
    /// Whether the agent is in streaming mode.
    pub is_streaming: bool,
    /// Custom metadata.
    pub metadata: serde_json::Value,
}

impl CallbackContext {
    /// Create a new callback context.
    #[must_use]
    pub const fn new(step_number: usize, max_steps: usize) -> Self {
        Self {
            agent_name: None,
            step_number,
            max_steps,
            is_streaming: false,
            metadata: serde_json::Value::Null,
        }
    }

    /// Set the agent name.
    #[must_use]
    pub fn with_agent_name(mut self, name: impl Into<String>) -> Self {
        self.agent_name = Some(name.into());
        self
    }

    /// Set streaming mode.
    #[must_use]
    pub const fn with_streaming(mut self, streaming: bool) -> Self {
        self.is_streaming = streaming;
        self
    }

    /// Set custom metadata.
    #[must_use]
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Get progress as a fraction (0.0 to 1.0).
    #[must_use]
    pub fn progress(&self) -> f64 {
        if self.max_steps == 0 {
            0.0
        } else {
            (self.step_number as f64) / (self.max_steps as f64)
        }
    }

    /// Check if this is the first step.
    #[must_use]
    pub const fn is_first_step(&self) -> bool {
        self.step_number == 1
    }

    /// Check if approaching max steps (within last 20%).
    #[must_use]
    pub fn is_near_limit(&self) -> bool {
        self.progress() >= 0.8
    }
}

impl Default for CallbackContext {
    fn default() -> Self {
        Self::new(0, 20)
    }
}

/// Reference-counted callback context for shared access.
pub type CallbackContextRef = Arc<CallbackContext>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_progress() {
        let ctx = CallbackContext::new(5, 10);
        assert!((ctx.progress() - 0.5).abs() < f64::EPSILON);
        assert!(!ctx.is_near_limit());

        let ctx = CallbackContext::new(9, 10);
        assert!(ctx.is_near_limit());
    }

    #[test]
    fn test_context_builder() {
        let ctx = CallbackContext::new(1, 20)
            .with_agent_name("test-agent")
            .with_streaming(true);

        assert_eq!(ctx.agent_name.as_deref(), Some("test-agent"));
        assert!(ctx.is_streaming);
        assert!(ctx.is_first_step());
    }
}
