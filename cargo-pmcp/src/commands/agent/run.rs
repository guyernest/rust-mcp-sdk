//! Lib-safe fixed-source agent runner seam (CLI-02).
//!
//! [`run_fixed_source`] builds the `pmcp-agent` loop over a scripted end-turn
//! [`CompletionSource`], a no-op [`NoopInvoker`], and an [`InMemoryStore`], then
//! runs [`AgentEngine`] to a terminal [`RunOutcome`]. It is the SAME production
//! path the `agent dev --source fixed` CLI arm and the plan-110-06 example both
//! call.
//!
//! This file is a LEAF: it references only `pmcp-agent` + `pmcp` types + std
//! (NO `clap` / `GlobalFlags` / `crate::commands` siblings), so it can be mounted
//! into the lib target via a `#[path]` seam (mirroring `templates_agent`) — which
//! is how the offline integration test and the 110-06 example reach it.

use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use serde_json::json;

use pmcp::types::sampling::{
    CreateMessageParams, CreateMessageResultWithTools, SamplingMessageContent,
};
use pmcp::types::Role;

use pmcp_agent::{
    AgentEngine, CompletionError, CompletionSource, InMemoryStore, ResolvedAgentConfig, RunOutcome,
    ToolCall, ToolCallResult, ToolInvoker,
};

/// Drive the agent loop offline against a scripted end-turn source.
///
/// Deterministic and network-free: the [`EndTurnSource`] ends the turn on its
/// first completion, so the loop terminates in one iteration with
/// [`RunOutcome::Completed`].
pub async fn run_fixed_source(config: ResolvedAgentConfig) -> RunOutcome {
    let engine = AgentEngine::new(
        EndTurnSource::default(),
        NoopInvoker,
        InMemoryStore::new(),
        config,
    );
    engine.run("agent-dev-run").await
}

/// A scripted [`CompletionSource`] that immediately ends the turn — the loop
/// completes in one iteration, fully offline.
#[derive(Default)]
struct EndTurnSource {
    calls: AtomicUsize,
}

#[async_trait]
impl CompletionSource for EndTurnSource {
    async fn create_message(
        &self,
        _params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools, CompletionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(CreateMessageResultWithTools::new(
            "fixed-model",
            Role::Assistant,
            vec![SamplingMessageContent::Text {
                text: "Done: this is the offline fixed-source agent-dev run.".to_string(),
                meta: None,
            }],
        )
        .with_stop_reason("end_turn"))
    }
}

/// A [`ToolInvoker`] that echoes an ok result. Shared by the fixed, openai-compat,
/// and sampling arms (the end-turn script never dispatches a tool, but the engine
/// and the server builder both require an invoker).
#[derive(Clone, Default)]
pub struct NoopInvoker;

#[async_trait]
impl ToolInvoker for NoopInvoker {
    async fn invoke(&self, call: ToolCall) -> ToolCallResult {
        ToolCallResult::ok(call.id, json!({ "result": format!("ran {}", call.name) }))
    }
}
