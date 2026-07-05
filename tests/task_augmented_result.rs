//! SEP-1686 task-augmented tool-result DX (Phase 104, TOUT-01/TOUT-03).
//!
//! Covers the client-side surface that lets integrators stop hand-reading
//! `result._meta[related-task]` and stop hand-copying poll fields:
//!
//! - `CallToolResult::with_related_task` / `related_task` twins keyed by
//!   `RELATED_TASK_META_KEY` (accessor round-trip, minimal-shape tolerance,
//!   no-`_meta` -> `None`).
//! - `Client::wait_for_task` / `wait_for_related_task` polling convenience that
//!   composes directly from `TaskMetadata` via `WaitForTaskOptions`.

use pmcp::types::tasks::{TaskMetadata, RELATED_TASK_META_KEY};
use pmcp::types::CallToolResult;

// ---------------------------------------------------------------------------
// Task 2: CallToolResult::with_related_task / related_task accessor twins.
// ---------------------------------------------------------------------------

#[test]
fn related_task_round_trip_recovers_task_id() {
    let meta = TaskMetadata::new("t9")
        .with_poll_interval(1500)
        .with_max_poll_duration_secs(60);
    let result = CallToolResult::new(vec![]).with_related_task(meta);

    let recovered = result
        .related_task()
        .expect("related_task must recover the attached metadata");
    assert_eq!(recovered.task_id, "t9");
    assert_eq!(recovered.poll_interval, Some(1500));
    assert_eq!(recovered.max_poll_duration_secs, Some(60));
}

#[test]
fn related_task_none_when_no_meta() {
    let result = CallToolResult::new(vec![]);
    assert!(
        result.related_task().is_none(),
        "a result with no _meta must yield None"
    );
}

#[test]
fn related_task_tolerates_minimal_shape() {
    // Server emitted only the minimal { taskId } native shape under the key.
    let mut meta_map = serde_json::Map::new();
    meta_map.insert(
        RELATED_TASK_META_KEY.to_string(),
        serde_json::json!({ "taskId": "t9" }),
    );
    let result = CallToolResult::new(vec![]).with_meta(meta_map);

    let recovered = result
        .related_task()
        .expect("minimal {taskId} shape must still yield Some");
    assert_eq!(recovered.task_id, "t9");
    assert_eq!(recovered.poll_interval, None);
    assert_eq!(recovered.max_poll_duration_secs, None);
}

#[test]
fn related_task_none_on_malformed_value() {
    // A malformed related-task value must not panic — returns None.
    let mut meta_map = serde_json::Map::new();
    meta_map.insert(
        RELATED_TASK_META_KEY.to_string(),
        serde_json::json!("not-an-object"),
    );
    let result = CallToolResult::new(vec![]).with_meta(meta_map);
    assert!(
        result.related_task().is_none(),
        "malformed _meta[related-task] must yield None, not panic"
    );
}

// ---------------------------------------------------------------------------
// Task 3: WaitForTaskOptions composition from TaskMetadata (no hand-copy).
// ---------------------------------------------------------------------------

#[test]
fn wait_options_from_metadata_copies_poll_fields() {
    let meta = TaskMetadata::new("t-compose")
        .with_poll_interval(250)
        .with_max_poll_duration_secs(30);

    let opts = pmcp::WaitForTaskOptions::from_metadata(&meta);
    assert_eq!(opts.poll_interval, Some(250));
    assert_eq!(opts.max_poll_duration_secs, Some(30));

    // From<TaskMetadata> is equivalent.
    let via_from: pmcp::WaitForTaskOptions = meta.clone().into();
    assert_eq!(via_from.poll_interval, Some(250));
    assert_eq!(via_from.max_poll_duration_secs, Some(30));
}

#[test]
fn wait_options_or_from_metadata_precedence() {
    let meta = TaskMetadata::new("t-compose")
        .with_poll_interval(250)
        .with_max_poll_duration_secs(30);

    // Explicit poll_interval wins; unset max_poll_duration_secs fills from meta.
    let opts = pmcp::WaitForTaskOptions {
        poll_interval: Some(999),
        max_poll_duration_secs: None,
    }
    .or_from_metadata(&meta);
    assert_eq!(opts.poll_interval, Some(999), "explicit value must win");
    assert_eq!(
        opts.max_poll_duration_secs,
        Some(30),
        "unset field must fill from metadata"
    );
}

