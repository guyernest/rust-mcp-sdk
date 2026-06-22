//! Shared task-lifecycle dispatch unit used by BOTH `Server` and `ServerCore`.
//!
//! Phase 101 landed the complete `tasks/*` lifecycle on `ServerCore` /
//! `ServerCoreBuilder` only. Phase 102 extracts that machinery into ONE shared
//! place (this module — research Option A) so the high-level `Server` / HTTP
//! dispatcher can serve the same lifecycle without re-implementing it (drift).
//!
//! This module hosts:
//! - [`apply_tasks_capability_rule`] — the endpoint-backed `tasks`-capability
//!   rule, a free function over explicit params (the two builders hold
//!   `tool_infos` at different lifecycle points, so it cannot be a method).
//! - [`default_tasks_capability`] — the FROZEN advertised `ServerTasksCapability`
//!   shape (do not re-derive its JSON).
//! - [`TaskDispatch`] — a borrow-struct over `(&task_store, &task_router)` that
//!   owns owner-resolution, the create-path response (with the self-enforcing
//!   create gate), `tasks/result` precedence, and `tasks/get|list|cancel`
//!   routing.
//! - [`success_response`] / [`error_response`] — the SINGLE-SOURCE JSON-RPC
//!   envelope builders (`ServerCore` delegates to these; there is exactly one
//!   copy of the wrapping logic).
//!
//! The ENTIRE module is gated `#[cfg(not(target_arch = "wasm32"))]` because every
//! task item is non-wasm (mirrors `ServerCore`'s task fields/methods).

#![cfg(not(target_arch = "wasm32"))]

use crate::error::{Error, Result};
use crate::server::auth::AuthContext;
use crate::server::task_store::TaskStore;
use crate::server::tasks::TaskRouter;
use crate::types::capabilities::{ServerCapabilities, ServerTasksCapability};
use crate::types::jsonrpc::ResponsePayload;
use crate::types::tasks::{TaskStatus, RELATED_TASK_META_KEY};
use crate::types::tools::TaskSupport;
use crate::types::{
    CallToolResult, ClientRequest, JSONRPCError, JSONRPCResponse, RequestId, ToolInfo,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Build the default server-level `tasks` capability advertised when a task
/// backend (a [`TaskStore`] or a [`TaskRouter`]) is present.
///
/// This is the exact FROZEN [`ServerTasksCapability`] shape the client
/// `assert_capability` expects; it must not be hand-rolled at any call site.
/// Both [`apply_tasks_capability_rule`] and `ServerCoreBuilder` use this single
/// definition so the advertised capability shape can never drift.
pub(crate) fn default_tasks_capability() -> ServerTasksCapability {
    ServerTasksCapability {
        list: Some(serde_json::json!({})),
        cancel: Some(serde_json::json!({})),
        requests: Some(crate::types::capabilities::ServerTasksRequestCapability {
            tools: Some(crate::types::capabilities::ServerTasksToolsCapability {
                call: Some(serde_json::json!({})),
            }),
        }),
    }
}

/// Apply the endpoint-backed `tasks`-capability rule (D-CAPABILITY-ENDPOINT-BACKED).
///
/// This is the SINGLE shared rule both `ServerCoreBuilder` and (Plan 02)
/// `ServerBuilder` call. It is a free function over explicit params rather than a
/// builder method because the two builders hold `tool_infos` at different
/// lifecycle points (`ServerCoreBuilder` fills it at `.tool()`; `ServerBuilder`
/// builds it locally inside `build()`).
///
/// The `tasks` capability advertised in `initialize` represents REAL endpoint
/// support, never tool metadata alone:
/// - It is auto-advertised only when a backend exists (`has_backend`) and the
///   author has not already configured a custom `tasks` capability (additive-only
///   — an explicit value is preserved verbatim).
/// - A tool declaring [`TaskSupport::Required`] with NO backend is a build-time
///   validation error (rather than a hollow capability whose `tasks/*` endpoints
///   cannot work).
/// - An `Optional`/`Forbidden` task tool with no backend is NOT an error and does
///   NOT by itself trigger advertisement.
///
/// # Errors
///
/// Returns a validation error if any registered tool declares
/// [`TaskSupport::Required`] but no `TaskStore` or `TaskRouter` backs the
/// `tasks/*` endpoints.
pub(crate) fn apply_tasks_capability_rule(
    capabilities: &mut ServerCapabilities,
    tool_infos: &HashMap<String, ToolInfo>,
    has_backend: bool,
) -> Result<()> {
    let has_required_task_tool = tool_infos.values().any(|info| {
        info.execution
            .as_ref()
            .and_then(|e| e.task_support)
            .is_some_and(|ts| matches!(ts, TaskSupport::Required))
    });

    if has_required_task_tool && !has_backend {
        return Err(Error::validation(
            "a tool declares TaskSupport::Required but no TaskStore or TaskRouter \
             is configured to back the tasks/* endpoints",
        ));
    }

    if capabilities.tasks.is_none() && has_backend {
        capabilities.tasks = Some(default_tasks_capability());
    }

    Ok(())
}

/// Create a success JSON-RPC response (SINGLE-SOURCE envelope builder).
///
/// `ServerCore::success_response` delegates to this; there is exactly one copy of
/// the wrapping logic so the shared unit and `ServerCore` cannot drift.
pub(crate) fn success_response(id: RequestId, result: Value) -> JSONRPCResponse {
    JSONRPCResponse {
        jsonrpc: "2.0".to_string(),
        id,
        payload: ResponsePayload::Result(result),
    }
}

/// Create an error JSON-RPC response (SINGLE-SOURCE envelope builder).
///
/// `ServerCore::error_response` delegates to this; there is exactly one copy of
/// the wrapping logic so the shared unit and `ServerCore` cannot drift.
pub(crate) fn error_response(id: RequestId, code: i32, message: String) -> JSONRPCResponse {
    JSONRPCResponse {
        jsonrpc: "2.0".to_string(),
        id,
        payload: ResponsePayload::Error(JSONRPCError {
            code,
            message,
            data: None,
        }),
    }
}

/// Borrow-struct holding the two task backend handles a dispatcher owns.
///
/// Both `Server` and `ServerCore` construct a `TaskDispatch` borrowing their own
/// `task_store`/`task_router` fields and call into it — the task-lifecycle logic
/// lives HERE, once, never as a divergent second copy.
pub(crate) struct TaskDispatch<'a> {
    /// Standard task backend (polling path). Presence flips `tasks` capability on.
    pub task_store: &'a Option<Arc<dyn TaskStore>>,
    /// Legacy experimental router backend (fall-through path).
    pub task_router: &'a Option<Arc<dyn TaskRouter>>,
}

