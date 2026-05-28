---
gsd_state_version: 1.0
milestone: v2.2
milestone_name: Configuration-Only MCP Servers
status: verifying
stopped_at: Completed 86-05-PLAN.md (Shape D config-driven deploy — H3/H1/H4/M3 + D-10 guard)
last_updated: "2026-05-27T21:18:16.579Z"
last_activity: 2026-05-27
progress:
  total_phases: 44
  completed_phases: 38
  total_plans: 171
  completed_plans: 171
  percent: 86
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-05-17)

**Core value:** Enterprise developers build production-grade SQL MCP servers from configuration + schema files alone — no Rust required — while preserving PMCP's security, tools/resources/prompts/tasks/skills standards and pmcp.run hosting integration.
**Current focus:** Phase 86 — shapes-b-c-d-scaffold-library-example-deploy

## Current Position

Phase: 999.1
Plan: Not started
Status: Phase complete — ready for verification
Last activity: 2026-05-27

**Carryover from v2.1:** Phase 81 (update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod) was executing at v2.1 close; will be tracked separately and folded into v2.1 completion. Operator follow-ups deferred from Phase 75 Wave 5 still pending: (a) merge Phase 75 Wave 5 + 75.5 to paiml/rust-mcp-sdk:main; (b) post-merge run `gh workflow run quality-badges.yml -R paiml/rust-mcp-sdk` and append observation to `.planning/phases/75-fix-pmat-issues/75-05-GATE-VERIFICATION.md` "## Badge flip observation" section.

## v2.2 Phase Plan (8 phases, 49 requirements)

| Phase | Goal | Reqs | Critical-path |
|-------|------|------|---------------|
| 82 | Builder DX Prerequisites — `tool_arc` / `prompt_arc` on public builder + in-process driver | 3 | yes — blocks 83+ |
| 83 | Toolkit Core Lift (`pmcp-server-toolkit`) — public crates.io crate, ToolInfo synthesis, code-mode wiring | 12 | yes — anchor, blocks 84/87/88 |
| 84 | SQL Connectors — Postgres/MySQL/Athena crates + SQLite feature flag, pure-Rust drivers | 10 | yes — anchor, blocks 85/86/88 |
| 85 | Shape A Pure-Config Binary + Reference Parity — `pmcp-sql-server`, open-images reproduction | 3 | yes — proves the lift |
| 86 | Shapes B/C/D — Scaffold + ≤15-line Example + Deploy | 5 | branching from 85 |
| 87 | Type 2 Authoring Skills Server (`pmcp-config-helper`) | 8 | branching from 83 |
| 88 | Dogfood — `crates/pmcp-server` on toolkit | 2 | branching from 83+84 |
| 89 | Documentation, Migration Guide & Examples Index | 6 | finalizes milestone |

**Execution order:** 82 → 83 → 84 → 85 → (86 ‖ 87) → 88 → 89

## Shipped Milestones

| Version | Name | Phases | Date |
|---------|------|--------|------|
| v1.0 | MCP Tasks Foundation | 1-3 | 2026-02-22 |
| v1.1 | Task-Prompt Bridge | 4-8 | 2026-02-23 |
| v1.2 | Pluggable Storage Backends | 9-13 | 2026-02-24 |
| v1.3 | MCP Apps Developer Experience | 14-19 | 2026-02-26 |
| v1.4 | Book & Course Update | 20-24 | 2026-02-28 |
| v1.5 | Cloud Load Testing Upload | 25-26 | 2026-03-01 |

## Performance Metrics

**Velocity:**

- Total plans completed: 152 (v1.0: 9, v1.1: 10, v1.2: 9, v1.3: 12, v1.4: 10, v1.5: 6, v1.6: 5, v1.7: 4, v2.0: 11)
- Total phases completed: 29

## Accumulated Context

### Decisions

See PROJECT.md Key Decisions table for full history.

v2.2 decisions (this session):

