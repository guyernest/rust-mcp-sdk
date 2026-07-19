//! The thin async iteration engine — orchestrates the three seams.
//!
//! [`AgentEngine`] is a thin `loop {}` that awaits ONLY the four seam calls
//! (`load` / `create_message` / `invoke_batch` / `save`); every between-await
//! computation delegates to the pure functions in [`super::decide`]. It:
//!
//! - LOADS resumable state via [`ConversationStore::load`] and RESUMES from the
//!   checkpoint phase (a `PendingTools` state dispatches the already-saved calls
//!   instead of re-running completion — Codex HIGH #1);
//! - orders checkpoints CRASH-SAFELY: it saves [`RunPhase::PendingTools`] BEFORE
//!   dispatching side-effecting tools and saves the final state BEFORE returning
//!   (Codex HIGH #2), so a crash mid-dispatch cannot silently repeat effects;
//! - returns retry classification as DATA via [`RunOutcome`] (AGNT-02); and
//! - enforces limits from counters only (no wall-clock) via [`check_limits`].

use pmcp::types::sampling::{CreateMessageParams, SamplingMessage};
use pmcp::types::ToolInfo;

use crate::config::ResolvedAgentConfig;
use crate::seams::{
    CompletionSource, ConversationStore, RetryClass, RunPhase, RunState, ToolInvoker,
};
use crate::trace::{DecisionStep, DecisionTrace, OutcomeTag};

use super::decide::{
    assistant_turn, check_limits, classify_retry, digest_tool_results, evaluate_submit_result,
    extract_token_usage, extract_tool_calls, is_end_turn, ErrorSignal,
};
use super::result::{IterationResult, LimitDecision, RunOutcome, TurnMessage};

/// Control returned by one engine step: keep looping or return an outcome.
enum StepControl {
    /// Under limits with no terminal decision — run another step.
    Continue,
    /// Terminal — return this outcome to the caller.
    Return(RunOutcome),
}

/// The agent iteration engine, generic over the three effect seams.
///
/// Construct with [`AgentEngine::new`] and drive with [`AgentEngine::run`] (or
/// [`AgentEngine::run_traced`] to also capture a [`DecisionTrace`]).
#[derive(Debug)]
pub struct AgentEngine<C, T, S> {
    completion: C,
    invoker: T,
    store: S,
    config: ResolvedAgentConfig,
}

