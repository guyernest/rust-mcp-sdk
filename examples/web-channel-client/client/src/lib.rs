//! # Browser WASM MCP client (Phase 103, the EXAMPLE deliverable / SC-3)
//!
//! This is the WASM half of the `web-channel-client` example — the "strong
//! reference" sibling to `examples/wasm-client` that the pmcp.run team lifts to
//! build a web-application **channel**. It lives in its OWN cdylib crate
//! (HIGH-2 package split), separate from the native demo server crate
//! (`../server`), so this wasm build pulls **no** native HTTP deps
//! (`pmcp` is taken with `default-features = false, features = ["wasm"]`).
//!
//! What it demonstrates (both browser-drivable over plain HTTP/Fetch today):
//!
//! 1. **OAuth via browser PKCE** (D-01/D-06/D-07) — a full-page redirect
//!    Authorization Code + PKCE flow built on the `pmcp` PKCE helper. The
//!    `code_verifier`, CSRF `state`, and resulting bearer token live in
//!    `sessionStorage`. The browser-specific orchestration (authorize-URL
//!    assembly, `?code=&state=` handling, Fetch token exchange) lives HERE in
//!    the example, NOT in the SDK (D-03).
//! 2. **MCP Tasks lifecycle over Fetch** (D-08/D-09) — driven through the
//!    **high-level** [`pmcp::Client`] over the fixed [`WasmHttpTransport`], so
//!    all four typed task helpers (`call_tool_with_task`, `tasks_get`,
//!    `tasks_result`, `tasks_cancel`) work in the browser. `main.js` drives an
//!    explicit 500 ms poll loop + a Cancel button against the methods exposed
//!    here.
//!
//! The bearer is threaded into the transport via `extra_headers`
//! (`Authorization: Bearer <token>`), which `WasmHttpTransport` injects on every
//! Fetch (see `src/shared/wasm_http.rs`).

#![cfg(target_arch = "wasm32")]

use std::cell::RefCell;
use std::rc::Rc;

use pmcp::client::Client;
use pmcp::shared::pkce::{code_challenge_s256, generate_code_verifier, generate_state};
use pmcp::types::ClientCapabilities;
use pmcp::{ToolCallResponse, WasmHttpConfig, WasmHttpTransport};
use serde::Serialize;
use serde_json::Value;
use wasm_bindgen::prelude::*;
use web_sys::Storage;

// ---------------------------------------------------------------------------
// Wasm error boilerplate (mirrors examples/wasm-client/src/lib.rs:25-69)
// ---------------------------------------------------------------------------

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "Error")]
    pub type JsError;
}

#[wasm_bindgen(module = "/src/utils.js")]
extern "C" {
    #[wasm_bindgen(js_name = "newError")]
    fn new_js_error(message: String, code: Option<i32>, data: JsValue) -> JsError;
}

#[derive(Serialize)]
struct StructuredError {
    message: String,
    code: Option<i32>,
    data: Option<Value>,
}

impl From<pmcp::Error> for StructuredError {
    fn from(err: pmcp::Error) -> Self {
        match err {
            pmcp::Error::Protocol {
                code,
                message,
                data,
            } => Self {
                message,
                code: Some(code.as_i32()),
                data,
            },
            other => Self {
                message: other.to_string(),
                code: None,
                data: None,
            },
        }
    }
}

fn to_js_error(err: pmcp::Error) -> JsValue {
    let structured: StructuredError = err.into();
    let data = serde_wasm_bindgen::to_value(&structured.data).unwrap_or(JsValue::NULL);
    new_js_error(structured.message, structured.code, data).into()
}

/// Build a plain JS `Error` from a message (no pmcp::Error in hand).
fn js_error(message: impl Into<String>) -> JsValue {
    new_js_error(message.into(), None, JsValue::NULL).into()
}

// ---------------------------------------------------------------------------
// sessionStorage helpers (D-06): origin-scoped, cleared on tab close.
// ---------------------------------------------------------------------------

/// Keys under which the PKCE flow stores its transient secrets in sessionStorage.
const KEY_VERIFIER: &str = "pmcp_pkce_verifier";
const KEY_STATE: &str = "pmcp_oauth_state";
const KEY_TOKEN: &str = "pmcp_bearer_token";

/// Get the window's `sessionStorage`. Errors if the DOM/storage is unavailable.
fn session_storage() -> std::result::Result<Storage, JsValue> {
    let window = web_sys::window().ok_or_else(|| js_error("no window object available"))?;
    window
        .session_storage()
        .map_err(|e| js_error(format!("sessionStorage access failed: {e:?}")))?
        .ok_or_else(|| js_error("sessionStorage is unavailable"))
}

