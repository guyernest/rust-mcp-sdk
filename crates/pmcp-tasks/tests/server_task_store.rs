//! `ServerTaskStore`: this crate's stores behind pmcp's `task_store(...)` trait.
//!
//! - A differential property test drives random operation sequences through
//!   `ServerTaskStore` and pmcp's own `InMemoryTaskStore` and requires the same
//!   outcome from both at every step.
//! - Unit tests cover what the property test can't reach: expiry, TTL
//!   clamping, input delivery, errors, anonymous owners and the config pmcp
//!   reads.
//! - An HTTP test runs a full task lifecycle through a real `Server` built with
//!   `task_store(ServerTaskStore)`.

use std::sync::Arc;
use std::time::Duration;

use pmcp::server::task_store::{
    InMemoryTaskStore as SdkInMemoryTaskStore, StoreConfig as SdkStoreConfig, TaskStore,
    TaskStoreError,
};
use pmcp::types::tasks::{Task, TaskStatus};
use pmcp::types::{CallToolResult, Content};
use pmcp_tasks::{
    GenericTaskStore, InMemoryBackend, ServerTaskStore, StoreConfig, TaskSecurityConfig,
};
use proptest::prelude::*;
use serde_json::json;

const DEFAULT_TTL: u64 = 60_000;
const MAX_TTL: u64 = 120_000;
const POLL: u64 = 750;
const MAX_TASKS: usize = 4;

fn generic(allow_anonymous: bool) -> GenericTaskStore<InMemoryBackend> {
    GenericTaskStore::new(InMemoryBackend::new())
        .with_config(StoreConfig {
            default_ttl_ms: Some(DEFAULT_TTL),
            max_ttl_ms: Some(MAX_TTL),
            ..StoreConfig::default()
        })
        .with_security(
            TaskSecurityConfig::default()
                .with_max_tasks_per_owner(MAX_TASKS)
                .with_allow_anonymous(allow_anonymous),
        )
        .with_poll_interval(POLL)
}

fn adapter() -> ServerTaskStore<InMemoryBackend> {
    ServerTaskStore::new(generic(true))
}

fn reference() -> SdkInMemoryTaskStore {
    SdkInMemoryTaskStore::with_config(SdkStoreConfig {
        default_ttl_ms: Some(DEFAULT_TTL),
        max_ttl_ms: Some(MAX_TTL),
        default_poll_interval_ms: POLL,
        max_tasks_per_owner: MAX_TASKS,
    })
}

fn text_result(text: &str) -> CallToolResult {
    CallToolResult::new(vec![Content::text(text)])
}

// ─── Differential property test ─────────────────────────────────────────────

const OWNERS: [&str; 2] = ["alice", "bob"];

#[derive(Debug, Clone)]
enum Op {
    Create {
        owner: usize,
        ttl: Option<u64>,
    },
    Get {
        owner: usize,
        task: usize,
    },
    UpdateStatus {
        owner: usize,
        task: usize,
        status: TaskStatus,
        message: Option<String>,
    },
    SetStatusMessage {
        owner: usize,
        task: usize,
        message: Option<String>,
    },
    Cancel {
        owner: usize,
        task: usize,
    },
    SetResult {
        owner: usize,
        task: usize,
        text: String,
    },
    GetResult {
        owner: usize,
        task: usize,
    },
    List {
        owner: usize,
    },
}

fn arb_status() -> impl Strategy<Value = TaskStatus> {
    prop::sample::select(vec![
        TaskStatus::Working,
        TaskStatus::InputRequired,
        TaskStatus::Completed,
        TaskStatus::Failed,
        TaskStatus::Cancelled,
    ])
}

fn arb_message() -> impl Strategy<Value = Option<String>> {
    prop::option::of("[a-z ]{0,12}")
}

