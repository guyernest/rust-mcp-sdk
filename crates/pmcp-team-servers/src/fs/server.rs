//! Builds the team-fs `pmcp::Server` advertising the 11 `fs__*` tools over a
//! [`crate::fs::backend::TeamFsBackend`] implementation.
//!
//! The fixed 11-tool surface is the `fs_tool_surface` equation of
//! `contracts/team-servers-v1.yaml`. Ten tools map 1:1 to
//! [`TeamFsBackend`](crate::fs::backend::TeamFsBackend) *storage* methods; the
//! eleventh, `fs__complete_task`, is owned by THIS server layer (task
//! completion is MCP protocol behavior, not storage — 109-02 review) and, when
//! carrying a related task, emits it under the SDK constant
//! [`RELATED_TASK_META_KEY`](pmcp::types::tasks::RELATED_TASK_META_KEY) via
//! [`ToolOutput::Result`].
//!
//! Because the set is fixed, unknown `fs__*` names are simply never advertised;
//! pmcp then returns a `Tool 'name' not found` error — it never panics.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use pmcp::types::tasks::TaskMetadata;
use pmcp::types::{CallToolResult, Content, ToolInfo};
use pmcp::{Error, RequestHandlerExtra, Result, Server, ToolHandler, ToolOutput, TypedTool};

use crate::fs::backend::{FsError, TeamFsBackend};

/// The exact, ordered set of 11 `fs__*` tool names this server advertises.
pub const FS_TOOL_NAMES: [&str; 11] = [
    "fs__list",
    "fs__read",
    "fs__write",
    "fs__append_file",
    "fs__head",
    "fs__stat",
    "fs__create_directory",
    "fs__get_download_url",
    "fs__sync_to_review",
    "fs__sync_from_review",
    "fs__complete_task",
];

type BoxFut = Pin<Box<dyn Future<Output = Result<Value>> + Send>>;

/// Maps a backend [`FsError`] onto a protocol [`Error`].
fn to_pmcp_err(e: FsError) -> Error {
    match e {
        FsError::Io(_) => Error::internal(e.to_string()),
        other => Error::validation(other.to_string()),
    }
}

/// Reads a required string argument.
fn arg_str(args: &Value, key: &str) -> Result<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| Error::validation(format!("missing required '{key}' string")))
}

/// A JSON schema for a single required `path` string.
fn path_schema() -> Value {
    json!({
        "type": "object",
        "properties": { "path": { "type": "string", "description": "Path relative to the workspace root" } },
        "required": ["path"]
    })
}

/// Builds the team-fs [`Server`] over `backend`, registering exactly the 11
/// `fs__*` tools.
///
/// `fs__list` advertises `annotations.read_only_hint == true`. The ten storage
/// tools dispatch to the matching [`TeamFsBackend`](crate::fs::backend::TeamFsBackend)
/// method; `fs__complete_task` is handled in this layer.
///
/// # Errors
///
/// Propagates any [`Server`] construction error.
pub fn build_team_fs_server(backend: Arc<dyn TeamFsBackend>) -> Result<Server> {
    Server::builder()
        .name("team-fs")
        .version(env!("CARGO_PKG_VERSION"))
        .tool_arc("fs__list", list_tool(backend.clone()))
        .tool_arc("fs__read", read_tool(backend.clone()))
        .tool_arc("fs__write", write_tool(backend.clone()))
        .tool_arc("fs__append_file", append_tool(backend.clone()))
        .tool_arc("fs__head", head_tool(backend.clone()))
        .tool_arc("fs__stat", stat_tool(backend.clone()))
        .tool_arc("fs__create_directory", mkdir_tool(backend.clone()))
        .tool_arc("fs__get_download_url", download_url_tool(backend.clone()))
        .tool_arc("fs__sync_to_review", sync_to_tool(backend.clone()))
        .tool_arc("fs__sync_from_review", sync_from_tool(backend))
        .tool_arc("fs__complete_task", Arc::new(CompleteTaskHandler))
        .build()
}

fn list_tool(backend: Arc<dyn TeamFsBackend>) -> Arc<dyn ToolHandler> {
    let tool = TypedTool::new_with_schema(
        "fs__list",
        json!({
            "type": "object",
            "properties": { "path": { "type": "string", "description": "Directory (default: root)" } }
        }),
        move |args: Value, _extra: RequestHandlerExtra| {
            let backend = backend.clone();
            Box::pin(async move {
                let path = args.get("path").and_then(Value::as_str).unwrap_or("");
                let entries = backend.list(path).await.map_err(to_pmcp_err)?;
                Ok(json!({ "entries": entries }))
            }) as BoxFut
        },
    )
    .with_description("List the entries of a directory relative to the workspace root.")
    .read_only();
    Arc::new(tool)
}