fn storage_set(key: &str, value: &str) -> std::result::Result<(), JsValue> {
    session_storage()?
        .set_item(key, value)
        .map_err(|e| js_error(format!("sessionStorage set {key} failed: {e:?}")))
}

fn storage_get(key: &str) -> std::result::Result<Option<String>, JsValue> {
    session_storage()?
        .get_item(key)
        .map_err(|e| js_error(format!("sessionStorage get {key} failed: {e:?}")))
}

fn storage_remove(key: &str) -> std::result::Result<(), JsValue> {
    session_storage()?
        .remove_item(key)
        .map_err(|e| js_error(format!("sessionStorage remove {key} failed: {e:?}")))
}

// ---------------------------------------------------------------------------
// WasmClient
// ---------------------------------------------------------------------------

/// Browser MCP client: PKCE login orchestration + high-level task lifecycle.
///
/// The OAuth `client_id` and `redirect_uri` are not held on the struct — JS passes
/// them in at each `begin_login`/`complete_login` call (the IdP binds the code to the
/// `redirect_uri` supplied at the token exchange), so there is no per-tab identity to cache.
#[wasm_bindgen]
pub struct WasmClient {
    /// High-level MCP client over the fixed Fetch transport (set by `connect`).
    ///
    /// Held in a `RefCell` so that EVERY exported method can take `&self`. That is
    /// the re-entrancy guard: under wasm-bindgen an exported `async fn(&mut self)`
    /// holds a MUTABLE borrow of the JS object for the whole lifetime of the
    /// returned promise, so a second call while one is in flight — e.g. a
    /// load-time auto-reconnect (`connect`) overlapping a user's Login click —
    /// aborts the module with "recursive use of an object detected which would
    /// lead to unsafe aliasing in rust". With `&self` methods wasm-bindgen only
    /// ever takes SHARED borrows, so overlap can never trip that check; genuine
    /// contention for the connected client is instead funnelled through
    /// `try_borrow`/`try_borrow_mut` on this cell and surfaced as a graceful
    /// "client busy" error.
    ///
    /// The client is stored behind an `Rc` so a task method can clone the handle
    /// out under a brief synchronous borrow and then `.await` on the clone — the
    /// `RefCell` borrow is never held across a suspension point.
    client: RefCell<Option<Rc<Client<WasmHttpTransport>>>>,
}

impl Default for WasmClient {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl WasmClient {
    /// Construct the client and install the panic hook + tracing.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_error_panic_hook::set_once();

        static INIT_TRACING: std::sync::Once = std::sync::Once::new();
        INIT_TRACING.call_once(|| {
            tracing_wasm::set_as_global_default();
        });

        Self {
            client: RefCell::new(None),
        }
    }

    /// Begin the OAuth Authorization Code + PKCE flow (D-01/D-07).
    ///
    /// Generates a fresh PKCE `code_verifier`/S256 `code_challenge` and a CSRF
    /// `state` via the `pmcp` PKCE helper, stores the verifier + state in
    /// `sessionStorage` (so they survive the full-page redirect), and returns
    /// the authorize URL for JS to navigate the browser to
    /// (`window.location = authorize_url`).
    #[wasm_bindgen]
    pub fn begin_login(
        &self,
        authorize_base: String,
        client_id: String,
        redirect_uri: String,
    ) -> std::result::Result<String, JsValue> {
        let verifier = generate_code_verifier().map_err(to_js_error)?;
        let challenge = code_challenge_s256(&verifier);
        let state = generate_state().map_err(to_js_error)?;

        storage_set(KEY_VERIFIER, &verifier)?;
        storage_set(KEY_STATE, &state)?;

        // Assemble the authorize URL HERE (D-01) — append response_type,
        // client_id, redirect_uri, the S256 challenge, and state. Mirrors the
        // native param set at src/client/oauth.rs:660-672 (do NOT cfg-port that
        // reqwest/tokio-bound module).
        let url = web_sys::Url::new(&authorize_base)
            .map_err(|e| js_error(format!("invalid authorize URL: {e:?}")))?;
        let params = url.search_params();
        params.append("response_type", "code");
        params.append("client_id", &client_id);
        params.append("redirect_uri", &redirect_uri);
        params.append("code_challenge", &challenge);
        params.append("code_challenge_method", "S256");
        params.append("state", &state);
        url.set_search(&String::from(params.to_string()));

        Ok(url.href())
    }

