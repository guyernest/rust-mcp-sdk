//! # Consumer example: durable/replay-shaped polling with the loop-free classifier
//!
//! This example is the companion runnable for the "Durable and replay consumers"
//! section of the Tasks chapter (Phase 105, D-10). It demonstrates the pattern a
//! durable/replay-shaped consumer (the pmcp.run durable poller is the motivating
//! shape) SHOULD use: instead of calling the blocking poll-to-terminal helper —
//! which owns a sleep/loop lifecycle that is non-deterministic under replay — the
//! consumer
//! drives a PLAIN poll loop instead of the SDK's blocking poll-to-terminal
//! convenience (the `Client` waiter that owns its own sleep/loop lifecycle),
//! where each iteration:
//!
//!   1. fetches the task once (`tasks_get`),
//!   2. classifies it with the pure, per-poll [`Task::poll_decision`] primitive,
//!   3. and — when still in progress — computes the wait with
//!      [`resolve_poll_interval`] (50 ms floor, so it never hot-spins) before a
//!      single wasm-safe [`pmcp::runtime::sleep`].
//!
//! Two invariants this example locks in as a regression harness (D-06/D-16, A1):
//!
//! - `poll_decision()` classifies WITHOUT any I/O — the network `tasks/get` and
//!   its serde decode happen in `tasks_get`, and only the already-deserialized
//!   [`Task`] is classified. That is the property a durable runtime relies on to
//!   keep the classification step replay-deterministic.
//! - The terminal [`CallToolResult`] comes from a SEPARATE `tasks/result` call the
//!   consumer owns, NOT from the `Terminal` decision variant (which carries only
//!   the status). That separate fetch is guarded behind a `terminal` flag, so the
//!   `input_required` path can NEVER reach it — fetching a result on a
//!   non-terminal task is a protocol misuse this example refuses to model.
//!
//! No fake durable runtime / replay simulator is built here (explicitly out of
//! scope, D-10) — the durable/replay wiring lives in the book prose. This is the
//! minimal single-server harness (A5) that proves the classifier + resolver loop
//! compiles and runs green.
//!
//! Every claim below is a HARD assertion (returns `Err` on failure), not just
//! printed output, so this example doubles as a regression harness (like `s47`).
//!
//! Run with: cargo run --example s48_durable_poll_decision --features full

#![cfg(not(target_arch = "wasm32"))]

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::server::core::ProtocolHandler;
use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
use pmcp::server::typed_tool::TypedTool;
use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::tasks::{resolve_poll_interval, TaskPollDecision, TaskStatus};
use pmcp::types::{CallToolResult, ClientCapabilities, Content, TaskSupport, ToolExecution};
use pmcp::{Client, Error};
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Minimal in-process duplex harness (A5).
//
// Copied down to the SMALLEST single-server pieces needed to mint ONE task from
// tests/task_augmented_result.rs `mod live` — an mpsc-backed duplex transport, a
// pump that answers requests, one "stay working" task tool, and a store-backed
// server builder. The full duplex test harness (counting pump, timeout/clamp
// cases) is NOT lifted; this example only needs to produce one task and drive it
// to terminal out-of-band.
// ---------------------------------------------------------------------------

/// In-process duplex transport (client <-> `ServerCore`), mpsc-backed.
#[derive(Debug)]
struct DuplexTransport {
    tx: mpsc::UnboundedSender<TransportMessage>,
    rx: mpsc::UnboundedReceiver<TransportMessage>,
    connected: bool,
}

impl DuplexTransport {
    fn pair() -> (Self, Self) {
        let (client_tx, server_rx) = mpsc::unbounded_channel();
        let (server_tx, client_rx) = mpsc::unbounded_channel();
        (
            Self {
                tx: client_tx,
                rx: client_rx,
                connected: true,
            },
            Self {
                tx: server_tx,
                rx: server_rx,
                connected: true,
            },
        )
    }
}

#[async_trait]
impl Transport for DuplexTransport {
    async fn send(&mut self, message: TransportMessage) -> pmcp::Result<()> {
        self.tx
            .send(message)
            .map_err(|_| Error::internal("duplex peer dropped"))
    }

    async fn receive(&mut self) -> pmcp::Result<TransportMessage> {
        self.rx
            .recv()
            .await
            .ok_or_else(|| Error::internal("duplex peer closed"))
    }

    async fn close(&mut self) -> pmcp::Result<()> {
        self.connected = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn transport_type(&self) -> &'static str {
        "in-process-duplex"
    }
}