fn read_tool(backend: Arc<dyn TeamFsBackend>) -> Arc<dyn ToolHandler> {
    let tool = TypedTool::new_with_schema(
        "fs__read",
        path_schema(),
        move |args: Value, _extra: RequestHandlerExtra| {
            let backend = backend.clone();
            Box::pin(async move {
                let path = arg_str(&args, "path")?;
                let bytes = backend.read(&path).await.map_err(to_pmcp_err)?;
                Ok(json!({
                    "content": String::from_utf8_lossy(&bytes),
                    "size": bytes.len(),
                }))
            }) as BoxFut
        },
    )
    .with_description("Read a file's full contents (relative to the workspace root).")
    .read_only();
    Arc::new(tool)
}

fn write_tool(backend: Arc<dyn TeamFsBackend>) -> Arc<dyn ToolHandler> {
    let tool = TypedTool::new_with_schema(
        "fs__write",
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "content": { "type": "string", "description": "UTF-8 text content to write" }
            },
            "required": ["path", "content"]
        }),
        move |args: Value, _extra: RequestHandlerExtra| {
            let backend = backend.clone();
            Box::pin(async move {
                let path = arg_str(&args, "path")?;
                let content = arg_str(&args, "content")?;
                backend
                    .write(&path, content.as_bytes())
                    .await
                    .map_err(to_pmcp_err)?;
                Ok(json!({ "ok": true, "path": path }))
            }) as BoxFut
        },
    )
    .with_description("Write UTF-8 text to a file, creating missing parent directories.");
    Arc::new(tool)
}

fn append_tool(backend: Arc<dyn TeamFsBackend>) -> Arc<dyn ToolHandler> {
    let tool = TypedTool::new_with_schema(
        "fs__append_file",
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "content": { "type": "string", "description": "UTF-8 text to append" }
            },
            "required": ["path", "content"]
        }),
        move |args: Value, _extra: RequestHandlerExtra| {
            let backend = backend.clone();
            Box::pin(async move {
                let path = arg_str(&args, "path")?;
                let content = arg_str(&args, "content")?;
                backend
                    .append_file(&path, content.as_bytes())
                    .await
                    .map_err(to_pmcp_err)?;
                Ok(json!({ "ok": true, "path": path }))
            }) as BoxFut
        },
    )
    .with_description("Append UTF-8 text to a file, creating it if absent.");
    Arc::new(tool)
}

fn head_tool(backend: Arc<dyn TeamFsBackend>) -> Arc<dyn ToolHandler> {
    let tool = TypedTool::new_with_schema(
        "fs__head",
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "maxBytes": { "type": "integer", "minimum": 0, "description": "Byte cap (default 4096)" }
            },
            "required": ["path"]
        }),
        move |args: Value, _extra: RequestHandlerExtra| {
            let backend = backend.clone();
            Box::pin(async move {
                let path = arg_str(&args, "path")?;
                let max_bytes = args
                    .get("maxBytes")
                    .and_then(Value::as_u64)
                    .map_or(4096usize, |n| usize::try_from(n).unwrap_or(usize::MAX));
                let bytes = backend.head(&path, max_bytes).await.map_err(to_pmcp_err)?;
                Ok(json!({
                    "content": String::from_utf8_lossy(&bytes),
                    "bytes": bytes.len(),
                }))
            }) as BoxFut
        },
    )
    .with_description("Read at most maxBytes from the start of a file (default 4096).")
    .read_only();
    Arc::new(tool)
}

fn stat_tool(backend: Arc<dyn TeamFsBackend>) -> Arc<dyn ToolHandler> {
    let tool = TypedTool::new_with_schema(
        "fs__stat",
        path_schema(),
        move |args: Value, _extra: RequestHandlerExtra| {
            let backend = backend.clone();
            Box::pin(async move {
                let path = arg_str(&args, "path")?;
                let stat = backend.stat(&path).await.map_err(to_pmcp_err)?;
                Ok(serde_json::to_value(stat).unwrap_or(Value::Null))
            }) as BoxFut
        },
    )
    .with_description("Return metadata (path, is_dir, size) for a path.")
    .read_only();
    Arc::new(tool)
}

fn mkdir_tool(backend: Arc<dyn TeamFsBackend>) -> Arc<dyn ToolHandler> {
    let tool = TypedTool::new_with_schema(
        "fs__create_directory",
        path_schema(),
        move |args: Value, _extra: RequestHandlerExtra| {
            let backend = backend.clone();
            Box::pin(async move {
                let path = arg_str(&args, "path")?;
                backend.create_directory(&path).await.map_err(to_pmcp_err)?;
                Ok(json!({ "ok": true, "path": path }))
            }) as BoxFut
        },
    )
    .with_description("Create a directory (and missing parents) under the workspace root.");
    Arc::new(tool)
}

fn download_url_tool(backend: Arc<dyn TeamFsBackend>) -> Arc<dyn ToolHandler> {
    let tool = TypedTool::new_with_schema(
        "fs__get_download_url",
        path_schema(),
        move |args: Value, _extra: RequestHandlerExtra| {
            let backend = backend.clone();
            Box::pin(async move {
                let path = arg_str(&args, "path")?;
                let url = backend.get_download_url(&path).await.map_err(to_pmcp_err)?;
                Ok(json!({ "url": url }))
            }) as BoxFut
        },
    )
    .with_description("Return a percent-encoded file:// download URL for a file.")
    .read_only();
    Arc::new(tool)
}

