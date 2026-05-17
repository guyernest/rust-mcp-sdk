# Requirements: PMCP SDK — Configuration-Only MCP Servers (v2.2 active, v2.1 retained)

**Active milestone defined:** 2026-05-17
**Active core value:** Enterprise developers build production-grade SQL MCP servers from configuration + schema files alone — no Rust required — while preserving PMCP's security, tools/resources/prompts/tasks/skills standards and pmcp.run hosting integration.

**Prior milestone (v2.1 rmcp Upgrades) defined:** 2026-04-10
**Prior milestone core value:** Close credibility and DX gaps where rmcp outshines PMCP — documentation accuracy, feature gate presentation, macro documentation, example index, and repo hygiene.

## v2.1 Requirements

Requirements for rmcp Upgrades milestone. Each maps to roadmap phases.

### Examples Cleanup

- [x] **EXMP-01**: Examples README replaced with accurate PMCP example index organized by category with required features and run commands
- [x] **EXMP-02**: All example .rs files in examples/ are registered in Cargo.toml with correct required-features (17 orphans resolved)
- [x] **EXMP-03**: No duplicate example number prefixes — each numbered prefix maps to exactly one file (08, 11, 12, 32 resolved)

### Protocol Accuracy

- [x] **PROT-01**: README MCP-Compatible badge and compatibility table show 2025-11-25, matching LATEST_PROTOCOL_VERSION in code

### Macros Documentation

- [ ] **MACR-01**: pmcp-macros README rewritten to document #[mcp_tool], #[mcp_server], #[mcp_prompt], #[mcp_resource] as primary APIs with working examples
- [ ] **MACR-02**: Migration section guiding users from deprecated #[tool]/#[tool_router] to #[mcp_tool]/#[mcp_server]
- [ ] **MACR-03**: pmcp-macros lib.rs uses include_str!("../README.md") so docs.rs shows the rewritten README

### docs.rs Pipeline

- [ ] **DRSD-01**: lib.rs contains cfg_attr(docsrs, feature(doc_auto_cfg)) enabling automatic feature badges on all feature-gated items
- [ ] **DRSD-02**: Cargo.toml [package.metadata.docs.rs] uses explicit feature list (~13 user-facing features) instead of all-features = true
- [ ] **DRSD-03**: Feature flag table added to lib.rs doc comments documenting all user-facing features with descriptions
- [ ] **DRSD-04**: Zero rustdoc warnings — all broken intra-doc links and unclosed HTML tags resolved, CI gate added

### General Polish

- [ ] **PLSH-01**: lib.rs crate-level doctests updated to show TypedToolWithOutput and current builder patterns (not legacy Server::builder())
- [ ] **PLSH-02**: CI enforcement: example file count matches Cargo.toml [[example]] count, cargo semver-checks on PRs
- [ ] **PLSH-03**: Transport matrix table in lib.rs docs linking to actual transport types

### Code Mode Support

Inserted into v2.1 via Phase 67.1 (INSERTED, 2026-04-11) — blocker for an imminent MCP server launch. External developers must be able to add Code Mode (validate → approve → execute) to their servers consistently, without depending on the pmcp-run internal crate. See `.planning/phases/67.1-code-mode-support/67.1-DECISIONS.md` for the locked design decisions and `pmcp-run/built-in/shared/pmcp-code-mode/SDK_DESIGN_SPEC.md` for the source spec.

- [ ] **CMSUP-01**: `crates/pmcp-code-mode/` exists in the rust-mcp-sdk workspace containing the moved Code Mode core — validation pipeline, `PolicyEvaluator` trait, `CedarPolicyEvaluator` (behind `cedar` feature), HMAC token infrastructure, GraphQL/JS/SQL validators — with all existing tests passing after the move and zero regressions against the pmcp-run source of truth
- [ ] **CMSUP-02**: Security hardening lands alongside the move — `TokenSecret` newtype backed by `secrecy` + `zeroize` replaces plain `Vec<u8>` token storage, blocks `Debug`/`Display` printing, and is documented in a crate-level threat model (README section or SECURITY.md); `NoopPolicyEvaluator` exists in `pmcp-code-mode` for tests and local development; `pub use async_trait::async_trait;` is re-exported from `pmcp-code-mode/src/lib.rs`
- [ ] **CMSUP-03**: `CodeExecutor` high-level trait exists in `pmcp-code-mode` with a single `execute(code, variables) -> Result<Value, ExecutionError>` method, supersedes per-server executor glue, and covers all four execution patterns (direct SQL, JS+HTTP, JS+SDK, JS+MCP); blanket impl for `PlanExecutor` explored and either implemented or explicitly documented as deferred
- [ ] **CMSUP-04**: `crates/pmcp-code-mode-derive/` proc macro crate exists and provides `#[derive(CodeMode)]` which emits a `register_code_mode_tools(builder)` method registering `validate_code` + `execute_code` tools against a `pmcp::ServerBuilder`, enforces `Send + Sync` at compile time, uses `#[pmcp_code_mode::async_trait]` via the re-export to avoid version conflicts, and has `trybuild` compile-pass + compile-fail snapshot coverage (missing required fields, non-`Send` fields, wrong field types)
- [ ] **CMSUP-05**: Contract YAMLs for `pmcp-code-mode` and `pmcp-code-mode-derive` exist under `../provable-contracts/contracts/` covering `PolicyEvaluator`/`CodeExecutor` trait invariants, HMAC token bind-to-code-hash semantics, derive-macro expansion contracts, and default-deny behavior; `pmat comply check` passes on both crates; property tests cover HMAC round-trip and validation-pipeline determinism; fuzz targets exist for GraphQL parser input, JavaScript parser input, and token verification in the core crate (macro-input fuzzing skipped as documented in 67.1-DECISIONS.md D7)
- [ ] **CMSUP-06**: A complete worked example in `examples/` (e.g. `XX_code_mode_graphql.rs`) demonstrates the end-to-end flow: `#[derive(CodeMode)]` annotation → `register_code_mode_tools(builder)` → `validate_code` call → approval token issued → `execute_code` call with token → result — runnable via `cargo run --example XX_code_mode_graphql` using `NoopPolicyEvaluator`; `crates/pmcp-code-mode/` and `crates/pmcp-code-mode-derive/` are slotted into the publish order documented in CLAUDE.md (`pmcp-widget-utils → pmcp → pmcp-code-mode → pmcp-code-mode-derive → mcp-tester → mcp-preview → cargo-pmcp`) with CRATE-README files ready for docs.rs, and `make quality-gate` passes workspace-wide

