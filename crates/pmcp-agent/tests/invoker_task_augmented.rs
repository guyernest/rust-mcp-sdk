//! AGNT-08: `ClientToolInvoker` honors task-augmented tool results via
//! `wait_for_related_task` (hard-capped), and dispatches bounded-parallel
//! batches preserving input order + ids.
//!
//! The invoker's collaborator is the object-safe `ConnectorClient` seam, so
//! these tests drive it through a controllable mock `ConnectorClient`. The mock
//! builds GENUINE `CallToolResult`s — including a real related-task envelope via
//! `CallToolResult::with_related_task` — so the invoker exercises the real
//! `related_task()` accessor and real `WaitForTaskOptions` plumbing. The mock's
//! `wait_for_related_task` also asserts the invoker set a hard
//! `max_poll_duration_secs` cap, and simulates a never-completing task by
//! returning a timeout error under that cap (never hanging).

#![cfg(not(target_arch = "wasm32"))]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use pmcp::types::tasks::TaskMetadata;
use pmcp::types::{CallToolResult, Content};
use pmcp::{Error, WaitForTaskOptions};

use pmcp_agent::invoker::{ClientToolInvoker, ConnectorClient, InvokerError};
use pmcp_agent::seams::{ToolCall, ToolInvoker};

/// How the mock should behave for a given tool name.
#[derive(Clone)]
enum Behavior {
    /// Return an immediate (non-task) result carrying `value`.
    Immediate(serde_json::Value),
    /// Return a task-augmented result; `wait_for_related_task` then yields the
    /// final `value`.
    Task { value: serde_json::Value },
    /// Return a task-augmented result whose task never completes — the invoker's
    /// hard cap must convert this into a timeout error entry.
    NeverCompletes,
}

/// A controllable, thread-safe mock `ConnectorClient`.
struct MockConnector {
    behavior: Behavior,
    /// Current in-flight `call_tool` count (for the concurrency-bound assertion).
    in_flight: AtomicUsize,
    /// Peak observed in-flight count.
    peak_in_flight: AtomicUsize,
    /// Whether `wait_for_related_task` ever saw a hard cap set by the invoker.
    saw_hard_cap: AtomicUsize,
    /// Small artificial dwell so concurrent calls actually overlap.
    dwell: Duration,
}

impl MockConnector {
    fn new(behavior: Behavior) -> Self {
        Self {
            behavior,
            in_flight: AtomicUsize::new(0),
            peak_in_flight: AtomicUsize::new(0),
            saw_hard_cap: AtomicUsize::new(0),
            dwell: Duration::from_millis(0),
        }
    }

    fn with_dwell(mut self, dwell: Duration) -> Self {
        self.dwell = dwell;
        self
    }

    fn task_result(id: &str) -> CallToolResult {
        CallToolResult::new(vec![Content::text("scheduled")])
            .with_related_task(TaskMetadata::new(id))
    }
}

#[async_trait]
impl ConnectorClient for MockConnector {
    async fn call_tool(
        &self,
        _name: &str,
        _arguments: serde_json::Value,
    ) -> Result<CallToolResult, InvokerError> {
        // Track concurrency across the artificial dwell window.
        let now = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak_in_flight.fetch_max(now, Ordering::SeqCst);
        if !self.dwell.is_zero() {
            tokio::time::sleep(self.dwell).await;
        }
        self.in_flight.fetch_sub(1, Ordering::SeqCst);

        match &self.behavior {
            Behavior::Immediate(value) => Ok(CallToolResult::structured(value.clone())),
            Behavior::Task { .. } => Ok(Self::task_result("task-1")),
            Behavior::NeverCompletes => Ok(Self::task_result("task-forever")),
        }
    }

    async fn wait_for_related_task(
        &self,
        _meta: &TaskMetadata,
        opts: WaitForTaskOptions,
    ) -> Result<CallToolResult, InvokerError> {
        // The invoker MUST set a hard cap so it can never poll forever.
        assert!(
            opts.max_poll_duration_secs.is_some(),
            "invoker must pass a hard max_poll_duration_secs cap"
        );
        self.saw_hard_cap.fetch_add(1, Ordering::SeqCst);

        match &self.behavior {
            Behavior::Task { value } => Ok(CallToolResult::structured(value.clone())),
            Behavior::NeverCompletes => {
                // Simulate the SDK poll budget being exhausted: a real
                // `wait_for_task` returns `Error::timeout` when the cap is hit.
                let cap_ms = opts
                    .max_poll_duration_secs
                    .unwrap_or(0)
                    .saturating_mul(1000);
                Err(InvokerError::Transport(Error::timeout(cap_ms).to_string()))
            },
            Behavior::Immediate(_) => unreachable!("immediate results carry no related task"),
        }
    }
}

