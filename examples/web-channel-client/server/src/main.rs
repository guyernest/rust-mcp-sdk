//! # Bundled offline demo server (Phase 103, D-04 / D-05)
//!
//! The NATIVE half of the `web-channel-client` example. It runs **fully offline**
//! — no external accounts, network, or secrets — and stands up, on ONE origin:
//!
//! 1. A bundled OAuth 2.0 IdP (`InMemoryOAuthProvider`) serving `GET /oauth2/authorize`
//!    + `POST /oauth2/token` (the routes the browser PKCE flow redirects to), and
//! 2. The MCP `StreamableHttpServer` router (`pmcp::axum::router`) which validates the
//!    bearer the IdP minted and serves the `tasks/*` lifecycle.
//!
//! The two are composed with `axum::Router::merge` — the EXACT public route-merge seam
//! the Wave-0 spike named (`103-SPIKE.md` Open Question 1; durable proof in
//! `tests/web_channel_oauth_route_merge_spike.rs`). `build_mcp_router` is NOT the seam.
//!
//! ## The novel long-running task (D-05)
//!
//! The `slow_summarize` tool returns `status: "working"` with **no nested result**, so
//! the SDK create-path mints a `Working` task in the store (it auto-completes ONLY when
//! the tool returns a nested `result`; see `task_dispatch.rs:260-275`). A background
//! `tokio::spawn` updater then discovers that minted task, sleeps a few seconds to
//! simulate work, and transitions it `Working -> Completed` via `set_result` +
//! `update_status`. This is what lets the browser poll `tasks/get` several times and
//! demonstrate `tasks/cancel` before the task completes.
//!
//! ## MEDIUM-6 — task-discovery race / single-user limitation (DOCUMENTED)
//!
//! The background updater must find the EXACT task the create-path minted. The ideal
//! design correlates the minted task to a UNIQUE marker the tool sets (a task variable
//! / echoed correlation id). **The SDK create-path cannot carry such a marker this
//! phase**: the tool handler never sees the store-minted id, and
//! `build_task_created_response` (`task_dispatch.rs:225-288`) propagates ONLY the `ttl`
//! and the terminal result from the tool's returned value onto the store-minted `Task`
//! — there is no task-variable / metadata field on `Task` (`types/tasks.rs:94-112`) the
//! tool can write and the updater can filter by.
//!
//! Therefore this demo is constrained to **single-user / no-concurrency** for the
//! delayed task, and that limitation is documented here, in the plan threat model
//! (`T-103-RACE`), and flagged for the plan-05 README. To make discovery as race-safe
//! as the surface allows WITHOUT a marker, the tool snapshots the owner's existing
//! `Working` task ids BEFORE returning, and the updater completes the SINGLE new
//! `Working` id that appears after the snapshot — never "most recent Working", which
//! could complete a concurrent client's task. If two of the SAME owner's tasks are
//! created concurrently the updater declines to guess (it completes nothing rather than
//! the wrong one), preserving correctness at the cost of liveness in that edge case.
//!
//! Run with: `cargo run --manifest-path examples/web-channel-client/server/Cargo.toml`

