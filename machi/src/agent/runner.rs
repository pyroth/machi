//! Runner — the agent execution engine.
//!
//! The [`Runner`] drives an [`Agent`] through its reasoning loop:
//!
//! 1. Build messages from instructions + conversation history
//! 2. Call the LLM with available tools
//! 3. Parse the response into a [`NextStep`]
//! 4. Execute tool calls (including managed agent sub-runs)
//! 5. Append results and loop back to step 2
//!
//! The loop terminates when the LLM produces a final text output, an error
//! occurs, or the maximum step count is exceeded.
//!
//! # Managed Agent Execution
//!
//! When a tool call targets a managed agent, the Runner spawns a recursive
//! child run for the sub-agent. Each sub-agent uses its own provider,
//! enabling heterogeneous multi-agent systems.

use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;

use serde_json::Value;
use tracing::{debug, warn};

use crate::callback::{NoopRunHooks, RunContext, RunHooks};
use crate::chat::{ChatRequest, ChatResponse, ToolChoice};
use crate::error::{Error, Result};
use crate::message::Message;
use crate::tool::{
    BoxedTool, ConfirmationHandler, ToolCallResult, ToolConfirmationRequest,
    ToolConfirmationResponse, ToolDefinition, ToolExecutionPolicy,
};
use crate::usage::Usage;

use super::config::Agent;
use super::hook::HookPair;
use super::result::{NextStep, RunConfig, RunResult, StepInfo, ToolCallRecord, ToolCallRequest};

/// Stateless execution engine that drives an [`Agent`] through its reasoning loop.
///
/// `Runner` owns no state — all per-run state lives in local variables within
/// [`Runner::run`]. This makes it safe to call `run` concurrently for different
/// agents or even the same agent with different inputs.
#[derive(Debug, Clone, Copy)]
pub struct Runner;