// ---------------------------------------------------------------------------
// Task 3: wait_for_task live lifecycle / timeout / clamp over a duplex harness.
// ---------------------------------------------------------------------------

mod live {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use async_trait::async_trait;
    use pmcp::server::builder::ServerCoreBuilder;
    use pmcp::server::core::ProtocolHandler;
    use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
    use pmcp::server::typed_tool::TypedTool;
    use pmcp::shared::{Transport, TransportMessage};
    use pmcp::types::tasks::{TaskMetadata, TaskStatus};
    use pmcp::types::{ClientCapabilities, TaskSupport, ToolExecution};
    use pmcp::{Client, Error, WaitForTaskOptions};
    use tokio::sync::mpsc;

    /// In-process duplex transport (client <-> ServerCore), mpsc-backed.
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

    /// Server pump that counts every inbound request (used to prove the poll
    /// loop does not hot-spin).
    fn spawn_counting_pump(
        mut server_transport: DuplexTransport,
        handler: Arc<dyn ProtocolHandler>,
        request_count: Arc<AtomicUsize>,
    ) {
        tokio::spawn(async move {
            while let Ok(message) = server_transport.receive().await {
                if let TransportMessage::Request { id, request } = message {
                    request_count.fetch_add(1, Ordering::SeqCst);
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

    /// A task tool that completes synchronously with terminal content.
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

    /// A task tool that stays pending forever (never terminal).
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

    fn build_server(
        tool_name: &str,
        handler: impl pmcp::ToolHandler + 'static,
    ) -> Arc<dyn ProtocolHandler> {
        let store = Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>;
        Arc::new(
            ServerCoreBuilder::new()
                .name("wait-for-task-server")
                .version("1.0.0")
                .tool(tool_name, handler)
                .task_store(store)
                .build()
                .expect("server builds"),
        )
    }

    async fn create_task(client: &mut Client<DuplexTransport>, tool: &str) -> String {
        let response = client
            .call_tool_with_task(tool.to_string(), serde_json::json!({}))
            .await
            .expect("task-augmented call");
        match response {
            pmcp::ToolCallResponse::Task(task) => task.task_id,
            pmcp::ToolCallResponse::Result(_) => panic!("expected a created task"),
        }
    }

    /// wait_for_task drives a completing task to terminal and returns the
    /// persisted, non-empty tasks/result CallToolResult.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wait_for_task_returns_terminal_result() {
        let server = build_server("complete_now", completing_task_tool());
        let (client_transport, server_transport) = DuplexTransport::pair();
        spawn_counting_pump(server_transport, server, Arc::new(AtomicUsize::new(0)));

        let mut client = Client::new(client_transport);
        client
            .initialize(ClientCapabilities::default())
            .await
            .expect("initialize");

        let task_id = create_task(&mut client, "complete_now").await;
        let result = client
            .wait_for_task(&task_id, WaitForTaskOptions::default())
            .await
            .expect("wait_for_task resolves to the terminal result");
        assert!(
            !result.content.is_empty(),
            "wait_for_task must return the persisted non-empty terminal content"
        );
    }

    /// wait_for_related_task composes directly from TaskMetadata (no hand-copy):
    /// a related-task handle drives the poller to the same terminal result.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wait_for_related_task_from_metadata_composes() {
        let server = build_server("complete_now", completing_task_tool());
        let (client_transport, server_transport) = DuplexTransport::pair();
        spawn_counting_pump(server_transport, server, Arc::new(AtomicUsize::new(0)));

        let mut client = Client::new(client_transport);
        client
            .initialize(ClientCapabilities::default())
            .await
            .expect("initialize");

        let task_id = create_task(&mut client, "complete_now").await;
        let meta = TaskMetadata::new(task_id).with_poll_interval(50);
        let result = client
            .wait_for_related_task(&meta, WaitForTaskOptions::default())
            .await
            .expect("wait_for_related_task resolves");
        assert!(!result.content.is_empty());
    }

    /// A never-terminal task with max_poll_duration_secs returns a Timeout error
    /// rather than looping forever, and (with a clamped zero poll interval) does
    /// NOT hot-spin — bounded by a sane number of polls in the timeout window.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wait_for_task_times_out_and_does_not_hot_spin() {
        let server = build_server("stay_pending", pending_task_tool());
        let (client_transport, server_transport) = DuplexTransport::pair();
        let request_count = Arc::new(AtomicUsize::new(0));
        spawn_counting_pump(server_transport, server, request_count.clone());

        let mut client = Client::new(client_transport);
        client
            .initialize(ClientCapabilities::default())
            .await
            .expect("initialize");

        let task_id = create_task(&mut client, "stay_pending").await;
        // Zero poll interval MUST be clamped to the floor (50 ms). Over a 1 s
        // budget that is ~20 polls; without the clamp it would be thousands.
        let opts = WaitForTaskOptions {
            poll_interval: Some(0),
            max_poll_duration_secs: Some(1),
        };
        let before = request_count.load(Ordering::SeqCst);
        let err = client
            .wait_for_task(&task_id, opts)
            .await
            .expect_err("a never-terminal task must time out");
        assert!(
            matches!(err, Error::Timeout(_)),
            "expected a Timeout error, got: {err}"
        );

        let polls = request_count.load(Ordering::SeqCst) - before;
        assert!(
            polls < 200,
            "zero poll_interval must be clamped to a floor (no hot spin); saw {polls} polls in ~1s"
        );
    }

    /// The sleep is clamped to the remaining budget: a huge poll interval must
    /// not overshoot `max_poll_duration_secs` by an unbounded factor (WR-01).
    /// With a 60 s interval and a 1 s budget, the timeout must be reported
    /// near the budget, not after the first full interval.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wait_for_task_timeout_is_not_overshot_by_large_interval() {
        let server = build_server("stay_pending", pending_task_tool());
        let (client_transport, server_transport) = DuplexTransport::pair();
        spawn_counting_pump(server_transport, server, Arc::new(AtomicUsize::new(0)));

        let mut client = Client::new(client_transport);
        client
            .initialize(ClientCapabilities::default())
            .await
            .expect("initialize");

        let task_id = create_task(&mut client, "stay_pending").await;
        let opts = WaitForTaskOptions {
            poll_interval: Some(60_000),
            max_poll_duration_secs: Some(1),
        };
        let started = std::time::Instant::now();
        let err = client
            .wait_for_task(&task_id, opts)
            .await
            .expect_err("a never-terminal task must time out");
        assert!(
            matches!(err, Error::Timeout(_)),
            "expected a Timeout error, got: {err}"
        );
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "timeout must be honored near the 1s budget, not the 60s interval; took {:?}",
            started.elapsed()
        );
    }