### rmcp Parity (Phase 69 research — seeds follow-on phases)

Seeded by Phase 69 research (`.planning/phases/69-rmcp-parity-research-gap-analysis-across-ergonomics-transpor/69-PROPOSALS.md`). One REQ-ID per proposal, mapping to the proposal as a whole; the proposal's 3–5 success criteria are its internal acceptance tests. Status remains pending until the follow-on phase ships.

- [ ] **PARITY-HANDLER-01**: Enrich `RequestHandlerExtra` with a typed-key extensions map and an optional peer back-channel, so middleware state transfer and in-handler server-to-client RPCs work without out-of-band plumbing.
- [ ] **PARITY-CLIENT-01**: Ship typed-input `call_tool_typed` / `get_prompt_typed` helpers and auto-paginating `list_all_tools` / `list_all_prompts` / `list_all_resources` convenience methods on `Client`, reducing client boilerplate to one call per operation.
- [x] **PARITY-MACRO-01**: Support rustdoc as a fallback source for `#[mcp_tool]` descriptions, so well-documented tool functions do not have to repeat themselves in the macro attribute.

### rmcp Foundation Evaluation (Phase 72 research)

Seeded by Phase 72 rmcp-foundations research (`.planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-RESEARCH.md` + `72-REVIEWS.md`). These REQ-IDs cover the artifacts that the phase itself produces (inventory, strategy matrix, PoC proposal, decision rubric, final recommendation). Status remains pending until Plan 03 ships the recommendation.

- [x] **RMCP-EVAL-01**: Produce a source-citation-backed inversion inventory covering every module family in `src/types/` and `src/shared/` (and `src/server/cancellation.rs`), identifying the nearest rmcp 1.5.0 equivalent and an overlap rating (EXACT / Partial / pmcp-superset / pmcp-exclusive / UNVERIFIED). Each row MUST carry a 9-column evidence schema: (1) pmcp module family, (2) pmcp defining `file:line`, (3) rmcp docs.rs anchor or GitHub blob URL, (4) exact symbols touched, (5) public API surface impacted, (6) owned impls/macros affected, (7) serde compatibility risk, (8) feature flag(s), (9) downstream crates touched.
- [x] **RMCP-EVAL-02**: Score the five architectural options (A. Full adopt / B. Hybrid wrapper / C1. Selective borrow — types only / C2. Selective borrow — transports only / D. Status quo + upstream PRs) against five criteria (maintenance reduction, migration cost, breaking-change surface, enterprise feature preservation, upgrade agility). All 25 cells scored with rationale; no `TBD`. E (Fork) documented as a contingency footnote only, not a scored row.
- [x] **RMCP-EVAL-03**: Propose 2-3 candidate PoC slices, each `≤500` LOC touched, each with explicit files list, hypothesis tested, pass criterion, and disqualifying outcome. One slice must be executable in `≤3` days. Plan 02 additionally EXECUTES Slice 1 as a throwaway time-boxed spike to resolve T3/T4 with real data.
- [x] **RMCP-EVAL-04**: Publish a decision rubric with `≥5` falsifiable thresholds (numeric or boolean), each citing a named data source (git log query, gh CLI query, mcp-tester run, PoC branch output, or CONTEXT.md entry). Post-reviews rubric adds T8 (historical churn on `src/types/` + `src/shared/`) and T9 (enterprise-feature preservation checklist) and updates T2 (PR merge latency) and T4 (broken-APIs + broken-examples + broken-downstream-crates subcounts).
- [x] **RMCP-EVAL-05**: Publish a final recommendation picking exactly one of {A, B, C1, C2, D, DEFER}, with a per-criterion subsection that engages every rubric criterion from RMCP-EVAL-04 and cites the inventory row(s) and matrix cell(s) supporting its conclusion. DEFER is an explicit, valid outcome when net-resolved thresholds < 3; E (Fork) is NOT a valid recommendation.

### Landing template runtime config (Phase 72.1)

Urgent INSERTED phase driven by CR-03 rev-2 from the pmcp.run platform team. The platform's Phase 71.1 actively strips `NEXT_PUBLIC_*` env vars on every landing deploy, leaving the current `cargo-pmcp` landing template non-functional for signup. See `.planning/phases/72.1-finalize-landing-support/72.1-CR-03-SOURCE.md` for the authoritative spec.

- [x] **LAND-CR03-01**: `cargo-pmcp 0.8.1` — landing template uses a runtime fetch of `/landing-config` via a new required shared `useLandingConfig` hook. All four template consumers (`app/signup/page.tsx`, `app/signup/callback/page.tsx`, `app/connect/page.tsx`, `app/components/Header.tsx`) route through the hook; all `NEXT_PUBLIC_COGNITO_*` / `NEXT_PUBLIC_LANDING_CLIENT_ID` / `NEXT_PUBLIC_SIGNUP_REDIRECT_AFTER` reads are deleted; `MCP_SERVER_NAME` branding reads stay; three stale rustdoc references in `cargo-pmcp/src/landing/config.rs` are rewritten to describe the runtime mechanism; patch version bump `0.8.0 → 0.8.1`. Verified by the 12 grep/build acceptance criteria in CR-03 §Acceptance criteria.