    /// Complete the flow on redirect return (D-07): validate `state` against the
    /// value stashed in `sessionStorage` (CSRF check, T-103-CSRF), then Fetch
    /// POST the authorization `code` + PKCE `code_verifier` to `/oauth2/token`
    /// and stash the resulting bearer.
    ///
    /// The token request is sent as `application/x-www-form-urlencoded` with
    /// `grant_type=authorization_code`, `code`, `code_verifier`, `redirect_uri`,
    /// and `client_id` — the EXACT shape the demo server's `POST /oauth2/token`
    /// (`Form<TokenRequest>`, oauth2.rs:256) decodes. The JSON response carries
    /// an `access_token` field (`AccessToken`, oauth2.rs:108) which is parsed
    /// and stored as the bearer.
    #[wasm_bindgen]
    pub async fn complete_login(
        &self,
        token_url: String,
        code: String,
        state: String,
        client_id: String,
        redirect_uri: String,
    ) -> std::result::Result<(), JsValue> {
        // CSRF: the returned state MUST equal the state we generated (T-103-CSRF).
        let expected = storage_get(KEY_STATE)?
            .ok_or_else(|| js_error("no stored OAuth state — start login again"))?;
        if state != expected {
            return Err(js_error("OAuth state mismatch — possible CSRF, aborting"));
        }

        let verifier = storage_get(KEY_VERIFIER)?
            .ok_or_else(|| js_error("no stored PKCE verifier — start login again"))?;

        // WR-01: the verifier and CSRF state are single-use. Consume them from
        // sessionStorage NOW — before the exchange — so that a FAILED exchange cannot
        // leave reusable secrets behind, and any retry is forced through a fresh
        // `begin_login` (which mints a new verifier + state). We already hold the
        // verifier in a local, so the exchange below still has what it needs.
        storage_remove(KEY_VERIFIER)?;
        storage_remove(KEY_STATE)?;

        let access_token =
            exchange_code(&token_url, &code, &verifier, &redirect_uri, &client_id).await?;

        storage_set(KEY_TOKEN, &access_token)?;

        Ok(())
    }

    /// `true` if a bearer token is stored (i.e. login completed in this tab).
    #[wasm_bindgen]
    pub fn is_logged_in(&self) -> std::result::Result<bool, JsValue> {
        Ok(storage_get(KEY_TOKEN)?.is_some())
    }

    /// Connect to the MCP endpoint with the stored bearer (D-08).
    ///
    /// Builds a high-level [`pmcp::Client`] over the fixed [`WasmHttpTransport`],
    /// threading the bearer into `extra_headers` as `Authorization: Bearer ...`
    /// (injected by the transport on every Fetch), then runs `initialize`.
    #[wasm_bindgen]
    pub async fn connect(&self, url: String) -> std::result::Result<(), JsValue> {
        let token = storage_get(KEY_TOKEN)?
            .ok_or_else(|| js_error("not logged in — complete the PKCE flow first"))?;

        let config = WasmHttpConfig {
            url,
            extra_headers: vec![("Authorization".to_string(), format!("Bearer {token}"))],
        };
        // Build and initialize a LOCAL client — the handshake `.await` runs before
        // the client is stored, so no borrow of `self.client` is held across it.
        let mut client = Client::new(WasmHttpTransport::new(config));
        client
            .initialize(ClientCapabilities::default())
            .await
            .map_err(to_js_error)?;
        // Publish the connected client. `try_borrow_mut` fails only if another
        // operation currently holds the cell (a concurrent task call) — surface
        // that as a busy error rather than panicking.
        *self
            .client
            .try_borrow_mut()
            .map_err(|_| js_error("client busy — another operation is in progress"))? =
            Some(Rc::new(client));
        Ok(())
    }

    /// Invoke the long-running tool as an MCP Task (D-09). Returns the
    /// store-minted `task_id` to poll. If the server answered synchronously
    /// (no task) this returns an error directing the caller to inspect the
    /// immediate result instead.
    #[wasm_bindgen]
    pub async fn invoke_task(
        &self,
        name: String,
        args: JsValue,
    ) -> std::result::Result<String, JsValue> {
        let arguments: Value = if args.is_null() || args.is_undefined() {
            Value::Object(serde_json::Map::new())
        } else {
            serde_wasm_bindgen::from_value(args)?
        };
        let client = self.connected_client()?;
        match client
            .call_tool_with_task(name, arguments)
            .await
            .map_err(to_js_error)?
        {
            ToolCallResponse::Task(task) => Ok(task.task_id),
            ToolCallResponse::Result(_) => {
                Err(js_error("server returned a synchronous result, not a task"))
            },
        }
    }

    /// Poll a task's current status (D-09). Returns the snake_case status string
    /// (`working`, `completed`, `failed`, `cancelled`, `input_required`) so the
    /// JS poll loop can decide whether to stop.
    #[wasm_bindgen]
    pub async fn poll_task(&self, task_id: String) -> std::result::Result<String, JsValue> {
        let client = self.connected_client()?;
        let task = client.tasks_get(&task_id).await.map_err(to_js_error)?;
        Ok(task.status.to_string())
    }

