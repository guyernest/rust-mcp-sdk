# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.6.0] - 2026-04-21

### Added

- **pmcp 2.6.0 — Typed client helpers** (Phase 73, PARITY-CLIENT-01):
  `Client::call_tool_typed<A: Serialize>`, `Client::call_tool_typed_with_task<A: Serialize>`,
  `Client::call_tool_typed_and_poll<A: Serialize>`, and `Client::get_prompt_typed<A: Serialize>`.
  Each serializes caller-provided `&A` via `serde_json::to_value` and delegates to the existing
  untyped sibling method. Serialization failures return `Error::validation` naming the argument
  source. Signatures match the live siblings exactly — `call_tool_typed_with_task` is two-arg;
  `call_tool_typed_and_poll` is three-arg with `max_polls: usize`.
- **pmcp 2.6.0 — Auto-paginating list helpers** (Phase 73, PARITY-CLIENT-01): `Client::list_all_tools`,
  `Client::list_all_prompts`, `Client::list_all_resources`, and
  **`Client::list_all_resource_templates`** (the last uses the distinct `resources/templates/list`
  capability). Each loops on `next_cursor` and returns the full concatenated item list. A bounded
  `max_iterations` safety cap (configured via `ClientOptions::max_iterations`, default 100) returns
  `Error::validation` rather than looping indefinitely on a buggy server. Empty-string cursors
  (`Some("")`) continue the loop; only `None` terminates. Each helper's rustdoc documents the
  memory-amplification caveat.
- **`pmcp::ClientOptions`** — new `#[non_exhaustive]` configuration struct. Constructed via
  `ClientOptions::default()` + the builder-style `with_max_iterations` setter (external crates)
  or via field-update syntax (`..Default::default()`) from inside the `pmcp` crate. Future
  client-level knobs can land non-breakingly. `max_iterations = 0` is a legal but degenerate
  value (documented in rustdoc; produces immediate `Error::Validation` from every `list_all_*`
  helper).
- **`Client::with_client_options(transport, options)`** — new constructor for wiring a custom
  `ClientOptions`. Does not collide with the pre-existing
  `Client::with_options(transport, info, ProtocolOptions)`. `ClientBuilder` intentionally does not
  expose a `.client_options()` setter in this release — builder-level parity is tracked for a future
  phase.
- **`examples/c09_client_list_all.rs`** — end-to-end demo exercising `Client::with_client_options`,
  `call_tool_typed`, `get_prompt_typed`, and all four `list_all_*` helpers (including
  `list_all_resource_templates`). Drives an MCP server over stdio — see the file header for pairing
  instructions; the binary is not self-contained.
- **`examples/c02_client_tools.rs`** updated to showcase `call_tool_typed` with a
  `#[derive(Serialize)]` struct instead of the prior hand-rolled `json!({...})` pattern.

### Fixed

- **REQUIREMENTS.md §55** — renamed `call_prompt_typed` to `get_prompt_typed` to match the
  MCP method name (`prompts/get`) and the shipped helper name (Phase 73 D-15).

## [2.5.0] - 2026-04-21

### Added

- **pmcp 2.5.0 — Dynamic Client Registration (RFC 7591) support in `OAuthHelper`** (Phase 74).
  `OAuthConfig` gains `client_name: Option<String>` and `dcr_enabled: bool` (default: `true`).
  When `dcr_enabled && client_id.is_none() && discovery.registration_endpoint.is_some()`,
  `OAuthHelper` auto-registers with the server's DCR endpoint before PKCE, eliminating
  the need to pre-provision a client_id against OAuth servers that support RFC 7591.
  Public `DcrRequest` / `DcrResponse` types are re-exported from `pmcp::client::oauth`
  so library consumers can build custom flows on top. New example
  `examples/c08_oauth_dcr.rs` demonstrates the library-user path.
- **`OAuthHelper::authorize_with_details()` + `AuthorizationResult` struct** (Phase 74,
  Blocker #6): returns the full set of OAuth artifacts (access_token, refresh_token,
  expires_at, scopes, effective issuer, effective client_id) so cache consumers can
  persist refresh state across runs. The existing `get_access_token()` API is
  preserved unchanged for simple bearer-header callers.
- **cargo-pmcp 0.9.0 — `cargo pmcp auth` command group** (Phase 74, Plan 02).
  Five subcommands (`login`, `logout`, `status`, `token`, `refresh`) manage per-server
  OAuth tokens in a new `~/.pmcp/oauth-cache.json` (schema_version: 1). `--client <name>`
  flag on `auth login` drives the SDK's new DCR path. `auth token <url>` prints the raw
  access token to stdout (`gh auth token` ergonomics). All server-connecting commands
  (`test/*`, `connect`, `preview`, `schema`, `dev`, `loadtest/run`, `pentest`) now
  consult the cache as the lowest-precedence auth source after explicit flags and
  env vars.

### Changed

- **BREAKING (minor-within-v2.x window):** `OAuthConfig::client_id` type changed `String` -> `Option<String>` to enable DCR auto-trigger when `client_id.is_none()`.
  Existing callers must wrap pre-registered ids in `Some(...)`:

  ```rust
  // Before (pmcp 2.4.x):
  OAuthConfig { client_id: "my-client".to_string(), /* ... */ }

  // After (pmcp 2.5.0+):
  OAuthConfig {
      client_id: Some("my-client".to_string()),
      client_name: None,
      dcr_enabled: false,  // opt out of DCR; use the provided id as-is
      /* ... */
  }
  ```

  Per the v2.x breaking-change window policy in MEMORY.md (v2.0 cleanup philosophy),
  this ships as a minor bump rather than a major.