fn arb_op() -> impl Strategy<Value = Op> {
    let owner = 0..OWNERS.len();
    let task = 0..8usize;
    prop_oneof![
        (
            owner.clone(),
            prop::option::of(prop::sample::select(vec![1_000u64, MAX_TTL, MAX_TTL * 5]))
        )
            .prop_map(|(owner, ttl)| Op::Create { owner, ttl }),
        (owner.clone(), task.clone()).prop_map(|(owner, task)| Op::Get { owner, task }),
        (owner.clone(), task.clone(), arb_status(), arb_message()).prop_map(
            |(owner, task, status, message)| Op::UpdateStatus {
                owner,
                task,
                status,
                message
            }
        ),
        (owner.clone(), task.clone(), arb_message()).prop_map(|(owner, task, message)| {
            Op::SetStatusMessage {
                owner,
                task,
                message,
            }
        }),
        (owner.clone(), task.clone()).prop_map(|(owner, task)| Op::Cancel { owner, task }),
        (owner.clone(), task.clone(), "[a-z]{1,8}").prop_map(|(owner, task, text)| Op::SetResult {
            owner,
            task,
            text
        }),
        (owner.clone(), task.clone()).prop_map(|(owner, task)| Op::GetResult { owner, task }),
        owner.prop_map(|owner| Op::List { owner }),
    ]
}

/// What an operation did, with the store-specific parts (ids, timestamps)
/// left out so two stores can be compared.
#[derive(Debug, PartialEq)]
enum Outcome {
    Task {
        status: TaskStatus,
        message: Option<String>,
        ttl: Option<u64>,
        poll: Option<u64>,
    },
    Unit,
    Result(serde_json::Value),
    Listed(Vec<TaskStatus>),
    NotFound,
    Expired,
    Invalid {
        from: TaskStatus,
        to: TaskStatus,
    },
    Internal,
    NoTask,
}

fn task_outcome(task: Task) -> Outcome {
    Outcome::Task {
        status: task.status,
        message: task.status_message,
        ttl: task.ttl,
        poll: task.poll_interval,
    }
}

fn error_outcome(err: TaskStoreError) -> Outcome {
    match err {
        TaskStoreError::NotFound { .. } => Outcome::NotFound,
        TaskStoreError::Expired { .. } => Outcome::Expired,
        TaskStoreError::InvalidTransition { from, to, .. } => Outcome::Invalid { from, to },
        _ => Outcome::Internal,
    }
}

fn outcome<T>(result: Result<T, TaskStoreError>, ok: impl FnOnce(T) -> Outcome) -> Outcome {
    result.map_or_else(error_outcome, ok)
}

