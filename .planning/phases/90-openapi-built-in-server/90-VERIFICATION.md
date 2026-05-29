---
phase: 90-openapi-built-in-server
verified: 2026-05-29T23:30:00Z
status: gaps_found
score: 9/11 must-haves verified
overrides_applied: 0
gaps:
  - truth: "oauth_passthrough per-request token reaches the outbound request at runtime (OAPI-03/OAPI-05)"
    status: failed
    reason: |
      The per-request passthrough token path is architecturally complete (TokenCaptureAuthProvider
      captures the inbound token into AuthContext, request_executor() derives a per-request
      HttpCodeExecutor with the token via with_inbound_token(), OAuthPassthroughAuth::apply()
      forwards it) but the seam is dead at runtime. ScriptToolHandler::handle() and
      ExecuteCodeHandler::handle() both take `_extra: RequestHandlerExtra` (ignored) and execute
      over `self.http_exec.clone()` — the fixed instance from dispatch time with inbound_token:None.
      The request_executor() function is defined (assemble.rs:130-133), exported, and unit-tested
      (assemble.rs:407-425) but has no runtime callers: grep confirms its only references are the
      pub use, the definition, and the test. Consequence: for AuthConfig::OAuthPassthrough{required:true}
      every code-mode and script-tool request fails with Auth("authentication required but no inbound
      token was provided") even when the MCP client supplied a valid Authorization header.
      For required:false the passthrough silently no-ops (NoAuth is installed instead of
      OAuthPassthroughAuth, so the token is never forwarded). The london-tube parity test
      (OAPI-08) passes because it uses api_key auth (not oauth_passthrough), which is unaffected.
    artifacts:
      - path: "crates/pmcp-server-toolkit/src/tools.rs"
        issue: "ScriptToolHandler::handle (line 665) takes _extra and uses self.http_exec.clone() — no request_executor() call; inbound_token stays None"
      - path: "crates/pmcp-server-toolkit/src/code_mode.rs"
        issue: "ExecuteCodeHandler::handle (line 418-421) takes _extra and executes self.executor (JsCodeExecutor wrapping the dispatch-time http_exec with inbound_token:None); no per-request token threading"
      - path: "crates/pmcp-openapi-server/src/assemble.rs"
        issue: "request_executor() (line 130) is defined and tested but never called from any handler path; the seam described in the doc-comment ('binary owns this seam') is not implemented"
    missing:
      - "In ScriptToolHandler::handle: replace self.http_exec.clone() with request_executor(&self.http_exec, &extra) to produce a per-request executor with the captured inbound token threaded in"
      - "In ExecuteCodeHandler::handle: derive a per-request http_exec via request_executor, wrap it in a new JsCodeExecutor, and execute over that rather than the fixed self.executor"
      - "OR: at startup, reject AuthConfig::OAuthPassthrough{required:true} in dispatch() with a clear startup error rather than silently constructing MissingTokenAuth that fails every request — fail loud rather than fail silently (the fallback documented in the REVIEW WR-01 fix option b)"
      - "Add an end-to-end test (wiremock backend + a captured inbound token) asserting the forwarded Authorization header actually reaches the backend for the oauth_passthrough case"

  - truth: "The Shape A binary boots and serves with any of the 5 auth variants (including oauth_passthrough required:true)"
    status: failed
    reason: "Follows from the same root cause as OAPI-03/OAPI-05 gap above. A server configured with [backend.auth] type='oauth_passthrough' and required=true will fail every code-mode and script-tool request, even when clients provide the Authorization header. Static-auth variants (none/api_key/bearer/basic/oauth2_client_credentials) are fully functional."
    artifacts:
      - path: "crates/pmcp-openapi-server/src/dispatch.rs"
        issue: "dispatch() calls create_auth_provider(&backend.auth) which returns MissingTokenAuth for required=true passthrough; the per-request token is never threaded into handlers at runtime"
    missing:
      - "Same fix as above (thread the token from RequestHandlerExtra into handlers); these two gaps share a single root cause"
deferred: []
human_verification: []
---

# Phase 90: OpenAPI Built-In Server Verification Report

**Phase Goal:** Deliver a config-driven OpenAPI MCP server that mirrors the completed SQL toolkit (Shape A binary pmcp-sql-server, Phases 83-86): a non-developer points a binary at a config.toml + an OpenAPI spec and gets a live MCP server — curated operation→tool mappings for the common ~20%, Code Mode (openapi-code-mode feature) for the long-tail ~80% — with zero Rust written.
**Verified:** 2026-05-29T23:30:00Z
**Status:** gaps_found
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | HttpConnector trait exists and a reqwest-backed HttpClient impl executes GET/POST returning JSON | VERIFIED | `crates/pmcp-server-toolkit/src/http/mod.rs` defines the trait; `client.rs` implements it; 248 tests pass |
| 2 | The AuthConfig enum's six modes (none/api_key/bearer/basic/oauth2_client_credentials/oauth_passthrough) apply credentials to outgoing requests | VERIFIED (partial) | Six variants defined in `http/auth.rs`; static providers verified via tests; `oauth_passthrough` per-request path is dead at runtime (see gap) |
| 3 | oauth_passthrough per-request token reaches the outbound request at runtime (OAPI-03/OAPI-05) | FAILED | `request_executor()` defined and unit-tested but has no runtime callers; `ScriptToolHandler::handle` and `ExecuteCodeHandler::handle` both take `_extra` and ignore it |
| 4 | base_url + path concatenation preserves an API-Gateway stage prefix via join_url | VERIFIED | `join_url` in `http/mod.rs:76-82`; tested via `test_join_url_preserves_prefix`; used by both `HttpClient` and `HttpCodeExecutor` |
| 5 | Single-call tool synthesizer maps [[tools]] path+method to live HTTP calls via HttpConnector | VERIFIED | `synthesize_from_config_with_http_connector_and_scripts` in `tools.rs`; london-tube parity test passes with real wiremock backend |
| 6 | Script tools (script="""...""") execute via the SAME JS engine as Code Mode (D-02 / OAPI-02b) | VERIFIED | `ScriptToolHandler` uses `PlanCompiler`+`PlanExecutor` over `HttpCodeExecutor`; `script_tool_engine_parity.rs` test asserts byte-equal output from both surfaces |
| 7 | OpenAPI spec parsed from --spec (optional at runtime; spec-free curated-only server boots) | VERIFIED | `OpenApiSchema::parse` in `http/schema.rs`; `--spec` is `Option<PathBuf>` in `cli.rs`; `no_spec_code_mode_warns_and_still_builds` test asserts spec-free boot |
| 8 | Shape A binary (pmcp-openapi-server) boots and serves via streamable HTTP from config+optional spec | VERIFIED (with caveat) | `crates/pmcp-openapi-server/` crate exists with `src/main.rs` shim; `run_serving()` pipeline verified by `http_smoke.rs` and `parity_replay.rs`; binary builds cleanly. Caveat: oauth_passthrough{required:true} fails at runtime (gap above) |
| 9 | cargo pmcp new --kind openapi-server scaffold emits a runnable crate (CF-3/CF-5) | VERIFIED | `execute_openapi_server` in `cargo-pmcp/src/commands/new.rs`; `templates/openapi_server.rs` emits 5 files; scaffold tests pass (`3 passed`) |
| 10 | london-tube wiremock parity: Shape A binary with api_key auth serves same tools as reference (OAPI-08/D-04) | VERIFIED | `parity_replay.rs` tests pass; wiremock asserts the `app_key=dummy` query param reaches the backend; `${TFL_APP_KEY}` env-ref expansion proven |
| 11 | Docs in three shapes: crate README + pmcp-book chapter + pmcp-course chapter (OAPI-09) | VERIFIED | `crates/pmcp-openapi-server/README.md` (10.7K), `pmcp-book/src/openapi-built-in-server.md` (9.8K), `pmcp-course/src/openapi-built-in-server.md` (12.1K) all exist |

**Score:** 9/11 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/pmcp-server-toolkit/src/http/mod.rs` | HttpConnector trait + HttpConnectorError + join_url | VERIFIED | Trait, error enum, join_url helper, all gated under `#[cfg(feature = "http")]` |
| `crates/pmcp-server-toolkit/src/http/client.rs` | reqwest-backed HttpClient implementing HttpConnector | VERIFIED | Full impl with path-concat, auth injection, error redaction |
| `crates/pmcp-server-toolkit/src/http/auth.rs` | Six-mode AuthConfig + HttpAuthProvider trait + create_auth_provider | VERIFIED | All six variants; `create_passthrough_auth_provider` exists; static providers working |
| `crates/pmcp-server-toolkit/src/http/schema.rs` | openapiv3 spec parser (D-03) | VERIFIED | `OpenApiSchema::parse`, `Operation` type with path/query/header param accessors |
| `crates/pmcp-server-toolkit/src/code_mode.rs` | HttpCodeExecutor (OAPI-05) + code_mode_tools_from_executor generalized to Arc<dyn CodeExecutor>+flavor | VERIFIED | `HttpCodeExecutor` implements `pmcp_code_mode::HttpExecutor`; `code_mode_tools_from_executor` takes `Arc<dyn CodeExecutor>` + `ValidationFlavor` |
| `crates/pmcp-server-toolkit/src/tools.rs` | ScriptToolHandler + synthesize_from_config_with_http_connector_and_scripts | VERIFIED | Both exist; ScriptToolHandler is feature-gated `openapi-code-mode`; synthesizer routes `is_script_tool()` to ScriptToolHandler |
| `crates/pmcp-server-toolkit/src/config.rs` | BackendSection + ToolDecl two-kind fields (path/method + script) | VERIFIED | `BackendSection` with `base_url`/`auth`/`http` fields; `ToolDecl::script` field + `is_script_tool()` method |
| `crates/pmcp-openapi-server/src/cli.rs` | Args with --config (required) + --spec (optional) + --http | VERIFIED | All three args; `--spec` is `Option<PathBuf>`; D-03 optionality tested |
| `crates/pmcp-openapi-server/src/dispatch.rs` | [backend] → (HttpConnector, HttpCodeExecutor) pair | VERIFIED | `dispatch()` builds the pair lazily; `DispatchError` Display redacted; but oauth_passthrough{required:true} constructs MissingTokenAuth (see gap) |
| `crates/pmcp-openapi-server/src/assemble.rs` | build_server + request_executor + TokenCaptureAuthProvider | ORPHANED (partial) | `build_server` is fully functional for static-auth backends. `request_executor` is defined, exported, and unit-tested — but has no runtime callers. `TokenCaptureAuthProvider` correctly captures the inbound token into AuthContext. The seam is documented but the binary doesn't own it as described. |
| `crates/pmcp-openapi-server/src/lib.rs` | run_serving entry point + load_config_and_spec + serve | VERIFIED | All three functions; `run_serving()` tested end-to-end by integration tests |
| `cargo-pmcp/src/commands/new.rs` | --kind openapi-server dispatch | VERIFIED | `execute_openapi_server()` branch at line 72-73 |
| `cargo-pmcp/src/templates/openapi_server.rs` | Template emitting 5 files (Cargo.toml/main.rs/config.toml/api.yaml/deploy.toml) | VERIFIED | `generate()` calls 5 `generate_<file>` functions; scaffold tests confirm all files emit |
| `crates/pmcp-openapi-server/tests/parity_replay.rs` | london-tube wiremock parity test (OAPI-08) | VERIFIED | 2 passing tests (1 ignored = live network gate); api_key query-param auth proven |
| `crates/pmcp-openapi-server/README.md` | Crate README (OAPI-09 shape 1) | VERIFIED | 10.7K file exists |
| `pmcp-book/src/openapi-built-in-server.md` | pmcp-book chapter (OAPI-09 shape 2) | VERIFIED | 9.8K file exists |
| `pmcp-course/src/openapi-built-in-server.md` | pmcp-course chapter (OAPI-09 shape 3) | VERIFIED | 12.1K file exists |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `ScriptToolHandler::handle` | `request_executor()` | per-request token threading | NOT_WIRED | `handle` takes `_extra` (ignored); `self.http_exec.clone()` used directly; `request_executor` never called at runtime |
| `ExecuteCodeHandler::handle` | per-request HttpCodeExecutor | `request_executor()` + `JsCodeExecutor::new()` | NOT_WIRED | `_extra` ignored; `self.executor` (fixed JsCodeExecutor from dispatch time) used; no per-request derivation |
| `TokenCaptureAuthProvider::validate_request` | `AuthContext::token` | `authorization_header.map(str::to_string)` | WIRED | Token correctly captured into AuthContext |
| `dispatch()` | `(HttpClient, HttpCodeExecutor)` pair | `create_auth_provider(&backend.auth)` + `reqwest::Client::new()` | WIRED (static path only) | Pair built correctly; but oauth_passthrough{required:true} results in MissingTokenAuth with no token |
| `build_server()` | `synthesize_from_config_with_http_connector_and_scripts` | connector + http_exec + exec_config | WIRED | Single-call + script tools synthesized over shared connector |
| `build_server()` | `code_mode_tools_from_executor` | `JsCodeExecutor::new(http_exec, exec_config)` | WIRED | Code Mode wired via Arc<dyn CodeExecutor>; ValidationFlavor::OpenApi |
| `HttpCodeExecutor::execute_request` | `self.auth.apply(..., self.inbound_token.as_deref())` | inbound_token field | WIRED (mechanism works) | The `apply()` call correctly passes `self.inbound_token`; but that field is always None because handlers never call `with_inbound_token()` |

---

### Data-Flow Trace (Level 4)

| Surface | Data path | Status |
|---------|-----------|--------|
| Static-auth single-call tools | `args` → `HttpClient::execute(operation, args)` → JSON response | FLOWING |
| Static-auth script tools | `args` → `PlanExecutor::execute(plan)` → `HttpCodeExecutor::execute_request` → JSON | FLOWING |
| Static-auth code-mode execute_code | code → `JsCodeExecutor::execute()` → `HttpCodeExecutor::execute_request` → JSON | FLOWING |
| oauth_passthrough single-call tools | N/A (single-call tools use `HttpClient::execute`, which is separate from the HttpCodeExecutor path; passthrough for single-call may be correct as the `HttpClient` also holds the auth provider built by `create_auth_provider` — needs separate verification) | UNCERTAIN |
| oauth_passthrough script tools (required:true) | `_extra` ignored → `self.http_exec.clone()` (inbound_token:None) → `MissingTokenAuth::apply(None)` → Auth error | DISCONNECTED |
| oauth_passthrough execute_code (required:true) | `_extra` ignored → `self.executor` (inbound_token:None baked in) → Auth error | DISCONNECTED |

---

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| pmcp-openapi-server binary builds | `cargo build -p pmcp-openapi-server` | 0 errors, 0 warnings | PASS |
| All pmcp-openapi-server tests pass | `cargo test -p pmcp-openapi-server` | 20 passed, 1 ignored | PASS |
| All toolkit tests pass (openapi-code-mode) | `cargo test -p pmcp-server-toolkit --features openapi-code-mode` | 248 passed | PASS |
| Scaffold emits runnable crate files | `cargo test -p cargo-pmcp --test scaffold_openapi_server` | 3 passed | PASS |
| London-tube parity replay (offline) | `cargo test -p pmcp-openapi-server --test parity_replay` | 2 passed, 1 ignored | PASS |

---

### Requirements Coverage

| Requirement | Plans | Description | Status | Evidence |
|-------------|-------|-------------|--------|---------|
| OAPI-01 | 90-01 | HttpConnector trait | SATISFIED | `crates/pmcp-server-toolkit/src/http/mod.rs`; trait + client impl |
| OAPI-02a | 90-03 | Single-call tool synth | SATISFIED | `synthesize_from_config_with_http_connector_and_scripts`; london-tube parity |
| OAPI-02b | 90-05 | Script tools (D-01) | SATISFIED | `ScriptToolHandler`; engine parity test |
| OAPI-03 | 90-01 | 5-variant outgoing auth (D-05) | PARTIAL | Six variants defined and unit-tested; oauth_passthrough per-request path dead at runtime for code-mode/script-tool handlers |
| OAPI-04 | 90-03 | openapiv3 --spec parser (D-03) | SATISFIED | `OpenApiSchema::parse`; spec optional at runtime |
| OAPI-05 | 90-04 | HttpCodeExecutor seam | PARTIAL | `HttpCodeExecutor` implements `HttpExecutor`; `with_inbound_token()` exists; but seam is dead — handlers never call it |
| OAPI-06 | 90-06 | Shape A binary | SATISFIED (with caveat) | `pmcp-openapi-server` binary builds and serves; oauth_passthrough{required:true} runtime failure is the caveat |
| OAPI-07 | 90-08 | --kind openapi-server scaffold + deploy | SATISFIED | `cargo pmcp new --kind openapi-server` emits all 5 files; scaffold tests pass |
| OAPI-08 | 90-07 | london-tube wiremock parity (D-04) | SATISFIED | Parity replay tests pass; api_key query-param auth verified end-to-end |
| OAPI-09 | 90-09 | Docs in three shapes | SATISFIED | README (10.7K) + pmcp-book chapter (9.8K) + pmcp-course chapter (12.1K) all exist |
| OAPI-10 | 90-04 | Generalize code_mode_tools_from_executor to Arc<dyn CodeExecutor>+flavor (D-02) | SATISFIED | `code_mode_tools_from_executor` takes `Arc<dyn CodeExecutor>` + `ValidationFlavor`; `script_tool_engine_parity` test proves one-engine |

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/pmcp-server-toolkit/src/tools.rs` | 665 | `_extra: RequestHandlerExtra` unused in `ScriptToolHandler::handle` | Blocker | The per-request inbound token cannot reach the outbound request; oauth_passthrough passthrough seam is dead |
| `crates/pmcp-server-toolkit/src/code_mode.rs` | 421 | `_extra: pmcp::RequestHandlerExtra` unused in `ExecuteCodeHandler::handle` | Blocker | Same root cause; execute_code path also fails for required passthrough |
| `crates/pmcp-openapi-server/src/assemble.rs` | 130-133 | `request_executor()` has no runtime callers; documented as "binary owns this seam" but not invoked | Blocker | Dead code that makes WR-01 the bug: the seam is defined and tested but never connected to the handler execution path |

---

### Human Verification Required

*(None — all gaps are statically verifiable from code. Tests pass for all working paths. The failing gap is confirmed through code inspection, not runtime ambiguity.)*

---

## Gaps Summary

**Root cause:** WR-01 (flagged in the pre-submission code review 90-REVIEW.md) is confirmed as a real runtime gap. The oauth_passthrough per-request token-forwarding path has three correctly-built pieces — `TokenCaptureAuthProvider` (captures the inbound token into `AuthContext`), `request_executor()` (derives a per-request `HttpCodeExecutor` with the captured token via `with_inbound_token`), and `OAuthPassthroughAuth::apply()` (forwards the token to the outbound request) — but the bridge between piece 1 and piece 2 is missing. `ScriptToolHandler::handle` and `ExecuteCodeHandler::handle` both receive `RequestHandlerExtra` as `_extra` and ignore it. They execute over a fixed `HttpCodeExecutor` (from dispatch time with `inbound_token: None`) without calling `request_executor()`. The result is that for `AuthConfig::OAuthPassthrough { required: true }`, every code-mode and script-tool invocation returns `Auth("authentication required but no inbound token was provided")` regardless of what the MCP client sent.

**Scope:** 2 failing truths with a single root cause. The fix is localized: wire `request_executor()` into the handler paths. The london-tube parity test (OAPI-08) is unaffected because it exercises `api_key` auth (the static path). All 268 tests across the two crates pass. The binary compiles, serves, and handles static-auth and curated-only configurations correctly.

**Impact on phase goal:** The phase goal ("with zero Rust written, a non-developer gets a live MCP server") is largely met for the ~80% of operators using static auth (api_key/bearer/basic/none). The `oauth_passthrough` required variant — SSO passthrough for backends where the MCP client's token is forwarded to the REST API — does not work at runtime for code-mode or script-tool requests. This is the narrower failure within a broadly functional delivery.

---

_Verified: 2026-05-29T23:30:00Z_
_Verifier: Claude (gsd-verifier)_