#![cfg(not(target_arch = "wasm32"))]

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::Query;
use axum::response::{IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::{Form, Json, Router};
use serde::Deserialize;

use pmcp::server::auth::oauth2::{
    GrantType, InMemoryOAuthProvider, OAuthClient, ResponseType, TokenRequest,
};
use pmcp::server::auth::traits::{AuthContext, AuthProvider};
use pmcp::server::auth::OAuthProvider;
use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
use pmcp::server::typed_tool::TypedTool;
use pmcp::types::tasks::TaskStatus;
use pmcp::types::{CallToolResult, Content, TaskSupport, ToolExecution};
use pmcp::{Result, Server};
use tokio::sync::Mutex;

/// The single demo identity the bundled IdP authenticates. The owner the task
/// create-path scopes to is `AuthContext.subject`, which this server maps from the
/// IdP-minted `TokenInfo.user_id` (see [`BearerAuthAdapter`]). The background updater
/// lists `store.list(DEMO_USER_ID)` for exactly this owner string.
const DEMO_USER_ID: &str = "demo-user";

/// The OAuth client id the browser PKCE flow registers under.
const DEMO_CLIENT_ID: &str = "web-channel-client";

/// The single redirect URI the demo client is registered with. The browser example
/// serves itself on loopback, so the IdP accepts ONLY this callback. Used BOTH at client
/// registration AND as the allowlist `oauth_authorize` validates the client-supplied
/// `redirect_uri` against (T-103-OPENREDIR), so a code is never delivered to an arbitrary
/// attacker URL.
const DEMO_REDIRECT_URI: &str = "http://127.0.0.1:8080/callback";

/// How long the demo task stays `Working` before the updater completes it. Long enough
/// for the browser to poll `tasks/get` several times and to demonstrate `tasks/cancel`.
const TASK_WORK_DURATION: Duration = Duration::from_secs(3);

/// Bearer-validating [`AuthProvider`] adapter over the bundled IdP.
///
/// The SDK's MCP request path calls [`AuthProvider::validate_request`] with the
/// `Authorization` header (`streamable_http_server.rs:758-765`); there is no public
/// concrete bearer `AuthProvider` in the SDK (only proxy/no-op variants), so the demo
/// supplies this thin adapter. It validates the bearer via
/// [`InMemoryOAuthProvider::validate_token`] and maps the resulting
/// `TokenInfo.user_id` onto `AuthContext.subject` — the exact owner string the task
/// create-path scopes the minted task to (`103-SPIKE.md` Open Question 2).
struct BearerAuthAdapter {
    idp: Arc<InMemoryOAuthProvider>,
}

#[async_trait::async_trait]
impl AuthProvider for BearerAuthAdapter {
    async fn validate_request(
        &self,
        authorization_header: Option<&str>,
    ) -> Result<Option<AuthContext>> {
        // Require a Bearer token — an absent/non-Bearer header is rejected (T-103-auth).
        let token = authorization_header
            .and_then(|h| h.strip_prefix("Bearer "))
            .ok_or_else(|| pmcp::Error::internal("missing or malformed Authorization: Bearer"))?;

        let info = self.idp.validate_token(token).await?;

        // Map the IdP principal onto the SDK's provider-agnostic AuthContext. The
        // `subject` is the task owner the create-path keys on.
        let mut ctx = AuthContext::new(info.user_id);
        ctx.scopes = info.scopes;
        ctx.client_id = Some(info.client_id);
        ctx.expires_at = Some(info.expires_at);
        ctx.token = Some(token.to_string());
        Ok(Some(ctx))
    }
}

/// Query parameters the browser sends to `GET /oauth2/authorize` (PKCE: `code_challenge`
/// + `S256`). Only the fields the demo IdP needs are captured.
#[derive(Debug, Deserialize)]
struct AuthorizeQuery {
    redirect_uri: String,
    #[serde(default)]
    state: Option<String>,
    code_challenge: String,
    #[serde(default)]
    scope: Option<String>,
}

/// Build the bundled IdP and pre-register the single demo client (so the browser's
/// `redirect_uri` validates and `exchange_code` accepts the demo `client_id`).
async fn build_idp() -> Result<Arc<InMemoryOAuthProvider>> {
    let idp = InMemoryOAuthProvider::new("http://127.0.0.1");
    idp.register_client(OAuthClient {
        client_id: DEMO_CLIENT_ID.to_string(),
        client_secret: None,
        client_name: "Web Channel Demo Client".to_string(),
        // The browser example serves itself on loopback; accept ONLY the loopback callback.
        redirect_uris: vec![DEMO_REDIRECT_URI.to_string()],
        grant_types: vec![GrantType::AuthorizationCode],
        response_types: vec![ResponseType::Code],
        scopes: vec!["read".to_string(), "write".to_string()],
        metadata: Default::default(),
    })
    .await?;
    Ok(Arc::new(idp))
}

/// `GET /oauth2/authorize`: validate the client-supplied `redirect_uri` against the
/// registered allowlist, then mint an authorization code bound to the PKCE `S256`
/// challenge and 302-redirect back to that (allowlisted) `redirect_uri` with
/// `?code=&state=`.
///
/// **T-103-OPENREDIR:** `InMemoryOAuthProvider::create_authorization_code` does NOT itself
/// check the supplied `redirect_uri` against the client's registered whitelist, so this
/// endpoint performs that check explicitly: a `redirect_uri` that is not the registered
/// [`DEMO_REDIRECT_URI`] is rejected with `400` WITHOUT any redirect, so an attacker can
/// never have the authorization code delivered to an arbitrary URL.
async fn oauth_authorize(
    idp: Arc<InMemoryOAuthProvider>,
    query: AuthorizeQuery,
) -> std::result::Result<Redirect, axum::http::StatusCode> {
    // T-103-OPENREDIR: only ever mint+redirect for the registered callback. Any other
    // redirect_uri is rejected (400, no redirect) so a code is never sent to an attacker URL.
    if query.redirect_uri != DEMO_REDIRECT_URI {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }

    let scopes = query
        .scope
        .as_deref()
        .unwrap_or("read")
        .split_whitespace()
        .map(str::to_string)
        .collect();

    let code = idp
        .create_authorization_code(
            DEMO_CLIENT_ID,
            DEMO_USER_ID,
            &query.redirect_uri,
            scopes,
            Some(query.code_challenge),
            Some("S256".to_string()),
        )
        .await
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;

    // WR-03: percent-encode the query values so a `state` containing `&`, `=`, `#`, or
    // non-ASCII cannot produce a malformed redirect URL. `redirect_uri` is the validated
    // allowlisted callback (no existing query string), so a bare `?` join is safe here.
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    serializer.append_pair("code", &code);
    if let Some(state) = query.state.as_deref() {
        serializer.append_pair("state", state);
    }
    let location = format!("{}?{}", query.redirect_uri, serializer.finish());
    Ok(Redirect::to(&location))
}

/// `POST /oauth2/token`: exchange the authorization code (with the PKCE `code_verifier`)
/// for a bearer access token. `exchange_code` runs `verify_pkce` so the code alone — without
/// the verifier — cannot be redeemed (T-103-PKCE-SRV).
async fn oauth_token(
    idp: Arc<InMemoryOAuthProvider>,
    request: TokenRequest,
) -> std::result::Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let token = idp
        .exchange_code(&request)
        .await
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;
    serde_json::to_value(token)
        .map(Json)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
}

