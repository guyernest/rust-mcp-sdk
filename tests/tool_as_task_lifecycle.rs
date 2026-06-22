//! Tools-as-Tasks live round-trip acceptance gate (Phase 101, TASKDX-04/05).
//!
//! This is the phase's acceptance gate: a LIVE in-process client round-trip
//! through the real `pmcp::Client` deserialization types, plus the
//! `pmcp::testing::assert_roundtrips_through_client` conformance helper fed
//! ACTUAL `ServerCore::handle_request` dispatch output (never an author-written
//! fixture). It proves all four original wire-shape bugs are impossible on the
//! SDK path:
//!
//! - finding #1 — typed, non-empty `tasks/result` from a persisted terminal
//!   `CallToolResult` (and a specified pending-result error before completion).
//! - finding #3 — id consistency: `CreateTaskResult.task.taskId` ==
//!   `tasks/get` task id == `_meta.relatedTask.taskId` (all the store-minted id).
//! - finding #4 — `initialize` through the real client observes the
//!   auto-advertised `tasks` capability.
//!
//! The high-level `pmcp::Server` (and thus `StreamableHttpServer`) does NOT
//! carry a `TaskStore`; the task path lives on `ServerCore`. So the loopback
//! harness here pairs a real `pmcp::Client` with a `ServerCore` over an
//! in-process duplex transport (the equivalent of the HTTP loopback for the
//! task-bearing dispatch path).

#![cfg(not(target_arch = "wasm32"))]

use std::sync::Arc;

use async_trait::async_trait;
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::server::core::ProtocolHandler;
use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
use pmcp::server::typed_tool::TypedTool;
use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::tasks::{CreateTaskResult, GetTaskResult, RELATED_TASK_META_KEY};
use pmcp::types::{
    CallToolRequest, CallToolResult, ClientCapabilities, ClientRequest, GetTaskPayloadRequest,
    GetTaskRequest, InitializeRequest, Request, RequestId, TaskSupport, ToolExecution,
};
use pmcp::{Client, Error, ErrorCode};
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// In-process duplex transport (client <-> ServerCore), mpsc-backed.
// ---------------------------------------------------------------------------

/// One half of an in-process duplex transport. The client side sends Requests
/// and receives Responses; the server side does the reverse. This is the
/// minimal pump the `Client` request/response loop needs (send then receive).
#[derive(Debug)]
struct DuplexTransport {
    tx: mpsc::UnboundedSender<TransportMessage>,
    rx: mpsc::UnboundedReceiver<TransportMessage>,
    connected: bool,
}

impl DuplexTransport {
    /// Create a connected client/server transport pair.
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

/// Spawn a server pump that serves `handler` over the server side of a duplex
/// transport: receive Request, dispatch, send Response. Runs until the client
/// side is dropped.
fn spawn_server_pump(mut server_transport: DuplexTransport, handler: Arc<dyn ProtocolHandler>) {
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

// ---------------------------------------------------------------------------
// Tool builders.
// ---------------------------------------------------------------------------

/// A `with_task_support` tool whose handler returns a Task-shaped value that
/// ALSO carries a nested `result` (the terminal `CallToolResult`). The dispatch
/// task-detection requires `taskId` + `status` to enter the task path, and
/// `extract_terminal_result` lifts the nested `result` so plan 01's
/// synchronous-completion path records create + `set_result` + Completed before
/// returning. Polling therefore observes `Completed` deterministically, and
/// `tasks/result` serves a non-empty terminal `CallToolResult`.
fn completing_task_tool() -> impl pmcp::ToolHandler {
    TypedTool::new_with_schema(
        "complete_now",
        serde_json::json!({ "type": "object" }),
        |_args: serde_json::Value, _extra| {
            Box::pin(async {
                Ok(serde_json::json!({
                    "taskId": "tool-fabricated",
                    "status": "completed",
                    "ttl": 60000,
                    "createdAt": "2026-06-21T00:00:00Z",
                    "lastUpdatedAt": "2026-06-21T00:00:00Z",
                    "result": {
                        "content": [ { "type": "text", "text": "terminal result payload" } ]
                    }
                }))
            })
        },
    )
    .with_description("A task tool that completes synchronously with content")
    .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required))
}

/// A `with_task_support` tool whose handler returns NO result content (only task
/// metadata), so the task stays genuinely pending — used to prove the specified
/// not-completed error from `tasks/result`.
fn pending_task_tool() -> impl pmcp::ToolHandler {
    TypedTool::new_with_schema(
        "stay_pending",
        serde_json::json!({ "type": "object" }),
        |_args: serde_json::Value, _extra| {
            Box::pin(async {
                Ok(serde_json::json!({
                    "taskId": "tool-fabricated",
                    "status": "working",
                    "ttl": 60000,
                    "createdAt": "2026-06-21T00:00:00Z",
                    "lastUpdatedAt": "2026-06-21T00:00:00Z"
                }))
            })
        },
    )
    .with_description("A task tool that stays pending (no terminal content)")
    .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required))
}