/// Runs one operation against a store, resolving task indexes through the
/// ids that store minted. Tasks are owned by whoever created them, so a task
/// index used with the other owner exercises owner isolation.
async fn apply(store: &dyn TaskStore, ids: &mut Vec<(usize, String)>, op: &Op) -> Outcome {
    let target = |task: usize| ids.get(task % ids.len().max(1)).map(|(_, id)| id.clone());
    match op {
        Op::Create { owner, ttl } => {
            let created = store.create(OWNERS[*owner], *ttl).await;
            if let Ok(task) = &created {
                ids.push((*owner, task.task_id.clone()));
            }
            outcome(created, task_outcome)
        },
        Op::List { owner } => outcome(store.list(OWNERS[*owner], None).await, |(tasks, _)| {
            let mut statuses: Vec<_> = tasks.into_iter().map(|t| t.status).collect();
            statuses.sort_by_key(|s| format!("{s}"));
            Outcome::Listed(statuses)
        }),
        Op::Get { owner, task } => match target(*task) {
            None => Outcome::NoTask,
            Some(id) => outcome(store.get(&id, OWNERS[*owner]).await, task_outcome),
        },
        Op::UpdateStatus {
            owner,
            task,
            status,
            message,
        } => match target(*task) {
            None => Outcome::NoTask,
            Some(id) => outcome(
                store
                    .update_status(&id, OWNERS[*owner], *status, message.clone())
                    .await,
                task_outcome,
            ),
        },
        Op::SetStatusMessage {
            owner,
            task,
            message,
        } => match target(*task) {
            None => Outcome::NoTask,
            Some(id) => outcome(
                store
                    .set_status_message(&id, OWNERS[*owner], message.clone())
                    .await,
                task_outcome,
            ),
        },
        Op::Cancel { owner, task } => match target(*task) {
            None => Outcome::NoTask,
            Some(id) => outcome(store.cancel(&id, OWNERS[*owner]).await, task_outcome),
        },
        Op::SetResult { owner, task, text } => match target(*task) {
            None => Outcome::NoTask,
            Some(id) => outcome(
                store
                    .set_result(&id, OWNERS[*owner], text_result(text))
                    .await,
                |()| Outcome::Unit,
            ),
        },
        Op::GetResult { owner, task } => match target(*task) {
            None => Outcome::NoTask,
            Some(id) => outcome(store.get_result(&id, OWNERS[*owner]).await, |result| {
                Outcome::Result(serde_json::to_value(result).unwrap())
            }),
        },
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Every operation sequence gives the same outcomes on `ServerTaskStore`
    /// as on pmcp's reference `InMemoryTaskStore`: the same statuses, messages,
    /// TTLs (clamped alike), results, per-owner limits, owner isolation and
    /// state-machine refusals.
    #[test]
    fn behaves_like_pmcps_in_memory_store(ops in prop::collection::vec(arb_op(), 1..40)) {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        rt.block_on(async {
            let (ours, theirs) = (adapter(), reference());
            let (mut our_ids, mut their_ids) = (Vec::new(), Vec::new());
            for (step, op) in ops.iter().enumerate() {
                let got = apply(&ours, &mut our_ids, op).await;
                let want = apply(&theirs, &mut their_ids, op).await;
                prop_assert_eq!(got, want, "step {}: {:?}", step, op);
            }
            Ok(())
        })?;
    }
}

// ─── Unit tests over the trait ──────────────────────────────────────────────

#[tokio::test]
async fn config_reports_what_the_wrapped_store_enforces() {
    let config = adapter().config().clone();
    assert_eq!(config.default_ttl_ms, Some(DEFAULT_TTL));
    assert_eq!(config.max_ttl_ms, Some(MAX_TTL));
    assert_eq!(config.default_poll_interval_ms, POLL);
    assert_eq!(config.max_tasks_per_owner, MAX_TASKS);
    let store = adapter();
    assert!(store.supports_results());
    assert!(store.supports_inputs());
}

#[tokio::test]
async fn a_requested_ttl_above_the_maximum_is_clamped() {
    let task = adapter().create("alice", Some(MAX_TTL * 10)).await.unwrap();
    assert_eq!(task.ttl, Some(MAX_TTL));
}

#[tokio::test]
async fn an_expired_task_reads_as_expired_and_leaves_the_list() {
    let store = adapter();
    let task = store.create("alice", Some(200)).await.unwrap();
    let id = task.task_id;
    store
        .set_result(&id, "alice", text_result("done"))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    for err in [
        store.get(&id, "alice").await.unwrap_err(),
        store.get_result(&id, "alice").await.unwrap_err(),
        store.get_error(&id, "alice").await.unwrap_err(),
        store.task_input_snapshot(&id, "alice").await.unwrap_err(),
        store
            .update_status(&id, "alice", TaskStatus::Completed, None)
            .await
            .unwrap_err(),
        store
            .set_status_message(&id, "alice", Some("late".into()))
            .await
            .unwrap_err(),
    ] {
        assert!(matches!(err, TaskStoreError::Expired { .. }), "{err}");
    }
    let (tasks, _) = store.list("alice", None).await.unwrap();
    assert!(tasks.is_empty(), "{tasks:?}");
    assert_eq!(store.cleanup_expired().await.unwrap(), 1);
}

#[tokio::test]
async fn status_message_reports_progress_without_moving_the_task() {
    let store = adapter();
    let id = store.create("alice", None).await.unwrap().task_id;
    let task = store
        .set_status_message(&id, "alice", Some("step 2/4".into()))
        .await
        .unwrap();
    assert_eq!(task.status, TaskStatus::Working);
    assert_eq!(task.status_message.as_deref(), Some("step 2/4"));
    let read = store.get(&id, "alice").await.unwrap();
    assert_eq!(read.status_message.as_deref(), Some("step 2/4"));

    store
        .update_status(&id, "alice", TaskStatus::Completed, Some("done".into()))
        .await
        .unwrap();
    let err = store
        .set_status_message(&id, "alice", Some("again".into()))
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            TaskStoreError::InvalidTransition {
                from: TaskStatus::Completed,
                to: TaskStatus::Completed,
                ..
            }
        ),
        "{err}"
    );
}

