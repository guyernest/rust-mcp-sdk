# Phase 103 Wave-0 Spike — De-Risking Findings (Plan 103-03)

Resolves the highest-risk unknowns BEFORE the demo server (plan 04) and browser
example (plan 05) are built. Every finding below is **empirically verified** in this
worktree, not assumed. The durable proof is the committed test
`tests/web_channel_oauth_route_merge_spike.rs` (HIGH-3), which **PASSES** under
`cargo test --features full`.

---

## Open Question 1 — IdP route-merge against the REAL public API (HIGH-3)

**Verdict: PUBLIC seam exists. NO SDK-internal change required.**

The exact public API plan 04 must use to serve `/oauth2/authorize` + `/oauth2/token`
alongside the MCP router on ONE origin (Open Question 1, preferred option (b)):

```rust
pmcp::axum::router(server: Arc<tokio::sync::Mutex<pmcp::Server>>) -> axum::Router
// or, with explicit origins:
pmcp::axum::router_with_config(server, RouterConfig { allowed_origins, .. }) -> axum::Router
```

- **Re-exported** at `src/lib.rs:56-60` as `pmcp::axum::{router, router_with_config, RouterConfig, AllowedOrigins}` (gated on `feature = "streamable-http"`).
- **Defined** in `src/server/axum_router.rs:72` (`router`) and `:91` (`router_with_config`).
- Returns a fully **layered** `axum::Router` — CORS (origin-locked, no `*`) + `DnsRebindingLayer` + `SecurityHeadersLayer` already applied (`axum_router.rs:105-110`).
- Because it is a plain `axum::Router`, the OAuth IdP routes are composed with `axum::Router::merge` — single origin, **no CORS** between browser PKCE and MCP traffic.

**NOT the seam to use:** `build_mcp_router` (`src/server/streamable_http_server.rs:286`)
is `pub(crate)` — internal, unlayered. Plan 04 must use the public
`pmcp::axum::router_with_config`, which is the layered, public entry. No escalation /
SDK change is needed.

**Why `StreamableHttpServer::start()` is insufficient on its own:** `start()`
(`streamable_http_server.rs`) binds a listener and serves the MCP router directly — it
does **not** expose a merge seam and does not serve `/oauth2/*` (RESEARCH Pitfall 4
confirmed: the streamable server only bearer-*validates*, it never *serves* IdP routes;
see also `s29_oauth_server.rs`, where `InMemoryOAuthProvider` runs over stdio and merely
logs URLs). The fix is to NOT call `start()`; instead get the router via
`pmcp::axum::router(server)`, `.merge()` the OAuth routes, and `axum::serve` one listener.

### Durable proof (HIGH-3)

`tests/web_channel_oauth_route_merge_spike.rs` — a `#[tokio::test]` (gated
`all(feature="streamable-http", feature="http-client", not(wasm32))`) that:

1. Builds a store-backed high-level `pmcp::Server` (auto-advertises `tasks`).
2. Builds the merged router via `build_merged_router()` = `pmcp::axum::router(server).merge(oauth_routes)`, where `oauth_routes` drives an `InMemoryOAuthProvider`.
3. Serves on an **ephemeral port** (`127.0.0.1:0`, bound-before-serve, server task `abort()`ed — no hardcoded port, no sleep, no hang).
4. Asserts `GET /oauth2/authorize` → 2xx, `POST /oauth2/token` → 2xx, **and** MCP `POST /` (`initialize`) → 2xx with a `jsonrpc:"2.0"` body — all on ONE origin.

If a refactor demoted the public seam, the test fails to compile or fails at runtime —
the route-merge result cannot silently regress into plan 04.

---

## Open Question 2 — Owner resolution under the bundled IdP bearer

**Verdict: the create-path scopes the task to `AuthContext.subject`, and
`store.list(subject)` finds the freshly-minted task.**

Chain (all verified against source in this worktree):

1. `TaskDispatcher::resolve_owner` (`src/server/task_dispatch.rs:182-186`): with a
   `TaskStore` configured (the s46 path), the owner **IS** `auth_context.subject`
   verbatim — or `"local"` when unauthenticated. Owner is NEVER taken from client
   params (IDOR mitigation T-103-IDOR / T-102-01).
