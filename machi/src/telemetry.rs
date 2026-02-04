//! Telemetry for agent execution using the `tracing` ecosystem.
//!
//! This module leverages Rust's standard `tracing` crate for observability:
//!
//! - **Zero Configuration**: Works out of the box with any tracing subscriber
//! - **OpenTelemetry Ready**: Add `tracing-opentelemetry` layer for OTLP export
//! - **Minimal Overhead**: Disabled instrumentation compiles away
//!
//! # Usage
//!
//! ```rust,ignore
//! // Basic: just initialize a tracing subscriber
//! tracing_subscriber::fmt::init();
//!
//! // With OpenTelemetry:
//! use tracing_subscriber::prelude::*;
//! tracing_subscriber::registry()
//!     .with(tracing_subscriber::fmt::layer())
//!     .with(tracing_opentelemetry::layer().with_tracer(tracer))
//!     .init();
//! ```

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::{Span, debug, debug_span, info, info_span};

use crate::memory::TokenUsage;

/// Metrics collected during an agent run.
///
/// This is a lightweight data structure returned after agent execution,
/// while the actual telemetry is handled by the tracing subscriber.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct RunMetrics {
    /// Total steps executed.
    pub steps: usize,
    /// Total input tokens.
    pub input_tokens: u64,
    /// Total output tokens.
    pub output_tokens: u64,
    /// Total duration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<Duration>,
    /// Tool calls made.
    pub tool_calls: usize,
    /// Errors encountered.
    pub errors: usize,
}

impl RunMetrics {
    /// Total tokens (input + output).
    #[must_use]
    pub const fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }

    /// Tokens per second rate.
    #[must_use]
    pub fn tokens_per_second(&self) -> Option<f64> {
        self.duration.map(|d| {
            let secs = d.as_secs_f64();
            if secs > 0.0 {
                self.total_tokens() as f64 / secs
            } else {
                0.0
            }
        })
    }

    /// Record token usage.
    pub fn record_tokens(&mut self, usage: &TokenUsage) {
        self.input_tokens += u64::from(usage.input_tokens);
        self.output_tokens += u64::from(usage.output_tokens);
    }

    /// Record a completed step.
    pub const fn record_step(&mut self) {
        self.steps += 1;
    }

    /// Record a tool call.
    pub const fn record_tool_call(&mut self) {
        self.tool_calls += 1;
    }

    /// Record an error.
    pub const fn record_error(&mut self) {
        self.errors += 1;
    }

    /// Complete the run with final duration.
    pub const fn complete(&mut self, duration: Duration) {
        self.duration = Some(duration);
    }
}

impl std::fmt::Display for RunMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Agent Run Metrics")?;
        writeln!(f, "  Steps:      {}", self.steps)?;
        writeln!(
            f,
            "  Tokens:     {} (in: {}, out: {})",
            self.total_tokens(),
            self.input_tokens,
            self.output_tokens
        )?;
        if let Some(d) = self.duration {
            writeln!(f, "  Duration:   {:.2}s", d.as_secs_f64())?;
            if let Some(rate) = self.tokens_per_second() {
                writeln!(f, "  Rate:       {rate:.1} tok/s")?;
            }
        }
        writeln!(f, "  Tool calls: {}", self.tool_calls)?;
        writeln!(f, "  Errors:     {}", self.errors)?;
        Ok(())
    }
}

/// Telemetry collector that integrates with tracing.
///
/// Automatically emits tracing spans and events while collecting metrics.
#[derive(Debug, Clone, Copy)]
pub struct Telemetry {
    start: Instant,
    metrics: RunMetrics,
}

impl Default for Telemetry {
    fn default() -> Self {
        Self::new()
    }
}

impl Telemetry {
    /// Create a new telemetry collector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            metrics: RunMetrics::default(),
        }
    }

    /// Reset for a new run.
    pub fn reset(&mut self) {
        self.start = Instant::now();
        self.metrics = RunMetrics::default();
    }

    /// Record a completed step with optional token usage.
    pub fn record_step(&mut self, step: usize, tokens: Option<&TokenUsage>) {
        self.metrics.record_step();
        if let Some(usage) = tokens {
            self.metrics.record_tokens(usage);
            info!(
                step,
                input_tokens = usage.input_tokens,
                output_tokens = usage.output_tokens,
                "step_completed"
            );
        } else {
            info!(step, "step_completed");
        }
    }

    /// Record a tool call.
    pub fn record_tool_call(&mut self, tool: &str) {
        self.metrics.record_tool_call();
        debug!(tool, "tool_called");
    }

    /// Record an error.
    pub fn record_error(&mut self, error: &str) {
        self.metrics.record_error();
        debug!(error, "step_error");
    }

    /// Complete the run and return final metrics.
    #[must_use]
    pub fn complete(&mut self) -> RunMetrics {
        let duration = self.start.elapsed();
        self.metrics.complete(duration);

        info!(
            steps = self.metrics.steps,
            input_tokens = self.metrics.input_tokens,
            output_tokens = self.metrics.output_tokens,
            duration_ms = duration.as_millis(),
            tool_calls = self.metrics.tool_calls,
            errors = self.metrics.errors,
            "run_completed"
        );

        self.metrics
    }

    /// Get current metrics snapshot.
    #[must_use]
    pub const fn metrics(&self) -> &RunMetrics {
        &self.metrics
    }

    /// Create a span for an agent run.
    #[must_use]
    pub fn run_span(task: &str) -> Span {
        info_span!("agent_run", task = %task)
    }

    /// Create a span for a step.
    #[must_use]
    pub fn step_span(step: usize) -> Span {
        info_span!("agent_step", step)
    }

    /// Create a span for a tool call.
    #[must_use]
    pub fn tool_span(tool: &str) -> Span {
        debug_span!("tool_call", tool = %tool)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_metrics() {
        let mut metrics = RunMetrics::default();
        metrics.record_step();
        metrics.record_step();
        metrics.record_tokens(&TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
        });
        metrics.record_tool_call();
        metrics.record_error();
        metrics.complete(Duration::from_secs(2));

        assert_eq!(metrics.steps, 2);
        assert_eq!(metrics.total_tokens(), 150);
        assert_eq!(metrics.tool_calls, 1);
        assert_eq!(metrics.errors, 1);
        assert!(metrics.tokens_per_second().is_some());
    }

    #[test]
    fn test_telemetry_collector() {
        let mut telemetry = Telemetry::new();
        telemetry.record_step(1, None);
        telemetry.record_tool_call("test");
        telemetry.record_step(
            2,
            Some(&TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
            }),
        );

        let metrics = telemetry.complete();
        assert_eq!(metrics.steps, 2);
        assert_eq!(metrics.tool_calls, 1);
    }
}