fn init_request() -> Request {
    Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
        pmcp::types::Implementation::new("test-client", "1.0.0"),
        ClientCapabilities::default(),
    ))))
}

fn build_completing_server() -> Arc<dyn ProtocolHandler> {
    let store = Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>;
    Arc::new(
        ServerCoreBuilder::new()
            .name("task-lifecycle-server")
            .version("1.0.0")
            .tool("complete_now", completing_task_tool())
            .task_store(store)
            .build()
            .expect("server builds"),
    )
}

// ---------------------------------------------------------------------------
// Live client round-trip (findings #1, #3, #4) over the duplex transport.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_round_trip_typed_lifecycle_id_consistency_and_capability() {
    let server = build_completing_server();
    let (client_transport, server_transport) = DuplexTransport::pair();
    spawn_server_pump(server_transport, server);

    let mut client = Client::new(client_transport);

    // finding #4: initialize through the real client sees the advertised `tasks`
    // capability (TASKDX-02 end-to-end via an endpoint-backed server).
    let init = client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize succeeds");
    assert!(
        init.capabilities.tasks.is_some(),
        "server must auto-advertise the `tasks` capability (TASKDX-02)"
    );

    // call(tool with task) -> CreateTaskResult with the store-minted id.
    let response = client
        .call_tool_with_task("complete_now".to_string(), serde_json::json!({}))
        .await
        .expect("task-augmented call succeeds");
    let client_task_id = match response {
        pmcp::ToolCallResponse::Task(task) => task.task_id,
        pmcp::ToolCallResponse::Result(_) => panic!("expected a created task, got a sync result"),
    };
    assert_ne!(
        client_task_id, "tool-fabricated",
        "wire id must be the store-minted id, not a tool-fabricated one"
    );

    // Poll tasks/get until terminal; the polled id must equal the client id
    // (finding #3 — proves it is the store-minted, pollable id, not a 404).
    let mut polled = client.tasks_get(&client_task_id).await.expect("tasks/get");
    for _ in 0..50 {
        assert_eq!(
            polled.task_id, client_task_id,
            "polled task id must equal the client-observed (store-minted) id"
        );
        if polled.status.is_terminal() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        polled = client.tasks_get(&client_task_id).await.expect("tasks/get");
    }
    assert!(
        polled.status.is_terminal(),
        "task should reach a terminal status (synchronous completion)"
    );

    // finding #1: typed, non-empty CallToolResult from tasks/result.
    let result = client
        .tasks_result(&client_task_id)
        .await
        .expect("tasks/result succeeds");
    assert!(
        !result.content.is_empty(),
        "tasks/result must carry the persisted non-empty terminal content"
    );
}

/// finding #3 (the `_meta` leg): the create envelope's
/// `_meta.relatedTask.taskId` equals the wire `task.taskId` (the store-minted
/// id). The client's `call_tool_with_task` discards `_meta`, so this leg is
/// asserted against the REAL `handle_request` envelope.
#[tokio::test]
async fn create_envelope_meta_related_task_id_matches_wire_task_id() {
    let server = build_completing_server();
    server
        .handle_request(RequestId::from(0i64), init_request(), None)
        .await;

    let mut call = CallToolRequest::new("complete_now", serde_json::json!({}));
    call.task = Some(serde_json::json!({}));
    let response = server
        .handle_request(
            RequestId::from(1i64),
            Request::Client(Box::new(ClientRequest::CallTool(call))),
            None,
        )
        .await;

    let value = expect_result(response);
    let wire_id = value["task"]["taskId"]
        .as_str()
        .expect("task.taskId is a string");
    let meta_id = value["_meta"][RELATED_TASK_META_KEY]["taskId"]
        .as_str()
        .expect("_meta.relatedTask.taskId is a string");
    assert_eq!(
        wire_id, meta_id,
        "_meta.relatedTask.taskId must equal the wire task.taskId (store-minted)"
    );
}

