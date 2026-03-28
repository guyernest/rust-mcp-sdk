---
phase: 61-add-oauth-support-to-mcp-preview
verified: 2026-03-27T00:00:00Z
status: gaps_found
score: 6/7 must-haves verified
gaps:
  - truth: "forward_mcp returns upstream 401/403 status codes instead of wrapping them as 502"
    status: failed
    reason: "forward_raw still uses check_response() which converts ALL non-2xx responses (including 401/403) into anyhow::Error. The forward_mcp handler matches Ok/Err and returns BAD_GATEWAY for all errors. The plan specified Result<RawForwardResult, ForwardError> but the implementation retained Result<RawForwardResult>."
    artifacts:
      - path: "crates/mcp-preview/src/proxy.rs"
        issue: "forward_raw at line 544 returns Result<RawForwardResult> using check_response() at line 560, which bails on ANY non-2xx including 401/403 (line 168: anyhow::bail!)"
      - path: "crates/mcp-preview/src/handlers/api.rs"
        issue: "forward_mcp handler at line 414 matches only Ok/Err from forward_raw — Err arm returns StatusCode::BAD_GATEWAY for all errors including auth failures (line 452)"
    missing:
      - "forward_raw must detect 401/403 before calling check_response and return them separately (either via ForwardError or McpRequestError, consistent with send_request pattern)"
      - "forward_mcp must match on auth-error variant and return the upstream status code (401 or 403) instead of BAD_GATEWAY"
human_verification:
  - test: "Full OAuth popup flow end-to-end"
    expected: "Login modal appears on 401/403, popup opens, code exchanges for token, session proceeds"
    why_human: "Requires live OAuth provider and browser interaction"
  - test: "Auth status indicator in header"
    expected: "Green Authenticated label appears after login, red Auth expired + Re-login button appears on token expiry"
    why_human: "Visual state requires browser rendering"
  - test: "Re-login button in events panel on mid-session 401/403"
    expected: "Events panel shows authExpired entry with inline Re-login button that reopens the login modal"
    why_human: "Requires triggering session-level 401/403 from a live server"
---

# Phase 61: Add OAuth Support to mcp-preview — Verification Report

**Phase Goal:** Add browser-based OAuth PKCE authentication to mcp-preview so developers can test MCP Apps against OAuth-protected servers on pmcp.run, with dynamic auth header updates, login modal, and CLI flag wiring.
**Verified:** 2026-03-27
**Status:** gaps_found — 6/7 must-haves verified; 1 gap blocks goal
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| #  | Truth                                                                                           | Status      | Evidence                                                                                 |
|----|------------------------------------------------------------------------------------------------|-------------|------------------------------------------------------------------------------------------|
| 1  | McpProxy auth_header is dynamically updatable at runtime without restarting the server         | VERIFIED  | `SyncRwLock<Option<String>>` at proxy.rs:224; `set_auth_header` at :248, `has_auth_header` at :253 |
| 2  | POST /api/auth/token-exchange accepts code + code_verifier and exchanges with token endpoint   | VERIFIED  | handlers/auth.rs:38 `token_exchange`; POSTs to `oauth.token_endpoint` with PKCE params; calls `set_auth_header` at :94 |
| 3  | GET /api/auth/callback serves a self-closing HTML page that posts authorization code to opener | VERIFIED  | handlers/auth.rs:106 `callback`; includes auth-callback.html via `include_str!`; postMessage at auth-callback.html:51 |
| 4  | GET /api/auth/status returns JSON with authenticated boolean                                   | VERIFIED  | handlers/auth.rs:114 `status`; returns `{"authenticated": has_auth_header()}` |
| 5  | GET /api/config includes oauth_config when configured                                          | VERIFIED  | handlers/api.rs:70-74 maps `OAuthPreviewConfig` to `OAuthConfigResponse`; added to `ConfigResponse` at :27 |
| 6  | forward_mcp returns upstream 401/403 status codes instead of wrapping them as 502             | FAILED    | `forward_raw` uses `check_response()` which converts ALL non-2xx to `anyhow::Error`; `forward_mcp` returns `BAD_GATEWAY` for all Err variants |
| 7  | cargo pmcp preview --oauth-client-id triggers OAuthPreviewConfig creation                     | VERIFIED  | preview.rs:57-94 matches `AuthMethod::OAuth`, calls `discover_oauth_endpoints`, constructs `OAuthPreviewConfig` |