### CLI auth subcommand + SDK DCR (Phase 74)

Consolidates OAuth handling for all server-connecting `cargo pmcp` commands into a dedicated `cargo pmcp auth` command group with a per-server-keyed token cache. Adds Dynamic Client Registration (DCR, RFC 7591) to the SDK so any PMCP-built client — not just the CLI — can auto-register with OAuth servers that advertise a `registration_endpoint`. Exposes DCR via a `--client <name>` flag on `auth login` for testing pmcp.run's client-branded login pages. Full decision log: `.planning/phases/74-add-cargo-pmcp-auth-subcommand-with-multi-server-oauth-token/74-CONTEXT.md`.

- [x] **SDK-DCR-01** (SDK, `pmcp` minor bump): Add public RFC 7591 Dynamic Client Registration support in `src/client/oauth.rs`. `OAuthConfig` gains `client_name: Option<String>` and `dcr_enabled: bool` fields (additive, no breaking change). `OAuthHelper` auto-performs DCR when `dcr_enabled && client_id.is_none() && discovery.registration_endpoint.is_some()`. Request body matches RFC 7591 public-PKCE-client shape. Response parsed for `client_id` (required) and optional `client_secret`. Error surface: actionable message when server does not advertise a registration_endpoint. Ships with fuzz + property + unit tests + working example per CLAUDE.md ALWAYS requirements.
- [x] **CLI-AUTH-01** (CLI, `cargo-pmcp 0.8.1 → 0.9.0` minor bump): New `cargo pmcp auth` command group with `login`, `logout`, `status`, `token`, `refresh` subcommands. Per-server-keyed token cache at `~/.pmcp/oauth-cache.json` with `schema_version: 1` (legacy `~/.pmcp/oauth-tokens.json` left untouched — users re-login once). `login` accepts `--client <name>` (mutually exclusive with `--oauth-client-id`) which sets `OAuthConfig::client_name` for SDK DCR. `logout` with no args errors (`--all` or `<url>` required). `token <url>` prints raw access token to stdout. `login` prints success message only (never the token). Precedence for all server-connecting commands: explicit flag > env var > cache. Transparent on-demand refresh when cached token is expired or within 60s of expiry. `cargo-pmcp/src/commands/pentest.rs` migrated from its duplicate `--api-key` flag to shared `AuthFlags`.

### CLI configure subcommand (Phase 77)

Seeded by Phase 77 cargo-pmcp configure commands research (`.planning/phases/77-cargo-pmcp-configure-commands/77-RESEARCH.md` + `77-VALIDATION.md`). One REQ-ID per Phase 77 testable behavior.

- [x] **REQ-77-01**: `cargo pmcp configure {add,use,list,show}` subcommand group ships under a new `configure/` module — each subcommand persists or reads target state and emits stable text/JSON output.
- [x] **REQ-77-02**: `~/.pmcp/config.toml` schema is a `#[serde(tag = "type")]` enum with variants `pmcp-run`, `aws-lambda`, `google-cloud-run`, `cloudflare-workers`; per-variant structs use `#[serde(deny_unknown_fields)]` so typos are rejected at parse time.
- [x] **REQ-77-03**: `.pmcp/active-target` workspace marker is a single-line plain-text file containing only the active target name; permissive on read (trim+UTF-8 normalize), strict on write.
- [x] **REQ-77-04**: `PMCP_TARGET=<name>` env var is the highest-priority target selector and emits a stderr override note when it overrides the workspace marker; the note fires even when `--quiet` is set. `--target <name>` is a new global flag on the top-level `Cli`.
- [x] **REQ-77-05**: A header banner is emitted to stderr by every target-consuming command before any AWS API / CDK / upload call; field ordering is fixed (api_url / aws_profile / region / source); banner is suppressible by `--quiet` (except the D-03 PMCP_TARGET override note).
- [x] **REQ-77-06**: Field-level precedence at command-execution time is `ENV > explicit --flag > active target > .pmcp/deploy.toml`; verified by property test.
- [x] **REQ-77-07**: `configure add` rejects raw-credential patterns (AKIA[0-9A-Z]{16}, ASIA[0-9A-Z]{16}, ghp_*, github_pat_*, sk_live_*, AIza*) with an actionable error pointing the user at AWS profile names / env-var refs / Secrets Manager ARNs.
- [x] **REQ-77-08**: `~/.pmcp/config.toml` writes are atomic via `tempfile::NamedTempFile::persist`; on Unix the file is `0o600`, the parent dir `0o700`; concurrent writers are last-writer-wins (no partial file).
- [x] **REQ-77-09**: When `~/.pmcp/config.toml` does not exist, `cargo pmcp deploy` and `cargo pmcp pmcp.run upload` behave byte-identically to Phase 76 — no banner about targets, no migration nag, zero touch.
- [x] **REQ-77-10**: ALWAYS gates pass: `cargo fuzz run pmcp_config_toml_parser -- -max_total_time=60`, `cargo test -p cargo-pmcp configure::config::proptests`, `cargo test -p cargo-pmcp configure::resolver::proptests::precedence_holds`, `cargo run --example multi_target_monorepo -p cargo-pmcp` all exit 0.
- [x] **REQ-77-11**: Banner emission integrates with ALL target-consuming entry points enumerated in 77-RESEARCH §7 (HIGH-2 per 77-REVIEWS.md): `commands/deploy/mod.rs` (8+ AWS-touching sites), `commands/test/upload.rs` (top of `execute` before `auth::get_credentials()`), `commands/loadtest/upload.rs` (same pattern), and `commands/landing/deploy.rs` (lines 69, 215, 334). The OnceLock-guarded `emit_resolved_banner_once` makes duplicate calls within a single process invocation safe.