impl TaskDispatch<'_> {
    /// Resolve the owner ID from the authentication context.
    ///
    /// Returns `None` if no backend is configured. With a `TaskRouter`, delegates
    /// to [`TaskRouter::resolve_owner`] (priority chain: OAuth subject > client ID
    /// > session ID > "local"). With only a `TaskStore`, derives the owner from
    /// the auth context directly. Owner is ALWAYS derived from auth/router, NEVER
    /// from client params (IDOR mitigation, T-102-01).
    pub(crate) fn resolve_owner(&self, auth_context: Option<&AuthContext>) -> Option<String> {
        // Legacy path: TaskRouter has its own resolve_owner logic.
        if let Some(router) = self.task_router {
            return Some(match auth_context {
                Some(ctx) => {
                    router.resolve_owner(Some(&ctx.subject), ctx.client_id.as_deref(), None)
                },
                None => router.resolve_owner(None, None, None),
            });
        }
        // Standard path: derive owner from auth context when task_store is configured.
        if self.task_store.is_some() {
            return Some(match auth_context {
                Some(ctx) => ctx.client_id.clone().unwrap_or_else(|| ctx.subject.clone()),
                None => "local".to_string(),
            });
        }
        None
    }

    /// Extract the terminal [`CallToolResult`] from a task-shaped tool value.
    ///
    /// Per `D-TERMINAL-RESULT-CONTRACT`: if the value carries a `result` object or
    /// a `content` array, deserialize it into a [`CallToolResult`]; otherwise the
    /// task is genuinely pending and there is no synchronous terminal result.
    pub(crate) fn extract_terminal_result(value: &Value) -> Option<CallToolResult> {
        if let Some(result_value) = value.get("result") {
            return serde_json::from_value::<CallToolResult>(result_value.clone()).ok();
        }
        if value.get("content").is_some() {
            return serde_json::from_value::<CallToolResult>(value.clone()).ok();
        }
        None
    }

    /// Build the `tools/call` create-task response.
    ///
    /// Per `D-STORE-MINTS-ID`: when a [`TaskStore`] is configured the store mints
    /// the canonical task id via `store.create()`; that store-minted id is
    /// reflected on the WIRE in BOTH `CreateTaskResult.task.taskId` AND the
    /// `_meta.relatedTask.taskId` envelope (never the tool's fabricated id). When
    /// the terminal result is present (synchronous completion) it is persisted via
    /// `store.set_result()` and the task is transitioned `Working -> Completed`
    /// BEFORE the response returns, so a subsequent `tasks/get` shows `Completed`.
    ///
    /// SIGNATURE NOTE: this fn does NOT take `task_id` or the terminal `result` as
    /// params — it RE-EXTRACTS them from `value` internally (the store-minted id
    /// comes back from `store.create`, and `extract_terminal_result(&value)`
    /// recovers the terminal result for persistence). A future refactor that stops
    /// re-extracting MUST add explicit params instead — never silently drop the
    /// terminal-result persistence (that would regress synchronous completion).
    ///
    /// Falls back to the legacy tool-fabricated envelope only when no store is
    /// configured (preserves prior behavior for router-only servers).
    pub(crate) async fn build_task_created_response(
        &self,
        id: RequestId,
        value: Value,
        auth_context: Option<&AuthContext>,
    ) -> JSONRPCResponse {
        // Re-extract the tool-fabricated task id and the terminal result from the
        // raw value (see SIGNATURE NOTE above). `task_id` here is only used for the
        // legacy no-store fallback envelope; with a store, the store-minted id wins.
        let tool_task_id = value
            .get("taskId")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let Some(store) = self.task_store.as_ref() else {
            // No store: preserve the legacy tool-fabricated envelope.
            let result_value = serde_json::json!({
                "task": value,
                "_meta": { RELATED_TASK_META_KEY: { "taskId": tool_task_id } }
            });
            return success_response(id, result_value);
        };

        let owner_id = self
            .resolve_owner(auth_context)
            .unwrap_or_else(|| "local".to_string());

        // Carry the tool's requested TTL onto the store-minted task, if present.
        let ttl = value.get("ttl").and_then(serde_json::Value::as_u64);

        let created = match store.create(&owner_id, ttl).await {
            Ok(task) => task,
            Err(e) => return error_response(id, -32603, e.to_string()),
        };
        let store_id = created.task_id.clone();

        // Synchronous completion: persist the terminal result and complete.
        let terminal_result = Self::extract_terminal_result(&value);
        let final_task = if let Some(call_result) = terminal_result {
            if let Err(e) = store.set_result(&store_id, &owner_id, call_result).await {
                return error_response(id, -32603, e.to_string());
            }
            match store
                .update_status(&store_id, &owner_id, TaskStatus::Completed, None)
                .await
            {
                Ok(task) => task,
                Err(e) => return error_response(id, -32603, e.to_string()),
            }
        } else {
            created
        };

        // Build the wire envelope from the STORE-minted task (typed, no
        // hand-written task JSON) so task.taskId == _meta id == store id.
        let create_result = crate::types::tasks::CreateTaskResult::new(final_task);
        let mut envelope = serde_json::to_value(create_result).unwrap_or_default();
        if let Some(obj) = envelope.as_object_mut() {
            obj.insert(
                "_meta".to_string(),
                serde_json::json!({ RELATED_TASK_META_KEY: { "taskId": store_id } }),
            );
        }
        success_response(id, envelope)
    }

    /// Self-enforcing create-path gate: decide whether a `tools/call` becomes a
    /// task and, if so, build the create response.
    ///
    /// This is the SINGLE source of truth for "should this `tools/call` become a
    /// task?". Both dispatchers call it; neither re-derives the gate. The helper
    /// enforces the COMPLETE gate INTERNALLY — the caller passes raw facts
    /// (`task_requested`, the tool's `task_support`, the produced `value`), never a
    /// pre-checked precondition.
    ///
    /// Returns `Some(envelope)` IFF ALL of:
    /// - `task_requested == true` (the request carried a `task` field), AND
    /// - a backend is present (`self.task_store.is_some()`), AND
    /// - `task_support ∈ {Required, Optional}`, AND
    /// - `value` carries BOTH a `taskId` and a `status` (task-shaped).
    ///
    /// `TaskSupport::Forbidden`/`None`, `task_requested == false`, an absent
    /// backend, or a non-task-shaped value ALL return `None` ("fall through to a
    /// normal `CallToolResult`") with NO error leak (T-102-11).
    // Why: proven by the in-module `gate_tests` truth-table in Plan 01; both
    // dispatchers (`ServerCore` + `Server`) wire it into their create-path in
    // Plan 02. The shared gate must exist and be proven HERE first.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) async fn maybe_build_task_created(
        &self,
        id: RequestId,
        value: &Value,
        task_support: Option<TaskSupport>,
        task_requested: bool,
        auth_context: Option<&AuthContext>,
    ) -> Option<JSONRPCResponse> {
        let gate_open = task_requested
            && self.task_store.is_some()
            && task_support
                .is_some_and(|ts| matches!(ts, TaskSupport::Required | TaskSupport::Optional));
        if !gate_open {
            return None;
        }
        // Task-shaped value check: must carry BOTH a taskId and a status.
        let is_task_shaped = value.get("taskId").and_then(Value::as_str).is_some()
            && value.get("status").is_some();
        if !is_task_shaped {
            return None;
        }
        Some(
            self.build_task_created_response(id, value.clone(), auth_context)
                .await,
        )
    }

    /// Handle a `tasks/result` request (store-first → router → -32002 → -32601).
    ///
    /// Serves from the configured [`TaskStore`] FIRST when it `supports_results()`,
    /// but FALLS THROUGH to the [`TaskRouter`] on store `NotFound`/unsupported —
    /// never a hard error when a router can serve it. When the store has no result
    /// and NO router is configured, returns the SPECIFIED "task not completed"
    /// error (`-32002`), distinct from the truly-no-backend `-32601` (FROZEN by
    /// Phase 101; T-102-03).
    pub(crate) async fn handle_tasks_result(
        &self,
        id: RequestId,
        params: &crate::types::tasks::GetTaskPayloadRequest,
        auth_context: Option<&AuthContext>,
    ) -> JSONRPCResponse {
        let owner_id = self
            .resolve_owner(auth_context)
            .unwrap_or_else(|| "local".to_string());

        // Store-first: serve a typed CallToolResult when the store persists one.
        if let Some(store) = self.task_store {
            if store.supports_results() {
                match store.get_result(&params.task_id, &owner_id).await {
                    Ok(call_result) => {
                        return success_response(
                            id,
                            serde_json::to_value(call_result).unwrap_or_default(),
                        );
                    },
                    // NotFound = store doesn't have it (absent / pending / owner
                    // mismatch): fall through to the router below.
                    Err(crate::server::task_store::TaskStoreError::NotFound { .. }) => {},
                    Err(e) => return error_response(id, -32603, e.to_string()),
                }
            }
        }

        // Router fallback — behavior UNCHANGED for router-backed servers.
        if let Some(task_router) = self.task_router {
            return match task_router
                .handle_tasks_result(serde_json::to_value(params).unwrap_or_default(), &owner_id)
                .await
            {
                Ok(result) => success_response(id, result),
                Err(e) => error_response(id, -32603, e.to_string()),
            };
        }

        // No router: distinguish "store exists but task not completed yet"
        // (specified error) from "no task backend at all".
        if self.task_store.is_some() {
            error_response(
                id,
                -32002,
                "task result not available: task not completed".to_string(),
            )
        } else {
            error_response(id, -32601, "tasks/result not supported".to_string())
        }
    }

    /// Route a `tasks/get` request (store-first, router fall-through).
    async fn route_tasks_get(
        &self,
        id: RequestId,
        params: &crate::types::tasks::GetTaskRequest,
        auth_context: Option<&AuthContext>,
    ) -> JSONRPCResponse {
        if let Some(store) = self.task_store {
            let owner_id = self
                .resolve_owner(auth_context)
                .unwrap_or_else(|| "local".to_string());
            match store.get(&params.task_id, &owner_id).await {
                Ok(task) => {
                    let result = crate::types::tasks::GetTaskResult::new(task);
                    success_response(id, serde_json::to_value(result).unwrap())
                },
                Err(e) => error_response(id, -32603, e.to_string()),
            }
        } else if let Some(task_router) = self.task_router {
            let owner_id = self
                .resolve_owner(auth_context)
                .unwrap_or_else(|| "local".to_string());
            match task_router
                .handle_tasks_get(serde_json::to_value(params).unwrap_or_default(), &owner_id)
                .await
            {
                Ok(result) => success_response(id, result),
                Err(e) => error_response(id, -32603, e.to_string()),
            }
        } else {
            error_response(id, -32601, "Tasks not enabled".to_string())
        }
    }

    /// Route a `tasks/list` request (store-first, router fall-through).
    async fn route_tasks_list(
        &self,
        id: RequestId,
        params: &crate::types::tasks::ListTasksRequest,
        auth_context: Option<&AuthContext>,
    ) -> JSONRPCResponse {
        if let Some(store) = self.task_store {
            let owner_id = self
                .resolve_owner(auth_context)
                .unwrap_or_else(|| "local".to_string());
            match store.list(&owner_id, params.cursor.as_deref()).await {
                Ok((tasks, next_cursor)) => {
                    let mut result = crate::types::tasks::ListTasksResult::new(tasks);
                    if let Some(cursor) = next_cursor {
                        result = result.with_next_cursor(cursor);
                    }
                    success_response(id, serde_json::to_value(result).unwrap())
                },
                Err(e) => error_response(id, -32603, e.to_string()),
            }
        } else if let Some(task_router) = self.task_router {
            let owner_id = self
                .resolve_owner(auth_context)
                .unwrap_or_else(|| "local".to_string());
            match task_router
                .handle_tasks_list(serde_json::to_value(params).unwrap_or_default(), &owner_id)
                .await
            {
                Ok(result) => success_response(id, result),
                Err(e) => error_response(id, -32603, e.to_string()),
            }
        } else {
            error_response(id, -32601, "Tasks not enabled".to_string())
        }
    }

    /// Route a `tasks/cancel` request (store-first, router fall-through).
    async fn route_tasks_cancel(
        &self,
        id: RequestId,
        params: &crate::types::tasks::CancelTaskRequest,
        auth_context: Option<&AuthContext>,
    ) -> JSONRPCResponse {
        if let Some(store) = self.task_store {
            let owner_id = self
                .resolve_owner(auth_context)
                .unwrap_or_else(|| "local".to_string());
            match store.cancel(&params.task_id, &owner_id).await {
                Ok(task) => {
                    let result = crate::types::tasks::CancelTaskResult::new(task);
                    success_response(id, serde_json::to_value(result).unwrap())
                },
                Err(e) => error_response(id, -32603, e.to_string()),
            }
        } else if let Some(task_router) = self.task_router {
            let owner_id = self
                .resolve_owner(auth_context)
                .unwrap_or_else(|| "local".to_string());
            match task_router
                .handle_tasks_cancel(serde_json::to_value(params).unwrap_or_default(), &owner_id)
                .await
            {
                Ok(result) => success_response(id, result),
                Err(e) => error_response(id, -32603, e.to_string()),
            }
        } else {
            error_response(id, -32601, "Tasks not enabled".to_string())
        }
    }

    /// Route any `tasks/*` endpoint request to its handler.
    ///
    /// Dispatches `TasksGet`/`TasksList`/`TasksCancel` to their per-endpoint
    /// helpers and `TasksResult` to [`Self::handle_tasks_result`]. Non-`tasks/*`
    /// variants return the FROZEN `-32601 "Method not supported"` (callers only
    /// pass `tasks/*` variants here).
    pub(crate) async fn route_tasks_endpoint(
        &self,
        id: RequestId,
        request: &ClientRequest,
        auth_context: Option<&AuthContext>,
    ) -> JSONRPCResponse {
        match request {
            ClientRequest::TasksGet(params) => self.route_tasks_get(id, params, auth_context).await,
            ClientRequest::TasksResult(params) => {
                self.handle_tasks_result(id, params, auth_context).await
            },
            ClientRequest::TasksList(params) => {
                self.route_tasks_list(id, params, auth_context).await
            },
            ClientRequest::TasksCancel(params) => {
                self.route_tasks_cancel(id, params, auth_context).await
            },
            _ => error_response(id, -32601, "Method not supported".to_string()),
        }
    }
}

