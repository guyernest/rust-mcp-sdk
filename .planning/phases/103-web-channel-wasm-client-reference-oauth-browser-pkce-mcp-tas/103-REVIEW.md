---
phase: 103-web-channel-wasm-client-reference-oauth-browser-pkce-mcp-tas
reviewed: 2026-06-30T00:00:00Z
depth: standard
files_reviewed: 7
files_reviewed_list:
  - src/shared/pkce.rs
  - src/shared/pending_slot.rs
  - src/shared/wasm_http.rs
  - examples/web-channel-client/server/src/main.rs
  - examples/web-channel-client/client/src/lib.rs
  - examples/web-channel-client/client/main.js
  - examples/web-channel-client/client/src/utils.js
findings:
  critical: 1
  warning: 3
  info: 2
  total: 6
status: issues_found
---

# Phase 103: Code Review Report

**Reviewed:** 2026-06-30
**Depth:** standard
**Files Reviewed:** 7
**Status:** issues_found

## Summary

Phase 103 adds a browser WASM MCP client example (OAuth2 browser-PKCE + MCP Tasks lifecycle), two reusable `pmcp` library additions (`src/shared/pkce.rs`, `src/shared/pending_slot.rs`), and a WASM HTTP transport (`src/shared/wasm_http.rs`).

The library additions (`pkce.rs`, `pending_slot.rs`, `wasm_http.rs`) are clean and correct: CSPRNG from `getrandom`, no panics, proper `Result` propagation, one-slot buffer semantics well-tested. The RFC 7636 Appendix B vector is pinned in both unit tests and property tests, and the S256 implementation matches the reference.

The example server (`main.rs`) and WASM client (`lib.rs`, `main.js`) contain one critical correctness/security bug in the demo IdP's authorize endpoint (the redirect_uri validation comment is false — the validation is not performed), two warnings in the browser client, and two info items.

---

## Critical Issues

### CR-01: `oauth_authorize` skips redirect_uri validation — docstring claim is false

**File:** `examples/web-channel-client/server/src/main.rs:157-188`

**Issue:** The `oauth_authorize` handler's docstring states:
> "The `redirect_uri` is validated against the registered client by `create_authorization_code`'s upstream `validate_authorization`; the demo does NOT echo an arbitrary client-supplied redirect (T-103-OPENREDIR)."

This claim is **false**. `create_authorization_code` in `InMemoryOAuthProvider` (inspected at `src/server/auth/oauth2.rs:507-534`) does NOT call `validate_authorization`. It stores the `redirect_uri` verbatim without checking it against the registered `redirect_uris` whitelist (line 481 of oauth2.rs is only reached via the separate `validate_authorization` path, which is called nowhere in the example).

As a result, an attacker can supply any `redirect_uri` to `GET /oauth2/authorize`, receive a 302 redirect to their controlled URL with a valid authorization code, then exchange it at `POST /oauth2/token` using the same attacker-controlled `redirect_uri` (which will pass the `exchange_code` check at oauth2.rs:580 because it matches what was stored). The PKCE `code_verifier` is the attacker's own, so the exchange succeeds and a real bearer is issued.

Because `DEMO_USER_ID` is hardcoded, the token is always for `"demo-user"` — so the attack yields a valid bearer for the single demo account, not an arbitrary account escalation. For a loopback demo server this is low operational risk, but the comment actively misleads anyone reading this as a reference implementation.

**Fix:** Either call `validate_authorization` before `create_authorization_code`, or remove the false claim from the comment and explicitly document that redirect_uri is NOT validated at the authorize step (it is only bound-and-checked at the token step). The minimal correct fix:

```rust
// In oauth_authorize, before calling create_authorization_code:
let auth_request = pmcp::server::auth::oauth2::AuthorizationRequest {
    client_id: DEMO_CLIENT_ID.to_string(),
    redirect_uri: query.redirect_uri.clone(),
    response_type: pmcp::server::auth::oauth2::ResponseType::Code,
    scope: query.scope.as_deref().unwrap_or("read").to_string(),
    state: query.state.clone(),
    code_challenge: Some(query.code_challenge.clone()),
    code_challenge_method: Some("S256".to_string()),
};
idp.validate_authorization(&auth_request)
    .await
    .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;
```

Alternatively, if the intent is a simplified demo that skips this check, update the comment to say so and reference T-103-OPENREDIR as a known limitation.

---

## Warnings

### WR-01: PKCE `code_verifier` and `state` left in sessionStorage after successful login

**File:** `examples/web-channel-client/client/src/lib.rs:232-243`

**Issue:** `complete_login` reads `KEY_VERIFIER` and `KEY_STATE` from sessionStorage, performs the state comparison and token exchange, then returns. It does NOT remove either key from sessionStorage on success. They remain until `logout()` is called. While the code verifier is single-use (the authorization code that consumes it has already been exchanged), leaving it in storage is unnecessary exposure surface: any XSS that fires after login can read both `pmcp_pkce_verifier` and `pmcp_oauth_state`, leaking ephemeral values that are no longer needed.

