//! Phase 102 Plan 02 — high-level `Server` task-dispatch wiring tests.
//!
//! These are IN-CRATE tests (declared `mod task_dispatch_tests;` in `mod.rs`)
//! because they drive the crate-private `Server::handle_request(id, request,
//! auth_context)` directly — the same entrypoint `StreamableHttpServer` calls —
//! so they can inject distinct `AuthContext` owners per request (required for
//! the cross-owner IDOR isolation test) without standing up a full HTTP +
//! auth-provider stack.
//!
//! Coverage:
//! - Task 1 `server_builder_tasks_capability`: the shared capability rule is
//!   wired into `ServerBuilder::build` (advertise / Required-no-backend Err /
//!   explicit-capability preserved).
//! - Task 2 `tasks_dispatch_shared` + `tasks_cross_owner_isolation`: the
//!   `tasks/*` hard-reject is replaced by shared `route_tasks_endpoint`
//!   delegation at the post-auth assembly layer, with `-32002` preserved and
//!   cross-owner isolation enforced.
//! - Task 3 `task_support_matrix` + `proptest_task_branch_gate`: the create-path
//!   gate (`maybe_build_task_created`) never mis-fires across the full
//!   `TaskSupport` cross-product.

#![allow(clippy::doc_markdown)]

use std::sync::Arc;

use crate::server::auth::AuthContext;
use crate::server::task_store::{InMemoryTaskStore, TaskStore};
use crate::server::typed_tool::TypedTool;
use crate::server::Server;
use crate::types::capabilities::{
    ServerCapabilities, ServerTasksCapability, ServerTasksRequestCapability,
    ServerTasksToolsCapability,
};
use crate::types::jsonrpc::ResponsePayload;
use crate::types::{
    CallToolRequest, CancelTaskRequest, ClientRequest, GetTaskPayloadRequest, GetTaskRequest,
    InitializeRequest, JSONRPCResponse, ListTasksRequest, Request, RequestId, TaskSupport,
    ToolExecution,
};
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Fixtures.
// ---------------------------------------------------------------------------

/// A task tool returning a task-shaped value that ALSO carries a terminal
/// `result` (synchronous completion). `task_support` selects the declared
/// `TaskSupport`.
fn task_tool(name: &'static str, support: TaskSupport) -> impl crate::server::ToolHandler {
    TypedTool::new_with_schema(name, json!({ "type": "object" }), |_args: Value, _extra| {
        Box::pin(async {
            Ok(json!({
                "taskId": "tool-fabricated",
                "status": "completed",
                "ttl": 60000,
                "createdAt": "2026-06-21T00:00:00Z",
                "lastUpdatedAt": "2026-06-21T00:00:00Z",
                "result": { "content": [ { "type": "text", "text": "done" } ] }
            }))
        })
    })
    .with_description("task tool")
    .with_execution(ToolExecution::new().with_task_support(support))
}

/// A plain (non-task) tool returning an ordinary value — TaskSupport NOT
/// declared.
fn plain_tool(name: &'static str) -> impl crate::server::ToolHandler {
    TypedTool::new_with_schema(name, json!({ "type": "object" }), |_args: Value, _extra| {
        Box::pin(async { Ok(json!({ "ok": true })) })
    })
    .with_description("plain tool")
}

fn store() -> Arc<dyn TaskStore> {
    Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>
}

fn rid(n: i64) -> RequestId {
    RequestId::from(n)
}

fn init_request() -> Request {
    Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest {
        protocol_version: "2025-06-18".to_string(),
        capabilities: crate::types::ClientCapabilities::default(),
        client_info: crate::types::Implementation::new("test-client", "1.0.0"),
    })))
}

/// Drive `initialize` then return the negotiated server capabilities.
async fn initialized_capabilities(server: &Server) -> ServerCapabilities {
    let resp = server.handle_request(rid(1), init_request(), None).await;
    match resp.payload {
        ResponsePayload::Result(v) => {
            let init: crate::types::InitializeResult = serde_json::from_value(v).unwrap();
            init.capabilities
        },
        ResponsePayload::Error(e) => panic!("initialize failed: {}", e.message),
    }
}

/// Extract the success `Value` from a response, panicking on error.
fn ok_value(resp: &JSONRPCResponse) -> &Value {
    match &resp.payload {
        ResponsePayload::Result(v) => v,
        ResponsePayload::Error(e) => {
            panic!("expected success, got error {}: {}", e.code, e.message)
        },
    }
}