## v2.2 Requirements

Requirements for the Configuration-Only MCP Servers milestone. Each maps to roadmap phases.

Derived from validated spikes 003 (schema-server-surface-diff), 004 (schema-server-thin-slice-sql), 005 (multi-dialect-sql-connector), 006 (authoring-skills-server). Foundation patterns + invariants are encoded in the auto-loaded `spike-findings-rust-mcp-sdk` skill.

**Reference implementation as ground truth.** The pmcp.run service already ships three production SQL-API servers under `~/Development/mcp/sdk/pmcp-run/built-in/sql-api/servers/` (`open-images`, `imdb`, `msr-vtt`). Their `config.toml` files (e.g. `servers/open-images/config.toml`) demonstrate the canonical shape this milestone lifts into the SDK:

- `[server]` + `[metadata]` blocks (id, name, description, tags, visibility)
- `[database]` with `type` (athena/postgres/mysql/sqlite), connection params, and `[[database.tables]]` blocks for code-mode schema enrichment
- `[code_mode]` policy (`enabled`, `allow_writes`, `allow_deletes`, `allow_ddl`, `require_limit`, `max_limit`, `blocked_tables`, `sensitive_columns`, `auto_approve_levels`, `token_ttl_seconds`, `token_secret`) + `[code_mode.limits]` (max tables/join depth/subquery depth)
- `[[tools]]` entries — curated tools defined as SQL queries with `:name` canonical placeholders, named `[[tools.parameters]]` (type, description, required, default, min/max, max_length), and `[tools.annotations]` (read_only_hint, destructive_hint, idempotent_hint, open_world_hint, cost_hint)
- Optional `ui_resource_uri` per tool wiring an MCP Apps widget

**Pareto split is intentional, not auto-conversion.** Curated `[[tools]]` cover the high-frequency business use cases (~80% of usage); `[code_mode]` covers the long-tail via LLM-generated SQL bounded by the policy. This dual-mode shape is the load-bearing design decision — it preserves the "high standard of well-designed MCP schema" while letting non-developers add tools by writing SQL + a name + a description.

### Builder DX Prerequisites

Upstream PMCP changes that unblock external toolkit authors (spike 004 surfaced these).

- [ ] **BLDR-01**: `pmcp::ServerBuilder::tool_arc(name, Arc<dyn ToolHandler>)` lifted from `ServerCoreBuilder` to the public builder so config-driven toolkits can share an `Arc<Handler>` between the builder and an in-process handler map without a 20-line delegating shim
- [ ] **BLDR-02**: `pmcp::ServerBuilder::prompt_arc(name, Arc<dyn PromptHandler>)` lifted from `ServerCoreBuilder` to the public builder
- [ ] **BLDR-03**: Public in-process driver for a built `pmcp::Server` OR an officially documented handler-level testing pattern so external toolkit integration tests can drive request flow without poking at private `Server::handle_request`

### Toolkit Core (`pmcp-server-toolkit`)

Lift `mcp-server-common` (~2.2k LoC at `pmcp-run/built-in/shared/`) and `pmcp-code-mode` shapes to a public, crates.io-published SDK crate (spike 003).

- [ ] **TKIT-01**: `crates/pmcp-server-toolkit/` exists in the workspace, builds cleanly, and is publishable to crates.io (slotted into the release publish order)
- [ ] **TKIT-02**: `AuthProvider` trait exposed in the public toolkit API with at least one concrete impl ready for downstream use
- [ ] **TKIT-03**: `SecretsProvider` trait exposed in the public toolkit API with at least one concrete impl ready for downstream use
- [ ] **TKIT-04**: `StaticResourceHandler` constructible from config exposed in the public toolkit API
- [ ] **TKIT-05**: `StaticPromptHandler` constructible from config exposed in the public toolkit API
- [ ] **TKIT-06**: HMAC token machinery (sign + verify, code-hash binding) exposed in the public toolkit API and integrated with `pmcp-code-mode`
- [ ] **TKIT-07**: `ToolInfo` synthesizer reads `[[tools]]` entries from a server's `config.toml` and produces complete `ToolInfo` definitions (name, description, input schema, `[tools.annotations]`) with zero per-tool Rust handlers required. The supported `config.toml` shape MUST be a superset of the existing `pmcp-run/built-in/sql-api/servers/*/config.toml` files — including `[[tools.parameters]]` (type, description, required, default, min/max, max_length) and `[tools.annotations]` (read_only_hint, destructive_hint, idempotent_hint, open_world_hint, cost_hint) — so reference servers port without schema rewrites
- [ ] **TKIT-08**: All three `pmcp-run` backend cores (`mcp-sql-server-core`, `mcp-graphql-server-core`, `mcp-openapi-server-core`) replace their path-deps on `pmcp-run/built-in/shared/` with versioned crates.io deps on `pmcp-server-toolkit` (independent release cadence unblocked)
- [ ] **TKIT-09**: `[code_mode]` config block (`enabled`, `allow_writes`, `allow_deletes`, `allow_ddl`, `require_limit`, `max_limit`, `blocked_tables`, `sensitive_columns`, `auto_approve_levels`, `token_ttl_seconds`, `token_secret`) plus `[code_mode.limits]` (`max_tables_per_query`, `max_join_depth`, `max_subquery_depth`) are parsed by the toolkit and wired into `pmcp-code-mode`'s validation pipeline + `CodeExecutor` with zero per-server Rust glue — same surface as `~/Development/mcp/sdk/pmcp-run/built-in/sql-api/servers/open-images/config.toml` lines 97–127
- [ ] **TKIT-10**: Code-mode prompt body assembly combines `build_code_mode_prompt` (CONN-04) with `[[database.tables]]` curated descriptions so the LLM is seeded with the dialect + per-table semantic hints (not just raw DDL); the assembled prompt matches the spirit of the reference servers' code-mode prompt

