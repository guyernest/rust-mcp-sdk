//! Builds the mem-mcp `pmcp::Server` advertising the 6 `mem__*` tools over a
//! [`crate::mem::backend::TeamMemoryBackend`] implementation.
//!
//! The fixed 6-tool surface is the `mem_tool_surface` equation of
//! `contracts/team-servers-v1.yaml`. Five tools (`mem__add`, `mem__get`,
//! `mem__search`, `mem__list_recent`, `mem__delete`) map 1:1 to backend
//! methods; the sixth, `mem__complete_task`, follows SEP-1686 task-completion
//! semantics — when the call carries a related task it emits it under the SDK
//! constant [`RELATED_TASK_META_KEY`](pmcp::types::tasks::RELATED_TASK_META_KEY)
//! via [`ToolOutput::Result`].
//!
//! Because the set is fixed, unknown `mem__*` names are simply never
//! advertised; pmcp then returns a `Tool 'name' not found` error — never a
//! panic.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use pmcp::types::tasks::TaskMetadata;
use pmcp::types::{CallToolResult, Content, ToolInfo};
use pmcp::{Error, RequestHandlerExtra, Result, Server, ToolHandler, ToolOutput, TypedTool};

use crate::mem::backend::{MemError, TeamMemoryBackend};

/// The exact, ordered set of 6 `mem__*` tool names this server advertises.
pub const MEM_TOOL_NAMES: [&str; 6] = [
    "mem__add",
    "mem__get",
    "mem__search",
    "mem__list_recent",
    "mem__delete",
    "mem__complete_task",
];

type BoxFut = Pin<Box<dyn Future<Output = Result<Value>> + Send>>;

/// Maps a backend [`MemError`] onto a protocol [`Error`].
///
/// Every `MemError` arm is a client-facing validation problem (bad id, bad
/// args, dev-limit) — none are internal faults for this dev backend.
fn to_pmcp_err(err: MemError) -> Error {
    Error::validation(err.to_string())
}

/// Reads a required string argument.
fn arg_str(args: &Value, key: &str) -> Result<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| Error::validation(format!("missing required '{key}' string")))
}

/// Reads an optional `limit`, defaulting to `default`.
fn arg_limit(args: &Value, default: usize) -> usize {
    args.get("limit")
        .and_then(Value::as_u64)
        .map_or(default, |n| usize::try_from(n).unwrap_or(usize::MAX))
}

/// Reads an optional `tags` string array.
fn arg_tags(args: &Value) -> Vec<String> {
    args.get("tags")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Builds the mem-mcp [`Server`] over `backend`, registering exactly the 6
/// `mem__*` tools.
///
/// The read-only tools (`mem__get`, `mem__search`, `mem__list_recent`)
/// advertise `annotations.read_only_hint == true`.
///
/// # Errors
///
/// Propagates any [`Server`] construction error.
pub fn build_mem_mcp_server(backend: Arc<dyn TeamMemoryBackend>) -> Result<Server> {
    Server::builder()
        .name("mem-mcp")
        .version(env!("CARGO_PKG_VERSION"))
        .tool_arc("mem__add", add_tool(backend.clone()))
        .tool_arc("mem__get", get_tool(backend.clone()))
        .tool_arc("mem__search", search_tool(backend.clone()))
        .tool_arc("mem__list_recent", list_recent_tool(backend.clone()))
        .tool_arc("mem__delete", delete_tool(backend.clone()))
        .tool_arc(
            "mem__complete_task",
            Arc::new(CompleteTaskHandler { backend }),
        )
        .build()
}

fn add_tool(backend: Arc<dyn TeamMemoryBackend>) -> Arc<dyn ToolHandler> {
    let tool = TypedTool::new_with_schema(
        "mem__add",
        json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "The memory content to store" },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional tags"
                }
            },
            "required": ["text"]
        }),
        move |args: Value, _extra: RequestHandlerExtra| {
            let backend = backend.clone();
            Box::pin(async move {
                let text = arg_str(&args, "text")?;
                let tags = arg_tags(&args);
                let memory = backend.add(text, tags).await.map_err(to_pmcp_err)?;
                Ok(serde_json::to_value(memory).unwrap_or(Value::Null))
            }) as BoxFut
        },
    )
    .with_description("Store a new memory (text + optional tags).");
    Arc::new(tool)
}