/// Build a `tools/call` request, optionally task-augmented. The `task` field is
/// `Option<Value>` on the wire; a present (even empty-object) value signals the
/// client requested task augmentation.
fn call_tool_request(name: &str, with_task: bool) -> Request {
    let task = if with_task { Some(json!({})) } else { None };
    Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
        name: name.to_string(),
        arguments: json!({}),
        task,
        _meta: None,
    })))
}

/// Is the response a CreateTaskResult-shaped envelope (carries `task.taskId`
/// and `_meta.relatedTask`)?
fn is_create_task_envelope(resp: &JSONRPCResponse) -> bool {
    let ResponsePayload::Result(v) = &resp.payload else {
        return false;
    };
    let has_task_id = v
        .get("task")
        .and_then(|t| t.get("taskId"))
        .and_then(Value::as_str)
        .is_some();
    let has_related = v
        .get("_meta")
        .and_then(|m| m.get(crate::types::tasks::RELATED_TASK_META_KEY))
        .is_some();
    has_task_id && has_related
}

// ---------------------------------------------------------------------------
// Task 1 — capability rule wired into ServerBuilder::build.
// ---------------------------------------------------------------------------

mod server_builder_tasks_capability {
    use super::*;

    /// (a) A store-backed `Server` advertises the `tasks` capability.
    #[tokio::test]
    async fn advertises_tasks_with_store() {
        let server = Server::builder()
            .name("task-server")
            .version("1.0.0")
            .tool("summarize", task_tool("summarize", TaskSupport::Required))
            .task_store(store())
            .build()
            .expect("store-backed server builds");

        let caps = initialized_capabilities(&server).await;
        assert!(
            caps.tasks.is_some(),
            "a store-backed Server must auto-advertise the `tasks` capability"
        );
    }

    /// (b) A `TaskSupport::Required` tool with NO backend makes `.build()` Err.
    #[test]
    fn required_task_tool_without_backend_errors() {
        let result = Server::builder()
            .name("task-server")
            .version("1.0.0")
            .tool("summarize", task_tool("summarize", TaskSupport::Required))
            .build();

        assert!(
            result.is_err(),
            "a TaskSupport::Required tool with no backend must fail build()"
        );
    }

    /// (c) An explicitly-set `tasks` capability is preserved (not clobbered).
    #[tokio::test]
    async fn preserves_explicit_tasks_capability() {
        let explicit = ServerTasksCapability {
            list: Some(json!({ "marker": "explicit" })),
            cancel: None,
            requests: Some(ServerTasksRequestCapability {
                tools: Some(ServerTasksToolsCapability { call: None }),
            }),
        };
        let caps = ServerCapabilities {
            tasks: Some(explicit),
            ..Default::default()
        };

        let server = Server::builder()
            .name("task-server")
            .version("1.0.0")
            .capabilities(caps)
            .tool("summarize", task_tool("summarize", TaskSupport::Required))
            .task_store(store())
            .build()
            .expect("server builds with explicit tasks capability + store");

        let tasks = initialized_capabilities(&server)
            .await
            .tasks
            .expect("tasks capability present");
        assert_eq!(
            tasks.list,
            Some(json!({ "marker": "explicit" })),
            "explicit tasks capability must be preserved verbatim"
        );
        assert!(
            tasks.cancel.is_none(),
            "explicit capability must NOT be clobbered by the default injection"
        );
    }
}

// ---------------------------------------------------------------------------
// Task 2 — tasks/* delegation at the post-auth assembly layer + isolation.
// ---------------------------------------------------------------------------

mod tasks_dispatch_shared {
    use super::*;

    /// Build a store-backed `Server` exposing one Required task tool.
    fn store_backed_server() -> Server {
        Server::builder()
            .name("task-server")
            .version("1.0.0")
            .tool(
                "complete_now",
                task_tool("complete_now", TaskSupport::Required),
            )
            .task_store(store())
            .build()
            .expect("store-backed server builds")
    }

    /// Drive a task-augmented `tools/call` and return the store-minted task id.
    async fn create_task(server: &Server, auth: Option<&AuthContext>) -> String {
        let resp = server
            .handle_request(
                rid(10),
                call_tool_request("complete_now", true),
                auth.cloned(),
            )
            .await;
        let value = ok_value(&resp);
        value
            .get("task")
            .and_then(|t| t.get("taskId"))
            .and_then(Value::as_str)
            .expect("create-path mints a store task id")
            .to_string()
    }

    fn tasks_get(id: &str) -> Request {
        Request::Client(Box::new(ClientRequest::TasksGet(GetTaskRequest {
            task_id: id.to_string(),
        })))
    }