/// Hand-wire the two IdP routes onto an `axum::Router` that drives the bundled IdP.
fn oauth_routes(idp: Arc<InMemoryOAuthProvider>) -> Router {
    let authorize_idp = idp.clone();
    let token_idp = idp;
    Router::new()
        .route(
            "/oauth2/authorize",
            get(move |Query(query): Query<AuthorizeQuery>| {
                let idp = authorize_idp.clone();
                async move { oauth_authorize(idp, query).await.into_response() }
            }),
        )
        .route(
            "/oauth2/token",
            post(move |Form(request): Form<TokenRequest>| {
                let idp = token_idp.clone();
                async move { oauth_token(idp, request).await.into_response() }
            }),
        )
}

/// Snapshot the ids of an owner's currently-`Working` tasks. Used by the tool to record
/// "what existed BEFORE me" so the updater can identify the single NEW Working task it
/// minted (MEDIUM-6 race-narrowing without a marker).
async fn working_task_ids(store: &Arc<dyn TaskStore>, owner: &str) -> HashSet<String> {
    match store.list(owner, None).await {
        Ok((tasks, _cursor)) => tasks
            .into_iter()
            .filter(|t| t.status == TaskStatus::Working)
            .map(|t| t.task_id)
            .collect(),
        Err(_) => HashSet::new(),
    }
}

/// Background updater: complete the single new `Working` task that appeared for `owner`
/// after `before` was snapshotted.
///
/// MEDIUM-6: this discovers the minted task WITHOUT guessing "most recent Working".
/// It waits briefly for the create-path to land the task, then diffs the owner's current
/// Working ids against the pre-create snapshot. Exactly-one new id => complete it.
/// Zero new ids (not landed yet) => retry within a bounded window. More than one new id
/// (concurrent same-owner creates — the documented single-user limitation) => decline to
/// guess and complete NOTHING, so a concurrent client's task is never wrongly completed.
async fn complete_delayed_task(store: Arc<dyn TaskStore>, owner: String, before: HashSet<String>) {
    // Bounded wait for the create-path to mint the task (no unbounded loop).
    let mut minted_id: Option<String> = None;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        let now = working_task_ids(&store, &owner).await;
        let new_ids: Vec<String> = now.difference(&before).cloned().collect();
        match new_ids.as_slice() {
            [] => continue,
            [only] => {
                minted_id = Some(only.clone());
                break;
            },
            _ => {
                // Concurrent same-owner creates: cannot safely correlate without a
                // marker (MEDIUM-6 limitation). Decline rather than risk the wrong task.
                tracing::warn!(
                    owner = %owner,
                    "multiple new Working tasks for one owner; declining to auto-complete (MEDIUM-6 single-user limitation)"
                );
                return;
            },
        }
    }

    let Some(task_id) = minted_id else {
        tracing::warn!(owner = %owner, "minted task not observed within the bounded window");
        return;
    };

    // Simulate multi-second work so the browser polls Working several times.
    tokio::time::sleep(TASK_WORK_DURATION).await;

    let result = CallToolResult::new(vec![Content::text(
        "summary: processed the request and produced a 3-point summary",
    )]);
    if let Err(e) = store.set_result(&task_id, &owner, result).await {
        tracing::error!(%task_id, error = %e, "failed to persist terminal result");
        return;
    }
    if let Err(e) = store
        .update_status(&task_id, &owner, TaskStatus::Completed, None)
        .await
    {
        tracing::error!(%task_id, error = %e, "failed to transition task to Completed");
    }
}