#[tokio::test]
async fn input_requests_round_trip_through_the_backend() {
    let store = adapter();
    let id = store.create("alice", None).await.unwrap().task_id;
    let requests = serde_json::from_value(json!({ "roots": { "method": "roots/list" } })).unwrap();
    let task = store
        .record_input_requests(&id, "alice", requests)
        .await
        .unwrap();
    assert_eq!(task.status, TaskStatus::InputRequired);

    let snapshot = store.task_input_snapshot(&id, "alice").await.unwrap();
    assert_eq!(snapshot.status, TaskStatus::InputRequired);
    assert_eq!(
        snapshot.outstanding().into_iter().collect::<Vec<_>>(),
        ["roots"]
    );

    let responses =
        serde_json::from_value(json!({ "roots": { "roots": [] }, "stray": { "roots": [] } }))
            .unwrap();
    let delivery = store
        .deliver_task_inputs(&id, "alice", responses)
        .await
        .unwrap();
    assert!(delivery.complete);
    assert!(delivery.accepted.contains("roots"));
    assert!(delivery.ignored.contains("stray"));
    assert_eq!(
        store.get(&id, "alice").await.unwrap().status,
        TaskStatus::Working
    );
    let snapshot = store.task_input_snapshot(&id, "alice").await.unwrap();
    assert!(snapshot.is_complete());
}

#[tokio::test]
async fn a_task_that_never_asked_for_input_has_no_snapshot() {
    let store = adapter();
    let id = store.create("alice", None).await.unwrap().task_id;
    let err = store.task_input_snapshot(&id, "alice").await.unwrap_err();
    assert!(matches!(err, TaskStoreError::NotFound { .. }), "{err}");
}

#[tokio::test]
async fn errors_are_stored_per_task_and_owner() {
    let store = adapter();
    let id = store.create("alice", None).await.unwrap().task_id;
    assert!(matches!(
        store.get_error(&id, "alice").await.unwrap_err(),
        TaskStoreError::NotFound { .. }
    ));
    let error = json!({ "code": -32603, "message": "backtest failed" });
    store.set_error(&id, "alice", error.clone()).await.unwrap();
    assert_eq!(store.get_error(&id, "alice").await.unwrap(), error);
    assert!(matches!(
        store.get_error(&id, "bob").await.unwrap_err(),
        TaskStoreError::NotFound { .. }
    ));
}

#[tokio::test]
async fn anonymous_owners_are_refused_unless_allowed() {
    let strict = ServerTaskStore::new(generic(false));
    for owner in ["local", ""] {
        let err = strict.create(owner, None).await.unwrap_err();
        assert!(
            matches!(&err, TaskStoreError::Internal { message } if message.contains("anonymous")),
            "{owner:?}: {err}"
        );
    }
    assert!(strict.create("alice", None).await.is_ok());
    assert!(adapter().create("local", None).await.is_ok());
}

#[tokio::test]
async fn the_wrapped_store_sees_the_same_tasks() {
    let store = adapter();
    let id = store.create("alice", None).await.unwrap().task_id;
    let record = store.inner().get(&id, "alice").await.unwrap();
    assert_eq!(record.request_method, "tools/call");
    let from: ServerTaskStore<_> = generic(true).into();
    assert_eq!(from.config().max_tasks_per_owner, MAX_TASKS);
}

// ─── A real server over HTTP ────────────────────────────────────────────────

mod over_http {
    use std::net::SocketAddr;

