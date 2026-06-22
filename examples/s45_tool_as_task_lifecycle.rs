//! # Server example: Tool-as-Task lifecycle (Phase 101)
//!
//! This is the RECOMMENDED pattern for exposing a tool as an async MCP Task:
//! register a `with_task_support(TaskSupport::Required)` tool plus a `TaskStore`
//! on `ServerCoreBuilder` and let the SDK serve `tasks/*` typed — no hand-written
//! `tasks/*` wire JSON.
//!
//! Demonstrates the all-typed MCP Tasks path end-to-end through a LIVE
//! in-process client round-trip. It registers a `with_task_support` tool plus
//! an `InMemoryTaskStore`, then drives the real `pmcp::Client` through:
//!
//! ```text
//! initialize -> call(tool with task) -> tasks/get (poll) -> tasks/result
//! ```
//!
//! It proves all four original wire-shape bugs from the tools-as-tasks incident
//! are impossible on the SDK path:
//!
//! 1. **id consistency** — the `CreateTaskResult.task.taskId` returned to the
//!    client is the store-minted id, and `tasks/get` polls that same id (not a
//!    404 on a tool-fabricated id).
//! 2. **advertised capability** — `initialize` shows the server auto-advertised
//!    the `tasks` capability (endpoint-backed).
//! 3. **typed `tasks/get`** — the polled status deserializes into the typed
//!    `Task` via the client.
//! 4. **typed `tasks/result`** — the terminal result deserializes into a typed,
//!    non-empty `CallToolResult`.
//!
//! The high-level `pmcp::Server` (and `StreamableHttpServer`) does not carry a
//! `TaskStore`; the task path lives on `ServerCore`. So this example pairs a
//! real `pmcp::Client` with a `ServerCore` over an in-process duplex transport
//! — the equivalent of an HTTP loopback for the task-bearing dispatch path.
//!
//! Run with: `cargo run --example s45_tool_as_task_lifecycle --features full`

#![cfg(not(target_arch = "wasm32"))]

use std::sync::Arc;

use async_trait::async_trait;
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::server::core::ProtocolHandler;
use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
use pmcp::server::typed_tool::TypedTool;
use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::{ClientCapabilities, TaskSupport, ToolExecution};
use pmcp::{Client, Error, ToolCallResponse};
use tokio::sync::mpsc;

/// One half of an in-process duplex transport (client <-> server).
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

/// Serve `handler` over the server side of a duplex transport until the client
/// half is dropped.
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

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    // A `with_task_support` tool that completes synchronously: it returns a
    // Task-shaped value carrying a nested `result` (the terminal CallToolResult),
    // so plan 01's create path records create + set_result + Completed before
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

    let store = Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>;
    let server: Arc<dyn ProtocolHandler> = Arc::new(
        ServerCoreBuilder::new()
            .name("tool-as-task-example")
            .version("1.0.0")
            .tool("summarize", task_tool)
            .task_store(store)
            .build()?,
    );

    let (client_transport, server_transport) = DuplexTransport::pair();
    spawn_server_pump(server_transport, server);
    let mut client = Client::new(client_transport);

    // 1) initialize — the server auto-advertised the `tasks` capability.
    let init = client.initialize(ClientCapabilities::default()).await?;
    println!(
        "initialize: tasks capability advertised = {}",
        init.capabilities.tasks.is_some()
    );
    assert!(
        init.capabilities.tasks.is_some(),
        "server must auto-advertise the tasks capability"
    );

    // 2) call(tool with task) -> CreateTaskResult with the store-minted id.
    let task_id = match client
        .call_tool_with_task("summarize".to_string(), serde_json::json!({}))
        .await?
    {
        ToolCallResponse::Task(task) => {
            println!("created task (store-minted id): {}", task.task_id);
            task.task_id
        },
        ToolCallResponse::Result(_) => {
            return Err(Error::internal(
                "expected a created task, got a sync result",
            ))
        },
    };

    // 3) poll tasks/get until terminal — typed Task each iteration.
    let mut task = client.tasks_get(&task_id).await?;
    let mut polls = 0;
    while !task.status.is_terminal() && polls < 50 {
        println!("  poll {polls}: status = {}", task.status);
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        task = client.tasks_get(&task_id).await?;
        polls += 1;
    }
    println!(
        "  terminal status = {} (polled id = {})",
        task.status, task.task_id
    );
    assert_eq!(
        task.task_id, task_id,
        "polled id must equal the store-minted id"
    );
    assert!(
        task.status.is_terminal(),
        "task must reach a terminal status"
    );

    // 4) tasks/result -> typed, non-empty CallToolResult.
    let result = client.tasks_result(&task_id).await?;
    assert!(
        !result.content.is_empty(),
        "tasks/result must carry the persisted terminal content"
    );
    println!("tasks/result content items: {}", result.content.len());
    println!("tool-as-task lifecycle OK — all four wire shapes verified through the live client");

    Ok(())
}
