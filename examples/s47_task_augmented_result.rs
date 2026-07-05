//! # Server example: Task-augmented tool results — BEFORE/AFTER migration (SEP-1686)
//!
//! This example IS the migration guide for the SEP-1686 junction (Phase 104,
//! TOUT-04): it shows the SAME "kick off async work and point the caller at a
//! task" tool written TWICE against a live HTTP loopback, so the diff between the
//! two handlers is the whole story.
//!
//! ## BEFORE — the hand-rolled `_meta` anti-pattern (pre-2.12)
//!
//! The tool returns a `serde_json::Value` that is ALREADY shaped like a
//! `CallToolResult` (it carries `content` plus an
//! `_meta["io.modelcontextprotocol/related-task"]` envelope) on the normal
//! Payload path. Dispatch has no way to know the handler meant that `Value` to BE
//! the result, so it stringifies the whole thing into `content[0].text`
//! (the `src/server/mod.rs` text-wrap) — the silent double-wrap bug that cost the
//! pmcp.run team a two-week production outage. The top-level `_meta` never reaches
//! the wire, so a `_meta`-sniffing client sees NO related task.
//!
//! Because that `Value` structurally resembles a `CallToolResult`, it also trips
//! the Plan 03 double-wrap tripwire (a `debug_assert!` in debug/CI builds). We
//! register the BEFORE tool with
//! [`suppress_double_wrap_check`](pmcp::ServerBuilder::suppress_double_wrap_check)
//! ONLY so this teaching example can DEMONSTRATE the broken shape without aborting
//! the process. Real tools should MIGRATE to the AFTER pattern, not suppress the
//! tripwire — suppression is documented as rare-and-reviewed for a reason.
//!
//! ## AFTER — the native `task_store()` + `ToolOutput::Result` pattern (2.12+)
//!
//! The tool references a REAL, store-minted task id (obtained from
//! [`TaskStore::create`](pmcp::server::task_store::TaskStore::create) — the same
//! native store machinery `s45`/`s46` use, so the id is minted by the store, NOT
//! hand-written) and returns a fully-formed `CallToolResult` via
//! [`ServerBuilder::tool_with_result`](pmcp::ServerBuilder::tool_with_result) +
//! [`CallToolResult::with_related_task`](pmcp::types::CallToolResult::with_related_task).
//! That envelope reaches the wire VERBATIM (`ToolOutput::Result`), so the
//! top-level `_meta[related-task]` survives and the client's
//! [`related_task()`](pmcp::types::CallToolResult::related_task) accessor detects
//! the task with zero glue.
//!
//! > ⚠️ A `tool_with_result` / `ToolOutput::Result` tool bypasses RESPONSE
//! > middleware (redaction/sanitization/audit) — the handler owns its OWN
//! > redaction of `content` and `_meta` (D-04a). See the migration guide
//! > (`docs/design/sep-1686-task-augmented-results.md`) for the full callout.
//!
//! Test-reliability practices (carried from the phase's HTTP test / `s46`):
//! - EPHEMERAL PORT — binds `127.0.0.1:0` and uses the address read back from
//!   `StreamableHttpServer::start()` (no hardcoded port).
//! - READINESS — `start()` binds the listener before returning (no fixed sleep).
//! - SHUTDOWN — the spawned server `JoinHandle` is `abort()`ed after the round-trip.
//!
//! Every claim below is a HARD assertion (returns `Err` on failure), not just
//! printed output, so this example doubles as a regression harness.
//!
//! Run with: `cargo run --example s47_task_augmented_result --features full`

#![cfg(not(target_arch = "wasm32"))]

use std::net::SocketAddr;
use std::sync::Arc;

use pmcp::server::streamable_http_server::StreamableHttpServer;
use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
use pmcp::server::typed_tool::TypedTool;
use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
use pmcp::types::tasks::TaskMetadata;
use pmcp::types::{CallToolResult, Content};
use pmcp::{Client, Error, Server};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::sync::Mutex;
use url::Url;