### SQL Connectors

Multi-dialect SQL connector trait + per-backend crates (spike 005). Three methods + two free helpers cleanly handle Postgres / MySQL / Athena / SQLite.

- [ ] **CONN-01**: `SqlConnector` trait in toolkit core exposes exactly three methods: `dialect() -> Dialect`, `execute(query, params) -> Result<...>`, `schema_text() -> Result<String>`. The `schema_text()` body MUST optionally fold in any per-table descriptions from `[[database.tables]]` config entries (as the reference servers already do) so curated descriptions reach the code-mode prompt
- [ ] **CONN-02**: `Dialect` enum in toolkit core with `Postgres`, `MySQL`, `Athena`, `SQLite` variants
- [ ] **CONN-03**: `translate_placeholders(canonical_query, dialect) -> String` free helper translating `:name` placeholders to dialect-specific forms (`$1`, `?`, `?`, `:name` respectively)
- [ ] **CONN-04**: `build_code_mode_prompt(connector) -> String` free helper assembling the dialect-aware code-mode bootstrap prompt body from a connector's `schema_text()`
- [ ] **CONN-05**: `pmcp-toolkit-postgres` crate using pure-Rust `tokio-postgres` implements `SqlConnector` with `information_schema`-driven `schema_text()`
- [ ] **CONN-06**: `pmcp-toolkit-mysql` crate using pure-Rust `sqlx` (MySQL driver) implements `SqlConnector` with `information_schema`-driven `schema_text()`
- [ ] **CONN-07**: `pmcp-toolkit-athena` crate using pure-Rust `aws-sdk-athena` implements `SqlConnector` with Glue catalog-driven `schema_text()`
- [ ] **CONN-08**: SQLite backend ships as a feature flag on the toolkit using `rusqlite` (bundled feature) — no separate crate

### DX Shapes

Four user-facing surfaces in this milestone (spike 004 menu).

- [ ] **SHAP-A-01**: Shape A — `pmcp-sql-server --config <file> --schema <file>` pure-config binary crate that spawns an MCP server from configuration + schema alone, zero Rust written by the developer. Acceptance check: running it against `pmcp-run/built-in/sql-api/servers/open-images/config.toml` (or `imdb` / `msr-vtt`) produces a server that responds to `tools/list`, `tools/call` for every `[[tools]]` entry, and the code-mode pair (`validate_code` / `execute_code`) with policy enforcement matching the production server's behavior
- [ ] **SHAP-B-01**: Shape B — `cargo pmcp new --kind sql-server` scaffolds a starter project with `Cargo.toml` (pinned toolkit + chosen backend dep), `main.rs` (12-line shape-C wiring), and `config.toml` (commented template) ready to `cargo run`
- [ ] **SHAP-C-01**: Shape C — A runnable `examples/` entry proves library use: an end-to-end MCP server in ≤15 lines of `main.rs` (library use of `pmcp-server-toolkit` + a chosen `pmcp-toolkit-<backend>` crate)
- [ ] **SHAP-D-01**: Shape D — `cargo pmcp deploy` packages a config-only server (pure-Rust Lambda binary) and deploys it to pmcp.run as a hosted target; the `cargo pmcp configure` target system from Phase 77 accommodates config-only server targets without breaking changes

### SEP-2640 Skills — Type 2 Authoring Skills

Validated by spike 006. New deliverable: an MCP server that ships SEP-2640 Skills for config authoring.

- [ ] **SKLL-01**: `crates/pmcp-config-helper/` MCP server crate exists and is publishable; a `pmcp-config-helper` binary runs the server with default skills bundled
- [ ] **SKLL-02**: Root `SKILL.md` covering general `config.toml` authoring conventions (curated-tool pareto, secrets refs, auth surface, code-mode opt-in)
- [ ] **SKLL-03**: Per-backend reference files (`references/postgres.md`, `references/mysql.md`, `references/athena.md`, `references/sqlite.md`) addressable via `resources/read`
- [ ] **SKLL-04**: At least one worked example bundle (complete `config.toml` + `schema.sql` for a representative use case) addressable via `resources/read`
- [ ] **SKLL-05**: Dual-surface invariant — `prompts/get` body for the bootstrap prompt is byte-equal to the root `SKILL.md` content; asserted in an in-binary integration test
- [ ] **SKLL-06**: SEP-2640 §9 compliance — supporting files (per-backend references, worked examples) are served via `resources/read` but MUST NOT appear in `resources/list`; asserted in an integration test against a representative client
- [ ] **SKLL-07**: Type 1 build-time skills in `ai-agents/` updated with toolkit-authoring patterns so coding agents writing Rust against the toolkit pick up canonical idioms (config DSL, connector trait usage, secrets binding)

### Reference-Implementation Compatibility (REF)

Prove the lift by porting at least one of the existing pmcp-run sql-api servers (`open-images`, `imdb`, `msr-vtt`) to the new toolkit, with the SDK consuming the same `config.toml` shape the platform team already wrote.