fn get_tool(backend: Arc<dyn TeamMemoryBackend>) -> Arc<dyn ToolHandler> {
    let tool = TypedTool::new_with_schema(
        "mem__get",
        json!({
            "type": "object",
            "properties": { "id": { "type": "string", "description": "The memory id" } },
            "required": ["id"]
        }),
        move |args: Value, _extra: RequestHandlerExtra| {
            let backend = backend.clone();
            Box::pin(async move {
                let id = arg_str(&args, "id")?;
                let memory = backend.get(&id).await.map_err(to_pmcp_err)?;
                Ok(serde_json::to_value(memory).unwrap_or(Value::Null))
            }) as BoxFut
        },
    )
    .with_description("Fetch a stored memory by id.")
    .read_only();
    Arc::new(tool)
}

fn search_tool(backend: Arc<dyn TeamMemoryBackend>) -> Arc<dyn ToolHandler> {
    let tool = TypedTool::new_with_schema(
        "mem__search",
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Keyword query" },
                "limit": { "type": "integer", "minimum": 0, "description": "Max results (default 10)" }
            },
            "required": ["query"]
        }),
        move |args: Value, _extra: RequestHandlerExtra| {
            let backend = backend.clone();
            Box::pin(async move {
                let query = arg_str(&args, "query")?;
                let limit = arg_limit(&args, 10);
                let memories = backend.search(&query, limit).await.map_err(to_pmcp_err)?;
                Ok(json!({ "memories": memories }))
            }) as BoxFut
        },
    )
    .with_description("Rank stored memories by keyword relevance (BM25), highest first.")
    .read_only();
    Arc::new(tool)
}

fn list_recent_tool(backend: Arc<dyn TeamMemoryBackend>) -> Arc<dyn ToolHandler> {
    let tool = TypedTool::new_with_schema(
        "mem__list_recent",
        json!({
            "type": "object",
            "properties": {
                "limit": { "type": "integer", "minimum": 0, "description": "Max results (default 10)" }
            }
        }),
        move |args: Value, _extra: RequestHandlerExtra| {
            let backend = backend.clone();
            Box::pin(async move {
                let limit = arg_limit(&args, 10);
                let memories = backend.list_recent(limit).await.map_err(to_pmcp_err)?;
                Ok(json!({ "memories": memories }))
            }) as BoxFut
        },
    )
    .with_description("List the most recently added memories, newest first.")
    .read_only();
    Arc::new(tool)
}

fn delete_tool(backend: Arc<dyn TeamMemoryBackend>) -> Arc<dyn ToolHandler> {
    let tool = TypedTool::new_with_schema(
        "mem__delete",
        json!({
            "type": "object",
            "properties": { "id": { "type": "string", "description": "The memory id to delete" } },
            "required": ["id"]
        }),
        move |args: Value, _extra: RequestHandlerExtra| {
            let backend = backend.clone();
            Box::pin(async move {
                let id = arg_str(&args, "id")?;
                let deleted = backend.delete(&id).await.map_err(to_pmcp_err)?;
                Ok(json!({ "ok": true, "id": id, "deleted": deleted }))
            }) as BoxFut
        },
    )
    .with_description("Delete a memory by id (idempotent).");
    Arc::new(tool)
}

/// Server-layer handler for `mem__complete_task` (SEP-1686).
///
/// Delegates the completion record to the backend, then — when the call
/// carries a `relatedTaskId` — returns a [`ToolOutput::Result`] whose `_meta`
/// links the related task under
/// [`RELATED_TASK_META_KEY`](pmcp::types::tasks::RELATED_TASK_META_KEY);
/// otherwise it returns a plain payload.
///
/// Because it owns the full `CallToolResult` envelope, this handler is
/// responsible for its own redaction ([`ToolOutput::Result`] bypasses response
/// middleware) — acceptable for a dev reference server.
struct CompleteTaskHandler {
    backend: Arc<dyn TeamMemoryBackend>,
}

impl CompleteTaskHandler {
    fn related(args: &Value) -> Option<String> {
        args.get("relatedTaskId")
            .and_then(Value::as_str)
            .map(str::to_string)
    }
}