impl<C, T, S> AgentEngine<C, T, S>
where
    C: CompletionSource,
    T: ToolInvoker,
    S: ConversationStore,
{
    /// Build an engine over the three seams and a resolved config.
    pub fn new(completion: C, invoker: T, store: S, config: ResolvedAgentConfig) -> Self {
        Self {
            completion,
            invoker,
            store,
            config,
        }
    }

    /// Run the agent loop for `run_id` to a terminal [`RunOutcome`].
    pub async fn run(&self, run_id: &str) -> RunOutcome {
        let mut trace = DecisionTrace::default();
        self.run_traced(run_id, &mut trace).await
    }

    /// Run the loop, recording the ordered decisions into `trace`.
    ///
    /// The observable [`DecisionTrace`] is the replay-safety artifact (AGNT-03):
    /// identical effect results yield an identical `trace`.
    pub async fn run_traced(&self, run_id: &str, trace: &mut DecisionTrace) -> RunOutcome {
        let mut state = match self.store.load(run_id).await {
            Ok(loaded) => loaded.unwrap_or_default(),
            Err(err) => {
                return finish(
                    trace,
                    RunOutcome::Failed {
                        error: err.to_string(),
                    },
                )
            },
        };

        // RESUME: a run checkpointed at PendingTools already saved its tool calls
        // BEFORE crashing/pausing — dispatch those, do NOT re-run completion.
        if state.phase == RunPhase::PendingTools {
            if let Err(outcome) = self.resume_pending(run_id, &mut state).await {
                return finish(trace, outcome);
            }
        }

        // Discover the tools to advertise ONCE per run (before the loop), so the
        // model is actually told what it may call (AGNT tool-use). Resume re-runs
        // discovery too — it is idempotent.
        let tools = self.resolve_tools().await;

        loop {
            match self.step(run_id, &mut state, trace, &tools).await {
                Ok(StepControl::Continue) => {},
                Ok(StepControl::Return(outcome)) | Err(outcome) => {
                    return finish(trace, outcome);
                },
            }
        }
    }

    /// Dispatch the already-saved pending tool calls of a resumed run.
    async fn resume_pending(&self, run_id: &str, state: &mut RunState) -> Result<(), RunOutcome> {
        let calls = std::mem::take(&mut state.pending_tool_calls);
        let results = self.invoker.invoke_batch(calls).await;
        let tool_turn = digest_tool_results(results);
        append_turn(&mut state.history, &tool_turn);
        state.phase = RunPhase::ToolsCompleted;
        state.iteration = state.iteration.saturating_add(1);
        self.save(run_id, state).await
    }

    /// Execute one completion step: complete → classify → (dispatch) → checkpoint.
    ///
    /// Awaits ONLY `create_message`, `invoke_batch`, and `save`. Returns
    /// `Err(outcome)` for a save failure (so the `?` shortcut keeps this small),
    /// `Ok(Return(..))` for a terminal decision, or `Ok(Continue)` to loop.
    async fn step(
        &self,
        run_id: &str,
        state: &mut RunState,
        trace: &mut DecisionTrace,
        tools: &[ToolInfo],
    ) -> Result<StepControl, RunOutcome> {
        let step_index = state.iteration;
        let params = self.build_params(state, tools);

        let result = match self.completion.create_message(params).await {
            Ok(result) => result,
            Err(err) => {
                // Save so the host can resume, then surface the classification.
                self.save(run_id, state).await?;
                let class = classify_retry(ErrorSignal::from_completion(&err));
                return Ok(StepControl::Return(retry_or_fail(class, err.to_string())));
            },
        };

        state.tokens_used = state
            .tokens_used
            .saturating_add(extract_token_usage(&result));
        let calls = extract_tool_calls(&result);
        let assistant = assistant_turn(&result);
        append_turn(&mut state.history, &assistant);

        let ended = is_end_turn(result.stop_reason.as_deref());
        let is_final =
            ended || evaluate_submit_result(&assistant, self.config.output_schema.as_ref());
        if is_final {
            state.phase = RunPhase::ReadyForCompletion;
            self.save(run_id, state).await?; // SAVE FINAL before returning.
            record_step(trace, step_index, ended, true, &[], None);
            return Ok(StepControl::Return(RunOutcome::Completed {
                result: IterationResult {
                    assistant_message: assistant,
                    tool_results_message: None,
                    is_final: true,
                },
            }));
        }

        let tool_ids = self.dispatch_tools(run_id, state, calls).await?;
        state.iteration = state.iteration.saturating_add(1);
        self.save(run_id, state).await?;

        let limit = check_limits(
            state.iteration,
            self.config.max_iterations,
            state.tokens_used,
            self.config.token_budget,
        );
        record_step(trace, step_index, ended, false, &tool_ids, Some(limit));
        match limit {
            LimitDecision::Stop => Ok(StepControl::Return(RunOutcome::LimitReached)),
            LimitDecision::Continue => Ok(StepControl::Continue),
        }
    }

    /// Checkpoint pending calls BEFORE dispatch, then dispatch and fold results.
    ///
    /// Returns the dispatched call ids (empty when there were no calls). The
    /// `PendingTools` save happens BEFORE `invoke_batch` so a crash cannot
    /// silently repeat side-effecting tools (Codex HIGH #2).
    async fn dispatch_tools(
        &self,
        run_id: &str,
        state: &mut RunState,
        calls: Vec<crate::seams::ToolCall>,
    ) -> Result<Vec<String>, RunOutcome> {
        if calls.is_empty() {
            return Ok(Vec::new());
        }
        let tool_ids: Vec<String> = calls.iter().map(|call| call.id.clone()).collect();
        state.pending_tool_calls.clone_from(&calls);
        state.phase = RunPhase::PendingTools;
        self.save(run_id, state).await?; // SAVE BEFORE DISPATCH.

        let results = self.invoker.invoke_batch(calls).await;
        let tool_turn = digest_tool_results(results);
        append_turn(&mut state.history, &tool_turn);
        state.phase = RunPhase::ToolsCompleted;
        state.pending_tool_calls.clear();
        Ok(tool_ids)
    }

    /// Build the next completion params from history + config + discovered tools
    /// (pure). `tools` is the resolved advertise-set; when non-empty it is passed
    /// to the model so it can actually request tool calls.
    fn build_params(&self, state: &RunState, tools: &[ToolInfo]) -> CreateMessageParams {
        let params = CreateMessageParams::new(state.history.clone())
            .with_system_prompt(self.config.instructions.clone())
            .with_max_tokens(self.config.max_tokens);
        if tools.is_empty() {
            params
        } else {
            params.with_tools(tools.to_vec())
        }
    }

    /// Discover the tools to advertise: everything the invoker lists via
    /// `tools/list`, narrowed to the config's tool-selection allow-list when one
    /// is present (an EMPTY selection advertises every discovered tool). Called
    /// once per run; a discovery failure yields an empty set (no tools advertised).
    async fn resolve_tools(&self) -> Vec<ToolInfo> {
        let advertised = self.invoker.list_tools().await;
        if self.config.tools.is_empty() {
            return advertised;
        }
        let allow: std::collections::HashSet<&str> =
            self.config.tools.iter().map(String::as_str).collect();
        advertised
            .into_iter()
            .filter(|tool| allow.contains(tool.name.as_str()))
            .collect()
    }

    /// Save `state`, mapping a store failure to a terminal `Failed` outcome.
    async fn save(&self, run_id: &str, state: &RunState) -> Result<(), RunOutcome> {
        self.store
            .save(run_id, state)
            .await
            .map_err(|err| RunOutcome::Failed {
                error: err.to_string(),
            })
    }
}