- **Phase numbering:** Continue from v2.1's last phase (81). v2.2 starts at Phase 82, no `--reset-phase-numbers` flag.
- **8 phases derived** from 49 requirements respecting spike-validated dependency order: BLDR → TKIT → CONN → SHAP-A → (SHAP-B/C/D ‖ SKLL) → DOGF → DOCS.
- **Anchor phases:** Phase 83 (TKIT, 12 reqs) and Phase 84 (CONN, 10 reqs) are the two intentionally-large "lift the proto-SDK" phases. All other phases sit at 2–8 reqs per the user's medium-sized-phase guidance.
- **TEST-* requirements distributed**, not collected — TEST-01 + TEST-07 → 84 (connector tests + fuzz), TEST-02 + TEST-03 → 83 (toolkit unit/property/doctest), TEST-04 → 87 (dual-surface + §9), TEST-05 + TEST-06 → 86 (scaffold-to-run + deploy integration). Per CLAUDE.md ALWAYS requirements each phase carries its own test types.
- **Phase 85 = Shape A + REF parity together** — reproducing open-images via Shape A IS the parity check for REF-02; separating them would force a synthetic intermediate.
- **Phase 87 (SKLL) is parallelisable with Phase 86 (B/C/D)** after Phase 83 lands — neither depends on the other.
- **Phase 89 absorbs REF-03** (migration recipe) into DOCS-01 (book chapter) rather than as a standalone phase — same audience, same surface, same artifact.
- **Hard-encoded invariants in success criteria:** REF-01 superset (no renames) called out in P83 SC-2 + P85 SC-2; pure-Rust Lambda + no-Docker in P84 SC-4; dual-mode intentional in P83 SC-2/SC-3; dual-surface byte-equality in P87 SC-2; SEP-2640 §9 list-exclusion in P87 SC-3.
- **[Plan 85-10] WR-02 design = HONOR (bind) execute_code variables, not reject** — `variables_to_params` maps the JSON-object input to `(name, value)` pairs (leading `:` stripped) and binds them; a schema-advertised input is never a silent no-op. None/non-object → empty so the parity scenario (None) is unaffected.
- **[Plan 85-10] SqlCodeExecutor::new is now fallible** (`Result<Self>`) so the ValidationPipeline is built ONCE and cached; `token_secret` is resolved at builder time, not per request (IN-01). A removed env var after startup no longer breaks in-flight requests.
- **[Plan 85-10] Robustness invariant: set-but-empty env vars are treated as UNSET** — `token_secret` (env:/${VAR}) and `AWS_REGION`/`AWS_DEFAULT_REGION` reject empty/whitespace (clear error or fall-through), never a degenerate present-empty value.
- **[Plan 85-10] Serve-task JoinError propagates as RunError::Serving** — `run()` does `handle.await.map_err(RunError::Serving)?` so a serve-task panic exits non-zero (supervisor restart), not silent exit 0.
- **[Plan 85-10] sqlite accepts the documented `database = ":memory:"/<path>` form** as a fallback when `file_path` is absent (file_path precedence); reconciles dispatch with config.rs DatabaseSection docs.

Inherited from v2.1 (see PROJECT.md + prior Decisions log):