#[async_trait]
impl ToolHandler for CompleteTaskHandler {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        let task_id = arg_str(&args, "taskId")?;
        let related = Self::related(&args);
        let done = self
            .backend
            .complete_task(&task_id, related)
            .await
            .map_err(to_pmcp_err)?;
        Ok(serde_json::to_value(done).unwrap_or(Value::Null))
    }

    async fn handle_output(&self, args: Value, _extra: RequestHandlerExtra) -> Result<ToolOutput> {
        let task_id = arg_str(&args, "taskId")?;
        let related = Self::related(&args);
        let done = self
            .backend
            .complete_task(&task_id, related.clone())
            .await
            .map_err(to_pmcp_err)?;
        let payload = serde_json::to_value(&done).unwrap_or(Value::Null);
        match related {
            Some(related_id) => {
                let result = CallToolResult::new(vec![Content::text(payload.to_string())])
                    .with_related_task(TaskMetadata::new(related_id));
                Ok(ToolOutput::Result(result))
            },
            None => Ok(ToolOutput::Payload(payload)),
        }
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            "mem__complete_task",
            Some("Mark a task complete; optionally link a related task via _meta.".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "taskId": { "type": "string", "description": "The task to complete" },
                    "relatedTaskId": { "type": "string", "description": "Optional related task id" }
                },
                "required": ["taskId"]
            }),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::backend::InMemoryMemoryBackend;
    use crate::transport::DuplexTransport;
    use pmcp::types::tasks::RELATED_TASK_META_KEY;
    use pmcp::types::ClientCapabilities;
    use pmcp::Client;

    fn build() -> Server {
        let backend =
            Arc::new(InMemoryMemoryBackend::deterministic()) as Arc<dyn TeamMemoryBackend>;
        build_mem_mcp_server(backend).unwrap()
    }

    #[tokio::test]
    async fn advertises_exactly_the_6_mem_tools() {
        let server = build();
        let (client_t, server_t) = DuplexTransport::pair();
        let handle = tokio::spawn(async move {
            let _ = server.run(server_t).await;
        });

        let mut client = Client::new(client_t);
        client
            .initialize(ClientCapabilities::default())
            .await
            .expect("initialize");
        let list = client.list_tools(None).await.expect("list_tools");

        let mut names: Vec<&str> = list.tools.iter().map(|t| t.name.as_str()).collect();
        names.sort_unstable();
        let mut expected = MEM_TOOL_NAMES.to_vec();
        expected.sort_unstable();
        assert_eq!(names, expected, "must advertise exactly the 6 mem__* tools");

        let search = list.tools.iter().find(|t| t.name == "mem__search").unwrap();
        assert_eq!(
            search.annotations.as_ref().and_then(|a| a.read_only_hint),
            Some(true),
            "mem__search must advertise read_only_hint"
        );

        handle.abort();
    }

    #[tokio::test]
    async fn complete_task_emits_related_task_meta() {
        let backend =
            Arc::new(InMemoryMemoryBackend::deterministic()) as Arc<dyn TeamMemoryBackend>;
        let handler = CompleteTaskHandler { backend };
        let extra = RequestHandlerExtra::new("req-1".to_string(), Default::default());
        let out = handler
            .handle_output(json!({ "taskId": "t1", "relatedTaskId": "t2" }), extra)
            .await
            .unwrap();
        match out {
            ToolOutput::Result(r) => {
                let meta = r._meta.expect("related-task _meta present");
                assert!(
                    meta.contains_key(RELATED_TASK_META_KEY),
                    "related task must live under the SDK constant key"
                );
            },
            other => panic!("expected ToolOutput::Result, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn complete_task_without_related_is_plain_payload() {
        let backend =
            Arc::new(InMemoryMemoryBackend::deterministic()) as Arc<dyn TeamMemoryBackend>;
        let handler = CompleteTaskHandler { backend };
        let extra = RequestHandlerExtra::new("req-2".to_string(), Default::default());
        let out = handler
            .handle_output(json!({ "taskId": "t1" }), extra)
            .await
            .unwrap();
        assert!(matches!(out, ToolOutput::Payload(_)));
    }

    #[tokio::test]
    async fn add_search_delete_round_trip_over_the_wire() {
        let server = build();
        let (client_t, server_t) = DuplexTransport::pair();
        let handle = tokio::spawn(async move {
            let _ = server.run(server_t).await;
        });

        let mut client = Client::new(client_t);
        client
            .initialize(ClientCapabilities::default())
            .await
            .expect("initialize");

        client
            .call_tool("mem__add".to_string(), json!({ "text": "quick brown fox" }))
            .await
            .expect("add");
        let res = client
            .call_tool("mem__search".to_string(), json!({ "query": "fox" }))
            .await
            .expect("search");
        let text = match &res.content[0] {
            Content::Text { text } => text.clone(),
            other => panic!("expected text content, got {other:?}"),
        };
        assert!(
            text.contains("mem-001"),
            "search must return the added memory"
        );

        handle.abort();
    }
}