- **cargo-pmcp `pentest`**: migrated from local `--api-key` flag to shared `AuthFlags`.
  `--api-key` continues to work identically; `--oauth-client-id` / `--oauth-issuer`
  / `--oauth-scopes` are now also accepted for OAuth-protected targets.

## [2.4.0] - 2026-04-17

### Added
- **pmcp 2.4.0 — rmcp parity: request extensions typemap and peer back-channel** (Phase 70). `RequestHandlerExtra` now carries an `http::Extensions` typemap (`.extensions()` / `.extensions_mut()`) that lets middleware attach arbitrary typed state visible to tool/prompt/resource handlers, and a `.peer()` accessor that returns an `Arc<dyn PeerHandle>` so handlers can send notifications, log, or cancel from inside a request without reaching back into the server. Closes the two concrete ergonomics gaps vs. the rmcp SDK surfaced in the rmcp-parity research report.
- **pmcp-macros 0.6.0**: `#[mcp_tool]` now harvests the annotated function's rustdoc comment as the tool description when the `description = "..."` attribute is omitted (PARITY-MACRO-01). Explicit attributes always win over rustdoc; when neither is present, the macro fails with a clear error naming both options. Backwards-compatible — all existing call sites continue to work unchanged.
- **pmcp-macros-support 0.1.0** (new workspace crate): pure non-proc-macro helpers for `pmcp-macros`, extracted so property tests and fuzz targets can consume the rustdoc-harvest normalizer without running into the proc-macro crate's public-API restrictions. Workspace-internal — external users should depend on `pmcp` (with the `macros` feature) or `pmcp-macros` directly, not on this crate.
- **pmcp-macros README**: New "Rustdoc-derived descriptions (pmcp-macros 0.6.0+)" migration section with a compiling `rust,no_run` doctest, plus a "Limitations" subsection enumerating unsupported rustdoc forms (`#[doc = include_str!(...)]`, `#[cfg_attr(..., doc = "...")]`, indented code fences, explicit empty-string descriptions).
- **pmcp-code-mode-derive 0.2.0** (first crates.io publish): companion proc-macro crate to `pmcp-code-mode` providing derive-macro support for Code Mode validation.
- **cargo-pmcp 0.8.0 — pmcp.run landing template: `[login]` section + sign-up flow**. Two change requests from the pmcp.run platform team:
  - **CR-01 — `[login]` section** (silent data-loss fix): `LandingConfig` now has a `login: Option<LoginConfig>` field with `primary_color` / `background_color` / `logo`. Previously any `[login]` block in `pmcp-landing.toml` was dropped by serde before reaching the platform's `deploy-landing` Lambda, so Cognito `UpdateManagedLoginBranding` was never fired end-to-end from a developer deploy. Hex-color validation is mirrored at parse time to catch bad colors locally instead of deferring to the Lambda.
  - **CR-02 — sign-up flow + `/connect` page + `[signup]` TOML**: new Next.js App Router routes (`app/signup/page.tsx`, `app/signup/callback/page.tsx`, `app/connect/page.tsx`), new `Header` and `ConnectSnippet` components (client-side with clipboard + `prompt()` fallback), commented `[signup] redirect_after` block in the template TOML, and a `SignupConfig` struct with open-redirect-safe path validation (rejects absolute URLs, protocol-relative `//host`, non-`/`-prefixed paths). `next build` is verified to succeed cleanly with all four platform-injected `NEXT_PUBLIC_*` env vars unset.

### Changed
- **pmcp-macros**: Error message for `#[mcp_tool]` without a description updated from ``mcp_tool requires at least `description = "..."` attribute`` to ``mcp_tool requires either a `description = "..."` attribute or a rustdoc comment on the function`` — names both fallback sources.
- **pmcp**: Minor version bump 2.3.0 → 2.4.0. Two additive feature surfaces land in this version: the macro surface accepts a newly valid source form (rustdoc-only tool functions via pmcp-macros 0.6.0), and handler code gains the extensions typemap + peer back-channel accessors on `RequestHandlerExtra`.
- Bumped `pmcp-macros` 0.5.0 → 0.6.0 (additive, backwards-compatible minor bump).
- **cargo-pmcp**: Minor bump 0.7.0 → 0.8.0. Covers the concurrent downstream bump for pmcp 2.4.0 (per CLAUDE.md §"Version Bump Rules") plus CR-01 and CR-02 above; intermediate `0.7.1` (CR-01) and `0.7.2` (CR-01 re-issue) were never published to crates.io.
- **mcp-tester**: Patch bump 0.5.0 → 0.5.1 (concurrent downstream bump for pmcp 2.4.0).
- **pmcp-code-mode**: Minor bump 0.4.0 → 0.5.0. Bumps the optional JS-parsing dependencies to the latest compatible swc set: `swc_ecma_parser` 32 → 38, `swc_ecma_ast` 19 → 23, `swc_ecma_visit` 19 → 23, `swc_common` 18 → 21. Supersedes dependabot #233, #235, #236, #237. Verified with `cargo test -p pmcp-code-mode --features openapi-code-mode --lib` (112/112 lib tests pass, no API drift).
- **Docs layout**: Repo root markdown files trimmed from 17 to 5 (README, CRATE-README, CHANGELOG, CLAUDE, QUALITY_REPORT). Five active reference docs moved to `docs/` (MIGRATION, TUTORIAL, RELEASE, TOYOTA_WAY, MIDDLEWARE_ROADMAP); seven historical artifacts moved to `docs/archive/` (MIGRATION_GUIDE, REFACTORING_{FIXES,ISSUE,SUMMARY}, RELEASE_NOTES_v1.5.0, WASM_FIXES_SUMMARY, WASM_POLISH_COMPLETE). All moves preserve file history via `git mv`; two inbound cross-links updated.