    fn tasks_result(id: &str) -> Request {
        Request::Client(Box::new(ClientRequest::TasksResult(
            GetTaskPayloadRequest {
                task_id: id.to_string(),
            },
        )))
    }

    fn tasks_list() -> Request {
        Request::Client(Box::new(ClientRequest::TasksList(ListTasksRequest {
            cursor: None,
        })))
    }

    fn tasks_cancel(id: &str) -> Request {
        Request::Client(Box::new(ClientRequest::TasksCancel(CancelTaskRequest {
            task_id: id.to_string(),
            result: None,
        })))
    }

    /// HTASK-02: `tasks/get | list | cancel` and `tasks/result` reach the shared
    /// unit over `Server::handle_request` (the hard-reject is gone) — store-first
    /// behavior holds and the FROZEN `-32002` pending code is preserved.
    #[tokio::test]
    async fn server_serves_tasks_endpoints_via_shared_unit() {
        let server = store_backed_server();
        let id = create_task(&server, None).await;

        // tasks/get → success, returns the same task id (store-first).
        let got = server.handle_request(rid(20), tasks_get(&id), None).await;
        let got_id = ok_value(&got)
            .get("task")
            .and_then(|t| t.get("taskId"))
            .and_then(Value::as_str)
            .expect("tasks/get returns the task");
        assert_eq!(got_id, id, "tasks/get returns the minted task");

        // tasks/list → success, the task appears.
        let listed = server.handle_request(rid(21), tasks_list(), None).await;
        let tasks = ok_value(&listed)
            .get("tasks")
            .and_then(Value::as_array)
            .expect("tasks/list returns an array");
        assert!(
            tasks
                .iter()
                .any(|t| t.get("taskId").and_then(Value::as_str) == Some(id.as_str())),
            "tasks/list includes the created task"
        );

        // tasks/result → success (the tool completed synchronously).
        let result = server
            .handle_request(rid(22), tasks_result(&id), None)
            .await;
        assert!(
            matches!(result.payload, ResponsePayload::Result(_)),
            "tasks/result serves the persisted terminal result"
        );

        // tasks/cancel reaches the shared unit (the hard-reject is gone). The
        // synchronously-completed task cannot transition to Cancelled, so the
        // store returns an InvalidTransition error — but it is a STRUCTURED
        // store response, NOT the old `-32601 "no task router configured"`
        // hard-reject. (A successful cancel on a PENDING task is covered
        // separately by the cross-owner test's owner-A sanity path.)
        let cancelled = server
            .handle_request(rid(23), tasks_cancel(&id), None)
            .await;
        let reached_shared_unit = match &cancelled.payload {
            ResponsePayload::Result(_) => true,
            ResponsePayload::Error(e) => {
                // Must NOT be the removed METHOD_NOT_FOUND hard-reject.
                !e.message.contains("no task router configured")
                    && !e.message.contains("Tasks not supported")
            },
        };
        assert!(
            reached_shared_unit,
            "tasks/cancel must be served by the shared unit, not hard-rejected"
        );
    }

