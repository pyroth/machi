//! Output guardrail types and traits.
//!
//! Output guardrails validate the agent's final output after generation,
//! enabling rejection of responses that violate safety policies, contain
//! PII, or fail format/quality checks.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::callback::RunContext;
use crate::error::Result;

use super::GuardrailOutput;

/// Trait for implementing output guardrail check logic.
///
/// Implement this trait on your own struct to define custom output validation.
/// The [`check`](OutputGuardrailCheck::check) method receives the run context,
/// agent name, and the final output value, and must return a [`GuardrailOutput`]
/// indicating whether the output passes.
#[async_trait]
pub trait OutputGuardrailCheck: Send + Sync {
    /// Check the agent's final output and return a guardrail output.
    ///
    /// # Arguments
    ///
    /// * `context` — the current run context (usage, step, state)
    /// * `agent_name` — name of the agent that produced the output
    /// * `output` — the final output value from the agent
    async fn check(
        &self,
        context: &RunContext,
        agent_name: &str,
        output: &Value,
    ) -> Result<GuardrailOutput>;
}

/// An output guardrail that validates the agent's final response.
///
/// Output guardrails are configured on an [`Agent`](crate::agent::Agent) or
/// [`RunConfig`](crate::agent::RunConfig) and are automatically executed by
/// the [`Runner`](crate::agent::Runner) after the agent produces a final output.
///
/// All output guardrails run concurrently. If any guardrail's tripwire is
/// triggered, the run returns an error and the output is not delivered.
#[derive(Clone)]
pub struct OutputGuardrail {
    /// Name of this guardrail (used in tracing and error messages).
    name: String,

    /// The guardrail check implementation.
    check: Arc<dyn OutputGuardrailCheck>,
}

impl OutputGuardrail {
    /// Create a new output guardrail with the given name and check logic.
    #[must_use]
    pub fn new(name: impl Into<String>, check: impl OutputGuardrailCheck + 'static) -> Self {
        Self {
            name: name.into(),
            check: Arc::new(check),
        }
    }

    /// Returns the name of this guardrail.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Execute this guardrail check.
    ///
    /// Returns an [`OutputGuardrailResult`] containing the guardrail reference
    /// and the check output.
    pub async fn run(
        &self,
        context: &RunContext,
        agent_name: &str,
        output: &Value,
    ) -> Result<OutputGuardrailResult> {
        let guardrail_output = self.check.check(context, agent_name, output).await?;
        Ok(OutputGuardrailResult {
            guardrail_name: self.name.clone(),
            agent_output: output.clone(),
            output: guardrail_output,
        })
    }
}

impl std::fmt::Debug for OutputGuardrail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutputGuardrail")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// The result of running an output guardrail.
#[derive(Debug, Clone)]
pub struct OutputGuardrailResult {
    /// Name of the guardrail that produced this result.
    pub guardrail_name: String,

    /// The agent output that was checked.
    pub agent_output: Value,

    /// The guardrail check output.
    pub output: GuardrailOutput,
}

impl OutputGuardrailResult {
    /// Returns `true` if the tripwire was triggered.
    #[must_use]
    pub const fn is_triggered(&self) -> bool {
        self.output.tripwire_triggered
    }
}