The docstring at line 207-217 calls `KEY_STATE` a "single-use anti-CSRF token" — single-use semantics require deletion after first use.

**Fix:** Remove both keys at the end of `complete_login` after the exchange succeeds:

```rust
// At the end of complete_login, after storage_set(KEY_TOKEN, ...):
let storage = session_storage()?;
for key in [KEY_VERIFIER, KEY_STATE] {
    storage
        .remove_item(key)
        .map_err(|e| js_error(format!("sessionStorage remove {key} failed: {e:?}")))?;
}
```

### WR-02: `onCancel` does not clear `currentTaskId` or restore button state on success

**File:** `examples/web-channel-client/client/main.js:145-157`

**Issue:** When the Cancel button fires and `client.cancel_task(currentTaskId)` succeeds, `onCancel` logs the result but does NOT:
- Clear `currentTaskId` (it remains non-null)
- Disable the Cancel button
- Re-enable the Invoke button

The already-queued `setTimeout(pollOnce, ...)` will fire on schedule, see `currentTaskId != null`, call `poll_task`, get `cancelled` (terminal), and then perform the cleanup — so the UI eventually self-corrects. However the window between cancel completion and the next scheduled poll (up to 500 ms) leaves the UI in an inconsistent state: the Cancel button is still enabled and clickable for an already-cancelled task, and `invoke_task` is still blocked. Clicking Cancel a second time during this window fires `tasks/cancel` again on an already-cancelled task.

**Fix:** In `onCancel`, clear `currentTaskId` and restore button state immediately after a successful cancel:

```js
async function onCancel() {
    if (!currentTaskId) return;
    try {
        const status = await client.cancel_task(currentTaskId);
        setTaskStatus(status);
        log(`Cancelled task -> ${status}`);
        // Restore UI immediately — don't wait for the next poll.
        currentTaskId = null;
        $('cancel-btn').disabled = true;
        $('invoke-btn').disabled = false;
    } catch (e) {
        log(`Cancel failed: ${e.message || e}`);
    }
}
```

### WR-03: `state` value echoed back in redirect URL without percent-encoding

**File:** `examples/web-channel-client/server/src/main.rs:185-187`

**Issue:** The state value from the client is appended directly into the Location redirect URL without percent-encoding:

```rust
location.push_str(&format!("&state={state}"));
```

For the specific state generated by `generate_state()` (base64url-no-pad characters `[A-Za-z0-9-_]`), this is safe because none of those characters are URL-special. However, the `AuthorizeQuery.state` field is `Option<String>` and accepts any string a client sends. A state value containing `&`, `=`, `#`, `?`, or non-ASCII characters would produce a malformed redirect URL (the browser would misparse the query parameters). Although the PKCE CSRF check in the browser client (`complete_login`) would then fail with a state mismatch, the failure mode is confusing (the redirect URL is silently malformed rather than the server returning 400).

**Fix:** Percent-encode the state value before appending it. The `urlencoding` crate is already a transitive dependency:

```rust
if let Some(state) = query.state {
    let encoded_state = urlencoding::encode(&state);
    location.push_str(&format!("&state={encoded_state}"));
}
```

Or validate that state contains only safe characters and return 400 if it does not.

---

## Info

### IN-01: Demo — sessionStorage token storage is an accepted tradeoff (acknowledged)

**File:** `examples/web-channel-client/client/src/lib.rs:96-102`

**Issue:** Bearer tokens stored in sessionStorage are accessible to any JavaScript running in the same page (XSS vector). The module docstring documents this as an explicit demo tradeoff ("sessionStorage tokens... are deliberate, documented demo choices"). Flagged here for completeness per review scope.

**Accepted tradeoff:** No change required. The sessionStorage boundary (origin-scoped, tab-lifetime) is appropriate for a demo. Production deployments should prefer HttpOnly cookies or memory-only storage with a backend token endpoint.

### IN-02: `AuthorizeQuery` silently discards `client_id` and `response_type` from the request

**File:** `examples/web-channel-client/server/src/main.rs:125-133`

**Issue:** The `AuthorizeQuery` struct does not capture `client_id` or `response_type` from the incoming GET request. Both are hardcoded to `DEMO_CLIENT_ID` and `"S256"` at the call site. A browser sending `client_id=anything-else` or `response_type=token` will receive a code as if it had sent the registered client id. Axum's query extractor silently ignores unknown fields, so malformed requests succeed silently instead of returning 400.

This is by design for a single-client demo, but differs from real OAuth2 behaviour (RFC 6749 §4.1.2.1 requires returning an error for unknown `response_type`). Callers using this as a reference for a real IdP would be misled.

**Fix:** Add `client_id: String` and `response_type: String` to `AuthorizeQuery` and validate them, or add a comment explicitly stating these fields are intentionally ignored.

---

_Reviewed: 2026-06-30_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