- 4 phases derived from 5 requirement categories following research-recommended dependency order: examples+protocol -> macros -> docs.rs pipeline -> polish
- EXMP and PROT combined into Phase 65 (both are credibility fixes, no dependency between them, co-deliverable)
- Phase ordering follows the docs.rs build pipeline dependency: content accuracy first, then rendering pipeline, then polish
- No new runtime dependencies for this milestone -- all fixes are config, content, and attribute changes
- [Phase 65]: All 17 orphan examples compile successfully -- registered all with import-derived feature flags (no deletions needed)
- [Phase 65]: examples/README.md replaced with PMCP example index — 63 examples categorized by Role/Capability/Complexity + migration reference
- [Phase ?]: Plan 82-02: Property test asserts public observable only — handle output byte-equality + has_tool — not the private capabilities field; capability-shape equivalence lives in Plan 82-01 Task 3 crate-internal test
- [Phase ?]: Plan 82-02: USAGE-narrowed negative grep rejects method-call sites + import statements for the private dispatch entry point; module-doc prose carefully avoids those literal token shapes so the grep stays at zero matches
- [Phase 83]: Single-prompt-per-handler shape for StaticPromptHandler reconciles plural source config with pmcp's singular trait via from_configs factory
- [Phase 83]: StaticResourceHandler enforces IndexMap (Pattern D) and Content::resource_with_text (PATTERNS §5) even though source used HashMap+struct-literal Content::Resource
- [Phase 83]: Local PromptInfoOut alias in prompts.rs avoids verify-regex false-positive against return-type braces while keeping rustfmt happy
- [Phase ?]: [Phase 83 Plan 04] ServerConfig three-entry-point API (from_toml / validate / from_toml_strict_validated) per review R8 — partial-merge callers parse without validating; production callers chain both via the convenience method
- [Phase ?]: [Phase 83 Plan 04] ParamDecl::default + enum_values typed as toml::Value (heterogeneous) — reference fixtures emit integer-, string-, and boolean-defaults; forcing a String coercion would shift the problem to Plan 05's synthesizer
- [Phase ?]: [Phase 83 Plan 04] All 3 reference fixtures pass validate() on first run — empirically confirms the R8 rule-set (4 required-field checks) is well-calibrated to production usage; no rule was weakened to accommodate a broken fixture
- [Phase ?]: Plan 83-05: SynthesizedToolHandler::handle returns Err(pmcp::Error::Internal) (not Value with is_error) — honors real ToolHandler::handle signature Result<Value> vs the plan example's Result<CallToolResult>. Semantically equivalent and Gemini-review-compliant.
- [Phase ?]: Plan 83-05: pub type SynthesizedTool = (String, ToolInfo, Arc<dyn ToolHandler>) introduced to satisfy clippy::type_complexity while preserving PATTERNS §9 tuple shape. Public alias enables Plan 08 to name the type.
- [Phase ?]: Plan 83-06 selected R1 split (validation_pipeline_from_config + code_mode_tools_from_executor) — pmcp-code-mode CodeExecutor requires backend injection; no config-only constructor exists.
- [Phase ?]: Plan 83-06 R9 inline-secret enforcement: token_secret defaults to env:VAR_NAME; inline literals rejected via ConfigValidationError::InlineSecretRejected unless allow_inline_token_secret_for_dev=true.
- [Phase ?]: Plan 83-06 toolkit code-mode feature now forwards pmcp-code-mode/sql-code-mode so SC-3 anchor (allow_writes=false rejects INSERT) compiles under --features code-mode.
- [Phase ?]: Plan 83-07: Accepted R2 minimization — SqlConnector ships only dialect() + schema_text() in 0.1.0
- [Phase ?]: Plan 83-07: MockSqlConnector stays pub(crate) — Plan 08 smoke test reaches it under --features sqlite
- [Phase ?]: Phase 83 Plan 09: 21 toolkit contract rows added in CRATE-ROOT module_path form per review R3; pmat comply check reports 0 ghost bindings (CB-1338)
- [Phase ?]: Phase 83 Plan 09: cargo publish --dry-run revealed expected D-08 cross-release dependency — pmcp must publish 2.9.x (or 2.8.2) with Phase 82 tool_arc before pmcp-server-toolkit 0.1.0 can ship. Publish-gate working as designed.
- [Phase ?]: Phase 83 Plan 09: 83-VALIDATION.md flipped to nyquist_compliant: true; 24 task rows ✅ green; Phase 83 fully validated.
- [Phase ?]: Phase 84 Wave 0: scaffolded pmcp-toolkit-{postgres,mysql,athena} crates + translate.rs RED proptest shell; kept documented '# NO aws-sdk-glue' comment per PATTERNS (no Glue dep in build graph)
- [Phase ?]: [Phase 84 Plan 01] SqlConnector extended to 3 methods (dialect/execute/schema_text); only in-tree impl MockSqlConnector needed updating — Wave 0 backend stubs do not yet impl the trait, so adding execute() with no default body broke nothing. ConnectorError gained Driver/Query/ParameterBind/Connection variants additively via #[non_exhaustive]; Connection carries a redaction Rustdoc mandate + leak-guard test (T-84-01-01).
- [Phase ?]: [Phase 84 Plan 02] translate_placeholders SqlWalker uses a CastTypeName one-shot swallow state for ::-casts (1::int / :id::text / 'foo'::text fall out of one rule); Placeholder is only entered when peek() is [A-Za-z_] so pending_name is never empty. 5 RED proptests GREEN + 4 H7 named edge tests; PMAT cog <=25, zero cognitive_complexity allows.
- [Phase ?]: [Phase 84 Plan 03] REVIEWS M1: widget_meta flip uses ToolInfo::with_meta_entry("ui", {resourceUri}) NOT with_widget_meta — the latter is #[cfg(feature="mcp-apps")] and the toolkit pulls pmcp with default-features=false (no mcp-apps); with_meta_entry is feature-independent, chainable (preserves annotations), and produces the ui.resourceUri shape ToolInfo::widget_meta() recognises so pmcp core with_widget_enrichment fires structuredContent (D-06). Verified clean on both default and --no-default-features builds.
- [Phase ?]: [Phase 84 Plan 03] synthesizer split additive (variant-not-overload): synthesize_inner(cfg, Option<Arc<dyn SqlConnector>>) shared by unchanged synthesize_from_config (None) + new synthesize_from_config_with_connector (Some) — all 11 P83 callers + line-91 re-export + line-130 fn-type assertion compile untouched. SynthesizedToolHandler holds Option<connector>; handle() executes SQL when wired (extract_named_params filters to declared params, T-84-03-01; ConnectorError via sanitized Display, T-84-03-02), explicit Err when not (T-84-03-05). build_code_mode_prompt alias (CONN-04) + DatabaseSection.url field (D-08) landed; database-url fuzz seed now parses valid.
- [Phase ?]: [Phase 84 Plan 04] SqliteConnector promoted to public type behind sqlite feature (CONN-08): Arc<Mutex<Connection>> + tokio::task::spawn_blocking per call (std::sync::Mutex held only inside the closure). sqlite feature now pulls dep:tokio with rt (Rule 3 — spawn_blocking otherwise unavailable; aws keeps sync, union on one optional dep). schema_text reads sqlite_master live (dropped spike schema_blob cache). MockSqlConnector kept pub(crate), coexists. Wave 1 complete: 108 lib tests green, 4-test D-06 integration anchor (REVIEWS H1) passes, :name-only seed (REVIEWS H4).
- [Phase 84]: PgParam ships 5 scalar variants; object/array params rejected with ConnectorError::ParameterBind (REVIEWS M2), JSON deferred to v0.3
- [Phase 84]: PostgresMock lives at src/dev_mock.rs under the dev_mock feature (REVIEWS H5); legacy tests/mock_postgres.rs removed so publishable examples can opt in without referencing tests/
- [Phase ?]: 84-06: MysqlConnector uses MySqlPool::connect_lazy — offline-safe constructor, malformed URL fails synchronously, TCP deferred to first use (REVIEWS M3)
- [Phase ?]: 84-06: MysqlMock lives at src/dev_mock.rs under the dev_mock feature (tests/mock_mysql.rs deleted) so the publishable example reaches it via the public path (REVIEWS H5)
- [Phase 84]: 84-07: AthenaConnector ships via aws-sdk-athena 1.106.0 with NO aws-sdk-glue (Landmine #4) — schema_text rides GetTableMetadata. from_config stays EXACTLY 2 args (region, workgroup) per D-08 LOCKED; AthenaConfig + with_* builders + from_athena_config carry the rest (REVIEWS M4); runtime gate rejects empty output_location before any AWS call.
- [Phase 84]: 84-07: paginated_get_query_results loops on next_token across all pages (REVIEWS M5); AthenaMock simulates multi-page via with_pages + PAGINATED_QUERY_MARKER so the M5 test asserts pagination without live Athena. AthenaMock lives at src/dev_mock.rs under dev_mock feature (H5); legacy tests/mock_athena.rs deleted. strip_aws_credentials is per-whitespace-token (cog ≤25). Wave 2 of Phase 84 COMPLETE (postgres+mysql+athena+sqlite).
- [Phase ?]: Phase 84 Plan 08: extended the single config-parser fuzz target corpus (D-14) with 3 per-backend + 4 REVIEWS M6 adversarial URL seeds; 60s/1.19M-run fuzz clean. Tracked only named seed-*.toml (libfuzzer hash entries left untracked per Phase 77/79 convention).
- [Phase ?]: Phase 84 Plan 08: aws-sdk-glue guard satisfied by intent (cargo tree shows zero Glue) — the two matches are intentional 'NO aws-sdk-glue' doc comments kept per Wave 0; verification sweep scoped to Phase 84's 4 crates because broad make quality-gate is blocked by pre-existing unrelated rust-1.95.0 pedantic lints in pmcp-widget-utils (deferred-items.md, NOT fixed).
- [Phase ?]: Phase 84 COMPLETE (9/9): CONN-01..08 + TEST-01 + TEST-07 closed; 84-VALIDATION nyquist_compliant true. CONN-07/TEST-01 Athena descriptions corrected from 'Glue catalog' to GetTableMetadata (Landmine #4/D-08).
- [Phase ?]: [Phase 85 Plan 01] ${VAR} token-secret expansion scoped to token_secret only (Codex MEDIUM #6); Athena output_location keeps ${...} verbatim; general from_toml_with_env_expansion deferred.
- [Phase ?]: [Phase 85 Plan 03] Scaffolded crates/pmcp-sql-server (Shape A binary): feature-gated 4-connector manifest (sqlite/postgres/mysql/athena all default-on, D-07), lib/main split (testable run() in lib.rs, 3-line tokio shim in main.rs). Vendored FOUR self-contained parity fixtures into the SDK repo (closes RESEARCH Open Question #1).
- [Phase ?]: [Phase 85 Plan 03] REVIEW FIX #1: vendored the DATA-BEARING chinook.db (~984 KB) verbatim, NOT a schema-only stub — generated.yaml asserts on real values (Rock/AC-DC/For Those About To Rock); an empty DB would fail the Plan 06 replay. Verified through the SAME SqliteConnector path the parity harness uses.
- [Phase ?]: [Phase 85 Plan 03] exclude = [tests/, .planning/, .pmat/, fuzz/] keeps the ~1MB chinook.db blob (and all fixtures) out of the published crate — cargo package --list ships only Cargo.toml + src/{lib,main}.rs. The separate chinook.ddl (11 CREATE TABLE) is the --schema text input (D-06), distinct from the .db data file.
- [Phase ?]: [Phase 85 Plan 03] rusqlite (bundled) added as dev-dep so the standalone-DDL test can execute_batch the 11-statement schema (toolkit's single-statement SqlConnector::execute can't); generated.yaml parses via serde_yaml → mcp_tester::TestScenario. Pre-existing rust-1.95.0 clippy lints in pmcp-server-toolkit dep surfaced under -D warnings but are out of scope (deferred-items.md).
- [Phase ?]: [Phase 85 Plan 02] try_code_mode_from_config_with_connector (LOCKED) registers validate_code + execute_code via SqlCodeExecutor; connectorless try_code_mode_from_config stays validation-only/no-tool (Codex HIGH #4 resolved).
- [Phase ?]: [Phase 85 Plan 02] Hand-built the two ToolHandlers in code_mode.rs (no pmcp-code-mode-derive dep); NoopPolicyEvaluator makes static [code_mode] flags THE authorization (D-13, SC-3). DELETE/DDL rejected on a read-only config.
- [Phase ?]: [Phase 85 Plan 02] execute_code success payload = {rows:<values>} mirroring production observable shape; single-method SqlConnector has no columns/rows_affected channel (Codex #6b). assemble_code_mode_prompt_with_schema is sync/connectorless (D-04/D-05/SC-1).
- [Phase 85]: Plan 85-04: DispatchError::SqliteOpen is a path-free variant (T-85-04-01) — rusqlite's open error echoes the file path, so the SQLite arm map_err's it instead of forwarding raw ConnectorError; URL backends already redact at source
- [Phase 85]: Plan 85-04: Athena dispatch offline-safety (T-85-04-04) CONFIRMED — explicit region (AWS_REGION/AWS_DEFAULT_REGION/us-east-1 fallback) stops aws_config::load() IMDS probe; creds lazy; no execute/schema_text at dispatch; no-creds test completes 0.43s under 10s timeout
- [Phase 85]: Plan 85-04: dispatch() per-backend arms split into feature-gated dispatch_<backend> helper pairs (#[cfg(feature)] impl + #[cfg(not)] FeatureMissing stub) keeping each cog <=25; clap Args re-exported at crate root
- [Phase 85]: Plan 85-05: build_server preserves ALL configured resources (merge_schema_resource overrides ONLY the /schema URI content) + the configured start_code_mode prompt (StaticPromptHandler::from_configs against the merged handler); code-mode via the LOCKED connector-aware API
- [Phase 85]: Plan 85-05: lib::run serves over streamable HTTP via StreamableHttpServer::with_config(default()).start() (Phase 56 adapter); serve(server,addr) returns (bound_addr,handle) non-blocking so tests drive it. SC-1 timeout-guards athena lazy startup (no creds); SC-2 all four configs parse+dispatch right dialect (all 3 non-sqlite fixtures are athena, not mysql)
- [Phase ?]: [Phase 85 Plan 06] run_serving extracted as a testable seam (bound_addr, handle); run() delegates then awaits — the parity test drives the IDENTICAL real binary path (config-load -> dispatch -> build_server -> serve), NOT a connector injection (Codex HIGH #5).
- [Phase ?]: [Phase 85 Plan 06] Rule 1 bug: extract_named_params applies declared [[tools.parameters]] defaults when the arg is omitted — fixes unbound-NULL :limit/:offset binding (SQLite LIMIT NULL -> datatype mismatch) that broke search_tracks/list_artists.
- [Phase ?]: [Phase 85 Plan 06] Rule 1 bug: ValidateCodeHandler surfaces a policy rejection as a tool Err (isError:true) — the reference observable the generated.yaml DELETE/DDL/no-LIMIT failure assertions verify (SC-3); Plan 85-02 rejection tests updated.
- [Phase ?]: [Phase 85 Plan 06] Phase 85 COMPLETE — Shape A reproduces the production Chinook reference; all 29 parity scenarios pass through the real --config --schema path via mcp-tester (REF-02/SC-3/SC-4).
- [Phase ?]: [Phase 85 Plan 07] Gap 1 closed: sql_require_limit added (additive #[serde(default)]) to pmcp-code-mode CodeModeConfig + enforced in check_sql_config_authorization Select arm (missing_limit rule); toolkit build_cm_config now maps section.require_limit -> cfg.sql_require_limit (was discarded _require_limit_gap). Bare SELECT rejected independent of sql_max_rows; LIMITed read + writes unaffected.
- [Phase ?]: [Phase 85 Plan 09] Gap 3 closed: assemble.rs synthesizes code-mode://instructions + code-mode://policies from [code_mode] config + dialect, merged (dedup-by-URI, operator override wins) before prompt resolution; the start_code_mode prompt's 2 previously-warn-skipped include_resources now resolve; policy body renders NON-secret fields only (token_secret never emitted, T-85-09-01); merge_schema_resource /schema override scoped to first match.
- [Phase ?]: 85-08: SC-3 negative-path parity gate strengthened — parity_chinook.rs now gates on per-step StepResult.success (continue_on_failure-independent) plus a presence guard for the 5 policy-rejection scenarios; fixtures byte-unchanged; two-sided regression proof recorded (Gap 1 reverted -> no-LIMIT step fails; restored -> green)
- [Phase ?]: [Plan 86-01] execute_batch is an inherent method on the concrete SqliteConnector (NOT the locked SqlConnector trait) — callers invoke it before the Arc<dyn> wrap (Review H2); mirrors execute()'s spawn_blocking shape, batch failures map to ConnectorError::Query
- [Phase ?]: [Plan 86-01] H1 path resolution decided ONCE: demo_db_path()=/tmp/demo.db under Lambda (LAMBDA_TASK_ROOT set), else relative demo.db; config.toml/schema.sql via pmcp::assets::load_string (Lambda /var/task/assets, local cwd/PMCP_ASSETS_DIR). Exported for 86-02/03/05.
- [Phase ?]: [Plan 86-01] toolkit http=[pmcp/streamable-http] feature is opt-in (NOT default) — required because [[example]] required-features can only name toolkit features and the toolkit pmcp dep is default-features=false; dev-dep tokio widened to rt-multi-thread, published deps tokio unchanged.
- [Phase ?]: [Plan 86-01 SPIKE] RESEARCH Open Q#2 = NO: find_lambda_package_dir (builder.rs:312) does NOT resolve a single-crate layout (needs <server>-lambda dir or *-lambda pkg with bootstrap, else bail). Plan 05 MUST add cargo-pmcp/src/deployment/builder.rs to files_modified; seam = build_lambda_binary call at builder.rs:132; scaffolded deploy.toml MUST set target_type=pmcp-run (get_target_id has no shape inference); bundler puts config.toml at zip root + assets under assets/ (H1 validated).
- [Phase ?]: [Plan 86-02] Shape C example main body is 12 statement lines (≤15 M4): hoisted PMCP_ASSETS_DIR (FIXTURES_DIR const) + HTTP boilerplate (private serve() helper) OUT of main because rustfmt forcibly wraps the builder chain + with_config args. The serve() helper inlines StreamableHttpServer (NOT pmcp_sql_server::serve, Pitfall §2); Plan 03 emits a call to the same shape.
- [Phase ?]: [Plan 86-02] M4 ≤15-line assertion counts STATEMENT lines not raw physical lines — skips rustfmt method-chain continuations (trimmed start '.') and lone closing-delimiter continuations; a wrapped statement counts once. Documented verbatim in both the example and the test.
- [Phase ?]: [Plan 86-02] Integration test spawns 'cargo run --example' (examples get no CARGO_BIN_EXE_<example>) and parses the printed PMCP_SQL_SERVER_ADDR= line; ChildGuard(Drop-kill) reaps the subprocess on panic, readiness timeout fails WITH captured stdout/stderr.
- [Phase 86]: Phase 86-03: cargo pmcp new --kind sql-server emits a single runnable crate; emitted main.rs IS the Plan 02 Shape C wiring (H1/H2) guarded by a golden drift test; validate_crate_name blocks path-traversal before fs::write
- [Phase 86]: TEST-05 (cargo-pmcp/tests/scaffold_sql_server.rs) compiles via --no-run; full cold-tempdir execution deferred to the orchestrator (15+ min build of the unpublished toolkit)
- [Phase 86]: scaffold_patch.rs [patch.crates-io] writer covers transitive unpublished deps (pmcp-code-mode, pmcp-widget-utils), not just pmcp + pmcp-server-toolkit
- [Phase 86]: No [dev-dependencies] added to cargo-pmcp/Cargo.toml: mcp-tester/tempfile/tokio/serde_json already in [dependencies] (inherited by integration tests)
- [Phase 86]: [Plan 86-05] cargo pmcp deploy handles a config-driven single-crate server with ZERO TargetEntry enum change (D-10): is_config_driven_project (config.toml+schema.sql+toolkit dep) detection + find_lambda_package_dir single-crate-root fallback (H3, project root IS the Lambda package); pmcp.run selected only by scaffold deploy.toml target_type=pmcp-run (M3, no shape inference)
- [Phase 86]: [Plan 86-05] H4 secret posture = in-bundle rewrite (mechanism b): bundle_assets_if_configured rewrites token_secret to ${CODE_MODE_SECRET} for both the zip-root config.toml and the runtime-read assets/config.toml when config-driven; local on-disk config.toml keeps the inline DEV secret for cargo run (D-06). deploy.toml emitted to both root and .pmcp/ (DeployConfig::load reads .pmcp/)
- [Phase ?]: [86-06] TEST-06 cloud deploy test is double-gated (PMCP_RUN_DEPLOY_TEST env early-return + #[ignore]) so it skips cleanly in credential-less CI and is authentic for an operator with creds
- [Phase ?]: [86-06] Deploy driven via the real cargo-pmcp binary subprocess asserting exit-0 (M1) not in-process; reuses Plan 04 append_crates_io_patch + ChildGuard; no new deploy code, no always-on mock (D-11)

### Roadmap Evolution

- **2026-05-17 — v2.2 ROADMAP block written.** 8 phases (82–89) covering BLDR (P82) → TKIT (P83) → CONN (P84) → SHAP-A + REF parity (P85) → SHAP-B/C/D (P86) ‖ SKLL (P87) → DOGF (P88) → DOCS+REF-03 (P89). 49/49 v2.2 requirements mapped 1:1 with no orphans. REQUIREMENTS.md traceability table updated to replace TBD (v2.2) entries with concrete phase numbers.
- See PROJECT.md / prior STATE.md `Roadmap Evolution` history for v1.0–v2.1 evolution log (long; not duplicated here).

### Pending Todos

- **OPERATOR DECISION REQUIRED before Wave 1**: D-10-B scope-expansion. Pick one of (1) split Phase 75 into 75 + 75.5 — recommended; (2) accept additional refactor effort in single phase; (3) raise cog threshold (rejected per CONTEXT.md). See `.planning/phases/75-fix-pmat-issues/75-00-SUMMARY.md` "SCOPE EXPANSION DETECTED" section.
- **Phase 86 — Ship SQLite-from-config example as Shape B/C dogfood.** Replace 526-line hand-coded `cargo-pmcp/src/templates/sqlite_explorer.rs` with `--template sqlite-explorer-config` (TOML-driven, ~50 lines, `code_mode.enabled = true`) + `examples/sqlite_from_config.rs` (≤15-line Shape C). Surfaces three design inputs: Phase 84 needs identifier-substitution on `SqlConnector`; Phase 85 needs `[database.seed]` block; Phase 86 keeps both Rust-driven + TOML-driven templates. See `.planning/todos/pending/2026-05-18-ship-sqlite-from-config-example-as-phase-86-shape-b-c-dogfood.md`.

### Blockers/Concerns

- Wave 5 must patch `quality-badges.yml` per D-11-B — without that, no amount of complexity reduction flips the badge.

### Quick Tasks Completed

| # | Description | Date | Commit | Status | Directory |
|---|-------------|------|--------|--------|-----------|
| 260516-b2p | AuthProvider::on_unauthorized + transport retry-once + MSRV 1.91 + pmcp 2.8.0 ripple | 2026-05-16 | aba393aa | Shipped (PR [#256](https://github.com/paiml/rust-mcp-sdk/pull/256)) | [260516-b2p-add-authprovider-on-unauthorized-hook-tr](./quick/260516-b2p-add-authprovider-on-unauthorized-hook-tr/) |
| 260517-hi5 | Extract `x-pmcp-claim-custom-*` headers in `extract_auth_from_proxy_headers` (Cognito `custom:*` attribute forwarding) | 2026-05-17 | bbc019ba | Done | [260517-hi5-extract-x-pmcp-claim-custom-headers-in-e](./quick/260517-hi5-extract-x-pmcp-claim-custom-headers-in-e/) |
| 260527-lo3 | Fix `cargo pmcp deploy` root-resolution regression — `find_deploy_root()` anchors on `.pmcp/deploy.toml` (cwd-inclusive) + `--manifest-path` override + init Jidoka hard-error guard. Fixes nested `multi-crate-isolated` monorepo layouts (regression vs 0.6.x). | 2026-05-27 | 0a3bdd9d | Done | [260527-lo3-fix-cargo-pmcp-deploy-root-resolution-re](./quick/260527-lo3-fix-cargo-pmcp-deploy-root-resolution-re/) |
| 260527-n51 | Port Google Cloud Run `[gcp]`+`[layout]`+`[runtime]` config schema + multi-crate-isolated Dockerfile generation + local `docker buildx`→`gcloud run deploy` path from 0.6.x into 0.14.0; `aws: Option<AwsConfig>` via `aws()` accessor (10 sites migrated); unify `--target` to accept backend-type OR named target, hard-error on unknown. 1169 tests pass; zero new default-clippy errors. (Pre-existing workspace rust-1.95 pedantic debt — widget-utils + 12 untouched cargo-pmcp sites — unrelated; recommend dedicated sweep.) | 2026-05-27 | 7d0186f2 | Done | [260527-n51-port-google-cloud-run-gcp-layout-config-](./quick/260527-n51-port-google-cloud-run-gcp-layout-config-/) |
| 260527-olf | Make cargo-pmcp `clippy::all`-clean under rust 1.95 — 22 mechanical fixes / 13 files (manual_contains, type aliases, checked_div, collapsible if/match, manual_map, &Path, doc list). NOTE: corrected the n51 premise — the real gate (`make lint` / CI) lints only root `pmcp` + allow-list and was already green; cargo-pmcp is ungated, so this is latent-debt hygiene, not a gate unblock. `make lint` ✓; all cargo-pmcp tests pass. | 2026-05-27 | c70045a2 | Done | [260527-olf-make-cargo-pmcp-clippy-all-clean-under-r](./quick/260527-olf-make-cargo-pmcp-clippy-all-clean-under-r/) |
| Phase 82-builder-dx-prerequisites P02 | 25min | 4 tasks | 1 files |
| Phase 83 P03 | 50min | 3 tasks | 3 files |
| Phase 83 P04 | 40 min | 3 tasks | 4 files |
| Phase 83 P05 | 22min | 3 tasks | 4 files |
| Phase 83 P06 | 35min | 4 tasks | 7 files |
| Phase 83-toolkit-core-lift-pmcp-server-toolkit P07 | 19min | 3 tasks | 3 files |
| Phase 83 P08 | 50min | - tasks | - files |
| Phase 83 P09 | 25 | 4 tasks | 6 files |
| Phase 84 P00 | 25 | 3 tasks | 18 files |
| Phase 84 P01 | 4 | 2 tasks | 2 files |
| Phase 84 P02 | 12min | 1 tasks | 1 files |
| Phase 84 P03 | 6min | 2 tasks | 5 files |
| Phase 84 P04 | 18 | 3 tasks | 5 files |
| Phase 84 P05 | 6min | 2 tasks | 6 files |
| Phase 84 P06 | 18min | 1 tasks | 6 files |
| Phase 84 P07 | 13min | 2 tasks | 5 files |
| Phase 84 P08 | 24min | 2 tasks | 11 files |
| Phase 85 P01 | 14m | 2 tasks | 5 files |
| Phase 85 P03 | 5min | 2 tasks | 9 files |
| Phase 85 P02 | 18min | 3 tasks | 4 files |
| Phase 85 P04 | 6min | 2 tasks | 6 files |
| Phase 85 P05 | 22min | 2 tasks | 7 files |
| Phase 85 P06 | 38min | 2 tasks | 8 files |
| Phase 85 P07 | 4m | 2 tasks | 3 files |
| Phase 85 P09 | 4min | 1 tasks | 1 files |
| Phase 85 P08 | 12m | 1 tasks | 1 files |
| Phase 85 P10 | 9min | 2 tasks | 6 files |
| Phase 86 P01 | 15min | 2 tasks | 3 files |
| Phase 86 P02 | 35min | 2 tasks | 5 files |
| Phase 86 P03 | 7min | 2 tasks | 5 files |
| Phase 86 P04 | 3min | 2 tasks | 2 files |
| Phase 86 P05 | 18min | 3 tasks | 4 files |
| Phase 86 P06 | 9min | 1 tasks | 1 files |

### Last Activity

**2026-05-17** — v2.2 ROADMAP defined. Eight phases (82–89) cover 49 requirements (BLDR ×3 + TKIT ×10 + CONN ×8 + SHAP ×4 + SKLL ×7 + REF ×3 + DOGF ×2 + DOCS ×5 + TEST ×7). Coverage 49/49 with no orphans and no duplicates. Critical path runs Phase 82 (Builder DX) → Phase 83 (TKIT anchor, 12 reqs) → Phase 84 (CONN anchor, 10 reqs) → Phase 85 (Shape A + REF parity) → branching to Phase 86 (Shapes B/C/D) ‖ Phase 87 (Type 2 authoring skills) → Phase 88 (dogfood) → Phase 89 (docs + migration). All five critical invariants encoded as named success-criteria items: REF-01 superset, pure-Rust Lambda + no-Docker, dual-mode intentional, dual-surface byte-equality, SEP-2640 §9 list-exclusion. Next: `/gsd-plan-phase 82`.

**2026-05-17** — Completed quick task 260517-hi5: extract `x-pmcp-claim-custom-*` headers in `extract_auth_from_proxy_headers` so Cognito `custom:*` attributes forwarded by pmcp.run mcp-proxy surface via `AuthContext.claims["custom:<snake>"]`. Additive change (no public-API break); 4 unit tests verbatim from spec; `docs/proxy-contract.md` created; CHANGELOG `[2.8.1]` entry. `make quality-gate` green end-to-end on clean worktree. Bumped to pmcp 2.8.1; PR #257 opened to upstream/main.

**2026-05-16** — Shipped v2.8.0 bundle release via PR [#256](https://github.com/paiml/rust-mcp-sdk/pull/256):

- Quick task 260516-b2p (AuthProvider::on_unauthorized + transport retry-once)
- Phase 80 (SEP-2640 Agent Skills)
- Phase 81 (pmcp-book + pmcp-course v2 topic updates)
- MSRV bump 1.83 → 1.91
- Workspace dep ripple to pmcp 2.8.0 (publishes 7 crates on `v2.8.0` tag)

## Session Continuity

Last session: 2026-05-27T16:13:22.118Z
Stopped at: Completed 86-05-PLAN.md (Shape D config-driven deploy — H3/H1/H4/M3 + D-10 guard)
Resume: Plan 85-03 COMPLETE — scaffolded `crates/pmcp-sql-server` (Shape A pure-config binary): feature-gated 4-connector manifest (sqlite/postgres/mysql/athena, all default-on D-07), lib/main split with a placeholder `lib::run()` Wave 2 replaces. Vendored FOUR self-contained parity fixtures into the SDK repo (closes RESEARCH Open Q#1): the DATA-BEARING `tests/fixtures/chinook.db` (~984 KB, REVIEW FIX #1 — real rows for the parity replay), the SEPARATE `chinook.ddl` (11 CREATE TABLE, the --schema text input D-06), `generated.yaml` (29-scenario contract), and `reference-config.toml` — all publish-excluded via `exclude = [tests/, …]`. 6-test `schema_fixture.rs` proves the DB returns Rock/AC-DC through the real SqliteConnector, the DDL builds a standalone 11-table schema, and generated.yaml parses as a `mcp_tester::TestScenario`. REF-02 fixture foundation done. Next: Plan 85-04 (Wave 2) fills `lib::run()` with the real config-load → connector-select → `pmcp::Server` assembly → transport-serve pipeline; Plan 85-06 replays the 29 scenarios against the vendored DB. NOTE: plan counter advanced 2→3 (monotonic) but plan 85-02 is a parallel Wave-1 plan whose SUMMARY is still pending; progress recalc (157/161, 98%) reflects disk truth.