### Internal
- New workspace crate `crates/pmcp-macros-support/` scaffolded with the pure normalization helper, unit tests for normalization vectors + unsupported-form cases, and 4 proptest invariants at 1000 cases each (reference-equivalence, determinism, no-panic on arbitrary UTF-8, mixed-attr-shape robustness).
- New trybuild compile-fail snapshots `mcp_tool_missing_description_and_rustdoc.rs` (empty-args) and `mcp_tool_nonempty_args_missing_description_and_rustdoc.rs` (non-empty-args) lock the new error wording against regression.
- New fuzz target `fuzz/fuzz_targets/rustdoc_normalize.rs` exercises the normalizer via `pmcp-macros-support` with mixed attribute shapes (plain doc + `#[doc(hidden)]` + `#[doc(alias = ...)]` + non-doc attrs).
- Shared resolver `pmcp-macros/src/mcp_common.rs::resolve_tool_args` is the single entry point consumed by both `#[mcp_tool]` parse sites (standalone fn in `mcp_tool.rs`, impl-block method in `mcp_server.rs::parse_mcp_tool_attr`) — eliminates the drift risk of duplicated call sequences.

## [2.3.0] - 2026-04-11

### `pmcp` 2.3.0 — no behavioral change, pmcp-macros bump signal

#### Changed
- **Dependency pin bump:** `pmcp-macros` dev-dep and optional-feature-dep both pinned at `0.5.0` (was `0.4.1`). See the `pmcp-macros` 0.5.0 sub-entry below for the breaking-change surface. `pmcp`'s own re-exported public API (`pub use pmcp_macros::{mcp_prompt, mcp_server, mcp_tool};`) is unchanged — users of the `macros` feature who only import `pmcp::mcp_tool` / `pmcp::mcp_server` / `pmcp::mcp_prompt` need no code changes. Users who depend on `pmcp-macros` directly and were still using the deprecated `#[tool]` / `#[tool_router]` / `#[prompt]` / `#[resource]` macros must migrate; see [pmcp-macros/CHANGELOG.md](pmcp-macros/CHANGELOG.md) for the migration guide with before/after code snippets.
- **Version bumped to 2.3.0** to signal the transitive macro-surface change to users checking `cargo update --dry-run` or crates.io diff feeds. A patch bump would have under-communicated the semver-legal breakage in the workspace's macro crate.

### `pmcp-macros` 0.5.0 — Deprecated macros removed, README rewritten

#### Removed (breaking)
- `#[tool]` macro (use `#[mcp_tool]`).
- `#[tool_router]` macro (use `#[mcp_server]`).
- `#[prompt]` zero-op stub (use `#[mcp_prompt]`).
- `#[resource]` zero-op stub (use `#[mcp_resource]`).
- `tool_router_dev` Cargo feature (gated the deleted `#[tool_router]` integration tests).

898 lines of deprecated/stub source removed across 6 files. `lib.rs` crate root shrank from 374 to 226 lines. See [pmcp-macros/CHANGELOG.md](pmcp-macros/CHANGELOG.md) for the complete migration guide including before/after code snippets for each removed macro.

#### Changed
- **Crate-level docs sourced from `pmcp-macros/README.md`** via `#![doc = include_str!("../README.md")]`. docs.rs and GitHub render the same 355-line document — no more stale `pmcp = "1.1"` crate-root docs.
- **README fully rewritten** (252 → 355 lines) to document `#[mcp_tool]`, `#[mcp_server]`, `#[mcp_prompt]`, and `#[mcp_resource]` as the primary API with `rust,no_run` doctest-verified examples. Zero `rust,ignore` fences; API drift is now caught automatically by `cargo test --doc -p pmcp-macros`.
- **Per-macro `///` documentation** references the renamed `examples/s23_mcp_tool_macro.rs` and `examples/s24_mcp_prompt_macro.rs` files from Phase 65 — the previous `63_`/`64_` numbers have been removed from both rustdoc comments and runnable example headers.
- **`docs/advanced/migration-from-typescript.md` and four pmcp-course chapters** updated to `#[mcp_tool]` / `#[mcp_server]` syntax (Phase 66 Wave 1 cleanup of downstream consumers).

## [2.2.0] - 2026-04-06

### `pmcp` 2.2.0 — IconInfo wire format spec compliance (CR-002)

#### Fixed
- **`IconInfo.url` renamed back to `IconInfo.src`** — matches MCP 2025-11-25 spec field name. ChatGPT's pydantic validator rejects responses where the icon field is named `url`. Wire format now emits `src`. `#[serde(alias = "url")]` retained so legacy servers serializing as `url` continue to deserialize correctly. Constructor and fluent API (`IconInfo::new(...)`, `with_mime_type`, `with_sizes`, `with_theme`) are unchanged — the only source-level breakage is direct field access (`icon.url`), which is not used in this workspace.
- **CR-002 regression tests** added: serialization asserts the wire key is `src` and never `url`; deserialization tests cover both new (`src`) and legacy (`url`) inputs; round-trip preserves value.

### `pmcp-macros` 0.4.1 — `#[mcp_tool]` alias matching

#### Fixed
- **`is_value_type()` recognizes common aliases** for `serde_json::Value`. Previously a tool returning `pmcp::Result<JsonValue>` (where `JsonValue` is `use serde_json::{Value as JsonValue}`) generated an `outputSchema` of `{"$schema": "...", "title": "AnyValue"}` — missing the required `"type": "object"` field, causing MCP clients like Gemini CLI to reject **all** tools on the server. The macro now matches `Value`, `JsonValue`, and the fully qualified `serde_json::Value` and skips schema generation for all three.

### `mcp-tester` 0.5.0 — outputSchema conformance check

#### Added
- **T-05: outputSchema validation** in `cargo pmcp test conformance --domain tools`. Validates that every tool with an `outputSchema` has `"type": "object"` at the root per the MCP spec. Skipped if no tools declare `outputSchema`. Catches the macro-generated `AnyValue` schema bug independent of the SDK fix above (defense in depth).

