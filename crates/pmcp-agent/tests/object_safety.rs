//! Compile-level proof that all three effect seams are object-safe (AGNT-01):
//! a developer can hold `Arc<dyn CompletionSource>`, `Arc<dyn ToolInvoker>`, and
//! `Arc<dyn ConversationStore>`. Also asserts a `RunState` serde round-trip
//! (replay-determinism sanity).

use std::sync::Arc;

use async_trait::async_trait;
use pmcp::types::content::Role;
use pmcp::types::sampling::{
    CreateMessageParams, CreateMessageResultWithTools, SamplingMessage, SamplingMessageContent,
};
use pmcp_agent::{
    CompletionError, CompletionSource, ConversationStore, InMemoryStore, RunPhase, RunState,
    ToolCall, ToolCallResult, ToolInvoker,
};

/// No-op completion source (proves `CompletionSource` is object-safe).
struct StubCompletion;

#[async_trait]
impl CompletionSource for StubCompletion {
    async fn create_message(
        &self,
        _params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools, CompletionError> {
        Ok(CreateMessageResultWithTools::new(
            "stub-model",
            Role::Assistant,
            vec![SamplingMessageContent::Text {
                text: "ok".into(),
                meta: None,
            }],
        ))
    }
}

/// No-op tool invoker (proves `ToolInvoker` is object-safe).
struct StubInvoker;

#[async_trait]
impl ToolInvoker for StubInvoker {
    async fn invoke(&self, call: ToolCall) -> ToolCallResult {
        ToolCallResult::ok(call.id, serde_json::Value::Null)
    }
}

#[test]
fn all_three_seams_are_object_safe() {
    let _completion: Arc<dyn CompletionSource> = Arc::new(StubCompletion);
    let _invoker: Arc<dyn ToolInvoker> = Arc::new(StubInvoker);
    let _store: Arc<dyn ConversationStore> = Arc::new(InMemoryStore::new());
}

#[test]
fn run_state_serde_round_trips_with_history_and_pending_tools() {
    let mut state = RunState::new();
    state.history.push(SamplingMessage::new(
        Role::User,
        SamplingMessageContent::Text {
            text: "hello".into(),
            meta: None,
        },
    ));
    state.pending_tool_calls.push(ToolCall {
        id: "call-1".into(),
        name: "search".into(),
        arguments: serde_json::json!({ "q": "rust" }),
        connector: None,
    });
    state.iteration = 2;
    state.tokens_used = 128;
    state.phase = RunPhase::PendingTools;

    let json = serde_json::to_string(&state).unwrap();
    let back: RunState = serde_json::from_str(&json).unwrap();
    // RunState has no PartialEq (SamplingMessage lacks it) — a re-serialized
    // round-trip proves the (de)serialization is lossless and deterministic.
    assert_eq!(json, serde_json::to_string(&back).unwrap());
}