/// Flatten a multi-block [`TurnMessage`] into per-block `SamplingMessage`s.
///
/// The SDK `SamplingMessage` holds a single content block, so a turn's blocks
/// are appended in order as separate messages.
fn append_turn(history: &mut Vec<SamplingMessage>, turn: &TurnMessage) {
    for block in &turn.content {
        history.push(SamplingMessage::new(turn.role, block.clone()));
    }
}

/// Map a retry class to the terminal outcome: fatal fails, else request retry.
fn retry_or_fail(class: RetryClass, message: String) -> RunOutcome {
    match class {
        RetryClass::Fatal => RunOutcome::Failed { error: message },
        retryable => RunOutcome::RetryRequired { class: retryable },
    }
}

/// Record one decision step into the trace.
fn record_step(
    trace: &mut DecisionTrace,
    iteration: u32,
    is_end_turn: bool,
    is_final: bool,
    tool_call_ids: &[String],
    limit: Option<LimitDecision>,
) {
    trace.steps.push(DecisionStep {
        iteration,
        is_end_turn,
        is_final,
        tool_call_ids: tool_call_ids.to_vec(),
        limit,
    });
}

/// Set the trace's terminal outcome tag and return the outcome.
fn finish(trace: &mut DecisionTrace, outcome: RunOutcome) -> RunOutcome {
    trace.outcome = Some(outcome_tag(&outcome));
    outcome
}