### `cargo-pmcp` 0.6.0 — Billing audience flag, sha2 0.11 fix, deploy hint fix

#### Added
- **`--audience {mcp|billing}` global flag** on `cargo pmcp secret set/get/list/delete` (per CR: pmcp.run billing audience). Default is `mcp` (backwards compatible). `billing` targets the subscription Lambda for servers that opt into Stripe billing via pmcp.run. Threaded through the GraphQL `setServerSecret`/`getServerSecret`/`listServerSecrets`/`deleteServerSecret` operations as `$audience: ServerSecretAudience`. Non-pmcp-run targets (local, aws) reject `--audience billing` with a clear error since they have no subscription-Lambda concept.
- **Platform warning display**: when `setServerSecret` succeeds but no subscription Lambda is registered yet, the platform's non-fatal warning (e.g., "Secret saved but no subscription Lambda is registered…") is shown on stderr in yellow. Exit code stays 0 — the secret was stored, the warning is about downstream propagation, not failure.
- **`Audience` enum** (`Mcp`/`Billing`) with `clap::ValueEnum` derive — gives tab-completion for free.

#### Fixed
- **`sha2` 0.11 `LowerHex` regression**: `format!("{:x}", hasher.finalize())` no longer compiles because `sha2` 0.11's `Array<u8, ...>` output type doesn't implement `LowerHex`. Replaced with explicit hex encoding in `cargo-pmcp/src/pentest/sarif.rs` (SARIF fingerprint) and the `cargo-pmcp/src/templates/oauth/proxy.rs` template (which would have generated uncompilable code for projects scaffolded against sha2 0.11).
- **`cargo pmcp deploy --target pmcp-run` missing-secret hint** now correctly suggests `cargo pmcp secret set --target pmcp-run` instead of `--target pmcp` (which doesn't exist).
- **`cargo pmcp deploy init` tsconfig template** uses `types: ["node"]` instead of `typeRoots: ["./node_modules/@types"]` to avoid TS2580 `Cannot find name 'process'` errors when `node_modules` isn't local (#696c7d4b).
- **`pmcp-server-lambda`** updated to set the new `max_request_bytes` field on `StreamableHttpServerConfig` introduced in pmcp 2.1.0.

#### Internal
- Removed unused `anyhow::Result` import in `pentest/attacks/transport_security.rs`.
- `is_value_type` zero-alloc refactor: direct `Ident` comparison instead of `String` allocation.

## [2.0.2] - 2026-03-24

### Fixed
- **`IconInfo.src` renamed to `IconInfo.url`** — matches MCP spec field name. Servers sending icons (like pmcp.run) caused `initialize` deserialization failure. `#[serde(alias = "src")]` added for backward compatibility.
- **Initialize error reporting** — `Client::initialize()` now reports the actual serde deserialization error instead of a generic "Invalid initialize result format" message.
- Bumped `mcp-tester` to 0.4.1, `mcp-preview` to 0.2.5, `pmcp-server` to 0.2.1, `cargo-pmcp` to 0.5.1 — all aligned to `pmcp` 2.0.2

## [2.0.1] - 2026-03-23

### MCP Tasks — Client API and Server Fixes

### Added
- **Client task methods**: `call_tool_with_task()`, `tasks_get()`, `tasks_result()`, `tasks_list()`, `tasks_cancel()` on the MCP Client
- **`call_tool_and_poll()`**: High-level convenience that calls a tool, auto-polls `tasks/get`, and returns the final `CallToolResult`
- **`ToolCallResponse` enum**: Distinguishes sync results from async task creation on `call_tool_with_task`
- **`RequestHandlerExtra.task_request`**: Tool handlers can check `extra.is_task_request()` to branch between sync and async paths
- **`with_execution()` builder**: All TypedTool variants now support declaring `TaskSupport` via `.with_execution(ToolExecution::new().with_task_support(TaskSupport::Optional))`
- **Task detection in `handle_call_tool`**: Standard `task_store` path returns `CreateTaskResult` with `_meta` related-task metadata when tool declares `taskSupport` and client sends `task` field
- **MCP Tasks documentation**: Book chapter (Ch 12.7), course chapter (Ch 21 with exercises), and updated `docs/TASKS_WITH_POLLING.md`

### Fixed
- **Requestor-driven task detection**: `CreateTaskResult` only returned when client explicitly sends `task` field in `tools/call` — non-task-aware clients (ChatGPT) get `CallToolResult` for compatibility
- **`tracing::warn!`** emitted when tool declares `TaskSupport::Required` but client doesn't send `task` field
- **`call_tool_and_poll` robustness**: Handles `InputRequired` status, only falls back on method-not-found errors (not transport/auth), honors server-updated `poll_interval`
- **Release workflow**: Added `pmcp-macros` publish step before `pmcp` to resolve crates.io dependency ordering

## [2.0.0] - 2026-03-22

### PMCP v2.0 — Aligned with the MCP TypeScript SDK v2.0

This is the first major version bump, marking full alignment with the MCP protocol v2025-03-26 and the TypeScript SDK v2.0 release. PMCP v2.0 brings MCP Apps, MCP Tasks, a conformance test suite, production-grade HTTP security, and improved developer ergonomics across the board.

### Added
- **MCP Protocol v2025-03-26**: Full support for the latest protocol specification with backward compatibility for `2024-11-05`
- **MCP Tasks** (`pmcp-tasks` crate): Experimental shared client/server task state with DynamoDB backend
  - Task lifecycle management (create, update, complete, cancel)
  - Task variables for shared client/server state
  - In-memory backend for dev/tests
- **Conformance Test Suite**: 19-scenario engine across 5 domains (initialize, tools, resources, prompts, notifications)
  - `cargo pmcp test conformance` CLI command with `--strict` and `--domain` flags
  - `mcp-tester conformance` with per-domain CI summary
- **Tower Middleware Stack**: Production-ready HTTP security
  - DNS rebinding protection with configurable allowed origins
  - CORS with origin-locked headers (no wildcard in production)
  - Configurable security headers layer
  - `AllowedOrigins` configuration (localhost, any, custom list)
- **Uniform Constructor DX**: Default impls, builders, and constructors for all protocol types
- **MCP Apps DevTools improvements**: Resizable/collapsible DevTools panel, "Dev Tools" toggle button, global "Clear All", Console tab removed (browser DevTools sufficient)
- **PMCP Server**: MCP server crate exposing SDK developer tools via Streamable HTTP
  - Protocol compliance testing, scenario generation, MCP Apps validation
  - Schema export, code scaffolding, documentation resources
  - Deployed on AWS Lambda at `https://pmcp-server.us-east.true-mcp.com/mcp`

### Changed
- Bumped `pmcp` to 2.0.0
- Bumped `mcp-tester` to 0.4.0
- Bumped `mcp-preview` to 0.3.0
- Bumped `cargo-pmcp` to 0.5.0
- Protocol version negotiation accepts both `2025-03-26` and `2024-11-05`
- `RouterConfig` and `StreamableHttpServerConfig` now include `allowed_origins` field

### Fixed
- Clippy warnings across workspace (derivable_impls, clone_on_copy, map_or patterns)
- Lambda server missing `allowed_origins` field in `StreamableHttpServerConfig`

## [1.19.0] - 2026-03-14

### Added
- **PMCP Server** (`pmcp-server` crate): MCP server exposing SDK developer tools via Streamable HTTP
  - `test_check`: Protocol compliance testing against remote MCP servers
  - `test_generate`: Test scenario generation from server schemas
  - `test_apps`: MCP Apps metadata validation (standard, ChatGPT, Claude Desktop modes)
  - `scaffold`: Code template generation for MCP servers, tools, and resources
  - `schema_export`: Schema discovery and export (JSON and Rust type stubs)
  - Documentation resources and workflow prompts
- **AWS Lambda deployment**: Lambda wrapper crate for running pmcp-server on AWS
- **MCP Registry**: Deployed server at `https://pmcp-server.us-east.true-mcp.com/mcp`
- **Release binaries**: Cross-platform pmcp-server binaries attached to GitHub releases

### Fixed
- `schema_export` and `test_apps` tools now correctly discover tools (was silently failing after `run_quick_test()`)
- `cargo-pmcp deploy`: workspace binary path resolution for Lambda builds
- `cargo-pmcp deploy`: OAuth Lambda copy using correct source path
- `pmcp-macros`: deduplicated `to_pascal_case` into shared `utils` module

### Changed
- Bumped `pmcp-macros` to 0.2.2
- Bumped `mcp-tester` to 0.3.4
- Bumped `cargo-pmcp` to 0.4.5

## [1.11.0] - 2026-02-26

### v1.3 MCP Apps Developer Experience

This release delivers the complete MCP Apps milestone — a full widget authoring, preview, and publishing pipeline for building interactive UI extensions on top of MCP servers.

### Added
- **MCP Apps Preview Server** (`mcp-preview` crate): Live widget preview with dual proxy and WASM bridge modes
  - Axum-based dev server with WebSocket hot-reload
  - Embedded bridge runtime for browser-based MCP communication
- **Widget Authoring**: File-based `WidgetDir` hot-reload and `cargo pmcp app new` scaffolding
  - Automatic bridge script injection via shared `pmcp-widget-utils` crate
- **Publishing Pipeline**: `cargo pmcp app manifest` (ChatGPT action manifest), `cargo pmcp app landing` (standalone demo pages), `cargo pmcp app build` (production bundles)
- **Shared Bridge Library**: TypeScript `App`, `PostMessageTransport`, and `AppBridge` classes for browser ↔ MCP communication
- **New crates**: `pmcp-widget-utils` (shared bridge injection), `mcp-e2e-tests` (browser test harness)
- **Example Apps**: Chess analyzer, interactive map, and data-viz dashboard — each with full preview support
- **E2E Browser Tests**: 20 chromiumoxide CDP tests across all three widget suites
- **cargo-pmcp loadtest module**: TOML config types, MCP client with full handshake and error classification

### Changed
- Bumped `cargo-pmcp` to 0.2.0 (new app subcommands)
- Bumped `mcp-preview` to 0.1.1

## [1.9.1] - 2025-12-29

### Added
- **cargo-pmcp validate command**: New CLI command for project-wide workflow validation
  - `cargo pmcp validate workflows` - Runs cargo check and workflow validation tests
  - `--generate` flag to create test scaffolding
  - `--verbose` and `--server` options for detailed output and workspace support

### Improved
- **TypedTool annotation convenience methods**: Added `.read_only()`, `.destructive()`, `.idempotent()`, `.open_world()` chainable methods
- **TypedToolWithOutput annotation merging**: User-provided annotations now automatically merge with auto-generated output schema
- **Course documentation**: Complete rewrite of Chapter 6 covering soft/hard workflow spectrum and resource embedding

## [1.6.1] - 2025-10-02

### Added
- **Enhanced Prompt Management**: Safer and more flexible way to add prompts to MCP servers
  - Improved workflow integration for tools and resources
  - Better error handling for prompt creation and management
  - Enhanced type safety for prompt arguments
  - Streamlined API for defining prompts with tools and resources workflows

### Improved
- Refined prompt builder patterns for better developer experience
- Enhanced validation for prompt configurations
- Better integration between prompts, tools, and resources

## [1.5.3] - 2025-09-26

### Fixed
- Removed accidentally committed 96MB spin binary from package
- Package size reduced from 98.2MB to ~2MB for successful crates.io publishing

## [1.5.2] - 2025-09-25 (Failed to publish)

### Fixed
- Release workflow to handle existing releases gracefully
- Cargo.toml version alignment for proper crates.io publishing
- Ensure correct tag checkout in GitHub Actions workflow

### Changed
- Updated release workflow to use GitHub CLI instead of deprecated actions/create-release

## [1.5.1] - 2025-09-25 (Skipped)

### Fixed
- Release workflow to handle existing releases gracefully
- Cargo.toml version alignment for proper crates.io publishing

### Changed
- Updated release workflow to use GitHub CLI instead of deprecated actions/create-release

## [1.5.0] - 2025-09-25

### Added
- **WASM MCP Server Support**: Complete WebAssembly deployment capabilities
  - Platform-agnostic WasmMcpServer implementation using PMCP SDK
  - Cloudflare Workers deployment with worker crate
  - Fermyon Spin deployment with spin-sdk
  - "Write once, deploy everywhere" architecture
  - Calculator tool example with comprehensive operations
- **MCP Scenario Testing**: YAML/JSON-based test scenarios
  - Declarative test definitions for MCP servers
  - Support for tool testing with assertions
  - Integration with mcp-tester for automated validation
  - Example scenarios for calculator tool testing
- **Streamable HTTP Transport**: Enhanced HTTP transport with empty response handling
  - Support for 200 OK with empty body
  - Proper Content-Type detection for responses
  - Improved error handling for edge cases

### Fixed
- JSON-RPC notification handling in WASM servers (notifications have no 'id' field)
- Verbose flag propagation in mcp-tester
- Scenario executor assertion logic for Success/Failure cases
- Windows release asset upload paths in GitHub Actions

### Changed
- Refactored WASM server into platform-specific implementations
- Separated core MCP logic from transport/platform layers
- Improved scenario executor to return actual tool responses

## [1.4.2] - 2025-01-15

### Added
- **MCP Server Tester**: Comprehensive testing tool for MCP server validation
  - Protocol compliance validation for JSON-RPC 2.0 and MCP
  - Multi-transport support (HTTP, HTTPS, WebSocket, stdio)
  - Layer-by-layer connection diagnostics
  - Tool testing with custom arguments
  - Server comparison capabilities
  - CI/CD ready with JSON output format
- **Release Workflow**: Automated binary builds and distribution
  - Pre-built binaries for Linux, macOS, and Windows
  - Automatic release creation for forks
  - Cross-platform path compatibility

### Fixed
- JSON-RPC 2.0 compatibility for HTTP transport (Issue #38)
- Null params handling for various MCP methods
- Transport layer fuzz test memory exhaustion issues
- Auth flows fuzz test integer overflow protection
- Windows path format compatibility in CI workflows

## [1.4.1] - 2025-01-16

### 🔧 Enhanced Developer Experience & TypeScript SDK Parity

### Added
- **ToolResult Type Alias (GitHub Issue #37)**
  - `ToolResult` type alias now available from crate root: `use pmcp::ToolResult;`
  - Full compatibility with existing `CallToolResult` - they are identical types
  - Comprehensive documentation with examples covering all usage patterns
  - Complete test suite including unit tests, property tests, and doctests
  - Resolves user confusion about importing tool result types

- **NEW: Complete Example Library with TypeScript SDK Parity**
  - `47_multiple_clients_parallel` - Multiple parallel clients with concurrent operations and error handling
  - `48_structured_output_schema` - Structured output schemas with advanced data validation and response formatting
  - `49_tool_with_sampling_server` - Tool with LLM sampling integration for text processing and summarization
  - All examples developed using Test-Driven Development (TDD) methodology
  - 100% TypeScript SDK feature compatibility verified

- **Enhanced Testing & Quality Assurance**
  - 72% line coverage with 100% function coverage across 390+ tests
  - Comprehensive property-based testing for all new functionality
  - Toyota Way quality standards with zero tolerance for defects
  - All quality gates passing: lint, coverage, and TDD validation

### Fixed
- Fixed GitHub issue #37 where `ToolResult` could not be imported from crate root
- Improved developer ergonomics for MCP tool implementations
- Enhanced API documentation with comprehensive usage examples

### Changed
- Updated to full compatibility with TypeScript SDK v1.17.5
- Improved type ergonomics across all tool-related APIs

## [1.4.0] - 2025-08-22

### 🚀 Enterprise Performance & Advanced Features

This major release introduces enterprise-grade features with significant performance improvements, advanced error recovery, and production-ready WebSocket server capabilities.

### Added
- **PMCP-4001: Complete WebSocket Server Implementation**
  - Production-ready server-side WebSocket transport with full connection lifecycle management
  - Automatic ping/pong keepalive and graceful connection handling
  - WebSocket-specific middleware integration and comprehensive error recovery
  - Connection monitoring and metrics collection for production deployments
  - Example: `25_websocket_server` demonstrating complete server setup

- **PMCP-4002: HTTP/SSE Transport Optimizations** 
  - 10x performance improvement in Server-Sent Events processing
  - Connection pooling with intelligent load balancing strategies
  - Optimized SSE parser with reduced memory allocations
  - Enhanced streaming performance for real-time applications
  - Example: `26_http_sse_optimizations` showing performance improvements

- **PMCP-4003: Advanced Connection Pooling & Load Balancing**
  - Smart connection pooling with health monitoring and automatic failover
  - Multiple load balancing strategies: round-robin, least-connections, weighted
  - Automatic unhealthy connection detection and replacement
  - Comprehensive connection pool metrics and monitoring integration
  - Example: `27_connection_pooling` demonstrating pool management

- **PMCP-4004: Enterprise Middleware System**
  - Advanced middleware chain with circuit breakers and rate limiting
  - Compression middleware with configurable algorithms (gzip, deflate, brotli)
  - Metrics collection middleware with performance monitoring
  - Priority-based middleware execution with dependency management
  - Example: `28_advanced_middleware` showing all middleware features

- **PMCP-4005: Advanced Error Recovery System**
  - Adaptive retry strategies with configurable jitter patterns (Full, Equal, Decorrelated)
  - Deadline-aware recovery with timeout propagation and management
  - Bulk operation recovery with partial failure handling
  - Health monitoring with cascade failure detection and prevention
  - Recovery coordination with event-driven architecture
  - Examples: `29_advanced_error_recovery`, `31_advanced_error_recovery`

- **PMCP-4006: SIMD Parsing Acceleration**
  - **10.3x SSE parsing speedup** using AVX2/SSE4.2 vectorization
  - Runtime CPU feature detection with automatic scalar fallbacks
  - Parallel JSON-RPC batch processing with 119.3% efficiency gains
  - Memory-efficient SIMD operations with comprehensive performance metrics
  - SIMD-accelerated Base64, HTTP headers, and JSON validation
  - Example: `32_simd_parsing_performance` with comprehensive benchmarks

### Performance Improvements
- **SSE parsing**: 10.3x speedup (336,921 vs 32,691 events/sec)
- **JSON-RPC parsing**: 195,181 docs/sec with 100% SIMD utilization
- **Batch processing**: 119.3% parallel efficiency with vectorized operations
- **Memory efficiency**: 580 bytes per document with optimized allocations
- **Base64 operations**: 252+ MB/s encoding/decoding throughput

### Enhanced Developer Experience
- Comprehensive examples for all new features with real-world use cases
- Property-based testing for robustness validation
- Performance benchmarks demonstrating improvements
- Production-ready configurations with monitoring integration

### Security & Reliability
- Circuit breaker patterns preventing cascade failures
- Health monitoring with automatic recovery coordination
- Rate limiting and throttling for DoS protection
- Comprehensive error handling with graceful degradation

## [1.2.1] - 2025-08-14

### Fixed
- Version bump to resolve crates.io publishing conflict

## [1.2.0] - 2025-08-14

### 🏭 Toyota Way Quality Excellence & PMAT Integration

This release implements systematic quality improvements using Toyota Way principles and PMAT (Pragmatic Modular Analysis Toolkit) integration for zero-defect development.

### Added
- **Toyota Way Implementation**: Complete zero-defect development workflow
  - Jidoka (Stop the Line): Quality gates prevent defective code from advancing
  - Genchi Genbutsu (Go and See): Direct code quality observation with PMAT analysis
  - Kaizen (Continuous Improvement): Systematic quality improvement processes
  - Pre-commit quality hooks enforcing complexity and formatting standards
  - Makefile targets for quality gate checks and continuous improvement
- **PMAT Quality Analysis Integration**: Comprehensive code quality metrics
  - TDG (Technical Debt Gradient) scoring: 0.76 (excellent quality)
  - Quality gate enforcement with complexity limits (≤25 cyclomatic complexity)
  - SATD (Self-Admitted Technical Debt) detection and resolution
  - Automated quality badges with GitHub Actions
  - Daily quality monitoring and trend analysis
- **Quality Badges System**: Real-time quality metrics visibility
  - TDG Score badge with color-coded quality levels
  - Quality Gate pass/fail status with automated updates
  - Complexity violations tracking and visualization
  - Technical debt hours estimation (436h managed debt)
  - Toyota Way quality report generation
- **SIMD Module Refactoring**: Reduced complexity while maintaining performance
  - Extracted `validate_utf8_simd` helper functions (34→<25 cyclomatic complexity)
  - Added `is_valid_continuation_byte` and `validate_multibyte_sequence` helpers
  - Separated SIMD fast-path from scalar validation logic
  - Maintained 10-50x performance improvements
- **Enhanced Security Documentation**: Comprehensive PKCE and OAuth guidance
  - Converted SATD comments to proper RFC-referenced documentation
  - Added security recommendations with clear do's and don'ts
  - Enhanced OAuth examples with GitHub, Google, and generic providers
  - PKCE security validation with SHA-256 recommendations

### Changed
- **Quality Standards**: Elevated to Toyota Way and PMAT-level excellence
  - Zero tolerance for clippy warnings and formatting issues
  - All functions maintain ≤25 cyclomatic complexity
  - Comprehensive error handling without unwrap() usage
  - 100% documentation with practical examples
- **CI/CD Pipeline**: Enhanced with quality gates and race condition fixes
  - Fixed parallel test execution with `--test-threads=1`
  - Added pre-commit hooks for immediate quality feedback
  - Quality gate enforcement before any commit acceptance
  - Toyota Way quality principles integrated throughout development

### Fixed
- **CI/CD Race Conditions**: Resolved intermittent test failures
  - Updated CI configuration to use sequential test execution
  - Fixed formatting inconsistencies across the codebase
  - Resolved all clippy violations with proper allows for test patterns
- **SATD Resolution**: Eliminated self-admitted technical debt
  - Converted security-related TODO comments to comprehensive documentation
  - Enhanced PKCE method documentation with RFC 7636 references
  - Added security warnings and recommendations for OAuth implementations

### Quality Metrics
- **TDG Score**: 0.76 (excellent - lower is better)
- **Quality Gate**: Passing with systematic quality enforcement
- **Technical Debt**: 436 hours estimated (actively managed and tracked)
- **Complexity**: All functions ≤25 cyclomatic complexity
- **Documentation**: 100% public API coverage with examples
- **Testing**: Comprehensive property-based and integration test coverage

### Toyota Way Integration
- **Jidoka**: Quality gates stop development for any quality violations
- **Genchi Genbutsu**: PMAT analysis provides direct quality observation
- **Kaizen**: Daily quality badge updates enable continuous improvement
- **Zero Defects**: No compromises on code quality or technical debt

## [1.1.1] - 2025-08-14

### Fixed
- Fixed getrandom v0.3 compatibility by changing feature from 'js' to 'std'
- Updated wasm target feature configuration for getrandom

### Changed
- Updated dependencies to latest versions:
  - getrandom: 0.2 → 0.3
  - rstest: 0.25 → 0.26
  - schemars: 0.8 → 1.0
  - darling: 0.20 → 0.21
  - jsonschema: 0.30 → 0.32
  - notify: 6.1 → 8.2

## [1.1.0] - 2025-08-12

### Added
- **Event Store**: Complete event persistence and resumability support for connection recovery
- **SSE Parser**: Full Server-Sent Events parser implementation for streaming responses
- **Enhanced URI Templates**: Complete RFC 6570 URI Template implementation with all operators
- **TypeScript SDK Feature Parity**: Additional features for full compatibility with TypeScript SDK
- **Development Documentation**: Added CLAUDE.md with AI-assisted development instructions

### Changed
- Replaced `lazy_static` with `std::sync::LazyLock` for modern Rust patterns
- Improved code quality with stricter clippy pedantic and nursery lints
- Optimized URI template expansion for better performance
- Enhanced SIMD implementations with proper safety documentation

### Fixed
- All clippy warnings with zero-tolerance policy
- URI template RFC 6570 compliance issues
- SIMD test expectations and implementations
- Rayon feature flag compilation issues
- Event store test compilation errors
- Disabled incomplete macro_tools example

### Performance
- Optimized JSON batch parsing
- Improved SSE parsing efficiency
- Better memory usage in event store

## [1.0.0] - 2025-08-08

### 🎉 First Stable Release!

PMCP has reached production maturity with zero technical debt, comprehensive testing, and full TypeScript SDK compatibility.

### Added
- **Production Ready**: Zero technical debt, all quality checks pass
- **Procedural Macro System**: New `#[tool]` macro for simplified tool/prompt/resource definitions
- **WASM/Browser Support**: Full WebAssembly support for running MCP clients in browsers
- **SIMD Optimizations**: 10-50x performance improvements for JSON parsing with AVX2 acceleration
- **Fuzzing Infrastructure**: Comprehensive fuzz testing with cargo-fuzz
- **TypeScript Interop Tests**: Integration tests ensuring compatibility with TypeScript SDK
- **Protocol Compatibility Documentation**: Complete guide verifying v1.17.2+ compatibility
- **Advanced Documentation**: Expanded docs covering all new features and patterns
- **Runtime Abstraction**: Cross-platform runtime for native and WASM environments

### Changed
- Default features now exclude experimental transports for better stability
- Improved test coverage with additional protocol tests
- Enhanced error handling with more descriptive error messages
- Updated minimum Rust version to 1.82.0
- All clippy warnings resolved
- All technical debt eliminated

### Fixed
- Resource watcher compilation with proper feature gating
- WebSocket transport stability improvements
- All compilation errors and warnings

### Performance
- 16x faster than TypeScript SDK for common operations
- 50x lower memory usage per connection
- 21x faster JSON parsing with SIMD optimizations
- 10-50x improvement in message throughput

## [0.7.0] - 2025-08-08 (Pre-release)

### Added
- **Procedural Macro System**: New `#[tool]` macro for simplified tool/prompt/resource definitions
- **WASM/Browser Support**: Full WebAssembly support for running MCP clients in browsers
- **SIMD Optimizations**: 10-50x performance improvements for JSON parsing with AVX2 acceleration
- **Fuzzing Infrastructure**: Comprehensive fuzz testing with cargo-fuzz
- **TypeScript Interop Tests**: Integration tests ensuring compatibility with TypeScript SDK
- **Protocol Compatibility Documentation**: Complete guide verifying v1.17.2+ compatibility
- **Advanced Documentation**: Expanded docs covering all new features and patterns
- **Runtime Abstraction**: Cross-platform runtime for native and WASM environments

### Changed
- Default features now exclude experimental transports (websocket, http) for better stability
- Improved test coverage with additional protocol tests
- Enhanced error handling with more descriptive error messages
- Updated minimum Rust version to 1.82.0

### Fixed
- Resource watcher compilation with proper feature gating
- WebSocket transport stability improvements
- Various clippy warnings and code quality issues

### Performance
- 16x faster than TypeScript SDK for common operations
- 50x lower memory usage per connection
- 21x faster JSON parsing with SIMD optimizations
- 10-50x improvement in message throughput

## [0.6.6] - 2025-01-07

### Added
- OIDC discovery support for authentication
- Transport isolation for enhanced security
- Comprehensive documentation updates

## [0.6.5] - 2025-01-06

### Added
- Initial comprehensive documentation
- Property-based testing framework
- Session management improvements

## [0.6.4] - 2025-01-05

### Added
- Comprehensive doctests for the SDK
- Improved examples for all major features
- Better error messages and debugging support

## [0.6.3] - 2025-01-04

### Added
- WebSocket server implementation
- Resource subscription support
- Request cancellation with CancellationToken

## [0.6.2] - 2025-01-03

### Added
- OAuth 2.0 authentication support
- Bearer token authentication
- Middleware system for request/response interception

## [0.6.1] - 2025-01-02

### Added
- Message batching and debouncing
- Retry logic with exponential backoff
- Progress notification support

## [0.6.0] - 2025-01-01

### Added
- Initial release with full MCP v1.0 protocol support
- stdio, HTTP/SSE transports
- Basic client and server implementations
- Comprehensive example suite