/// Pump that answers every inbound request from the server handler.
fn spawn_pump(mut server_transport: DuplexTransport, handler: Arc<dyn ProtocolHandler>) {
    tokio::spawn(async move {
        while let Ok(message) = server_transport.receive().await {
            if let TransportMessage::Request { id, request } = message {
                let response = handler.handle_request(id, request, None).await;
                if server_transport
                    .send(TransportMessage::Response(response))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    });
}

/// A task tool that starts the task in `working` (never terminal on its own).
/// The example transitions it to `completed` out-of-band via the shared store,
/// so the poll loop observes at least one in-progress iteration first.
fn stay_working_tool() -> impl pmcp::ToolHandler {
    TypedTool::new_with_schema(
        "start_durable_job",
        serde_json::json!({ "type": "object" }),
        |_args: serde_json::Value, _extra| {
            Box::pin(async {
                Ok(serde_json::json!({
                    "taskId": "tool-fabricated",
                    "status": "working",
                    "ttl": 60000,
                    "createdAt": "2026-07-05T00:00:00Z",
                    "lastUpdatedAt": "2026-07-05T00:00:00Z",
                    // A poll hint the classifier surfaces via InProgress { poll_hint }.
                    "pollInterval": 60
                }))
            })
        },
    )
    .with_description("Starts a durable job that stays working until completed out-of-band")
    .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required))
}

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    // The shared store: the server dispatches through it, and the example drives
    // the task to terminal through the SAME handle (mirroring a worker completing
    // durable work behind the scenes).
    let store = Arc::new(InMemoryTaskStore::new());
    let server: Arc<dyn ProtocolHandler> = Arc::new(
        ServerCoreBuilder::new()
            .name("durable-poll-decision-server")
            .version("1.0.0")
            .tool("start_durable_job", stay_working_tool())
            .task_store(store.clone() as Arc<dyn TaskStore>)
            .build()?,
    );

    let (client_transport, server_transport) = DuplexTransport::pair();
    spawn_pump(server_transport, server);

    let mut client = Client::new(client_transport);
    client.initialize(ClientCapabilities::default()).await?;

    // Kick off the task-augmented call; the store mints the real task id.
    let task_id = match client
        .call_tool_with_task("start_durable_job".to_string(), serde_json::json!({}))
        .await?
    {
        pmcp::ToolCallResponse::Task(task) => task.task_id,
        pmcp::ToolCallResponse::Result(_) => {
            return Err(Error::internal(
                "expected a created task, got a synchronous result",
            ))
        },
    };
    println!("started durable job, task id: {task_id}");

    // Simulate the worker finishing the durable work shortly after: persist the
    // terminal result, then flip Working -> Completed. "local" is the owner an
    // unauthenticated duplex session resolves to. Doing this on a background task
    // lets the poll loop observe at least one InProgress iteration first.
    let worker_store = store.clone();
    let worker_task_id = task_id.clone();
    tokio::spawn(async move {
        pmcp::runtime::sleep(Duration::from_millis(120)).await;
        let terminal = CallToolResult::new(vec![Content::text("durable job finished")]);
        // Assert the mutations rather than swallowing them: if the store's owner
        // assumption ever drifts (e.g. "local" stops resolving), this panics loudly
        // in the worker task instead of silently leaving the task Working forever.
        worker_store
            .set_result(&worker_task_id, "local", terminal)
            .await
            .expect("worker: set_result on the durable task must succeed");
        worker_store
            .update_status(&worker_task_id, "local", TaskStatus::Completed, None)
            .await
            .expect("worker: Working -> Completed transition must succeed");
    });

    // ---- The pattern: a PLAIN poll loop over the loop-free classifier. ----
    //
    // A durable runtime would memoize each `tasks_get` as a `ctx.step` and replace
    // the sleep with `ctx.wait(interval)`; here we use the concrete equivalents.
    let mut terminal = false;
    // Wall-clock deadline so a setup regression (e.g. the worker failing to reach
    // Completed) makes this harness FAIL loudly instead of spinning forever and
    // hanging `make test-examples`. The worker completes in ~120 ms; 10 s is ample.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if Instant::now() >= deadline {
            return Err(Error::internal(
                "poll loop did not reach a terminal/input-required decision within 10s \
                 — the durable worker likely never transitioned the task to Completed",
            ));
        }
        // The network fetch + serde decode happen HERE, inside tasks_get. What we
        // classify below is an already-deserialized Task (the replay-deterministic
        // property a durable step relies on — see the book section).
        let task = client.tasks_get(&task_id).await?;
        match task.poll_decision() {
            TaskPollDecision::Terminal { status } => {
                println!("terminal: {status:?} — will fetch the result via a SEPARATE call");
                terminal = true;
                break;
            },
            TaskPollDecision::InputRequired => {
                // A real durable consumer routes this to elicitation and resumes
                // polling. This example simply reports and exits WITHOUT fetching a
                // result: the task is NOT terminal and has no result yet. The
                // `if terminal { ... }` guard below makes tasks_result unreachable
                // on this path (A1 / D-16).
                println!("input required — would route to elicitation; not fetching a result");
                break;
            },
            TaskPollDecision::InProgress { poll_hint } => {
                // resolve_poll_interval floors the wait at 50 ms, so a bad/zero
                // hint can never turn this loop into a hot spin (T-105-01).
                let interval = resolve_poll_interval(None, poll_hint);
                println!("in progress (poll_hint={poll_hint:?}) — sleeping {interval} ms");
                pmcp::runtime::sleep(Duration::from_millis(interval)).await;
            },
            // `TaskPollDecision` is `#[non_exhaustive]` (D-15, future-proofing): a
            // future SDK could add a variant WITHOUT a breaking change. External
            // consumers must carry this wildcard. It is NOT runtime handling of
            // unknown statuses (an unknown status fails at deserialization, before
            // classification) — it is the semver affordance. Treat an unrecognized
            // decision defensively as "keep polling", never as terminal.
            other => {
                let interval = resolve_poll_interval(None, None);
                println!("unrecognized decision {other:?} — polling defensively in {interval} ms");
                pmcp::runtime::sleep(Duration::from_millis(interval)).await;
            },
        }
    }

    if terminal {
        // The Terminal decision carried only the status; the CallToolResult comes
        // from a SEPARATE tasks/result call the consumer owns (D-06/D-16). In a
        // durable runtime this is its own memoized step.
        let result = client.tasks_result(&task_id).await?;
        let text = result
            .content
            .first()
            .and_then(|c| match c {
                Content::Text { text } => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_default();
        println!("terminal result content: {text:?}");

        // HARD assertion — makes this example a compile+run regression harness.
        if text != "durable job finished" {
            return Err(Error::internal(format!(
                "expected terminal result 'durable job finished', got {text:?}"
            )));
        }
        println!("OK: classifier-driven poll loop reached Terminal and fetched the owned result");
    }

    Ok(())
}