/// finding #1 (MED): `tasks/result` on a not-yet-Completed task returns the
/// SPECIFIED not-completed error (`-32002`), surfaced through the live client —
/// distinct from `NotFound`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pending_task_result_returns_specified_not_completed_error() {
    let store = Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>;
    let server: Arc<dyn ProtocolHandler> = Arc::new(
        ServerCoreBuilder::new()
            .name("pending-server")
            .version("1.0.0")
            .tool("stay_pending", pending_task_tool())
            .task_store(store)
            .build()
            .expect("server builds"),
    );

    let (client_transport, server_transport) = DuplexTransport::pair();
    spawn_server_pump(server_transport, server);
    let mut client = Client::new(client_transport);
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize");

    let response = client
        .call_tool_with_task("stay_pending".to_string(), serde_json::json!({}))
        .await
        .expect("task-augmented call");
    let task_id = match response {
        pmcp::ToolCallResponse::Task(task) => task.task_id,
        pmcp::ToolCallResponse::Result(_) => panic!("expected a created (pending) task"),
    };

    let err = client
        .tasks_result(&task_id)
        .await
        .expect_err("pending task result must be an error, not Ok");
    assert_eq!(
        err.error_code(),
        Some(ErrorCode(-32002)),
        "pending tasks/result must return the specified -32002 not-completed error, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// Conformance helper on REAL dispatch output (TASKDX-04, finding repudiation).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn conformance_helper_consumes_real_dispatch_output_for_all_three_types() {
    let server = build_completing_server();
    server
        .handle_request(RequestId::from(0i64), init_request(), None)
        .await;

    // create -> CreateTaskResult envelope (serde ignores the extra _meta).
    let mut call = CallToolRequest::new("complete_now", serde_json::json!({}));
    call.task = Some(serde_json::json!({}));
    let create_value = expect_result(
        server
            .handle_request(
                RequestId::from(1i64),
                Request::Client(Box::new(ClientRequest::CallTool(call))),
                None,
            )
            .await,
    );
    pmcp::testing::assert_roundtrips_through_client::<CreateTaskResult>(create_value.clone());

    let task_id = create_value["task"]["taskId"]
        .as_str()
        .expect("task id")
        .to_string();

    // tasks/get -> GetTaskResult.
    let get_value = expect_result(
        server
            .handle_request(
                RequestId::from(2i64),
                Request::Client(Box::new(ClientRequest::TasksGet(GetTaskRequest {
                    task_id: task_id.clone(),
                }))),
                None,
            )
            .await,
    );
    pmcp::testing::assert_roundtrips_through_client::<GetTaskResult>(get_value);

    // tasks/result -> CallToolResult.
    let result_value = expect_result(
        server
            .handle_request(
                RequestId::from(3i64),
                Request::Client(Box::new(ClientRequest::TasksResult(
                    GetTaskPayloadRequest { task_id },
                ))),
                None,
            )
            .await,
    );
    pmcp::testing::assert_roundtrips_through_client::<CallToolResult>(result_value);
}

fn expect_result(response: pmcp::types::JSONRPCResponse) -> serde_json::Value {
    match response.payload {
        pmcp::types::jsonrpc::ResponsePayload::Result(value) => value,
        pmcp::types::jsonrpc::ResponsePayload::Error(error) => {
            panic!("expected a result payload, got error: {error:?}")
        },
    }
}

// ---------------------------------------------------------------------------
// Constrained property round-trips (NOT arbitrary full protocol values).
// ---------------------------------------------------------------------------

mod property {
    use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
    use pmcp::types::tasks::TaskStatus;
    use pmcp::types::{CallToolResult, Content};
    use proptest::prelude::*;

    /// A bounded `TaskStatus` strategy — a small enum, not arbitrary values.
    fn task_status_strategy() -> impl Strategy<Value = TaskStatus> {
        prop_oneof![
            Just(TaskStatus::Working),
            Just(TaskStatus::Completed),
            Just(TaskStatus::Failed),
            Just(TaskStatus::Cancelled),
        ]
    }

    /// A bounded `CallToolResult` strategy: 0..=3 short text-content items.
    fn call_tool_result_strategy() -> impl Strategy<Value = CallToolResult> {
        proptest::collection::vec("[a-z ]{0,16}", 0..=3)
            .prop_map(|texts| CallToolResult::new(texts.into_iter().map(Content::text).collect()))
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(48))]

        /// CallToolResult survives serde_json::to_value -> from_value unchanged
        /// in content arity (the exact transform the wire path performs).
        #[test]
        fn call_tool_result_serde_round_trip(result in call_tool_result_strategy()) {
            let value = serde_json::to_value(&result).expect("serialize");
            let back: CallToolResult = serde_json::from_value(value).expect("deserialize");
            prop_assert_eq!(back.content.len(), result.content.len());
        }

        /// TaskStatus survives serde round-trip and terminal classification is stable.
        #[test]
        fn task_status_serde_round_trip(status in task_status_strategy()) {
            let value = serde_json::to_value(status).expect("serialize");
            let back: TaskStatus = serde_json::from_value(value).expect("deserialize");
            prop_assert_eq!(back.is_terminal(), status.is_terminal());
        }

        /// A persisted CallToolResult round-trips through store set_result ->
        /// get_result (owner-scoped) for a bounded set of generated values.
        #[test]
        fn store_set_get_result_round_trip(result in call_tool_result_strategy()) {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                let store = InMemoryTaskStore::new();
                let task = store.create("owner-1", Some(60_000)).await.expect("create");
                store
                    .set_result(&task.task_id, "owner-1", result.clone())
                    .await
                    .expect("set_result");
                let fetched = store
                    .get_result(&task.task_id, "owner-1")
                    .await
                    .expect("get_result");
                prop_assert_eq!(fetched.content.len(), result.content.len());
                Ok(())
            })
            .unwrap();
        }
    }
}
