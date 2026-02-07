//! Input guardrail types and traits.
//!
//! Input guardrails validate user input before or alongside the first LLM
//! call, enabling early rejection of off-topic, unsafe, or policy-violating
//! requests without incurring LLM costs.

use std::sync::Arc;

use async_trait::async_trait;

use crate::callback::RunContext;
use crate::error::Result;
use crate::message::Message;

use super::GuardrailOutput;

/// Trait for implementing input guardrail check logic.
///
/// Implement this trait on your own struct to define custom input validation.
/// The [`check`](InputGuardrailCheck::check) method receives the run context,
/// agent name, and the full message list (system prompt + history + user input),
/// and must return a [`GuardrailOutput`] indicating whether the input passes.
#[async_trait]
pub trait InputGuardrailCheck: Send + Sync {
    /// Check the input messages and return a guardrail output.
    ///
    /// # Arguments
    ///
    /// * `context` — the current run context (usage, step, state)
    /// * `agent_name` — name of the agent being executed
    /// * `input` — the full message list being sent to the LLM
    async fn check(
        &self,
        context: &RunContext,
        agent_name: &str,
        input: &[Message],
    ) -> Result<GuardrailOutput>;
}

/// An input guardrail that validates user input before or alongside the LLM.
///
/// Input guardrails are configured on an [`Agent`](crate::agent::Agent) or
/// [`RunConfig`](crate::agent::RunConfig) and are automatically executed by
/// the [`Runner`](crate::agent::Runner) during the first step of a run.
///
/// # Execution Modes
///
/// - **Sequential** (`run_in_parallel: false`): Runs before the LLM call.
///   If triggered, the LLM call is never made.
/// - **Parallel** (`run_in_parallel: true`, default): Runs concurrently with
///   the first LLM call. If triggered, the LLM result is discarded.
#[derive(Clone)]
pub struct InputGuardrail {
    /// Name of this guardrail (used in tracing and error messages).
    name: String,

    /// Whether to run concurrently with the first LLM call.
    run_in_parallel: bool,

    /// The guardrail check implementation.
    check: Arc<dyn InputGuardrailCheck>,
}

impl InputGuardrail {
    /// Create a new input guardrail with the given name and check logic.
    ///
    /// By default, the guardrail runs in parallel with the first LLM call.
    #[must_use]
    pub fn new(name: impl Into<String>, check: impl InputGuardrailCheck + 'static) -> Self {
        Self {
            name: name.into(),
            run_in_parallel: true,
            check: Arc::new(check),
        }
    }

    /// Set whether this guardrail runs in parallel with the LLM call.
    ///
    /// - `true` (default): Runs concurrently — lower latency but the LLM
    ///   call is still made even if the guardrail triggers.
    /// - `false`: Runs before the LLM call — higher latency but avoids
    ///   unnecessary LLM costs when the guardrail triggers.
    #[must_use]
    pub const fn run_in_parallel(mut self, parallel: bool) -> Self {
        self.run_in_parallel = parallel;
        self
    }

    /// Returns the name of this guardrail.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns whether this guardrail runs in parallel with the LLM.
    #[must_use]
    pub const fn is_parallel(&self) -> bool {
        self.run_in_parallel
    }

    /// Execute this guardrail check.
    ///
    /// Returns an [`InputGuardrailResult`] containing the guardrail reference
    /// and the check output.
    pub async fn run(
        &self,
        context: &RunContext,
        agent_name: &str,
        input: &[Message],
    ) -> Result<InputGuardrailResult> {
        let output = self.check.check(context, agent_name, input).await?;
        Ok(InputGuardrailResult {
            guardrail_name: self.name.clone(),
            output,
        })
    }
}

impl std::fmt::Debug for InputGuardrail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputGuardrail")
            .field("name", &self.name)
            .field("run_in_parallel", &self.run_in_parallel)
            .finish_non_exhaustive()
    }
}

/// The result of running an input guardrail.
#[derive(Debug, Clone)]
pub struct InputGuardrailResult {
    /// Name of the guardrail that produced this result.
    pub guardrail_name: String,

    /// The guardrail check output.
    pub output: GuardrailOutput,
}

impl InputGuardrailResult {
    /// Returns `true` if the tripwire was triggered.
    #[must_use]
    pub const fn is_triggered(&self) -> bool {
        self.output.tripwire_triggered
    }
}