/// Input for the AFTER tool. Empty, but typed — `tool_with_result` requires a
/// `DeserializeOwned + JsonSchema` input type (schema is generated for it).
#[derive(Debug, Deserialize, JsonSchema)]
struct StartArgs {}

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    // ---------------------------------------------------------------------
    // Store setup: mint a REAL task id via the native store create-path.
    //
    // This is the id the AFTER tool will point the caller at. It is minted by
    // the store (NOT a hand-written literal), so `tasks/get`/`tasks/result`
    // would resolve it — the whole point of task-augmented results.
    // ---------------------------------------------------------------------
    let store = Arc::new(InMemoryTaskStore::new());
    let minted = store
        .create("s47-owner", None)
        .await
        .map_err(|e| Error::internal(format!("store.create failed: {e}")))?;
    let after_task_id = minted.task_id.clone();
    println!("store-minted task id (AFTER points here): {after_task_id}");

    // The clone the AFTER closure captures and stamps into `with_related_task`.
    let closure_task_id = after_task_id.clone();

    // ---------------------------------------------------------------------
    // BEFORE tool — hand-rolled `_meta` on the Payload path (the anti-pattern).
    //
    // Its handler returns a `Value` that is ALREADY CallToolResult-shaped. On the
    // Payload path dispatch will stringify this whole object into content[0].text,
    // so the top-level `_meta` is LOST — the silent double-wrap bug.
    // ---------------------------------------------------------------------
    let before_tool = TypedTool::new_with_schema(
        "before_hand_rolled",
        serde_json::json!({ "type": "object" }),
        |_args: serde_json::Value, _extra| {
            Box::pin(async {
                Ok(serde_json::json!({
                    "content": [
                        { "type": "text", "text": "processing started (hand-rolled)" }
                    ],
                    // The pre-2.12 workaround: stuff related-task into `_meta`
                    // and hope it survives. It does NOT — it gets stringified.
                    "_meta": {
                        "io.modelcontextprotocol/related-task": {
                            "taskId": "hand-rolled-not-store-minted"
                        }
                    }
                }))
            })
        },
    )
    .with_description("BEFORE: hand-rolls _meta on the Payload path (anti-pattern)");

    // ---------------------------------------------------------------------
    // Build the high-level Server with BOTH tools + the native task store.
    //
    // `suppress_double_wrap_check("before_hand_rolled")` is applied ONLY so this
    // demo can run the broken BEFORE shape without the Plan 03 debug-assert
    // aborting the process. Do NOT suppress in real code — migrate to the AFTER
    // pattern instead. The AFTER tool uses the native store machinery
    // (`task_store()` — same family as `with_task_store`) + `tool_with_result`,
    // returning a `CallToolResult` (a `ToolOutput::Result`) that reaches the wire
    // verbatim, with a store-minted related-task id.
    // ---------------------------------------------------------------------
    let server = Server::builder()
        .name("s47-task-augmented-result")
        .version("1.0.0")
        .tool("before_hand_rolled", before_tool)
        .suppress_double_wrap_check("before_hand_rolled")
        .tool_with_result::<StartArgs>("after_native", move |_args, _extra| {
            let task_id = closure_task_id.clone();
            Box::pin(async move {
                // AFTER: own the full envelope; point at the store-minted task.
                Ok(
                    CallToolResult::new(vec![Content::text("processing started (native)")])
                        .with_related_task(TaskMetadata::new(task_id)),
                )
            })
        })
        .task_store(store.clone() as Arc<dyn TaskStore>)
        .build()?;
    let server = Arc::new(Mutex::new(server));

    // EPHEMERAL PORT + READINESS: bind 127.0.0.1:0, read the OS-assigned address
    // back from start() (already accepting connections — no sleep).
    let bind_addr: SocketAddr = "127.0.0.1:0".parse().expect("valid loopback addr");
    let http_server = StreamableHttpServer::new(bind_addr, server);
    let (bound, server_handle) = http_server.start().await?;
    println!("HTTP server listening on http://{bound}");

    // Run the BEFORE/AFTER round-trip, then ALWAYS abort the server task.
    let result = run_migration_demo(bound, &after_task_id).await;

    // SHUTDOWN: cancel the spawned server task — no lingering listener, no hang.
    server_handle.abort();
    println!("server task aborted (clean shutdown)");

    result
}

/// Drive both tools over a live HTTP client and assert the migration story.
async fn run_migration_demo(bound: SocketAddr, expected_task_id: &str) -> pmcp::Result<()> {
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
    client
        .initialize(pmcp::types::ClientCapabilities::default())
        .await?;

    // ---- BEFORE: the hand-rolled _meta is LOST (stringified into text). ----
    let before = client
        .call_tool("before_hand_rolled".to_string(), serde_json::json!({}))
        .await?;
    println!("\nBEFORE (hand-rolled):");
    println!("  related_task() = {:?}", before.related_task());
    if before.related_task().is_some() {
        return Err(Error::internal(
            "BEFORE demo invariant broken: the hand-rolled _meta unexpectedly survived at \
             result top level — the anti-pattern would not be demonstrated",
        ));
    }
    // The whole hand-rolled envelope was stringified into content[0].text — the bug.
    let before_text = before
        .content
        .first()
        .and_then(|c| match c {
            Content::Text { text } => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default();
    if !before_text.contains("_meta") {
        return Err(Error::internal(
            "BEFORE demo invariant broken: expected the hand-rolled envelope (incl. its _meta) \
             to be stringified into content[0].text",
        ));
    }
    println!("  content[0].text contains the stringified _meta envelope (the double-wrap bug)");

    // ---- AFTER: the native result carries related-task at top level. ----
    let after = client
        .call_tool("after_native".to_string(), serde_json::json!({}))
        .await?;
    println!("\nAFTER (native task_store + tool_with_result):");
    let related = after.related_task().ok_or_else(|| {
        Error::internal("AFTER result must expose related_task() == Some over the wire")
    })?;
    println!("  related_task().taskId = {}", related.task_id);
    if related.task_id != expected_task_id {
        return Err(Error::internal(format!(
            "AFTER related-task id must equal the store-minted id ({expected_task_id}), got {}",
            related.task_id
        )));
    }
    // Content is the real text, NOT a stringified envelope.
    let after_text = after
        .content
        .first()
        .and_then(|c| match c {
            Content::Text { text } => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default();
    if after_text.contains("_meta") {
        return Err(Error::internal(
            "AFTER content must NOT be a stringified envelope (no _meta in content[0].text)",
        ));
    }
    println!("  content[0].text = {after_text:?} (not stringified)");

    println!(
        "\nMigration OK: BEFORE loses _meta (anti-pattern), AFTER carries the store-minted \
         related-task at result top level (native pattern)."
    );
    Ok(())
}
