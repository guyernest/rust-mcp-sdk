# Requirements: PMCP SDK rmcp Upgrades

**Defined:** 2026-04-10
**Core Value:** Close credibility and DX gaps where rmcp outshines PMCP — documentation accuracy, feature gate presentation, macro documentation, example index, and repo hygiene.

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

- [ ] **REQ-77-01**: `cargo pmcp configure {add,use,list,show}` subcommand group ships under a new `configure/` module — each subcommand persists or reads target state and emits stable text/JSON output.
- [x] **REQ-77-02**: `~/.pmcp/config.toml` schema is a `#[serde(tag = "type")]` enum with variants `pmcp-run`, `aws-lambda`, `google-cloud-run`, `cloudflare-workers`; per-variant structs use `#[serde(deny_unknown_fields)]` so typos are rejected at parse time.
- [x] **REQ-77-03**: `.pmcp/active-target` workspace marker is a single-line plain-text file containing only the active target name; permissive on read (trim+UTF-8 normalize), strict on write.
- [ ] **REQ-77-04**: `PMCP_TARGET=<name>` env var is the highest-priority target selector and emits a stderr override note when it overrides the workspace marker; the note fires even when `--quiet` is set. `--target <name>` is a new global flag on the top-level `Cli`.
- [ ] **REQ-77-05**: A header banner is emitted to stderr by every target-consuming command before any AWS API / CDK / upload call; field ordering is fixed (api_url / aws_profile / region / source); banner is suppressible by `--quiet` (except the D-03 PMCP_TARGET override note).
- [ ] **REQ-77-06**: Field-level precedence at command-execution time is `ENV > explicit --flag > active target > .pmcp/deploy.toml`; verified by property test.
- [x] **REQ-77-07**: `configure add` rejects raw-credential patterns (AKIA[0-9A-Z]{16}, ASIA[0-9A-Z]{16}, ghp_*, github_pat_*, sk_live_*, AIza*) with an actionable error pointing the user at AWS profile names / env-var refs / Secrets Manager ARNs.
- [x] **REQ-77-08**: `~/.pmcp/config.toml` writes are atomic via `tempfile::NamedTempFile::persist`; on Unix the file is `0o600`, the parent dir `0o700`; concurrent writers are last-writer-wins (no partial file).
- [ ] **REQ-77-09**: When `~/.pmcp/config.toml` does not exist, `cargo pmcp deploy` and `cargo pmcp pmcp.run upload` behave byte-identically to Phase 76 — no banner about targets, no migration nag, zero touch.
- [ ] **REQ-77-10**: ALWAYS gates pass: `cargo fuzz run pmcp_config_toml_parser -- -max_total_time=60`, `cargo test -p cargo-pmcp configure::config::proptests`, `cargo test -p cargo-pmcp configure::resolver::proptests::precedence_holds`, `cargo run --example multi_target_monorepo -p cargo-pmcp` all exit 0.
- [ ] **REQ-77-11**: Banner emission integrates with ALL target-consuming entry points enumerated in 77-RESEARCH §7 (HIGH-2 per 77-REVIEWS.md): `commands/deploy/mod.rs` (8+ AWS-touching sites), `commands/test/upload.rs` (top of `execute` before `auth::get_credentials()`), `commands/loadtest/upload.rs` (same pattern), and `commands/landing/deploy.rs` (lines 69, 215, 334). The OnceLock-guarded `emit_resolved_banner_once` makes duplicate calls within a single process invocation safe.

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
| REQ-77-01 | Phase 77 | Pending |
| REQ-77-02 | Phase 77 | Complete |
| REQ-77-03 | Phase 77 | Complete |
| REQ-77-04 | Phase 77 | Pending |
| REQ-77-05 | Phase 77 | Pending |
| REQ-77-06 | Phase 77 | Pending |
| REQ-77-07 | Phase 77 | Complete |
| REQ-77-08 | Phase 77 | Complete |
| REQ-77-09 | Phase 77 | Pending |
| REQ-77-10 | Phase 77 | Pending |
| REQ-77-11 | Phase 77 | Pending |

**Coverage:**
- v2.1 requirements: 42 total (20 pre-seed + 3 seeded by Phase 69 + 5 seeded by Phase 72 + 1 seeded by Phase 72.1 CR-03 + 2 seeded by Phase 74 + 11 seeded by Phase 77)
- Mapped to phases: 42
- Unmapped: 0

---
*Requirements defined: 2026-04-10*
*Last updated: 2026-04-16 — added 3 PARITY-* IDs seeded by Phase 69 rmcp parity research*
*Last updated: 2026-04-17 — PARITY-MACRO-01 closed by Phase 71 (pmcp 2.4.0 / pmcp-macros 0.6.0 / pmcp-macros-support 0.1.0 — rustdoc fallback)*
*Last updated: 2026-04-19 — added 5 RMCP-EVAL-* IDs seeded by Phase 72 rmcp foundation evaluation research (reviews-mode revised)*
*Last updated: 2026-04-20 — Phase 72 Plan 03 closed RMCP-EVAL-05 (recommendation = D). Traceability updated.*
*Last updated: 2026-04-20 — added LAND-CR03-01 seeded by Phase 72.1 CR-03 rev-2 (cargo-pmcp 0.8.1 landing runtime fetch).*
*Last updated: 2026-04-20 — Phase 72.1 complete: cargo-pmcp 0.8.1 landing template runtime /landing-config fetch (AC-11 manual offline gate approved by operator guy).*
*Last updated: 2026-04-26 — added 11 REQ-77-* IDs seeded by Phase 77 cargo pmcp configure commands research.*