    /// A task that enters `input_required` surfaces an error instead of
    /// spinning forever under the default (unbounded) options (CR-01):
    /// `input_required` is NOT terminal and needs client-side action the
    /// poller cannot provide.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wait_for_task_surfaces_input_required_instead_of_hanging() {
        let store = Arc::new(InMemoryTaskStore::new());
        let server: Arc<dyn ProtocolHandler> = Arc::new(
            ServerCoreBuilder::new()
                .name("wait-for-task-server")
                .version("1.0.0")
                .tool("stay_pending", pending_task_tool())
                .task_store(store.clone() as Arc<dyn TaskStore>)
                .build()
                .expect("server builds"),
        );
        let (client_transport, server_transport) = DuplexTransport::pair();
        spawn_counting_pump(server_transport, server, Arc::new(AtomicUsize::new(0)));

        let mut client = Client::new(client_transport);
        client
            .initialize(ClientCapabilities::default())
            .await
            .expect("initialize");

        let task_id = create_task(&mut client, "stay_pending").await;
        // `Working -> InputRequired` is a legal transition; "local" is the
        // owner an unauthenticated (duplex) session resolves to.
        store
            .update_status(&task_id, "local", TaskStatus::InputRequired, None)
            .await
            .expect("transition to input_required");

        // The outer timeout is a CI safety net: on regression (poller ignores
        // input_required) this call would otherwise hang forever.
        let err = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            client.wait_for_task(&task_id, WaitForTaskOptions::default()),
        )
        .await
        .expect("wait_for_task must return promptly on input_required, not hang")
        .expect_err("input_required must surface as an error");
        assert!(
            matches!(err, Error::Validation(_)),
            "expected a validation error, got: {err}"
        );
    }
}
