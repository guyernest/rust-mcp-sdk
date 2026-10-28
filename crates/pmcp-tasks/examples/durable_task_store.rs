//! # A pmcp server whose tasks live in a `pmcp-tasks` backend
//!
//! `ServerTaskStore` puts this crate's stores behind the trait a pmcp server's
//! `task_store(...)` takes. Task state then lives wherever the backend keeps
//! it: the in-memory backend here, or `DynamoDbBackend` / `RedisBackend`, so
//! tasks survive a restart and every instance sees the same tasks.
//!
//! The server runs a long tool as a task. Its handler hands the work to a
//! background worker through `on_task_created`, and the worker reports progress
//! with `set_status_message` and finishes with a result. A client calls the
//! tool over real HTTP, polls the task, and reads the result. Then the example
//! reads the same task straight from the backend, which is what a second
//! instance would see.
//!
//! Every step is a hard assertion, not just printed output.
//!
//! Run with: `cargo run -p pmcp-tasks --example durable_task_store`

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use pmcp::server::streamable_http_server::StreamableHttpServer;
use pmcp::server::task_store::{CreatedTask, TaskStore};
use pmcp::server::typed_tool::TypedTool;
use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
use pmcp::types::tasks::TaskStatus;
use pmcp::types::{CallToolResult, ClientCapabilities, Content, TaskSupport, ToolExecution};
use pmcp::{Client, Error, RequestHandlerExtra, Server, ToolCallResponse};
use pmcp_tasks::{GenericTaskStore, InMemoryBackend, ServerTaskStore, TaskSecurityConfig};
use serde_json::{json, Value};
use url::Url;

const STEPS: u32 = 3;

/// The background worker, keyed by the id the backend minted.
async fn worker(store: Arc<dyn TaskStore>, created: CreatedTask) {
    let (id, owner) = (created.task.task_id, created.owner_id);
    for step in 1..=STEPS {
        tokio::time::sleep(Duration::from_millis(20)).await;
        let message = format!("step {step}/{STEPS}");
        if let Err(e) = store.set_status_message(&id, &owner, Some(message)).await {
            eprintln!("progress update failed: {e}");
        }
    }
    let result = CallToolResult::new(vec![Content::text("report ready")]);
    if let Err(e) = store.set_result(&id, &owner, result).await {
        eprintln!("storing the result failed: {e}");
        return;
    }
    if let Err(e) = store
        .update_status(&id, &owner, TaskStatus::Completed, None)
        .await
    {
        eprintln!("completing the task failed: {e}");
    }
}

fn report_tool(store: Arc<dyn TaskStore>) -> impl pmcp::ToolHandler {
    TypedTool::new_with_schema(
        "build_report",
        json!({ "type": "object" }),
        move |_args: Value, extra: RequestHandlerExtra| {
            let store = Arc::clone(&store);
            if extra.will_create_task() {
                extra.on_task_created(move |created| {
                    tokio::spawn(worker(store, created));
                });
                return Box::pin(async {
                    Ok(json!({ "taskId": "assigned-by-the-store", "status": "working" }))
                });
            }
            Box::pin(async { Ok(json!({ "report": "built inline" })) })
        },
    )
    .with_description("Build a report; long, so it runs as a task when the client can poll")
    .with_execution(ToolExecution::new().with_task_support(TaskSupport::Optional))
}

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    // In production, swap the backend:
    //   GenericTaskStore::new(DynamoDbBackend::new(client, "mcp-tasks".into()))
    // and configure an auth provider instead of allowing anonymous owners.
    let store = Arc::new(ServerTaskStore::new(
        GenericTaskStore::new(InMemoryBackend::new())
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true)),
    ));
    let server = Server::builder()
        .name("durable-task-store-example")
        .version("1.0.0")
        .tool(
            "build_report",
            report_tool(Arc::clone(&store) as Arc<dyn TaskStore>),
        )
        .task_store(Arc::clone(&store) as Arc<dyn TaskStore>)
        .build()?;

    let bind_addr: SocketAddr = "127.0.0.1:0".parse().expect("valid loopback addr");
    let (bound, server_handle) = StreamableHttpServer::new_concurrent(bind_addr, Arc::new(server))
        .start()
        .await?;
    println!("task server listening on http://{bound}");

    let result = run_client(bound, &store).await;
    server_handle.abort();
    result
}

async fn run_client(
    bound: SocketAddr,
    store: &ServerTaskStore<InMemoryBackend>,
) -> pmcp::Result<()> {
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
    client.initialize(ClientCapabilities::default()).await?;

    // 1) Ask for a task.
    let task_id = match client
        .call_tool_with_task("build_report".to_string(), json!({}))
        .await?
    {
        ToolCallResponse::Task(task) => task.task_id,
        ToolCallResponse::Result(_) => {
            return Err(Error::internal("expected a task, got a synchronous result"))
        },
    };
    println!("1) task created: {task_id}");

    // 2) Poll until the worker finishes, printing its progress.
    let mut task = client.tasks_get(&task_id).await?;
    let mut last_message = None;
    for _ in 0..200 {
        if task.status_message.is_some() && task.status_message != last_message {
            println!(
                "2) {} - {}",
                task.status,
                task.status_message.as_deref().unwrap_or("")
            );
            last_message.clone_from(&task.status_message);
        }
        if task.status.is_terminal() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
        task = client.tasks_get(&task_id).await?;
    }
    if task.status != TaskStatus::Completed || last_message.is_none() {
        return Err(Error::internal(format!(
            "expected a completed task with progress, got {} / {last_message:?}",
            task.status
        )));
    }

    // 3) Read the result.
    let result = client.tasks_result(&task_id).await?;
    println!("3) tasks/result: {:?}", result.content);
    if result.content.is_empty() {
        return Err(Error::internal(
            "tasks/result must carry the worker's result",
        ));
    }

    // 4) The task is in the backend, where another instance would read it.
    // Unauthenticated v1 requests are owned by "local".
    let record = store
        .inner()
        .get(&task_id, "local")
        .await
        .map_err(|e| Error::internal(e.to_string()))?;
    println!(
        "4) backend record: {} ({}), result stored: {}",
        record.task.task_id,
        record.task.status,
        record.result.is_some()
    );
    if record.result.is_none() {
        return Err(Error::internal("the backend must hold the result"));
    }

    println!("durable task store lifecycle OK");
    Ok(())
}