fn sync_to_tool(backend: Arc<dyn TeamFsBackend>) -> Arc<dyn ToolHandler> {
    let tool = TypedTool::new_with_schema(
        "fs__sync_to_review",
        path_schema(),
        move |args: Value, _extra: RequestHandlerExtra| {
            let backend = backend.clone();
            Box::pin(async move {
                let path = arg_str(&args, "path")?;
                backend.sync_to_review(&path).await.map_err(to_pmcp_err)?;
                Ok(json!({ "ok": true, "path": path }))
            }) as BoxFut
        },
    )
    .with_description(
        "Copy workspace/<path> into the sibling review/ tree (recursive, overwrite).",
    );
    Arc::new(tool)
}

fn sync_from_tool(backend: Arc<dyn TeamFsBackend>) -> Arc<dyn ToolHandler> {
    let tool = TypedTool::new_with_schema(
        "fs__sync_from_review",
        path_schema(),
        move |args: Value, _extra: RequestHandlerExtra| {
            let backend = backend.clone();
            Box::pin(async move {
                let path = arg_str(&args, "path")?;
                backend.sync_from_review(&path).await.map_err(to_pmcp_err)?;
                Ok(json!({ "ok": true, "path": path }))
            }) as BoxFut
        },
    )
    .with_description("Copy review/<path> back into the workspace (recursive, overwrite).");
    Arc::new(tool)
}

/// Server-layer handler for `fs__complete_task`.
///
/// Marks the referenced task complete. When the call carries a `relatedTaskId`,
/// it returns a [`ToolOutput::Result`] whose `_meta` carries the related task
/// under [`RELATED_TASK_META_KEY`](pmcp::types::tasks::RELATED_TASK_META_KEY);
/// otherwise it returns a plain payload. This is deliberately NOT a
/// [`TeamFsBackend`](crate::fs::backend::TeamFsBackend) method — task
/// completion is protocol behavior, not storage.
///
/// Because it owns the full `CallToolResult` envelope, this handler is
/// responsible for its own redaction ([`ToolOutput::Result`] bypasses response
/// middleware) — acceptable for a dev reference server.
#[derive(Debug)]
struct CompleteTaskHandler;

impl CompleteTaskHandler {
    fn task_id(args: &Value) -> Result<String> {
        arg_str(args, "taskId")
    }

    fn payload(task_id: &str) -> Value {
        json!({ "ok": true, "taskId": task_id, "status": "completed" })
    }
}

#[async_trait]
impl ToolHandler for CompleteTaskHandler {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        let task_id = Self::task_id(&args)?;
        Ok(Self::payload(&task_id))
    }

    async fn handle_output(&self, args: Value, _extra: RequestHandlerExtra) -> Result<ToolOutput> {
        let task_id = Self::task_id(&args)?;
        let payload = Self::payload(&task_id);
        match args.get("relatedTaskId").and_then(Value::as_str) {
            Some(related) => {
                let result = CallToolResult::new(vec![Content::text(payload.to_string())])
                    .with_related_task(TaskMetadata::new(related));
                Ok(ToolOutput::Result(result))
            },
            None => Ok(ToolOutput::Payload(payload)),
        }
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            "fs__complete_task",
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
    use crate::fs::local::LocalDirBackend;
    use crate::transport::DuplexTransport;
    use pmcp::types::tasks::RELATED_TASK_META_KEY;
    use pmcp::types::ClientCapabilities;
    use pmcp::Client;
    use tempfile::TempDir;

    fn build(tmp: &TempDir) -> Server {
        let backend = Arc::new(LocalDirBackend::new(tmp.path()).unwrap()) as Arc<dyn TeamFsBackend>;
        build_team_fs_server(backend).unwrap()
    }

    #[tokio::test]
    async fn advertises_exactly_the_11_fs_tools() {
        let tmp = TempDir::new().unwrap();
        let server = build(&tmp);

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
        let mut expected = FS_TOOL_NAMES.to_vec();
        expected.sort_unstable();
        assert_eq!(names, expected, "must advertise exactly the 11 fs__* tools");

        // fs__list carries read_only_hint == true.
        let list_tool = list.tools.iter().find(|t| t.name == "fs__list").unwrap();
        assert_eq!(
            list_tool
                .annotations
                .as_ref()
                .and_then(|a| a.read_only_hint),
            Some(true),
            "fs__list must advertise read_only_hint"
        );

        handle.abort();
    }

    #[tokio::test]
    async fn complete_task_emits_related_task_meta() {
        let handler = CompleteTaskHandler;
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
        let handler = CompleteTaskHandler;
        let extra = RequestHandlerExtra::new("req-2".to_string(), Default::default());
        let out = handler
            .handle_output(json!({ "taskId": "t1" }), extra)
            .await
            .unwrap();
        assert!(matches!(out, ToolOutput::Payload(_)));
    }
}