**Score:** 6/7 truths verified

---

## Required Artifacts

| Artifact                                              | Expected                                                              | Status    | Details                                                                    |
|-------------------------------------------------------|-----------------------------------------------------------------------|-----------|----------------------------------------------------------------------------|
| `crates/mcp-preview/src/proxy.rs`                    | SyncRwLock auth_header + set/has methods + McpRequestError enum       | VERIFIED  | Lines 8, 203-217, 224, 248, 253. McpRequestError has AuthRequired(u16, String) and Other(anyhow::Error). |
| `crates/mcp-preview/src/handlers/auth.rs`            | token_exchange, callback, status HTTP handlers                        | VERIFIED  | All three handlers present and substantive. token_exchange calls set_auth_header after token response. |
| `crates/mcp-preview/assets/auth-callback.html`       | OAuth popup callback page                                             | VERIFIED  | Contains `type: 'oauth-callback'`, `window.opener.postMessage`, closes popup after 1.5s |
| `crates/mcp-preview/src/server.rs`                   | OAuthPreviewConfig struct, oauth_config field on PreviewConfig        | VERIFIED  | OAuthPreviewConfig at :25-34; oauth_config: Option<OAuthPreviewConfig> at :83; Default sets None at :97 |
| `crates/mcp-preview/assets/index.html`               | OAuthManager class with PKCE, popup, login modal, auth status         | VERIFIED  | OAuthManager at :1644; _generateCodeVerifier/:1665, _generateCodeChallenge/:1673, crypto.subtle.digest/'SHA-256'/:1675, code_challenge_method: 'S256'/:1694, window.open/:1703, oauth-login-modal/:1766, oauth-sign-in-btn/:1776, auth-status-indicator/:1804, relogin-btn/:1819 |
| `cargo-pmcp/src/commands/preview.rs`                 | OAuth config wiring from AuthFlags to OAuthPreviewConfig              | VERIFIED  | discover_oauth_endpoints/:152, AuthMethod::OAuth match/:58, oauth_config constructed before resolve_auth_header/:98, graceful fallback at :101-115 |

---

## Key Link Verification

| From                                        | To                                        | Via                                         | Status    | Details                                                                  |
|---------------------------------------------|-------------------------------------------|---------------------------------------------|-----------|--------------------------------------------------------------------------|
| `handlers/auth.rs`                          | `proxy.rs`                                | `state.proxy.set_auth_header()`             | WIRED     | auth.rs:94 calls `set_auth_header(Some(format!("Bearer {}", access_token)))` |
| `server.rs`                                 | `handlers/auth.rs`                        | axum route registration `/api/auth/*`       | WIRED     | server.rs:160-162 registers token-exchange, callback, status routes |
| `index.html` OAuthManager                   | `/api/auth/token-exchange`                | `fetch` POST in `_handleCallback`           | WIRED     | index.html:1736 `fetch('/api/auth/token-exchange', { method: 'POST' })` |
| `index.html`                                | `/api/config`                             | fetch reads `oauth_config`                  | WIRED     | index.html:1949 `if (config.oauth_config) { this.oauth.setConfig(...) }` |
| `cargo-pmcp/src/commands/preview.rs`        | `mcp_preview::OAuthPreviewConfig`         | config struct construction                  | WIRED     | preview.rs:74-79 constructs OAuthPreviewConfig; passed to PreviewConfig at :129 |
| `handlers/api.rs` forward_mcp              | `proxy.rs` forward_raw                    | 401/403 propagation via ForwardError        | NOT WIRED | forward_raw returns `Result<RawForwardResult>` (anyhow); check_response bails on 401/403; forward_mcp returns BAD_GATEWAY |

---

## Requirements Coverage

No requirement IDs were declared in plan frontmatter. Phase has no formal REQUIREMENTS.md entries.

---

## Anti-Patterns Found