/// Project a [`RunOutcome`] to its comparable [`OutcomeTag`].
fn outcome_tag(outcome: &RunOutcome) -> OutcomeTag {
    match outcome {
        RunOutcome::Completed { .. } => OutcomeTag::Completed,
        RunOutcome::LimitReached => OutcomeTag::LimitReached,
        RunOutcome::RetryRequired { class } => OutcomeTag::RetryRequired {
            class: class.clone(),
        },
        RunOutcome::Failed { .. } => OutcomeTag::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::AgentEngine;
    use crate::config::ResolvedAgentConfig;
    use crate::seams::{
        CompletionError, CompletionSource, ConversationStore, InMemoryStore, RunPhase, RunState,
        ToolCall, ToolCallResult, ToolInvoker,
    };
    use crate::trace::{DecisionTrace, OutcomeTag};
    use async_trait::async_trait;
    use pmcp::types::content::Role;
    use pmcp::types::sampling::{
        CreateMessageParams, CreateMessageResultWithTools, SamplingMessageContent,
    };
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn cfg() -> ResolvedAgentConfig {
        ResolvedAgentConfig::new("be helpful", "test-model", 100_000, 10)
    }

    fn text_completion(stop: &str, text: &str) -> CreateMessageResultWithTools {
        CreateMessageResultWithTools::new(
            "test-model",
            Role::Assistant,
            vec![SamplingMessageContent::Text {
                text: text.into(),
                meta: None,
            }],
        )
        .with_stop_reason(stop)
    }

    fn tool_use_completion(id: &str, name: &str) -> CreateMessageResultWithTools {
        CreateMessageResultWithTools::new(
            "test-model",
            Role::Assistant,
            vec![SamplingMessageContent::ToolUse {
                name: name.into(),
                id: id.into(),
                input: json!({}),
                meta: None,
            }],
        )
        .with_stop_reason("tool_use")
    }

    /// Scripted completion source returning a fixed sequence.
    struct ScriptSource {
        script: Vec<CreateMessageResultWithTools>,
        cursor: AtomicUsize,
    }
    impl ScriptSource {
        fn new(script: Vec<CreateMessageResultWithTools>) -> Self {
            Self {
                script,
                cursor: AtomicUsize::new(0),
            }
        }
    }
    #[async_trait]
    impl CompletionSource for ScriptSource {
        async fn create_message(
            &self,
            _params: CreateMessageParams,
        ) -> Result<CreateMessageResultWithTools, CompletionError> {
            let i = self.cursor.fetch_add(1, Ordering::SeqCst);
            self.script
                .get(i)
                .cloned()
                .ok_or_else(|| CompletionError::Decode("script exhausted".into()))
        }
    }

    /// Completion source that always errors (for the retry-as-data path).
    struct ErrSource(CompletionError);
    #[async_trait]
    impl CompletionSource for ErrSource {
        async fn create_message(
            &self,
            _params: CreateMessageParams,
        ) -> Result<CreateMessageResultWithTools, CompletionError> {
            Err(match &self.0 {
                CompletionError::Capacity(m) => CompletionError::Capacity(m.clone()),
                CompletionError::Transport(m) => CompletionError::Transport(m.clone()),
                CompletionError::Decode(m) => CompletionError::Decode(m.clone()),
                CompletionError::Auth => CompletionError::Auth,
            })
        }
    }

    /// Invoker that records whether it was called and echoes ok results.
    #[derive(Clone, Default)]
    struct RecordingInvoker {
        dispatched: Arc<AtomicUsize>,
    }
    #[async_trait]
    impl ToolInvoker for RecordingInvoker {
        async fn invoke(&self, call: ToolCall) -> ToolCallResult {
            self.dispatched.fetch_add(1, Ordering::SeqCst);
            ToolCallResult::ok(call.id, json!({"ok": true}))
        }
    }

    #[tokio::test]
    async fn end_turn_completes_after_one_iteration_without_dispatch() {
        let src = ScriptSource::new(vec![text_completion("end_turn", "done")]);
        let inv = RecordingInvoker::default();
        let engine = AgentEngine::new(src, inv.clone(), InMemoryStore::new(), cfg());

        let mut trace = DecisionTrace::default();
        let outcome = engine.run_traced("run-1", &mut trace).await;

        assert!(matches!(
            outcome,
            crate::iteration::RunOutcome::Completed { .. }
        ));
        assert_eq!(inv.dispatched.load(Ordering::SeqCst), 0);
        assert_eq!(trace.outcome, Some(OutcomeTag::Completed));
        assert_eq!(trace.steps.len(), 1);
        assert!(trace.steps[0].is_final);
    }

    #[tokio::test]
    async fn tool_loop_dispatches_then_completes() {
        let src = ScriptSource::new(vec![
            tool_use_completion("tu-1", "search"),
            text_completion("stop", "answer"),
        ]);
        let inv = RecordingInvoker::default();
        let engine = AgentEngine::new(src, inv.clone(), InMemoryStore::new(), cfg());

        let outcome = engine.run("run-2").await;
        assert!(matches!(
            outcome,
            crate::iteration::RunOutcome::Completed { .. }
        ));
        assert_eq!(inv.dispatched.load(Ordering::SeqCst), 1);
    }

    fn tool_info(name: &str) -> pmcp::types::ToolInfo {
        serde_json::from_value(json!({ "name": name, "inputSchema": { "type": "object" } }))
            .expect("valid ToolInfo")
    }

    /// Records the tool names advertised in the params it receives, then ends.
    struct CaptureSource {
        seen: Arc<std::sync::Mutex<Vec<String>>>,
    }
    #[async_trait]
    impl CompletionSource for CaptureSource {
        async fn create_message(
            &self,
            params: CreateMessageParams,
        ) -> Result<CreateMessageResultWithTools, CompletionError> {
            let names = params
                .tools
                .unwrap_or_default()
                .into_iter()
                .map(|t| t.name)
                .collect::<Vec<_>>();
            *self.seen.lock().unwrap() = names;
            Ok(text_completion("end_turn", "done"))
        }
    }

    /// Invoker that advertises two tools via `tools/list` discovery.
    struct DiscoveringInvoker;
    #[async_trait]
    impl ToolInvoker for DiscoveringInvoker {
        async fn invoke(&self, call: ToolCall) -> ToolCallResult {
            ToolCallResult::ok(call.id, json!({}))
        }
        async fn list_tools(&self) -> Vec<pmcp::types::ToolInfo> {
            vec![tool_info("search"), tool_info("calculator")]
        }
    }

    #[tokio::test]
    async fn discovered_tools_are_advertised_to_the_model() {
        // No config allow-list → every discovered tool is passed to the model.
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let engine = AgentEngine::new(
            CaptureSource { seen: seen.clone() },
            DiscoveringInvoker,
            InMemoryStore::new(),
            cfg(),
        );
        let _ = engine.run("run-tools").await;
        let mut names = seen.lock().unwrap().clone();
        names.sort();
        assert_eq!(names, vec!["calculator".to_string(), "search".to_string()]);
    }

    #[tokio::test]
    async fn tool_selection_allow_list_filters_discovered_tools() {
        // A non-empty config.tools narrows discovery to the selected names.
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut config = cfg();
        config.tools = vec!["search".to_string()];
        let engine = AgentEngine::new(
            CaptureSource { seen: seen.clone() },
            DiscoveringInvoker,
            InMemoryStore::new(),
            config,
        );
        let _ = engine.run("run-tools-2").await;
        assert_eq!(*seen.lock().unwrap(), vec!["search".to_string()]);
    }

    #[tokio::test]
    async fn saves_pending_tools_before_dispatch() {
        // A store that snapshots the phase seen at the moment invoke_batch runs.
        #[derive(Clone)]
        struct SpyStore {
            inner: Arc<InMemoryStore>,
        }
        let store = SpyStore {
            inner: Arc::new(InMemoryStore::new()),
        };
        #[async_trait]
        impl ConversationStore for SpyStore {
            async fn load(
                &self,
                run_id: &str,
            ) -> Result<Option<RunState>, crate::seams::StoreError> {
                self.inner.load(run_id).await
            }
            async fn save(
                &self,
                run_id: &str,
                state: &RunState,
            ) -> Result<(), crate::seams::StoreError> {
                self.inner.save(run_id, state).await
            }
        }

        // Invoker asserts the store already holds PendingTools + the pending call
        // BEFORE this dispatch runs.
        #[derive(Clone)]
        struct AssertingInvoker {
            store: SpyStore,
        }
        #[async_trait]
        impl ToolInvoker for AssertingInvoker {
            async fn invoke(&self, call: ToolCall) -> ToolCallResult {
                let saved = self.store.load("run-3").await.unwrap().unwrap();
                assert_eq!(saved.phase, RunPhase::PendingTools);
                assert_eq!(saved.pending_tool_calls.len(), 1);
                assert_eq!(saved.pending_tool_calls[0].id, call.id);
                ToolCallResult::ok(call.id, json!({}))
            }
        }

        let src = ScriptSource::new(vec![
            tool_use_completion("tu-9", "act"),
            text_completion("stop", "final"),
        ]);
        let engine = AgentEngine::new(
            src,
            AssertingInvoker {
                store: store.clone(),
            },
            store,
            cfg(),
        );
        let outcome = engine.run("run-3").await;
        assert!(matches!(
            outcome,
            crate::iteration::RunOutcome::Completed { .. }
        ));
    }

    #[tokio::test]
    async fn resumes_pending_tools_without_rerunning_completion() {
        // Seed a store already checkpointed at PendingTools.
        let store = InMemoryStore::new();
        let mut seeded = RunState::new();
        seeded.phase = RunPhase::PendingTools;
        seeded.pending_tool_calls = vec![ToolCall {
            id: "tu-r".into(),
            name: "resume-tool".into(),
            arguments: json!({}),
            connector: None,
        }];
        store.save("run-4", &seeded).await.unwrap();

        // The completion script's FIRST entry is the post-tools follow-up; if the
        // engine wrongly re-ran completion first it would consume this as the
        // resumed turn and dispatch again.
        let src = ScriptSource::new(vec![text_completion("stop", "after-resume")]);
        let inv = RecordingInvoker::default();
        let engine = AgentEngine::new(src, inv.clone(), store, cfg());

        let outcome = engine.run("run-4").await;
        assert!(matches!(
            outcome,
            crate::iteration::RunOutcome::Completed { .. }
        ));
        // Exactly one dispatch: the resumed pending call (not a fresh extraction).
        assert_eq!(inv.dispatched.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn completion_capacity_error_returns_retry_required() {
        let src = ErrSource(CompletionError::Capacity("429".into()));
        let engine = AgentEngine::new(
            src,
            RecordingInvoker::default(),
            InMemoryStore::new(),
            cfg(),
        );
        let outcome = engine.run("run-5").await;
        match outcome {
            crate::iteration::RunOutcome::RetryRequired { class } => {
                assert_eq!(
                    class,
                    crate::seams::RetryClass::Capacity { attempt_hint: 0 }
                );
            },
            other => panic!("expected RetryRequired, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn completion_auth_error_returns_failed() {
        let src = ErrSource(CompletionError::Auth);
        let engine = AgentEngine::new(
            src,
            RecordingInvoker::default(),
            InMemoryStore::new(),
            cfg(),
        );
        let outcome = engine.run("run-6").await;
        assert!(matches!(
            outcome,
            crate::iteration::RunOutcome::Failed { .. }
        ));
    }

    #[tokio::test]
    async fn limit_reached_when_max_iterations_hit() {
        // Never-ending tool loop; max_iterations = 2 forces LimitReached.
        let script: Vec<_> = (0..10)
            .map(|i| tool_use_completion(&format!("tu-{i}"), "loop"))
            .collect();
        let src = ScriptSource::new(script);
        let mut config = cfg();
        config.max_iterations = 2;
        let engine = AgentEngine::new(
            src,
            RecordingInvoker::default(),
            InMemoryStore::new(),
            config,
        );
        let outcome = engine.run("run-7").await;
        assert!(matches!(
            outcome,
            crate::iteration::RunOutcome::LimitReached
        ));
    }
}
