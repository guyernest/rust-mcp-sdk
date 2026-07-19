//! AGNT-03 replay-safety property + golden fixtures.
//!
//! The load-bearing durability contract: feeding IDENTICAL effect results to the
//! engine must yield IDENTICAL decision SEQUENCES — not merely identical final
//! results (Codex MEDIUM). The property runs the engine TWICE over one recorded
//! [`EffectTrace`] via `ReplaySource`/`ReplayInvoker` and asserts the two
//! [`DecisionTrace`]s are byte-for-byte equal. Two golden fixtures pin the exact
//! terminal outcome + decision sequence for an end-turn run and a tool-loop run.

use pmcp::types::content::Role;
use pmcp::types::sampling::{CreateMessageResultWithTools, SamplingMessageContent};
use pmcp_agent::iteration::AgentEngine;
use pmcp_agent::trace::OutcomeTag;
use pmcp_agent::{
    ConversationStore, DecisionTrace, EffectTrace, InMemoryStore, ReplayInvoker, ReplaySource,
    ResolvedAgentConfig, ToolCallResult,
};
use proptest::prelude::*;
use serde_json::json;

/// A config with high limits so the loop terminates on model decisions, not
/// limits — keeping the property focused on decision determinism.
fn replay_config() -> ResolvedAgentConfig {
    ResolvedAgentConfig::new("be helpful", "golden-model", u32::MAX, 1000)
}

/// Run the engine once over `trace`, returning the recorded decision sequence.
///
/// Uses `futures::executor::block_on`: every seam here is pure/in-memory (no
/// timers or real I/O), so no tokio runtime is needed and each proptest case is
/// cheap.
fn run_once(trace: &EffectTrace, config: &ResolvedAgentConfig) -> DecisionTrace {
    futures::executor::block_on(async {
        let source = ReplaySource::from_trace(trace);
        let invoker = ReplayInvoker::from_trace(trace);
        let store = InMemoryStore::new();
        let run_id = "replay";
        if let Some(state) = &trace.initial_state {
            store.save(run_id, state).await.expect("seed initial state");
        }
        let engine = AgentEngine::new(source, invoker, store, config.clone());
        let mut decisions = DecisionTrace::default();
        engine.run_traced(run_id, &mut decisions).await;
        decisions
    })
}

fn text_completion(stop: &str, text: &str) -> CreateMessageResultWithTools {
    CreateMessageResultWithTools::new(
        "golden-model",
        Role::Assistant,
        vec![SamplingMessageContent::Text {
            text: text.into(),
            meta: None,
        }],
    )
    .with_stop_reason(stop)
}

fn tool_use_completion(id: &str) -> CreateMessageResultWithTools {
    CreateMessageResultWithTools::new(
        "golden-model",
        Role::Assistant,
        vec![SamplingMessageContent::ToolUse {
            name: "act".into(),
            id: id.into(),
            input: json!({}),
            meta: None,
        }],
    )
    .with_stop_reason("tool_use")
}

/// One scripted step of a generated run.
#[derive(Clone, Debug)]
enum ScriptStep {
    /// A terminal completion (stop_reason "stop").
    Final,
    /// A tool-use completion paired with one tool-batch result.
    Tool,
}

fn arb_script() -> impl Strategy<Value = Vec<ScriptStep>> {
    prop::collection::vec(
        prop_oneof![Just(ScriptStep::Final), Just(ScriptStep::Tool)],
        0..6,
    )
}

/// Build a CONSISTENT `EffectTrace`: one tool-batch per tool completion, always
/// terminated by a final completion (so the run Completes deterministically
/// rather than exhausting the trace).
fn build_trace(steps: &[ScriptStep]) -> EffectTrace {
    let mut completions = Vec::new();
    let mut batches = Vec::new();
    let mut ended = false;
    for (i, step) in steps.iter().enumerate() {
        match step {
            ScriptStep::Tool => {
                let tid = format!("tu-{i}");
                completions.push(tool_use_completion(&tid));
                batches.push(vec![ToolCallResult::ok(tid, json!({ "i": i as u64 }))]);
            },
            ScriptStep::Final => {
                completions.push(text_completion("stop", "final"));
                ended = true;
                break;
            },
        }
    }
    if !ended {
        completions.push(text_completion("stop", "final"));
    }
    EffectTrace::new(completions, batches)
}

proptest! {
    /// AGNT-03: identical effect results ⇒ identical DECISION SEQUENCES.
    #[test]
    fn identical_trace_yields_identical_decision_sequence(steps in arb_script()) {
        let trace = build_trace(&steps);
        let config = replay_config();
        let first = run_once(&trace, &config);
        let second = run_once(&trace, &config);
        // Full DecisionTrace equality = same step-by-step decisions AND outcome,
        // not just the final result.
        prop_assert_eq!(&first, &second);
        // The run terminated with an observable outcome.
        prop_assert!(first.outcome.is_some());
        // A generated run always terminates on a final completion.
        prop_assert_eq!(first.outcome, Some(OutcomeTag::Completed));
    }
}

#[test]
fn golden_end_turn_completes_in_one_step() {
    let trace: EffectTrace =
        serde_json::from_str(include_str!("fixtures/golden_trace_end_turn.json"))
            .expect("valid end-turn fixture");
    let decisions = run_once(&trace, &replay_config());

    assert_eq!(decisions.outcome, Some(OutcomeTag::Completed));
    assert_eq!(decisions.steps.len(), 1);
    assert!(decisions.steps[0].is_end_turn);
    assert!(decisions.steps[0].is_final);
    assert!(decisions.steps[0].tool_call_ids.is_empty());

    // Deterministic across runs.
    assert_eq!(decisions, run_once(&trace, &replay_config()));
}

#[test]
fn golden_tool_loop_dispatches_then_completes() {
    let trace: EffectTrace =
        serde_json::from_str(include_str!("fixtures/golden_trace_tool_loop.json"))
            .expect("valid tool-loop fixture");
    let decisions = run_once(&trace, &replay_config());

    assert_eq!(decisions.outcome, Some(OutcomeTag::Completed));
    assert_eq!(decisions.steps.len(), 2);

    // Step 0: dispatched tu-1, not final.
    assert!(!decisions.steps[0].is_end_turn);
    assert!(!decisions.steps[0].is_final);
    assert_eq!(decisions.steps[0].tool_call_ids, vec!["tu-1".to_string()]);

    // Step 1: end_turn ("stop"), final, no dispatch.
    assert!(decisions.steps[1].is_end_turn);
    assert!(decisions.steps[1].is_final);
    assert!(decisions.steps[1].tool_call_ids.is_empty());

    // Deterministic across runs.
    assert_eq!(decisions, run_once(&trace, &replay_config()));
}