| File                                        | Line | Pattern                                   | Severity | Impact                                                                  |
|---------------------------------------------|------|-------------------------------------------|----------|-------------------------------------------------------------------------|
| `crates/mcp-preview/src/proxy.rs`          | 164-171 | `check_response` converts 401/403 to anyhow::Error | Blocker | forward_raw silently converts auth failures to generic errors; forward_mcp returns 502 to WASM client |
| `crates/mcp-preview/src/handlers/api.rs`   | 452  | `Err(e) => (StatusCode::BAD_GATEWAY, ...)` in forward_mcp | Blocker | WASM client path cannot detect 401/403 auth failures; browser receives 502 and cannot trigger re-login flow |
| `crates/mcp-preview/src/proxy.rs`          | 229  | `pub fn new(base_url: &str)` dead_code warning | Warning | Compiler emits dead_code warning; not a runtime issue but violates zero-warning policy |

---

## Scope Note: WASM Path vs Non-WASM Path

The 401/403 propagation gap only affects the WASM bridge path (`/api/mcp` via `forward_raw`/`forward_mcp`). The non-WASM paths work correctly:

- `list_tools` (via `send_request`) → `MccRequestError::AuthRequired` → HTTP 401/403 to browser (VERIFIED)
- `call_tool` (via `send_request`) → `McpRequestError::AuthRequired` → HTTP 401/403 to browser (VERIFIED)
- `list_resources` (via `send_request`) → auth error handled (VERIFIED)

The `forward_raw` path is used when the WASM client (embedded in the browser) sends raw JSON-RPC to `/api/mcp`. The 401/403 detection in `executeTool()` at index.html:2576 checks `response.status`, but since `forward_raw` → `check_response` converts 401 to an `anyhow::Error` and `forward_mcp` returns `BAD_GATEWAY (502)`, the browser will receive 502 — not 401/403 — and the auth detection condition will not trigger.

The fix requires either:
- Changing `forward_raw` to detect 401/403 before `check_response` and returning them as a typed error (analogous to `send_request`'s approach), OR
- Changing `forward_mcp` to inspect the raw response status before delegating to `forward_raw`

---

## Human Verification Required

### 1. Full OAuth Popup Flow

**Test:** Start `cargo pmcp preview <oauth-protected-server> --oauth-client-id <id> --oauth-issuer <url>`. Observe that login modal appears when session fails with 401/403.
**Expected:** Modal shows "Authentication Required", clicking Sign In opens popup to authorization endpoint, after login popup closes and session proceeds normally.
**Why human:** Requires live OAuth provider, browser interaction, and popup behavior.

### 2. Auth Status Indicator

**Test:** After successful OAuth login in the browser, observe the header area.
**Expected:** A green "Authenticated" label appears. If token expires mid-session, it changes to red "Auth expired" with a Re-login button.
**Why human:** Visual state requires browser rendering and live token expiry.

### 3. Re-login Events Panel Entry

**Test:** Trigger a 401/403 during an active tool call (simulate by revoking token on server side).
**Expected:** Events panel shows an authExpired entry with inline Re-login button that opens the login modal when clicked.
**Why human:** Requires triggering mid-session auth failure from a live server.

---

## Gaps Summary

One gap blocks full goal achievement:

**401/403 propagation for the WASM bridge path** — `forward_raw` uses `check_response()` which wraps all non-2xx responses (including 401/403) as generic `anyhow::Error`. The `forward_mcp` handler then returns `BAD_GATEWAY (502)` to the browser for all errors. This means the WASM client receives a 502 when the MCP server returns 401/403, and the `executeTool()` auth-detection condition at index.html:2576 (checking `response.status === 401 || response.status === 403`) will never trigger on this path.

The fix is small and isolated to `proxy.rs` (`forward_raw`) and `handlers/api.rs` (`forward_mcp`) — analogous to how `send_request` already handles this correctly.

All other must-haves are fully implemented and wired:
- Dynamic `SyncRwLock` auth header with `set/has` methods
- Three `/api/auth/*` endpoints (token-exchange, callback, status) — substantive and wired
- Full browser-side `OAuthManager` PKCE flow (code_verifier, code_challenge, S256, popup, postMessage)
- Login modal with Sign In button triggering popup
- 401/403 detection in `initSession()` and `executeTool()` for the non-WASM path
- Auth status indicator and re-login button in events panel
- CLI OAuth flags wiring via `discover_oauth_endpoints` + `OAuthPreviewConfig`
- Both crates compile (1 dead_code warning on `McpProxy::new`)

---

_Verified: 2026-03-27_
_Verifier: Claude (gsd-verifier)_