impl Runner {
    /// Execute an agent run to completion.
    ///
    /// The agent's own [`provider`](Agent::provider) is used for LLM calls.
    /// Each managed sub-agent uses its own provider, enabling heterogeneous
    /// multi-agent systems with different LLMs.
    ///
    /// # Arguments
    ///
    /// * `agent` — the agent to run (must have a provider configured)
    /// * `input` — the user's input message
    /// * `config` — run-level configuration (hooks, session, limits)
    ///
    /// # Returns
    ///
    /// A [`RunResult`] containing the final output, usage stats, and step history.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Agent`] if no provider is configured on the agent,
    /// [`Error::MaxSteps`] if the step limit is exceeded, or propagates
    /// LLM / tool errors encountered during execution.
    pub fn run<'a>(
        agent: &'a Agent,
        input: &'a str,
        config: RunConfig,
    ) -> Pin<Box<dyn Future<Output = Result<RunResult>> + Send + 'a>> {
        Box::pin(Self::run_inner(agent, input, config))
    }

    /// Internal async implementation of the agent run loop.
    async fn run_inner(agent: &Agent, input: &str, config: RunConfig) -> Result<RunResult> {
        let provider = agent.provider.as_deref().ok_or_else(|| {
            Error::agent(format!(
                "Agent '{}' has no provider configured. Call .provider() before running.",
                agent.name
            ))
        })?;
        let max_steps = config.max_steps.unwrap_or(agent.max_steps);
        let noop = NoopRunHooks;
        let run_hooks: &dyn RunHooks = config.hooks.as_deref().unwrap_or(&noop);
        let hooks = HookPair::new(run_hooks, agent.hooks.as_deref(), &agent.name);

        let mut context = RunContext::new().with_agent_name(&agent.name);
        let mut messages = Vec::new();
        let mut step_history = Vec::new();
        let mut cumulative_usage = Usage::zero();
        let mut auto_approved: HashSet<String> = HashSet::new();

        // Resolve system instructions.
        let system_prompt = agent.resolve_instructions();

        // Build initial messages: system + user input.
        if !system_prompt.is_empty() {
            messages.push(Message::system(&system_prompt));
        }
        messages.push(Message::user(input));

        // Load session history (inserted between system prompt and user input).
        if let Some(ref session) = config.session {
            let history = session.get_messages(None).await?;
            if !history.is_empty() {
                let insert_pos = messages.len().saturating_sub(1);
                messages.splice(insert_pos..insert_pos, history);
            }
        }

        // Collect tool definitions: regular tools + managed agent tool stubs.
        let all_definitions = Self::collect_all_definitions(agent);

        hooks.agent_start(&context).await;

        let system_ref = (!system_prompt.is_empty()).then_some(system_prompt.as_str());

        for step in 1..=max_steps {
            context.advance_step();
            debug!(agent = %agent.name, step, "Starting step");

            let request = Self::build_request(agent, &messages, &all_definitions);

            hooks.llm_start(&context, system_ref, &messages).await;

            let response = provider.chat(&request).await?;

            hooks.llm_end(&context, &response).await;

            // Accumulate usage.
            if let Some(usage) = response.usage {
                cumulative_usage += usage;
                context.add_usage(usage);
            }

            let next_step = Self::classify_response(&response);
            let (next_step, forbidden) = Self::apply_policies(next_step, agent, &auto_approved);

            match next_step {
                NextStep::FinalOutput { ref output } => {
                    // Append assistant message to history.
                    messages.push(response.message.clone());

                    step_history.push(StepInfo {
                        step,
                        response: response.clone(),
                        tool_calls: Vec::new(),
                    });

                    let output_value = output.clone();
                    hooks.agent_end(&context, &output_value).await;

                    // Persist to session if configured.
                    if let Some(ref session) = config.session {
                        let to_save = vec![Message::user(input), response.message.clone()];
                        let _ = session.add_messages(&to_save).await;
                    }

                    return Ok(RunResult {
                        output: output_value,
                        usage: cumulative_usage,
                        steps: step,
                        step_history,
                        agent_name: agent.name.clone(),
                    });
                }

                NextStep::ToolCalls { ref calls } => {
                    messages.push(response.message.clone());
                    Self::append_denied_messages(
                        &forbidden,
                        "forbidden by execution policy",
                        &mut messages,
                    );

                    let tool_records = Self::execute_tool_calls(
                        calls,
                        agent,
                        &context,
                        &hooks,
                        &mut messages,
                        config.max_tool_concurrency,
                    )
                    .await?;

                    step_history.push(StepInfo {
                        step,
                        response,
                        tool_calls: tool_records,
                    });
                }

                NextStep::NeedsApproval {
                    ref pending_approval,
                    ref approved,
                } => {
                    messages.push(response.message.clone());
                    Self::append_denied_messages(
                        &forbidden,
                        "forbidden by execution policy",
                        &mut messages,
                    );

                    let handler = config.confirmation_handler.as_deref().ok_or_else(|| {
                        Error::agent(
                            "Tool execution requires approval but no confirmation handler is configured",
                        )
                    })?;

                    let (confirmed, denied) =
                        Self::seek_confirmations(pending_approval, handler, &mut auto_approved)
                            .await;

                    Self::append_denied_messages(&denied, "denied by user", &mut messages);

                    // Execute approved + confirmed calls.
                    let executable: Vec<ToolCallRequest> =
                        approved.iter().chain(&confirmed).cloned().collect();

                    let tool_records = if executable.is_empty() {
                        Vec::new()
                    } else {
                        Self::execute_tool_calls(
                            &executable,
                            agent,
                            &context,
                            &hooks,
                            &mut messages,
                            config.max_tool_concurrency,
                        )
                        .await?
                    };

                    step_history.push(StepInfo {
                        step,
                        response,
                        tool_calls: tool_records,
                    });
                }

                NextStep::MaxStepsExceeded => {
                    unreachable!("MaxStepsExceeded is only set outside the loop");
                }
            }
        }

        // Exceeded max steps.
        let err = Error::max_steps(max_steps);
        hooks.error(&context, &err).await;

        Err(err)
    }

    /// Collect [`ToolDefinition`]s from regular tools and managed agents.
    fn collect_all_definitions(agent: &Agent) -> Vec<ToolDefinition> {
        agent
            .tools
            .iter()
            .map(|t| t.definition())
            .chain(agent.managed_agents.iter().map(Agent::tool_definition))
            .collect()
    }

    /// Build a [`ChatRequest`] for the current step.
    fn build_request(
        agent: &Agent,
        messages: &[Message],
        definitions: &[ToolDefinition],
    ) -> ChatRequest {
        let mut request = ChatRequest::with_messages(&agent.model, messages.to_vec());
        if !definitions.is_empty() {
            request = request
                .tools(definitions.to_vec())
                .tool_choice(ToolChoice::Auto)
                .parallel_tool_calls(true);
        }
        request
    }

    /// Classify an LLM response into a [`NextStep`].
    fn classify_response(response: &ChatResponse) -> NextStep {
        if let Some(tool_calls) = response.tool_calls() {
            let calls: Vec<ToolCallRequest> =
                tool_calls.iter().map(ToolCallRequest::from).collect();
            if !calls.is_empty() {
                return NextStep::ToolCalls { calls };
            }
        }
        NextStep::FinalOutput {
            output: response.text().map_or(Value::Null, Value::String),
        }
    }

    /// Execute tool calls concurrently and append results to messages.
    ///
    /// Runs up to `max_concurrency` calls in parallel per chunk using
    /// [`futures::future::join_all`], preserving the original call order.
    /// When `max_concurrency` is `None`, all calls run simultaneously.
    async fn execute_tool_calls(
        calls: &[ToolCallRequest],
        agent: &Agent,
        context: &RunContext,
        hooks: &HookPair<'_>,
        messages: &mut Vec<Message>,
        max_concurrency: Option<usize>,
    ) -> Result<Vec<ToolCallRecord>> {
        let concurrency = max_concurrency.unwrap_or(calls.len()).max(1);
        let mut records = Vec::with_capacity(calls.len());

        for chunk in calls.chunks(concurrency) {
            let mut futs = Vec::with_capacity(chunk.len());
            for call in chunk {
                futs.push(Self::execute_single_tool(call, agent, context, hooks));
            }
            records.extend(futures::future::join_all(futs).await);
        }

        // Append tool result messages in original call order.
        for record in &records {
            messages.push(Message::tool(&record.id, &record.result));
        }

        Ok(records)
    }

    /// Execute a single tool call with lifecycle hooks.
    ///
    /// Fires `tool_start` before dispatch and `tool_end` after, then returns
    /// the completed [`ToolCallRecord`].
    async fn execute_single_tool(
        call: &ToolCallRequest,
        agent: &Agent,
        context: &RunContext,
        hooks: &HookPair<'_>,
    ) -> ToolCallRecord {
        hooks.tool_start(context, &call.name).await;

        let (result_str, success) =
            if let Some(sub) = agent.managed_agents.iter().find(|a| a.name == call.name) {
                Self::dispatch_managed_agent(sub, &call.arguments).await
            } else if let Some(tool) = agent.tools.iter().find(|t| t.name() == call.name) {
                Self::dispatch_tool(tool, call).await
            } else {
                warn!(tool = %call.name, "Tool not found");
                (format!("Tool '{}' not found", call.name), false)
            };

        hooks.tool_end(context, &call.name, &result_str).await;

        ToolCallRecord {
            id: call.id.clone(),
            name: call.name.clone(),
            arguments: call.arguments.clone(),
            result: result_str,
            success,
        }
    }

    /// Run a managed sub-agent with the given task arguments.
    async fn dispatch_managed_agent(sub_agent: &Agent, args: &Value) -> (String, bool) {
        let task = args.get("task").and_then(Value::as_str).unwrap_or_default();
        match Self::run(sub_agent, task, RunConfig::default()).await {
            Ok(result) => {
                let output = serde_json::to_string(&result.output)
                    .unwrap_or_else(|_| result.output.to_string());
                (output, true)
            }
            Err(e) => (
                format!("Managed agent '{}' failed: {e}", sub_agent.name),
                false,
            ),
        }
    }

    /// Execute a regular tool call and format the result for the LLM.
    async fn dispatch_tool(tool: &BoxedTool, call: &ToolCallRequest) -> (String, bool) {
        let result = tool.call_json(call.arguments.clone()).await;
        let record = ToolCallResult {
            id: call.id.clone(),
            name: call.name.clone(),
            result,
        };
        (record.to_string_for_llm(), record.is_success())
    }

    /// Partition tool calls by execution policy.
    ///
    /// Returns a `(next_step, forbidden)` tuple:
    /// - `next_step` is either `ToolCalls` (all auto-approved) or
    ///   `NeedsApproval` (some require confirmation).
    /// - `forbidden` contains calls blocked by [`ToolExecutionPolicy::Forbidden`].
    fn apply_policies(
        next: NextStep,
        agent: &Agent,
        auto_approved: &HashSet<String>,
    ) -> (NextStep, Vec<ToolCallRequest>) {
        let NextStep::ToolCalls { calls } = next else {
            return (next, Vec::new());
        };

        let mut approved = Vec::new();
        let mut pending = Vec::new();
        let mut forbidden = Vec::new();

        for call in calls {
            let policy = agent
                .tool_policies
                .get(&call.name)
                .copied()
                .unwrap_or(ToolExecutionPolicy::Auto);

            if policy.is_forbidden() {
                forbidden.push(call);
            } else if policy.requires_confirmation() && !auto_approved.contains(&call.name) {
                pending.push(call);
            } else {
                approved.push(call);
            }
        }

        let step = if pending.is_empty() {
            NextStep::ToolCalls { calls: approved }
        } else {
            NextStep::NeedsApproval {
                pending_approval: pending,
                approved,
            }
        };

        (step, forbidden)
    }

    /// Sequentially request confirmation for each pending tool call.
    ///
    /// Returns `(confirmed, denied)` vectors. Calls approved with
    /// [`ToolConfirmationResponse::ApproveAll`] are added to
    /// `auto_approved` so future invocations skip confirmation.
    async fn seek_confirmations(
        pending: &[ToolCallRequest],
        handler: &dyn ConfirmationHandler,
        auto_approved: &mut HashSet<String>,
    ) -> (Vec<ToolCallRequest>, Vec<ToolCallRequest>) {
        let mut confirmed = Vec::new();
        let mut denied = Vec::new();

        for call in pending {
            let request =
                ToolConfirmationRequest::new(&call.id, &call.name, call.arguments.clone());
            let response = handler.confirm(&request).await;

            if response.is_approved() {
                if matches!(response, ToolConfirmationResponse::ApproveAll) {
                    auto_approved.insert(call.name.clone());
                }
                confirmed.push(call.clone());
            } else {
                denied.push(call.clone());
            }
        }

        (confirmed, denied)
    }

    /// Append tool-result denial messages for forbidden or denied calls.
    fn append_denied_messages(
        calls: &[ToolCallRequest],
        reason: &str,
        messages: &mut Vec<Message>,
    ) {
        for call in calls {
            messages.push(Message::tool(
                &call.id,
                format!("Tool '{}' {reason}", call.name),
            ));
        }
    }
}