- [ ] **REF-01**: The toolkit's `config.toml` schema is a superset of the existing pmcp-run sql-api server configs — any of the three reference servers' configs parse cleanly without modification (additive new keys are allowed; renames are not)
- [ ] **REF-02**: At least one reference server (open-images recommended given Athena coverage) is reproduced end-to-end as a Shape A invocation: same tools, same code-mode policy, same observable behavior — verified by replaying a representative subset of `~/Development/mcp/sdk/pmcp-run/built-in/sql-api/reference/scenarios/` against both implementations and asserting result parity
- [ ] **REF-03**: Migration note in DOCS-01 (book chapter) documents how a pmcp-run SQL-API server author moves from the in-tree path-deps to the public toolkit (one-page recipe: swap the dep, drop the duplicate domain crates, regenerate)

### Dogfood — `crates/pmcp-server`

Demonstrate the toolkit's reach by rebuilding the SDK's own dev-tools MCP server.

- [ ] **DOGF-01**: `crates/pmcp-server` (SDK dev-tools MCP server) rewritten on top of `pmcp-server-toolkit` with at least one config-driven tool surface
- [ ] **DOGF-02**: Behavioral parity verified — the rewritten `pmcp-server` passes the existing test suite (or a documented superset) unchanged; no functional regression for downstream users

### Documentation

- [ ] **DOCS-01**: New book chapter (`pmcp-book/src/`) for config-only MCP servers — overview, the four shapes, per-backend recipes, deployment
- [ ] **DOCS-02**: New course tutorial (`pmcp-course/src/`) for config-only MCP servers — hands-on walk-through from `cargo pmcp new --kind sql-server` to deployed pmcp.run server
- [ ] **DOCS-03**: PMCP README + `CRATE-README.md` updated with config-first positioning ("build production MCP servers from config alone")
- [ ] **DOCS-04**: `examples/README.md` index updated with config-only example entries (Shape A binary use, Shape C library use)
- [ ] **DOCS-05**: cargo-pmcp README documents `new --kind sql-server` scaffolding and `deploy` for config-only server targets

### Testing

ALWAYS requirements from CLAUDE.md plus toolkit-specific coverage.

- [ ] **TEST-01**: Integration tests for each per-backend SQL crate against authentic in-process mocks (Postgres `$1`+`information_schema`, MySQL `?`+`information_schema`, Athena `?`+Glue catalog) plus a real SQLite — no Docker, no testcontainers
- [ ] **TEST-02**: Toolkit core unit + property tests covering placeholder translation invariants, code-mode prompt assembly, ToolInfo synthesis from `[[tools]]` config entries
- [ ] **TEST-03**: Public API doctest coverage for `pmcp-server-toolkit` (all public types + helpers compile and run as `rust,no_run` or `rust` doctests)
- [ ] **TEST-04**: `pmcp-config-helper` integration test asserts dual-surface byte-equality (SKLL-05) and SEP-2640 §9 list-exclusion (SKLL-06)
- [ ] **TEST-05**: `cargo pmcp new --kind sql-server` scaffold-to-run end-to-end test (scaffold a project in a tempdir, `cargo run` it against an embedded SQLite, hit `tools/list` and one `tools/call`)
- [ ] **TEST-06**: `cargo pmcp deploy` integration test for at least one config-only server target (mock or real pmcp.run target)
- [ ] **TEST-07**: Fuzz target for `config.toml` parser ensuring malformed config never panics (extends Phase 77 `pmcp_config_toml_parser` pattern)

## Previous Requirements

<details>
<summary>v2.0 Protocol Type Construction DX (Complete)</summary>

| ID | Phase | Status |
|----|-------|--------|
| PROTO-TYPE-DX | Phase 54.1 | Complete |

</details>

<details>
<summary>v1.6 CLI DX Overhaul (27/27 Complete)</summary>

- [x] FLAG-01..09 (Phase 27-28)
- [x] AUTH-01..06 (Phase 29)
- [x] TEST-01..08 (Phase 30)
- [x] CMD-01..02 (Phase 31)
- [x] HELP-01..02 (Phase 32)

</details>

<details>
<summary>v1.5 Cloud Load Testing Upload (6/6 Complete)</summary>

- [x] CLI-01..04 (Phase 25-26)
- [x] UPLD-01..03 (Phase 25-26)
- [x] VALD-01..02 (Phase 25-26)

</details>

## Future Requirements

Deferred to later milestone. Tracked but not in current roadmap.

### Configuration-Only Servers — Follow-on Backends

- **GQL-TKIT-01**: `crates/pmcp-toolkit-graphql/` per-backend crate after a GraphQL-analog of spike 005 confirms connector shape (next milestone)
- **OAPI-TKIT-01**: `crates/pmcp-toolkit-openapi/` per-backend crate — gated by Spike 007 (`openapi-auth-policy-pluggability`), not yet run; OpenAPI may stay at `pmcp-run` if policy pluggability proves not viable
- **PMACRO-SQL-01**: `#[pmcp::sql_server]` proc-macro (deferred — public toolkit on crates.io is the prerequisite)
- **PMACRO-OAPI-01**: `#[pmcp::openapi_server]` proc-macro (deferred — Spike 007 + OpenAPI toolkit are the prerequisite)
- **CFG-HELPER-EXT-01**: Extend `pmcp-config-helper` skills bundle to cover GraphQL + OpenAPI config authoring once those toolkits land
- **FED-01**: Cross-backend tool federation — a single MCP server serving tools from Postgres + Athena + ... (composed via toolkit core)
- **ARCH-DIST-01**: SEP-2640 archive distribution (`Content::Resource.blob` field + `application/gzip` + base64 blob) — blocked on upstream protocol-types gap from spike 001

### Usage-Driven Tool Promotion (long-term vision)

Capture real code-mode usage from deployed servers, surface repeated patterns, and promote them to curated `[[tools]]` config entries — improving LLM task-completion success rate and giving non-developers a natural path to define new tools from common business use cases.

