//! # Server example: HTTP tool-as-task lifecycle (Phase 102)
//!
//! This is the RECOMMENDED pattern for exposing a tool as an async MCP Task over
//! **real HTTP**: register a `with_task_support(TaskSupport::Required)` tool plus a
//! `TaskStore` on the HIGH-LEVEL [`Server`](pmcp::Server) builder and serve it over
//! [`StreamableHttpServer`](pmcp::server::streamable_http_server::StreamableHttpServer).
//! The SDK serves `tasks/*` typed from the store — you never hand-write `tasks/*`
//! wire JSON, and the store mints the task id.
//!
//! This is a **pmcp.run-shaped** task server: it serves `tasks/*` through the
//! high-level `Server` + `StreamableHttpServer` with **NO `ServerCore::handle_request`
//! shim**. The server is built only via `Server::builder()`. Phase 101's example
//! (`s45_tool_as_task_lifecycle`) had to pair a `pmcp::Client` with a `ServerCore`
//! over an in-process duplex transport because the HTTP path rejected `tasks/*`;
//! Phase 102 wired the shared task-dispatch unit into the `Server`, so this example
//! drives the lifecycle over a genuine HTTP loopback instead.
//!
//! It drives the all-typed MCP Tasks path end-to-end through a LIVE HTTP client:
//!
//! ```text
//! initialize -> call(tool with task) -> tasks/get (poll) -> tasks/result
//! ```
//!
//! and proves all four original wire-shape bugs from the tools-as-tasks incident
//! are impossible on the HTTP SDK path:
//!
//! 1. **id consistency** — the `CreateTaskResult.task.taskId` returned to the client
//!    is the store-minted id, and `tasks/get` polls that same id (not a 404 on a
//!    tool-fabricated id).
//! 2. **advertised capability** — `initialize` shows the server auto-advertised the
//!    `tasks` capability (endpoint-backed).
//! 3. **typed `tasks/get`** — the polled status deserializes into the typed `Task`.
//! 4. **typed `tasks/result`** — the terminal result deserializes into a typed,
//!    non-empty `CallToolResult`.
//!
//! Test-reliability practices (carried from the phase's HTTP test):
//! - EPHEMERAL PORT — binds `127.0.0.1:0` and uses the bound address read back from
//!   `StreamableHttpServer::start()` (no hardcoded port).
//! - READINESS — `start()` binds the listener before returning, so the address is
//!   already accepting connections (no fixed sleep).
//! - SHUTDOWN — the spawned server `JoinHandle` is `abort()`ed after the round-trip,
//!   so the example process exits cleanly and cannot hang.
//!
//! Every lifecycle step is a HARD assertion (returns `Err` on failure), not merely
//! printed output.
//!
//! Run with: `cargo run --example s46_http_tool_as_task --features full`

#![cfg(not(target_arch = "wasm32"))]

use std::net::SocketAddr;
use std::sync::Arc;

use pmcp::server::streamable_http_server::StreamableHttpServer;
use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
use pmcp::server::typed_tool::TypedTool;
use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
use pmcp::types::{ClientCapabilities, TaskSupport, ToolExecution};
use pmcp::{Client, Error, Server, ToolCallResponse};
use tokio::sync::Mutex;
use url::Url;

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    // A `with_task_support` tool that completes synchronously: it returns a
    // Task-shaped value carrying a nested `result` (the terminal CallToolResult),
    // so the shared create path records create + set_result + Completed before
    // returning. `tasks/get` then sees Completed and `tasks/result` serves the
    // non-empty content.
    let task_tool = TypedTool::new_with_schema(
        "summarize",
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
                        "content": [
                            { "type": "text", "text": "summary: 3 items processed" }
                        ]
                    }
                }))
            })
        },
    )
    .with_description("Summarize asynchronously as an MCP Task")
    .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required));

    // Build the HIGH-LEVEL Server via Server::builder() ONLY — no ServerCore shim.
    let store = Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>;
    let server = Server::builder()
        .name("http-tool-as-task-example")
        .version("1.0.0")
        .tool("summarize", task_tool)
        .task_store(store) // presence of a store auto-advertises the `tasks` capability
        .build()?;
    let server = Arc::new(Mutex::new(server));

    // EPHEMERAL PORT: bind 127.0.0.1:0 and read back the OS-assigned address from
    // start(). start() binds the listener before returning, so `bound` is already
    // accepting connections — a deterministic readiness signal, no sleep.
    let bind_addr: SocketAddr = "127.0.0.1:0".parse().expect("valid loopback addr");
    let http_server = StreamableHttpServer::new(bind_addr, server);
    let (bound, server_handle) = http_server.start().await?;
    println!("HTTP task server listening on http://{bound}");

    // Run the round-trip, then ALWAYS abort the server task so the process exits.
    let result = run_lifecycle(bound).await;

    // SHUTDOWN: cancel the spawned server task — no lingering listener, no hang.
    server_handle.abort();
    println!("server task aborted (clean shutdown)");

    result
}

/// Drive the four-step lifecycle over a live HTTP client against `bound`.
async fn run_lifecycle(bound: SocketAddr) -> pmcp::Result<()> {
    let config = StreamableHttpTransportConfig {
        url: Url::parse(&format!("http://{bound}")).map_err(|e| Error::Internal(e.to_string()))?,
        extra_headers: vec![],
        auth_provider: None,
        session_id: None,
        enable_json_response: true,
        on_resumption_token: None,
        http_middleware_chain: None,
    };
    let mut client = Client::new(StreamableHttpTransport::new(config));

    // 1) initialize — the server auto-advertised the `tasks` capability.
    let init = client.initialize(ClientCapabilities::default()).await?;
    println!(
        "1) initialize: tasks capability advertised = {}",
        init.capabilities.tasks.is_some()
    );
    if init.capabilities.tasks.is_none() {
        return Err(Error::internal(
            "server must auto-advertise the tasks capability over HTTP",
        ));
    }

    // 2) call(tool with task) -> CreateTaskResult with the store-minted id.
    let task_id = match client
        .call_tool_with_task("summarize".to_string(), serde_json::json!({}))
        .await?
    {
        ToolCallResponse::Task(task) => {
            println!("2) created task (store-minted id): {}", task.task_id);
            task.task_id
        },
        ToolCallResponse::Result(_) => {
            return Err(Error::internal(
                "expected a created task, got a sync result",
            ))
        },
    };
    if task_id == "tool-fabricated" {
        return Err(Error::internal(
            "wire id must be the store-minted id, not the tool-fabricated one",
        ));
    }

    // 3) poll tasks/get until terminal — typed Task each iteration.
    let mut task = client.tasks_get(&task_id).await?;
    let mut polls = 0;
    while !task.status.is_terminal() && polls < 50 {
        println!("3) poll {polls}: status = {}", task.status);
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        task = client.tasks_get(&task_id).await?;
        polls += 1;
    }
    println!(
        "3) terminal status = {} (polled id = {})",
        task.status, task.task_id
    );
    if task.task_id != task_id {
        return Err(Error::internal("polled id must equal the store-minted id"));
    }
    if !task.status.is_terminal() {
        return Err(Error::internal("task must reach a terminal status"));
    }

    // 4) tasks/result -> typed, non-empty CallToolResult.
    let result = client.tasks_result(&task_id).await?;
    if result.content.is_empty() {
        return Err(Error::internal(
            "tasks/result must carry the persisted terminal content",
        ));
    }
    println!("4) tasks/result content items: {}", result.content.len());
    println!(
        "HTTP tool-as-task lifecycle OK — all four wire shapes verified through a live HTTP client"
    );

    Ok(())
}
