//! Phase 103 Wave-0 DURABLE de-risking artifact (plan 103-03, HIGH-3).
//!
//! This test is the durable proof for RESEARCH **Open Question 1**: can the
//! OAuth2 IdP HTTP routes (`GET /oauth2/authorize`, `POST /oauth2/token`) be
//! served on the SAME origin as the MCP `StreamableHttpServer` router — so the
//! browser PKCE flow and the MCP `tasks/*` traffic share one origin and avoid
//! CORS entirely (Open Question 1 option (b), the preferred single-origin merge)?
//!
//! ## Route-merge verdict (the crux of HIGH-3)
//!
//! The merge seam is **PUBLIC** — no SDK-internal change is required. The exact
//! public API is:
//!
//! ```text
//! pmcp::axum::router(server: Arc<Mutex<Server>>) -> axum::Router
//! ```
//!
//! (re-exported at `src/lib.rs:56` from `pmcp::server::axum_router::router` /
//! `router_with_config`, see `src/server/axum_router.rs:72,91`). It returns a
//! fully-layered `axum::Router` (CORS + DNS-rebinding + security headers already
//! applied), and because it is a plain `axum::Router` it can be composed with
//! hand-wired OAuth routes via `axum::Router::merge`. `build_mcp_router`
//! (`streamable_http_server.rs:286`) is `pub(crate)` and is NOT the seam plan 04
//! should use — `pmcp::axum::router_with_config` is the public, layered entry.
//!
//! Because the seam is PUBLIC, this test stands up the REAL merged router and
//! drives it over a live ephemeral-port loopback listener. It asserts that BOTH
//! the merged `/oauth2/*` routes AND the MCP `POST /` endpoint respond on ONE
//! origin. If a future refactor demoted the seam to `pub(crate)`, this test would
//! fail to compile — the regression cannot silently land.
//!
//! ## Owner-resolution note (Open Question 2)
//!
//! See `tests/tool_as_task_lifecycle_http.rs` for the full owner-isolation proof.
//! The decision recorded in `103-SPIKE.md`: with a `TaskStore` configured,
//! `TaskDispatcher::resolve_owner` (`src/server/task_dispatch.rs:182-186`) returns
//! `AuthContext.subject` verbatim (or `"local"` when unauthenticated). The bundled
//! IdP mints `TokenInfo.user_id` from the `user_id` chosen at the authorize step
//! (`oauth2.rs:522,606,624`); a `TokenValidator`/`AuthProvider` adapter maps that
//! `user_id` onto `AuthContext.subject`, so `store.list(subject)` finds the
//! create-path-minted Working task. Plan 04's background updater lists for that
//! same `subject` string.

#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

use std::net::SocketAddr;
use std::sync::Arc;

use axum::routing::{get, post};
use axum::{Json, Router};
use pmcp::server::auth::oauth2::InMemoryOAuthProvider;
use pmcp::server::auth::OAuthProvider;
use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
use pmcp::server::typed_tool::TypedTool;
use pmcp::types::{TaskSupport, ToolExecution};
use pmcp::Server;
use tokio::sync::Mutex;

/// A trivially-completing task tool (mirrors s46) so the merged server advertises
/// the `tasks` capability and the MCP POST endpoint is fully wired.
fn task_tool() -> impl pmcp::ToolHandler {
    TypedTool::new_with_schema(
        "summarize",
        serde_json::json!({ "type": "object" }),
        |_args: serde_json::Value, _extra| {
            Box::pin(async {
                Ok(serde_json::json!({
                    "taskId": "tool-fabricated",
                    "status": "completed",
                    "ttl": 60000,
                    "createdAt": "2026-06-30T00:00:00Z",
                    "lastUpdatedAt": "2026-06-30T00:00:00Z",
                    "result": { "content": [ { "type": "text", "text": "ok" } ] }
                }))
            })
        },
    )
    .with_description("Summarize as an MCP Task")
    .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required))
}