    /// Fetch a completed task's result as JSON (call once the status is terminal).
    #[wasm_bindgen]
    pub async fn task_result(&self, task_id: String) -> std::result::Result<JsValue, JsValue> {
        let client = self.connected_client()?;
        let result = client.tasks_result(&task_id).await.map_err(to_js_error)?;
        serde_wasm_bindgen::to_value(&result).map_err(Into::into)
    }

    /// Cancel a running task (D-09, the Cancel button). Returns the resulting
    /// status string.
    #[wasm_bindgen]
    pub async fn cancel_task(&self, task_id: String) -> std::result::Result<String, JsValue> {
        let client = self.connected_client()?;
        let task = client.tasks_cancel(&task_id).await.map_err(to_js_error)?;
        Ok(task.status.to_string())
    }

    /// Clear the stored bearer (and the transient PKCE secrets), e.g. on logout.
    #[wasm_bindgen]
    pub fn logout(&self) -> std::result::Result<(), JsValue> {
        for key in [KEY_TOKEN, KEY_VERIFIER, KEY_STATE] {
            storage_remove(key)?;
        }
        // Best-effort drop of the connected client. If an operation is in flight
        // (cell borrowed), the tokens are already cleared and the client is
        // dropped when that op completes — no panic, no blocking.
        if let Ok(mut slot) = self.client.try_borrow_mut() {
            *slot = None;
        }
        Ok(())
    }
}

impl WasmClient {
    /// Clone out an `Rc` handle to the connected client for the duration of one
    /// task call. The `RefCell` borrow is released before the caller `.await`s (so
    /// no borrow is held across a suspension point), and a concurrent `connect`
    /// or `logout` (which want `try_borrow_mut`) degrades to a graceful "client
    /// busy" error rather than a `RefCell` panic. `None` maps to "not connected".
    fn connected_client(
        &self,
    ) -> std::result::Result<Rc<Client<WasmHttpTransport>>, JsValue> {
        self.client
            .try_borrow()
            .map_err(|_| js_error("client busy — another operation is in progress"))?
            .as_ref()
            .cloned()
            .ok_or_else(|| js_error("not connected — call connect() after login"))
    }
}

/// Fetch POST the token exchange and parse the `access_token` from the JSON
/// response. Kept free of `WasmClient` state so the method stays small.
async fn exchange_code(
    token_url: &str,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
    client_id: &str,
) -> std::result::Result<String, JsValue> {
    use web_sys::{Headers, Request, RequestInit, Response};

    let window = web_sys::window().ok_or_else(|| js_error("no window object available"))?;

    // application/x-www-form-urlencoded body matching Form<TokenRequest>.
    let form = web_sys::UrlSearchParams::new()
        .map_err(|e| js_error(format!("URLSearchParams failed: {e:?}")))?;
    form.append("grant_type", "authorization_code");
    form.append("code", code);
    form.append("code_verifier", verifier);
    form.append("redirect_uri", redirect_uri);
    form.append("client_id", client_id);
    let body = String::from(form.to_string());

    let headers = Headers::new().map_err(|e| js_error(format!("Headers failed: {e:?}")))?;
    headers
        .set("Content-Type", "application/x-www-form-urlencoded")
        .map_err(|e| js_error(format!("set Content-Type failed: {e:?}")))?;
    headers
        .set("Accept", "application/json")
        .map_err(|e| js_error(format!("set Accept failed: {e:?}")))?;

    let init = RequestInit::new();
    init.set_method("POST");
    init.set_headers(&headers);
    init.set_body(&JsValue::from_str(&body));

    let request = Request::new_with_str_and_init(token_url, &init)
        .map_err(|e| js_error(format!("build token request failed: {e:?}")))?;

    let response_value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| js_error(format!("token Fetch failed: {e:?}")))?;
    let response: Response = response_value
        .dyn_into()
        .map_err(|e| js_error(format!("invalid token response: {e:?}")))?;

    if !response.ok() {
        return Err(js_error(format!(
            "token endpoint returned HTTP {}",
            response.status()
        )));
    }

    let text_promise = response
        .text()
        .map_err(|e| js_error(format!("read token body failed: {e:?}")))?;
    let text = wasm_bindgen_futures::JsFuture::from(text_promise)
        .await
        .map_err(|e| js_error(format!("await token body failed: {e:?}")))?
        .as_string()
        .ok_or_else(|| js_error("token response body is not text"))?;

    let json: Value = serde_json::from_str(&text)
        .map_err(|e| js_error(format!("token response is not JSON: {e}")))?;
    json.get("access_token")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| js_error("token response missing access_token"))
}