/// Build the `slow_summarize` task tool (D-05).
///
/// The handler returns `status: "working"` with NO nested `result`, so the create-path
/// mints a `Working` task (Pitfall 3). Before returning it snapshots the owner's existing
/// Working ids and spawns the background updater with that snapshot, so the updater can
/// identify the single NEW Working id (MEDIUM-6).
fn slow_summarize_tool(store: Arc<dyn TaskStore>) -> impl pmcp::ToolHandler {
    TypedTool::new_with_schema(
        "slow_summarize",
        serde_json::json!({
            "type": "object",
            "properties": { "text": { "type": "string" } }
        }),
        move |_args: serde_json::Value, extra| {
            let store = store.clone();
            Box::pin(async move {
                // Owner the create-path will scope the minted task to: AuthContext.subject
                // (the IdP user_id), or "local" when unauthenticated (matches resolve_owner).
                let owner = extra
                    .auth_context()
                    .map(|ctx| ctx.subject.clone())
                    .unwrap_or_else(|| "local".to_string());

                // Snapshot existing Working tasks BEFORE the create lands, then spawn the
                // marker-free, race-narrowed updater (MEDIUM-6).
                let before = working_task_ids(&store, &owner).await;
                tokio::spawn(complete_delayed_task(store.clone(), owner, before));

                // Return a Working task shape with NO nested result: the store mints a
                // Working task and the background updater completes it after the delay.
                Ok(serde_json::json!({
                    "taskId": "tool-fabricated",
                    "status": "working",
                    "ttl": 60000,
                    "createdAt": "2026-06-30T00:00:00Z",
                    "lastUpdatedAt": "2026-06-30T00:00:00Z"
                }))
            })
        },
    )
    .with_description("Summarize text as a multi-second MCP Task (Working -> Completed)")
    .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required))
}

/// Build the merged `axum::Router`: the public MCP router (`pmcp::axum::router`) merged
/// with the bundled IdP's `/oauth2/*` routes onto ONE origin (HIGH-3, the spike-named
/// public seam — `axum::Router::merge`). The MCP router enforces the bearer via the
/// `auth_provider` set on the `Server`.
fn build_merged_router(server: Arc<Mutex<Server>>, idp: Arc<InMemoryOAuthProvider>) -> Router {
    let mcp_router = pmcp::axum::router(server);
    mcp_router.merge(oauth_routes(idp))
}

/// Assemble the demo server: store-backed MCP `Server` with the bearer adapter +
/// `slow_summarize` tool, plus the bundled IdP, merged onto one router.
async fn build_demo_router() -> Result<Router> {
    let idp = build_idp().await?;

    let store = Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>;
    let auth = BearerAuthAdapter { idp: idp.clone() };

    let server = Server::builder()
        .name("web-channel-demo")
        .version("1.0.0")
        .tool("slow_summarize", slow_summarize_tool(store.clone()))
        .task_store(store) // presence of a store auto-advertises the `tasks` capability
        .auth_provider(auth) // bearer validation on every MCP request (T-103-auth)
        .build()?;
    let server = Arc::new(Mutex::new(server));

    Ok(build_merged_router(server, idp))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "web_channel_demo_server=info,pmcp=info".into()),
        )
        .init();

    let app = build_demo_router().await?;

    // Bind a fixed loopback port the browser example points at (plan 05). Use an env
    // override for flexibility; default to 8787 to avoid clashing with the client's 8080.
    let port: u16 = std::env::var("WEB_CHANNEL_SERVER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8787);
    let bind: SocketAddr = ([127, 0, 0, 1], port).into();

    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(|e| pmcp::Error::internal(format!("bind {bind}: {e}")))?;
    let bound = listener
        .local_addr()
        .map_err(|e| pmcp::Error::internal(format!("local_addr: {e}")))?;

    tracing::info!("web-channel demo server (MCP + OAuth2 IdP) listening on http://{bound}");
    tracing::info!("  IdP authorize: http://{bound}/oauth2/authorize");
    tracing::info!("  IdP token:     http://{bound}/oauth2/token");
    tracing::info!("  MCP endpoint:  http://{bound}/");

    axum::serve(listener, app)
        .await
        .map_err(|e| pmcp::Error::internal(format!("serve: {e}")))?;
    Ok(())
}