#[cfg(test)]
mod gate_tests {
    use super::*;
    use crate::server::task_store::InMemoryTaskStore;
    use crate::types::RequestId;

    fn store_backend() -> Option<Arc<dyn TaskStore>> {
        Some(Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>)
    }

    fn task_shaped_value() -> Value {
        serde_json::json!({
            "taskId": "tool-fabricated",
            "status": "completed",
            "result": { "content": [{ "type": "text", "text": "done" }] }
        })
    }

    fn id() -> RequestId {
        RequestId::from(1i64)
    }

    /// task_requested == false → None regardless of other inputs.
    #[tokio::test]
    async fn gate_rejects_when_not_task_requested() {
        let store = store_backend();
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
        };
        let value = task_shaped_value();
        let out = dispatch
            .maybe_build_task_created(id(), &value, Some(TaskSupport::Required), false, None)
            .await;
        assert!(out.is_none(), "task_requested=false must yield None");
    }

    /// task_requested == true but no backend → None.
    #[tokio::test]
    async fn gate_rejects_when_no_backend() {
        let store = None;
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
        };
        let value = task_shaped_value();
        let out = dispatch
            .maybe_build_task_created(id(), &value, Some(TaskSupport::Required), true, None)
            .await;
        assert!(out.is_none(), "no backend must yield None");
    }

    /// task_requested, backend, TaskSupport::Forbidden → None (no error leak).
    #[tokio::test]
    async fn gate_rejects_forbidden_no_error_leak() {
        let store = store_backend();
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
        };
        let value = task_shaped_value();
        let out = dispatch
            .maybe_build_task_created(id(), &value, Some(TaskSupport::Forbidden), true, None)
            .await;
        assert!(out.is_none(), "Forbidden must yield None, never an error");
    }

    /// task_requested, backend, TaskSupport::None → None.
    #[tokio::test]
    async fn gate_rejects_no_task_support() {
        let store = store_backend();
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
        };
        let value = task_shaped_value();
        let out = dispatch
            .maybe_build_task_created(id(), &value, None, true, None)
            .await;
        assert!(out.is_none(), "no task_support must yield None");
    }

    /// Required-with-backend, value missing taskId/status → None.
    #[tokio::test]
    async fn gate_rejects_non_task_shaped_value() {
        let store = store_backend();
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
        };
        let value = serde_json::json!({ "foo": "bar" });
        let out = dispatch
            .maybe_build_task_created(id(), &value, Some(TaskSupport::Required), true, None)
            .await;
        assert!(out.is_none(), "non-task-shaped value must yield None");
    }

    /// Assert the Some-case three-way store-minted-id invariant on an envelope.
    fn assert_store_minted(resp: &JSONRPCResponse) {
        let ResponsePayload::Result(value) = &resp.payload else {
            panic!("expected a success result envelope");
        };
        let wire_task_id = value
            .get("task")
            .and_then(|t| t.get("taskId"))
            .and_then(Value::as_str)
            .expect("task.taskId present");
        let meta_id = value
            .get("_meta")
            .and_then(|m| m.get(RELATED_TASK_META_KEY))
            .and_then(|r| r.get("taskId"))
            .and_then(Value::as_str)
            .expect("_meta.relatedTask.taskId present");
        assert_eq!(
            wire_task_id, meta_id,
            "three-way invariant: task.taskId == _meta.relatedTask.taskId"
        );
        assert_ne!(
            wire_task_id, "tool-fabricated",
            "wire id must be store-minted, not the tool-fabricated id"
        );
    }

    /// task_requested, backend, TaskSupport::Optional, task-shaped → Some + invariant.
    #[tokio::test]
    async fn gate_accepts_optional_task_shaped() {
        let store = store_backend();
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
        };
        let value = task_shaped_value();
        let out = dispatch
            .maybe_build_task_created(id(), &value, Some(TaskSupport::Optional), true, None)
            .await;
        let resp = out.expect("Optional + task-shaped must yield Some");
        assert_store_minted(&resp);
    }

    /// task_requested, backend, TaskSupport::Required, task-shaped → Some + invariant.
    #[tokio::test]
    async fn gate_accepts_required_task_shaped() {
        let store = store_backend();
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
        };
        let value = task_shaped_value();
        let out = dispatch
            .maybe_build_task_created(id(), &value, Some(TaskSupport::Required), true, None)
            .await;
        let resp = out.expect("Required + task-shaped must yield Some");
        assert_store_minted(&resp);
    }
}
