//! Guardrail module — safety checks for agent inputs and outputs.
//!
//! Guardrails are validation checks that run alongside agent execution to
//! ensure inputs and outputs meet safety, quality, and policy criteria.
//!
//! Inspired by the [OpenAI Agents SDK](https://github.com/openai/openai-agents-python)
//! guardrail design, this module provides two guardrail types:
//!
//! - **[`InputGuardrail`]** — validates user input before or alongside the
//!   first LLM call (e.g., off-topic detection, content filtering).
//! - **[`OutputGuardrail`]** — validates the agent's final output after
//!   generation (e.g., PII detection, format checking, policy compliance).
//!
//! # Tripwire Mechanism
//!
//! Each guardrail returns a [`GuardrailOutput`] containing a `tripwire_triggered`
//! flag. When any guardrail triggers its tripwire, the agent run is immediately
//! halted and an [`Error::InputGuardrailTriggered`](crate::Error) or
//! [`Error::OutputGuardrailTriggered`](crate::Error) is returned.
//!
//! # Execution Modes
//!
//! Input guardrails support two execution modes via [`InputGuardrail::run_in_parallel`]:
//!
//! - **Sequential** (`false`): Runs before the first LLM call. If the tripwire
//!   triggers, the LLM call is never made — saving cost and latency.
//! - **Parallel** (`true`, default): Runs concurrently with the first LLM call
//!   via `tokio::join!`. If the tripwire triggers, the LLM result is discarded.
//!
//! Output guardrails always run after the agent produces a final output,
//! and are executed concurrently with each other.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use machi::prelude::*;
//!
//! struct ContentFilter;
//!
//! #[async_trait::async_trait]
//! impl InputGuardrailCheck for ContentFilter {
//!     async fn check(
//!         &self,
//!         _context: &RunContext,
//!         _agent_name: &str,
//!         input: &[Message],
//!     ) -> Result<GuardrailOutput> {
//!         let text = input.iter()
//!             .filter_map(|m| m.text())
//!             .collect::<String>();
//!         if text.contains("forbidden") {
//!             Ok(GuardrailOutput::tripwire("Forbidden content detected"))
//!         } else {
//!             Ok(GuardrailOutput::pass())
//!         }
//!     }
//! }
//!
//! let agent = Agent::new("safe-agent")
//!     .instructions("You are a helpful assistant.")
//!     .model("gpt-4o")
//!     .provider(provider.clone())
//!     .input_guardrail(InputGuardrail::new("content-filter", ContentFilter));
//! ```

mod input;
mod output;

pub use input::{InputGuardrail, InputGuardrailCheck, InputGuardrailResult};
pub use output::{OutputGuardrail, OutputGuardrailCheck, OutputGuardrailResult};

use serde_json::Value;

/// The output of a guardrail check function.
///
/// Contains a boolean tripwire flag and optional structured information
/// about the check that was performed. When `tripwire_triggered` is `true`,
/// the agent run is halted immediately.
#[derive(Debug, Clone)]
pub struct GuardrailOutput {
    /// Whether the tripwire was triggered.
    ///
    /// If `true`, the agent's execution will be immediately halted and
    /// an error will be returned to the caller.
    pub tripwire_triggered: bool,

    /// Optional structured information about the guardrail's output.
    ///
    /// Can contain details about the checks performed, confidence scores,
    /// detected issues, or any other metadata useful for debugging and
    /// observability.
    pub output_info: Value,
}

impl GuardrailOutput {
    /// Create a passing guardrail output (tripwire not triggered).
    #[must_use]
    pub const fn pass() -> Self {
        Self {
            tripwire_triggered: false,
            output_info: Value::Null,
        }
    }

    /// Create a failing guardrail output (tripwire triggered).
    ///
    /// The `info` parameter should describe why the tripwire was triggered,
    /// and will be included in the resulting error for observability.
    #[must_use]
    pub fn tripwire(info: impl Into<Value>) -> Self {
        Self {
            tripwire_triggered: true,
            output_info: info.into(),
        }
    }

    /// Create a passing output with additional diagnostic information.
    ///
    /// Useful when the guardrail passes but you want to record metadata
    /// (e.g., confidence scores, partial matches) for observability.
    #[must_use]
    pub fn pass_with_info(info: impl Into<Value>) -> Self {
        Self {
            tripwire_triggered: false,
            output_info: info.into(),
        }
    }

    /// Returns `true` if the tripwire was triggered.
    #[must_use]
    pub const fn is_triggered(&self) -> bool {
        self.tripwire_triggered
    }
}

/// Convenience conversion: a string becomes a tripwire output.
impl From<&str> for GuardrailOutput {
    fn from(reason: &str) -> Self {
        Self::tripwire(Value::String(reason.to_owned()))
    }
}