/// Build the SINGLE merged `axum::Router`: the public MCP router from
/// `pmcp::axum::router()` merged with two hand-wired OAuth2 IdP routes that drive
/// an `InMemoryOAuthProvider`. This is the exact composition plan 04 will reuse.
fn build_merged_router(server: Arc<Mutex<Server>>, idp: Arc<InMemoryOAuthProvider>) -> Router {
    // PUBLIC seam — returns a fully-layered axum::Router (Open Question 1 verdict).
    let mcp_router: Router = pmcp::axum::router(server);

    let idp_authorize = idp.clone();
    let idp_token = idp;

    let oauth_routes = Router::new()
        // GET /oauth2/authorize — exercises the IdP metadata/authorize surface.
        .route(
            "/oauth2/authorize",
            get(move || {
                let idp = idp_authorize.clone();
                async move {
                    let meta = idp.metadata().await.expect("idp metadata");
                    // Real IdP would 302 to redirect_uri?code=...; for the merge
                    // proof we return the authorize endpoint it advertises.
                    Json(serde_json::json!({
                        "authorization_endpoint": meta.authorization_endpoint,
                        "ready": true,
                    }))
                }
            }),
        )
        // POST /oauth2/token — exercises the IdP token surface.
        .route(
            "/oauth2/token",
            post(move || {
                let idp = idp_token.clone();
                async move {
                    let meta = idp.metadata().await.expect("idp metadata");
                    Json(serde_json::json!({
                        "token_endpoint": meta.token_endpoint,
                        "ready": true,
                    }))
                }
            }),
        );

    // The merge: ONE origin serves MCP `/` AND the `/oauth2/*` routes.
    mcp_router.merge(oauth_routes)
}

/// DURABLE proof (HIGH-3): stand up the merged MCP + OAuth router on one
/// ephemeral-port listener and assert BOTH the OAuth routes AND the MCP POST
/// endpoint respond. Uses an ephemeral port (no hardcoded port), binds before
/// serving (deterministic readiness), and aborts the server task on completion
/// (no hang).
#[tokio::test]
async fn merged_mcp_and_oauth2_routes_respond_on_one_origin() {
    // --- Build the store-backed high-level server (auto-advertises `tasks`). ---
    let store = Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>;
    let server = Server::builder()
        .name("web-channel-oauth-merge-spike")
        .version("1.0.0")
        .tool("summarize", task_tool())
        .task_store(store)
        .build()
        .expect("server builds");
    let server = Arc::new(Mutex::new(server));

    // --- Bundled IdP building block (the demo's IdP, plan 04 reuses this). ---
    let idp = Arc::new(InMemoryOAuthProvider::new("http://127.0.0.1"));

    // --- ONE merged router on ONE origin. ---
    let app = build_merged_router(server, idp);

    // --- Bind an ephemeral port; the listener is accepting before serve(). ---
    let bind: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(bind).await.expect("bind");
    let bound = listener.local_addr().expect("local_addr");
    let server_task = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });

    let base = format!("http://{bound}");
    let client = reqwest::Client::new();

    // --- Assert the OAuth GET /oauth2/authorize route responds. ---
    let authorize = client
        .get(format!("{base}/oauth2/authorize"))
        .send()
        .await
        .expect("authorize request");
    assert!(
        authorize.status().is_success(),
        "GET /oauth2/authorize must respond on the merged origin, got {}",
        authorize.status()
    );

    // --- Assert the OAuth POST /oauth2/token route responds. ---
    let token = client
        .post(format!("{base}/oauth2/token"))
        .send()
        .await
        .expect("token request");
    assert!(
        token.status().is_success(),
        "POST /oauth2/token must respond on the merged origin, got {}",
        token.status()
    );

    // --- Assert the MCP POST `/` endpoint responds on the SAME origin. ---
    // An `initialize` JSON-RPC request proves the MCP router is live and merged
    // (a non-empty JSON-RPC response, NOT a 404 from a missing route).
    let init = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "spike", "version": "1.0.0" }
        }
    });
    let mcp = client
        .post(&base)
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .json(&init)
        .send()
        .await
        .expect("mcp initialize request");
    assert!(
        mcp.status().is_success(),
        "MCP POST / (initialize) must respond on the merged origin, got {}",
        mcp.status()
    );
    let body: serde_json::Value = mcp.json().await.expect("mcp body is json");
    assert_eq!(
        body["jsonrpc"], "2.0",
        "MCP endpoint must return a JSON-RPC response, got: {body}"
    );

    server_task.abort();
}