fn call(id: &str, name: &str) -> ToolCall {
    ToolCall {
        id: id.to_string(),
        name: name.to_string(),
        arguments: serde_json::json!({}),
        connector: None,
    }
}

#[tokio::test]
async fn invoke_drives_task_augmented_result_to_final_value() {
    let connector = Arc::new(MockConnector::new(Behavior::Task {
        value: serde_json::json!({ "answer": 42 }),
    }));
    let invoker = ClientToolInvoker::new(connector.clone(), 5);

    let result = invoker.invoke(call("c1", "slow_tool")).await;

    assert_eq!(result.id, "c1");
    assert!(!result.is_error, "final task result must not be an error");
    assert_eq!(result.content, serde_json::json!({ "answer": 42 }));
    assert_eq!(
        connector.saw_hard_cap.load(Ordering::SeqCst),
        1,
        "the task path must go through wait_for_related_task under a hard cap"
    );
}

#[tokio::test]
async fn invoke_returns_immediate_result_directly() {
    let connector = Arc::new(MockConnector::new(Behavior::Immediate(
        serde_json::json!({ "ok": true }),
    )));
    let invoker = ClientToolInvoker::new(connector.clone(), 5);

    let result = invoker.invoke(call("c2", "fast_tool")).await;

    assert_eq!(result.id, "c2");
    assert!(!result.is_error);
    assert_eq!(result.content, serde_json::json!({ "ok": true }));
    // No related task ⇒ wait_for_related_task never touched.
    assert_eq!(connector.saw_hard_cap.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn invoke_batch_is_bounded_input_ordered_and_id_matched() {
    // A dwell makes calls overlap; a bound of 2 must cap peak concurrency at 2.
    let connector = Arc::new(
        MockConnector::new(Behavior::Immediate(serde_json::json!({ "ok": true })))
            .with_dwell(Duration::from_millis(50)),
    );
    let invoker = ClientToolInvoker::new(connector.clone(), 5).with_max_concurrency(2);

    let calls = vec![
        call("a", "t1"),
        call("b", "t2"),
        call("c", "t3"),
        call("d", "t4"),
        call("e", "t5"),
    ];
    let results = invoker.invoke_batch(calls.clone()).await;

    // Exactly one result per input, input order + id correlation preserved.
    assert_eq!(results.len(), calls.len());
    for (input, result) in calls.iter().zip(results.iter()) {
        assert_eq!(
            input.id, result.id,
            "results must be id-matched in input order"
        );
        assert!(!result.is_error);
    }
    let peak = connector.peak_in_flight.load(Ordering::SeqCst);
    assert!(peak >= 2, "expected real overlap, saw peak {peak}");
    assert!(
        peak <= 2,
        "invoke_batch must bound concurrency to 2, saw {peak}"
    );
}

#[tokio::test]
async fn invoke_batch_of_three_task_calls_matches_ids_in_order() {
    let connector = Arc::new(MockConnector::new(Behavior::Task {
        value: serde_json::json!({ "done": true }),
    }));
    let invoker = ClientToolInvoker::new(connector, 5);

    let calls = vec![call("x", "t1"), call("y", "t2"), call("z", "t3")];
    let results = invoker.invoke_batch(calls.clone()).await;

    assert_eq!(results.len(), 3);
    let ids: Vec<&str> = results.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, vec!["x", "y", "z"]);
    for r in &results {
        assert!(!r.is_error);
        assert_eq!(r.content, serde_json::json!({ "done": true }));
    }
}

#[tokio::test]
async fn never_completing_task_hits_hard_max_and_returns_timeout_error() {
    let connector = Arc::new(MockConnector::new(Behavior::NeverCompletes));
    let invoker = ClientToolInvoker::new(connector, 1);

    let result = invoker.invoke(call("t", "hang_tool")).await;

    // Error surfaces as DATA (not a panic/hang) with the id preserved.
    assert_eq!(result.id, "t");
    assert!(
        result.is_error,
        "a never-completing task must return an error entry"
    );
    let msg = result.error.unwrap_or_default();
    assert!(
        msg.to_lowercase().contains("timeout") || msg.to_lowercase().contains("timed out"),
        "expected a timeout error, got: {msg}"
    );
}