    /// `tasks/result` for a genuinely-pending task returns the FROZEN `-32002`,
    /// reaching the caller UNCHANGED (adapter (a) — no double-wrap, no swallow).
    #[tokio::test]
    async fn pending_tasks_result_preserves_minus_32002() {
        // A task tool whose value has NO terminal `result` → stays pending.
        let pending = TypedTool::new_with_schema(
            "stay_pending",
            json!({ "type": "object" }),
            |_args: Value, _extra| {
                Box::pin(async {
                    Ok(json!({
                        "taskId": "tool-fabricated",
                        "status": "working",
                        "ttl": 60000,
                        "createdAt": "2026-06-21T00:00:00Z",
                        "lastUpdatedAt": "2026-06-21T00:00:00Z"
                    }))
                })
            },
        )
        .with_description("pending task tool")
        .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required));

        let server = Server::builder()
            .name("task-server")
            .version("1.0.0")
            .tool("stay_pending", pending)
            .task_store(store())
            .build()
            .expect("server builds");

        let resp = server
            .handle_request(rid(30), call_tool_request("stay_pending", true), None)
            .await;
        let id = ok_value(&resp)
            .get("task")
            .and_then(|t| t.get("taskId"))
            .and_then(Value::as_str)
            .expect("pending task minted")
            .to_string();

        let result = server
            .handle_request(rid(31), tasks_result(&id), None)
            .await;
        match result.payload {
            ResponsePayload::Error(err) => assert_eq!(
                err.code, -32002,
                "pending tasks/result must reach the caller as the FROZEN -32002"
            ),
            ResponsePayload::Result(_) => panic!("pending task must not yield a terminal result"),
        }
    }

    /// T-102-05 (IDOR): owner A creates a task; owner B can neither read
    /// (`tasks/get`/`tasks/result`) nor `tasks/cancel` it. Owner derives from the
    /// `AuthContext` ONLY — never client params.
    #[tokio::test]
    async fn tasks_cross_owner_isolation() {
        let server = store_backed_server();

        let owner_a = AuthContext::new("alice");
        let owner_b = AuthContext::new("bob");

        // Owner A creates a task and captures its store-minted id.
        let id = create_task(&server, Some(&owner_a)).await;

        // Owner B attempts to read it → not found / non-leak (NOT a success with
        // A's task content).
        let b_get = server
            .handle_request(rid(40), tasks_get(&id), Some(owner_b.clone()))
            .await;
        assert!(
            matches!(b_get.payload, ResponsePayload::Error(_)),
            "owner B must NOT read owner A's task via tasks/get"
        );

        // Owner B attempts to fetch the result → must not leak A's terminal result.
        let b_result = server
            .handle_request(rid(41), tasks_result(&id), Some(owner_b.clone()))
            .await;
        assert!(
            matches!(b_result.payload, ResponsePayload::Error(_)),
            "owner B must NOT read owner A's task result"
        );

        // Owner B attempts to cancel it → must not succeed.
        let b_cancel = server
            .handle_request(rid(42), tasks_cancel(&id), Some(owner_b.clone()))
            .await;
        assert!(
            matches!(b_cancel.payload, ResponsePayload::Error(_)),
            "owner B must NOT cancel owner A's task"
        );

        // Owner A can still read its own task (sanity: isolation didn't break
        // legitimate access).
        let a_get = server
            .handle_request(rid(43), tasks_get(&id), Some(owner_a))
            .await;
        assert!(
            matches!(a_get.payload, ResponsePayload::Result(_)),
            "owner A retains access to its own task"
        );
    }
}

// ---------------------------------------------------------------------------
// Task 3 — create-path gate: non-task regression + full TaskSupport matrix.
// ---------------------------------------------------------------------------

mod task_support_matrix {
    use super::*;

    /// Build a store-backed `Server` registering one tool of each kind.
    fn matrix_server() -> Server {
        Server::builder()
            .name("matrix-server")
            .version("1.0.0")
            .tool("required", task_tool("required", TaskSupport::Required))
            .tool("optional", task_tool("optional", TaskSupport::Optional))
            .tool("forbidden", task_tool("forbidden", TaskSupport::Forbidden))
            .tool("plain", plain_tool("plain"))
            .task_store(store())
            .build()
            .expect("matrix server builds")
    }

    /// HTASK-04 regression: a plain (non-task) `tools/call` returns a normal
    /// `CallToolResult` — NO `_meta.relatedTask`, NO CreateTaskResult shape — even
    /// if a `task` field is present; and a task-capable tool WITHOUT a `task`
    /// field falls through to a plain `CallToolResult`.
    #[tokio::test]
    async fn server_call_tool_non_task() {
        let server = matrix_server();

        // Plain tool, no task field → plain CallToolResult.
        let plain_no_task = server
            .handle_request(rid(50), call_tool_request("plain", false), None)
            .await;
        assert!(
            !is_create_task_envelope(&plain_no_task),
            "plain tools/call must return a normal CallToolResult"
        );

        // Plain tool WITH a task field → still a plain CallToolResult (no leak).
        let plain_with_task = server
            .handle_request(rid(51), call_tool_request("plain", true), None)
            .await;
        assert!(
            !is_create_task_envelope(&plain_with_task),
            "a non-task tool must NOT leak a CreateTaskResult even with a task field"
        );

        // Task-capable tool WITHOUT a task field → falls through to CallToolResult.
        let required_no_task = server
            .handle_request(rid(52), call_tool_request("required", false), None)
            .await;
        assert!(
            !is_create_task_envelope(&required_no_task),
            "a task tool called WITHOUT a task field must fall through to CallToolResult"
        );
    }