- **USAGE-01**: Code-mode usage capture — deployed servers emit anonymized telemetry (query text after parameter extraction, frequency, success/failure, latency) to a configurable sink (CloudWatch / S3 / OTLP). Privacy-respecting (no parameter values, no result rows). Gated by an explicit `[telemetry] enabled = true` opt-in in `config.toml`.
- **USAGE-02**: Pattern-suggestion workflow — operator-facing `cargo pmcp toolkit suggest-tools <server>` reads captured usage, clusters semantically-equivalent queries (parameter abstraction), ranks by frequency × success rate, and prints proposed `[[tools]]` config entries the operator can paste into `config.toml`
- **USAGE-03**: Type 2 skill in `pmcp-config-helper` ("promote pattern from logs") that walks a non-developer through reviewing a suggestion + naming the tool + writing a user-facing description — closing the loop from real usage to curated tool definition entirely through the MCP server's UI

### Documentation Depth

- **DOCD-01**: Per-capability code examples in README (book/course fill this role today)
- **DOCD-02**: Separate crate-level README distinct from repo README for docs.rs
- **DOCD-03**: Community showcase ("Built with PMCP") section when real projects exist

### CLI Enhancements

- **CLIH-01**: `cargo pmcp init` interactive project setup wizard
- **CLIH-02**: `cargo pmcp config` command for managing .pmcp/config.toml
- **CLIH-03**: `cargo pmcp update` self-update mechanism

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| GraphQL toolkit in v2.2 | Separate milestone — connector shape spike not yet run; SQL is enough to prove the v2.2 thesis end-to-end |
| OpenAPI toolkit in v2.2 | Spike 007 (`openapi-auth-policy-pluggability`) has not run; until ONE `PolicyEvaluator` trait is proven viable across AVP / OPA / Cedar / bespoke RBAC, OpenAPI stays at `pmcp-run` |
| Unifying per-backend executors behind a single `SchemaServer<S, C>` trait | Spike 003 disproved viability — executors, parameter binding, and policy surfaces diverge semantically; `code_mode.rs` LoC spread of 545 / 767 / 1560 reflects real divergence |
| Docker / testcontainers for SQL backend testing | Pure-Rust Lambda binaries are the deployment target (see `feedback_avoid_docker_pure_rust_lambda` memory). Use authentic in-process mocks + pure-Rust drivers (`tokio-postgres`, `sqlx`, `aws-sdk-athena`, `rusqlite` bundled) |
| `#[pmcp::sql_server]` / `#[pmcp::openapi_server]` proc-macros in v2.2 | The toolkit being public on crates.io is the prerequisite — without it, the macro would expand to types nobody can depend on |
| SEP-2640 archive distribution (gzip blob) | `Content::Resource` lacks a `blob` field today (upstream protocol-types gap from spike 001); SEP-2640 §4 marks archive distribution as optional; ship text-mode skills first |
| Copying rmcp's trait-based architecture docs | Different SDK architecture; would be misleading |
| Per-capability inline README sections | Would make README 2000+ lines; book/course serve this role |
| Example subdirectory reorganization | High churn for low gain; flat numbering works |
| document-features crate | Adds build dep for something a manual table does equally well |
| Removing book/course/ecosystem from README | These are genuine PMCP differentiators rmcp lacks |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| EXMP-01 | Phase 65 | Complete |
| EXMP-02 | Phase 65 | Complete |
| EXMP-03 | Phase 65 | Complete |
| PROT-01 | Phase 65 | Complete |
| MACR-01 | Phase 66 | Pending |
| MACR-02 | Phase 66 | Pending |
| MACR-03 | Phase 66 | Pending |
| DRSD-01 | Phase 67 | Pending |
| DRSD-02 | Phase 67 | Pending |
| DRSD-03 | Phase 67 | Pending |
| DRSD-04 | Phase 67 | Pending |
| PLSH-01 | Phase 68 | Pending |
| PLSH-02 | Phase 68 | Pending |
| PLSH-03 | Phase 68 | Pending |
| CMSUP-01 | Phase 67.1 | Pending |
| CMSUP-02 | Phase 67.1 | Pending |
| CMSUP-03 | Phase 67.1 | Pending |
| CMSUP-04 | Phase 67.1 | Pending |
| CMSUP-05 | Phase 67.1 | Pending |
| CMSUP-06 | Phase 67.1 | Pending |
| PARITY-HANDLER-01 | Phase 70 | Pending |
| PARITY-CLIENT-01 | TBD | Pending |
| PARITY-MACRO-01 | Phase 71 | Complete |
| RMCP-EVAL-01 | Phase 72 | Complete |
| RMCP-EVAL-02 | Phase 72 | Complete |
| RMCP-EVAL-03 | Phase 72 | Complete |
| RMCP-EVAL-04 | Phase 72 | Complete |
| RMCP-EVAL-05 | Phase 72 | Complete |
| LAND-CR03-01 | Phase 72.1 | Complete |
| SDK-DCR-01 | Phase 74 | Complete |
| CLI-AUTH-01 | Phase 74 | Complete |
| REQ-77-01 | Phase 77 | Complete |
| REQ-77-02 | Phase 77 | Complete |
| REQ-77-03 | Phase 77 | Complete |
| REQ-77-04 | Phase 77 | Complete |
| REQ-77-05 | Phase 77 | Complete |
| REQ-77-06 | Phase 77 | Complete |
| REQ-77-07 | Phase 77 | Complete |
| REQ-77-08 | Phase 77 | Complete |
| REQ-77-09 | Phase 77 | Complete |
| REQ-77-10 | Phase 77 | Complete |
| REQ-77-11 | Phase 77 | Complete |
| BLDR-01 | TBD (v2.2) | Pending |
| BLDR-02 | TBD (v2.2) | Pending |
| BLDR-03 | TBD (v2.2) | Pending |
| TKIT-01 | TBD (v2.2) | Pending |
| TKIT-02 | TBD (v2.2) | Pending |
| TKIT-03 | TBD (v2.2) | Pending |
| TKIT-04 | TBD (v2.2) | Pending |
| TKIT-05 | TBD (v2.2) | Pending |
| TKIT-06 | TBD (v2.2) | Pending |
| TKIT-07 | TBD (v2.2) | Pending |
| TKIT-08 | TBD (v2.2) | Pending |
| TKIT-09 | TBD (v2.2) | Pending |
| TKIT-10 | TBD (v2.2) | Pending |
| CONN-01 | TBD (v2.2) | Pending |
| CONN-02 | TBD (v2.2) | Pending |
| CONN-03 | TBD (v2.2) | Pending |
| CONN-04 | TBD (v2.2) | Pending |
| CONN-05 | TBD (v2.2) | Pending |
| CONN-06 | TBD (v2.2) | Pending |
| CONN-07 | TBD (v2.2) | Pending |
| CONN-08 | TBD (v2.2) | Pending |
| SHAP-A-01 | TBD (v2.2) | Pending |
| SHAP-B-01 | TBD (v2.2) | Pending |
| SHAP-C-01 | TBD (v2.2) | Pending |
| SHAP-D-01 | TBD (v2.2) | Pending |
| SKLL-01 | TBD (v2.2) | Pending |
| SKLL-02 | TBD (v2.2) | Pending |
| SKLL-03 | TBD (v2.2) | Pending |
| SKLL-04 | TBD (v2.2) | Pending |
| SKLL-05 | TBD (v2.2) | Pending |
| SKLL-06 | TBD (v2.2) | Pending |
| SKLL-07 | TBD (v2.2) | Pending |
| REF-01 | TBD (v2.2) | Pending |
| REF-02 | TBD (v2.2) | Pending |
| REF-03 | TBD (v2.2) | Pending |
| DOGF-01 | TBD (v2.2) | Pending |
| DOGF-02 | TBD (v2.2) | Pending |
| DOCS-01 | TBD (v2.2) | Pending |
| DOCS-02 | TBD (v2.2) | Pending |
| DOCS-03 | TBD (v2.2) | Pending |
| DOCS-04 | TBD (v2.2) | Pending |
| DOCS-05 | TBD (v2.2) | Pending |
| TEST-01 | TBD (v2.2) | Pending |
| TEST-02 | TBD (v2.2) | Pending |
| TEST-03 | TBD (v2.2) | Pending |
| TEST-04 | TBD (v2.2) | Pending |
| TEST-05 | TBD (v2.2) | Pending |
| TEST-06 | TBD (v2.2) | Pending |
| TEST-07 | TBD (v2.2) | Pending |