2. The create path scopes the minted task to that same owner (`src/server/mod.rs:1361-1365`),
   so `store.list(owner, None)` for the SAME `subject` returns the Working task — this
   is the D-05 background-updater discovery heuristic (Pattern 3).
3. `AuthContext.subject` (`src/server/auth/traits.rs:62-64`) is the `sub` claim.

**What sets `subject` under the bundled IdP:** `InMemoryOAuthProvider` mints
`TokenInfo.user_id` from the `user_id` chosen at the authorize step
(`oauth2.rs:507-522` create_authorization_code → `:606` exchange_code → `:613-624`
create_access_token). A bearer-validating `AuthProvider` adapter maps that
`TokenInfo.user_id` → `AuthContext.subject`. **So the owner string the updater must
list for is exactly the `user_id` the demo picks at `/oauth2/authorize`** (e.g.
`"demo-user"`).

Cross-owner isolation for this exact owner-derivation is already proven over the live
HTTP boundary in `tests/tool_as_task_lifecycle_http.rs:289-325` (owner B cannot
`tasks/get`/`result`/`cancel` owner A's task).

### Bearer validator decision for plan 04

**Use `MockValidator` (a `TokenValidator`) wrapped in a thin `AuthProvider` adapter —
NOT `InMemoryOAuthProvider.validate_token` directly.** Rationale:

- The server bearer-check path calls `AuthProvider::validate_request(Option<&str>) -> Result<Option<AuthContext>>` (`src/server/auth/traits.rs:460`, wired at `streamable_http_server.rs:758-765` via `Server::get_auth_provider`, set through `ServerBuilder::auth_provider` `builder.rs:434`).
- `InMemoryOAuthProvider` implements the `OAuthProvider` **IdP** trait (mints/validates `TokenInfo`), NOT `AuthProvider`. Its `validate_token` returns `TokenInfo`, not `AuthContext`.
- `MockValidator` (`src/server/auth/mock.rs`) implements `TokenValidator` and sets `AuthContext.subject = user_id` (`mock.rs:233-234`), but `MockValidator` does NOT itself implement `AuthProvider`. There is **no public concrete `AuthProvider` bearer validator** in the SDK today (only `ProxyProvider`/`NoOpAuthProvider`).
- **Plan-04 action:** add a small in-example `AuthProvider` adapter that (a) for the
  bundled-IdP path, validates the bearer via `InMemoryOAuthProvider::validate_token` and
  maps `TokenInfo.user_id` → `AuthContext{ subject: user_id, .. }`; or (b) for a
  dev-shortcut, wraps `MockValidator` and forwards its `AuthContext`. Either way the
  resulting `subject` is the task owner.

**Alternative (no auth_provider) path also available:** if plan 04 prefers the
proxy-header model (as s46 / the lifecycle test use), the server takes NO
`auth_provider` and `extract_auth_from_proxy_headers` (`streamable_http_server.rs:788-793`)
sets `AuthContext.subject` from the `x-pmcp-user-id` header. The owner is then that
header value. The bundled-IdP browser story, however, wants a real bearer →
`auth_provider` adapter is the recommended path.

---

## HIGH-2 — Example PACKAGE SPLIT (wasm client vs native server)

**Verdict: TWO separate manifests under one example directory keep the wasm build
free of native HTTP deps.** Cargo unifies dependency features **per package build**, so
one package cannot carry both a wasm cdylib (`pmcp` `default-features=false`,
`features=["wasm"]`) and a native server (`pmcp` `full`/`streamable-http`) — the wasm
build would otherwise pull `hyper`/`tokio`/`axum` into the cdylib. The split below
isolates feature sets per package:

```
examples/web-channel-client/
├── client/                      # plan 05 — WASM cdylib
│   ├── Cargo.toml               #   pmcp = { default-features = false, features = ["wasm"] }
│   │                            #   crate-type = ["cdylib"]
│   ├── .cargo/config.toml       #   plan 05 ONLY (see getrandom verdict below)
│   └── src/lib.rs
└── server/                      # plan 04 — native binary
    ├── Cargo.toml               #   pmcp = { features = ["full"] }  (or streamable-http+oauth)
    └── src/main.rs              #   pmcp::axum::router(server).merge(oauth_routes)
```

Scope fence intact: still ONE example directory under `examples/web-channel-client/`,
still no published `crates/` library. The wasm client crate's `[dependencies]` never
names a native HTTP stack; the server crate is a separate package whose features do not
leak into the cdylib build.

**Note:** this plan does NOT create `client/.cargo/config.toml` (belongs to plan 05) and
does NOT create `server/.cargo/config.toml` — the getrandom verdict below shows no
native-side cfg is needed.

---

## Open Question 4 / Pitfall 2 — getrandom wasm backend cfg

**Verdict: cfg-needed = NO. The `--cfg getrandom_backend="wasm_js"` rustflag is NOT
required.**

- **Resolved getrandom version:** `getrandom v0.4.2` (direct dep of `pmcp`, pinned `getrandom = "0.4"` in root `Cargo.toml:89`; the wasm32 target table adds `features=["wasm_js"]` at `Cargo.toml:132`). The tree also contains transitive `getrandom@0.2.17` and `@0.3.4` pulled by other crates, but the pmcp/PKCE path uses `0.4.2`. Plan 01's move of getrandom into the cross-target `[dependencies]` table did **not** change this resolution.
- **Empirical build:** `cargo build --target wasm32-unknown-unknown --no-default-features --features wasm` **exits 0**, including a forced clean recompile of `getrandom@0.4.2` for the wasm target (`cargo clean -p getrandom --target wasm32-unknown-unknown` then rebuild — getrandom recompiled cleanly, no "no backend" / "unsupported target" error).
- **Why no rustflag:** getrandom **0.4** selects the wasm backend from the **Cargo feature** `wasm_js` (already set at `Cargo.toml:132`), unlike getrandom 0.3 which required the `--cfg getrandom_backend="wasm_js"` rustflag. The feature alone satisfies the backend on `wasm32-unknown-unknown`.
- **Consequence for plan 05:** the wasm client crate does **not** need a
  `[target.wasm32-unknown-unknown]` `rustflags` line for getrandom. (If plan 05's crate
  pulls a *different* getrandom major via a new dep, re-verify — but for the pmcp wasm
  feature path, none is required.)
- **No file created:** `examples/web-channel-client/server/.cargo/config.toml` is
  **absent** — no native-side getrandom cfg is needed.

---

## Decision summary for downstream plans

| # | Question | Decision (implementable fact) | Source / proof |
|---|----------|------------------------------|----------------|
| 1 | Route-merge seam | `pmcp::axum::router(server) -> axum::Router`, then `.merge(oauth_routes)`, `axum::serve` ONE listener. PUBLIC, no SDK change. | `src/lib.rs:56`, `axum_router.rs:72,91`; test PASSES |
| 1d | Durable proof | `tests/web_channel_oauth_route_merge_spike.rs` (asserts MCP + both /oauth2/* on one origin) | committed, PASSES |
| 2 | Owner string | `AuthContext.subject` = the IdP `user_id` chosen at `/oauth2/authorize`; `store.list(subject)` finds the task | `task_dispatch.rs:182-186`, `mod.rs:1361`, `oauth2.rs:522,624` |
| 2b | Bearer validator | `AuthProvider` adapter mapping `InMemoryOAuthProvider::validate_token`'s `TokenInfo.user_id` → `AuthContext.subject` (or wrap `MockValidator`); no public concrete bearer `AuthProvider` exists | `traits.rs:460`, `mock.rs:233`, `builder.rs:434` |
| H2 | Package split | `examples/web-channel-client/{client (wasm cdylib), server (native)}` — two manifests, wasm build dep-clean | this doc |
| 4 | getrandom cfg | cfg-needed = NO; getrandom 0.4.2 uses the `wasm_js` **feature**, no rustflag, no config file | wasm build exits 0 after clean recompile |

No throwaway probe **binary** was created under `examples/web-channel-client/` — the
durable artifact is the committed in-tree test (T-103-SC: no new packages introduced).