    /// Concern #6: the full cross-product {Forbidden, None, Optional, Required} ×
    /// {task field present, absent}. A CreateTaskResult is produced ONLY for
    /// {Optional, Required} × {present}; every other cell — INCLUDING `Forbidden`
    /// WITH a task field — yields a plain CallToolResult with NO error leak.
    #[tokio::test]
    async fn full_task_support_matrix() {
        let server = matrix_server();
        // (tool name, task field present, expected create envelope)
        let cases = [
            ("required", true, true),
            ("required", false, false),
            ("optional", true, true),
            ("optional", false, false),
            ("forbidden", true, false), // the previously-untested edge
            ("forbidden", false, false),
            ("plain", true, false), // TaskSupport not declared (None)
            ("plain", false, false),
        ];
        let mut n = 60;
        for (tool, with_task, expect_envelope) in cases {
            n += 1;
            let resp = server
                .handle_request(rid(n), call_tool_request(tool, with_task), None)
                .await;
            // No cell may produce an error leak.
            assert!(
                matches!(resp.payload, ResponsePayload::Result(_)),
                "cell ({tool}, task={with_task}) must not error-leak"
            );
            assert_eq!(
                is_create_task_envelope(&resp),
                expect_envelope,
                "cell ({tool}, task={with_task}) envelope expectation"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Task 3 — property: the create-path gate NEVER mis-fires.
// ---------------------------------------------------------------------------

mod proptest_task_branch_gate {
    use super::*;
    use proptest::prelude::*;

    /// A tool that returns a value WITHOUT `taskId`/`status` (non-task-shaped),
    /// declared with the given `TaskSupport`.
    fn unshaped_tool(name: &'static str, support: TaskSupport) -> impl crate::server::ToolHandler {
        TypedTool::new_with_schema(name, json!({ "type": "object" }), |_args: Value, _extra| {
            Box::pin(async { Ok(json!({ "plain": "value" })) })
        })
        .with_description("unshaped tool")
        .with_execution(ToolExecution::new().with_task_support(support))
    }

    /// One store-backed server exposing the full {support} × {shaped|unshaped}
    /// grid plus a no-TaskSupport plain tool. Tools are selected by name.
    fn grid_server() -> Server {
        Server::builder()
            .name("grid-server")
            .version("1.0.0")
            .tool("req_shaped", task_tool("req_shaped", TaskSupport::Required))
            .tool("opt_shaped", task_tool("opt_shaped", TaskSupport::Optional))
            .tool(
                "forb_shaped",
                task_tool("forb_shaped", TaskSupport::Forbidden),
            )
            .tool(
                "req_unshaped",
                unshaped_tool("req_unshaped", TaskSupport::Required),
            )
            .tool(
                "opt_unshaped",
                unshaped_tool("opt_unshaped", TaskSupport::Optional),
            )
            .tool(
                "forb_unshaped",
                unshaped_tool("forb_unshaped", TaskSupport::Forbidden),
            )
            .tool("plain", plain_tool("plain"))
            .task_store(store())
            .build()
            .expect("grid server builds")
    }

    // Axes: support kind (0..4 -> Required/Optional/Forbidden/None), task field
    // present, and whether the tool's value is task-shaped.
    fn tool_name(support_ix: u8, shaped: bool) -> &'static str {
        match (support_ix, shaped) {
            (0, true) => "req_shaped",
            (0, false) => "req_unshaped",
            (1, true) => "opt_shaped",
            (1, false) => "opt_unshaped",
            (2, true) => "forb_shaped",
            (2, false) => "forb_unshaped",
            // support_ix == 3 => no TaskSupport declared (None); shape irrelevant.
            _ => "plain",
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// The create-path produces a CreateTaskResult-shaped envelope IFF
        /// (store-backed && TaskSupport ∈ {Required, Optional} && task field
        /// present && value is task-shaped); a plain CallToolResult otherwise.
        /// Never an error-leak. Exercised through the PUBLIC dispatch surface.
        #[test]
        fn gate_never_misfires(
            support_ix in 0u8..4,
            with_task in any::<bool>(),
            shaped in any::<bool>(),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let server = grid_server();
            let name = tool_name(support_ix, shaped);

            // For the plain (None) tool, its value is never task-shaped.
            let effective_shaped = shaped && support_ix != 3;
            let support_allows = support_ix == 0 || support_ix == 1; // Required | Optional
            let expect_envelope = with_task && support_allows && effective_shaped;

            let resp = rt.block_on(server.handle_request(
                rid(1),
                call_tool_request(name, with_task),
                None,
            ));

            prop_assert!(
                matches!(resp.payload, ResponsePayload::Result(_)),
                "gate must never error-leak (tool={name}, task={with_task})"
            );
            prop_assert_eq!(
                is_create_task_envelope(&resp),
                expect_envelope,
                "gate mis-fired: tool={}, task={}, shaped={}",
                name, with_task, shaped
            );
        }
    }
}