**Coverage:**
- v2.1 requirements: 42 total (20 pre-seed + 3 seeded by Phase 69 + 5 seeded by Phase 72 + 1 seeded by Phase 72.1 CR-03 + 2 seeded by Phase 74 + 11 seeded by Phase 77)
- v2.2 requirements: 49 total (BLDR ×3 + TKIT ×10 + CONN ×8 + SHAP ×4 + SKLL ×7 + REF ×3 + DOGF ×2 + DOCS ×5 + TEST ×7)
- Mapped to phases: v2.1 42 / 42; v2.2 0 / 49 (roadmap pending)
- Unmapped: v2.2 has 49 IDs pending phase assignment until ROADMAP.md is created

---
*Requirements defined: 2026-04-10*
*Last updated: 2026-04-16 — added 3 PARITY-* IDs seeded by Phase 69 rmcp parity research*
*Last updated: 2026-04-17 — PARITY-MACRO-01 closed by Phase 71 (pmcp 2.4.0 / pmcp-macros 0.6.0 / pmcp-macros-support 0.1.0 — rustdoc fallback)*
*Last updated: 2026-04-19 — added 5 RMCP-EVAL-* IDs seeded by Phase 72 rmcp foundation evaluation research (reviews-mode revised)*
*Last updated: 2026-04-20 — Phase 72 Plan 03 closed RMCP-EVAL-05 (recommendation = D). Traceability updated.*
*Last updated: 2026-04-20 — added LAND-CR03-01 seeded by Phase 72.1 CR-03 rev-2 (cargo-pmcp 0.8.1 landing runtime fetch).*
*Last updated: 2026-04-20 — Phase 72.1 complete: cargo-pmcp 0.8.1 landing template runtime /landing-config fetch (AC-11 manual offline gate approved by operator guy).*
*Last updated: 2026-04-26 — added 11 REQ-77-* IDs seeded by Phase 77 cargo pmcp configure commands research.*
*Last updated: 2026-05-17 — milestone v2.2 (Configuration-Only MCP Servers) defined. Added 49 IDs across BLDR / TKIT / CONN / SHAP / SKLL / REF / DOGF / DOCS / TEST categories, derived from spikes 003–006 + the auto-loaded `spike-findings-rust-mcp-sdk` skill. Grounded in the existing `pmcp-run/built-in/sql-api/servers/` reference implementation (open-images / imdb / msr-vtt) — the toolkit must consume their `config.toml` shape (incl. dual-mode curated `[[tools]]` + `[code_mode]` policy + `[[database.tables]]` schema enrichment) as a superset, not redesign it. Long-term USAGE-01..03 vision (usage-driven tool promotion from code-mode logs) captured in Future Requirements. Phase assignment pending ROADMAP.md creation.*
