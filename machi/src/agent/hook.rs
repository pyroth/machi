//! Hook dispatch bridge for the agent execution engine.
//!
//! [`HookPair`] combines run-level [`RunHooks`] and per-agent [`AgentHooks`]
//! into a single dispatcher, providing a clean interface for firing lifecycle
//! events without duplicating the dual-call pattern at every event point.

use serde_json::Value;

use crate::callback::{AgentHooks, RunContext, RunHooks};
use crate::chat::ChatResponse;
use crate::error::Error;
use crate::message::Message;

/// Dispatches lifecycle events to both run-level and agent-level hooks.
///
/// At each event point in the agent loop, two hooks may fire: a global
/// [`RunHooks`] and an optional per-agent [`AgentHooks`]. This struct
/// bundles them together so callers need only a single method call.
pub(super) struct HookPair<'a> {
    run: &'a dyn RunHooks,
    agent: Option<&'a dyn AgentHooks>,
    name: &'a str,
}

impl<'a> HookPair<'a> {
    /// Create a new hook pair from run-level hooks, optional agent hooks,
    /// and the agent name used for run-hook dispatch.
    pub fn new(run: &'a dyn RunHooks, agent: Option<&'a dyn AgentHooks>, name: &'a str) -> Self {
        Self { run, agent, name }
    }

    pub async fn agent_start(&self, ctx: &RunContext) {
        self.run.on_agent_start(ctx, self.name).await;
        if let Some(ah) = self.agent {
            ah.on_start(ctx).await;
        }
    }

    pub async fn agent_end(&self, ctx: &RunContext, output: &Value) {
        self.run.on_agent_end(ctx, self.name, output).await;
        if let Some(ah) = self.agent {
            ah.on_end(ctx, output).await;
        }
    }

    pub async fn llm_start(&self, ctx: &RunContext, system: Option<&str>, msgs: &[Message]) {
        self.run.on_llm_start(ctx, self.name, system, msgs).await;
        if let Some(ah) = self.agent {
            ah.on_llm_start(ctx, system, msgs).await;
        }
    }

    pub async fn llm_end(&self, ctx: &RunContext, response: &ChatResponse) {
        self.run.on_llm_end(ctx, self.name, response).await;
        if let Some(ah) = self.agent {
            ah.on_llm_end(ctx, response).await;
        }
    }

    pub async fn tool_start(&self, ctx: &RunContext, tool_name: &str) {
        self.run.on_tool_start(ctx, self.name, tool_name).await;
        if let Some(ah) = self.agent {
            ah.on_tool_start(ctx, tool_name).await;
        }
    }

    pub async fn tool_end(&self, ctx: &RunContext, tool_name: &str, result: &str) {
        self.run
            .on_tool_end(ctx, self.name, tool_name, result)
            .await;
        if let Some(ah) = self.agent {
            ah.on_tool_end(ctx, tool_name, result).await;
        }
    }

    pub async fn error(&self, ctx: &RunContext, err: &Error) {
        self.run.on_error(ctx, self.name, err).await;
        if let Some(ah) = self.agent {
            ah.on_error(ctx, err).await;
        }
    }
}