    use pmcp::server::streamable_http_server::StreamableHttpServer;
    use pmcp::server::task_store::CreatedTask;
    use pmcp::server::typed_tool::TypedTool;
    use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
    use pmcp::types::{ClientCapabilities, TaskSupport, ToolExecution};
    use pmcp::{Client, RequestHandlerExtra, Server, ToolCallResponse};
    use serde_json::Value;
    use url::Url;

    use super::*;

    async fn worker(store: Arc<dyn TaskStore>, created: CreatedTask) {
        let (id, owner) = (created.task.task_id, created.owner_id);
        store
            .set_status_message(&id, &owner, Some("halfway".into()))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(30)).await;
        store
            .set_result(&id, &owner, text_result("worker done"))
            .await
            .unwrap();
        store
            .update_status(&id, &owner, TaskStatus::Completed, None)
            .await
            .unwrap();
    }

    fn slow_tool(store: Arc<dyn TaskStore>) -> impl pmcp::ToolHandler {
        TypedTool::new_with_schema(
            "slow",
            json!({ "type": "object" }),
            move |_args: Value, extra: RequestHandlerExtra| {
                let store = Arc::clone(&store);
                extra.on_task_created(move |created| {
                    tokio::spawn(worker(store, created));
                });
                Box::pin(async { Ok(json!({ "taskId": "pending", "status": "working" })) })
            },
        )
        .with_execution(ToolExecution::new().with_task_support(TaskSupport::Optional))
    }

    async fn serve(store: Arc<dyn TaskStore>) -> (SocketAddr, tokio::task::JoinHandle<()>) {
        let server = Server::builder()
            .name("server-task-store")
            .version("1.0.0")
            .tool("slow", slow_tool(Arc::clone(&store)))
            .task_store(store)
            .build()
            .unwrap();
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        StreamableHttpServer::new_concurrent(addr, Arc::new(server))
            .start()
            .await
            .unwrap()
    }

    async fn client(addr: SocketAddr) -> Client<StreamableHttpTransport> {
        let config = StreamableHttpTransportConfig {
            url: Url::parse(&format!("http://{addr}")).unwrap(),
            extra_headers: vec![],
            auth_provider: None,
            session_id: None,
            enable_json_response: true,
            on_resumption_token: None,
            http_middleware_chain: None,
        };
        let mut client = Client::new(StreamableHttpTransport::new(config));
        client
            .initialize(ClientCapabilities::default())
            .await
            .unwrap();
        client
    }

    #[tokio::test]
    async fn a_server_runs_a_task_lifecycle_on_the_backend() {
        let store = Arc::new(adapter());
        let (addr, handle) = serve(Arc::clone(&store) as Arc<dyn TaskStore>).await;
        let client = client(addr).await;

        let task_id = match client
            .call_tool_with_task("slow".to_string(), json!({}))
            .await
            .unwrap()
        {
            ToolCallResponse::Task(task) => task.task_id,
            ToolCallResponse::Result(result) => panic!("expected a task, got {result:?}"),
        };
        // The id the client polls is the one the backend holds.
        let record = store.inner().get(&task_id, "local").await.unwrap();
        assert_eq!(record.task.task_id, task_id);

        let mut task = client.tasks_get(&task_id).await.unwrap();
        for _ in 0..200 {
            if task.status.is_terminal() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
            task = client.tasks_get(&task_id).await.unwrap();
        }
        assert_eq!(task.status, TaskStatus::Completed);

        let result = client.tasks_result(&task_id).await.unwrap();
        assert_eq!(
            serde_json::to_value(&result.content).unwrap(),
            json!([{ "type": "text", "text": "worker done" }])
        );
        handle.abort();
    }

    #[tokio::test]
    async fn an_unauthenticated_server_cannot_create_tasks_by_default() {
        let store = Arc::new(ServerTaskStore::new(generic(false)));
        let (addr, handle) = serve(store as Arc<dyn TaskStore>).await;
        let client = client(addr).await;
        let err = client
            .call_tool_with_task("slow".to_string(), json!({}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("anonymous"), "{err}");
        handle.abort();
    }
}
