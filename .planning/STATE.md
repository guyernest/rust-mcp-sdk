---
gsd_state_version: 1.0
milestone: v2.2
milestone_name: Configuration-Only MCP Servers
status: executing
stopped_at: Completed 84-04-PLAN.md
last_updated: "2026-05-26T21:31:45.319Z"
last_activity: 2026-05-26
progress:
  total_phases: 44
  completed_phases: 35
  total_plans: 155
  completed_plans: 151
  percent: 97
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-05-17)

**Core value:** Enterprise developers build production-grade SQL MCP servers from configuration + schema files alone — no Rust required — while preserving PMCP's security, tools/resources/prompts/tasks/skills standards and pmcp.run hosting integration.
**Current focus:** Phase 84 — sql-connectors-postgres-mysql-athena-sqlite

## Current Position

Phase: 84 (sql-connectors-postgres-mysql-athena-sqlite) — EXECUTING
Plan: 6 of 9
Status: Ready to execute
Last activity: 2026-05-26

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

- Total plans completed: 114 (v1.0: 9, v1.1: 10, v1.2: 9, v1.3: 12, v1.4: 10, v1.5: 6, v1.6: 5, v1.7: 4, v2.0: 11)
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

Last session: 2026-05-26T21:31:15.951Z
Stopped at: Completed 84-04-PLAN.md
Resume: Wave 1 of Phase 84 is COMPLETE (84-01 trait/errors → 84-02 translate_placeholders → 84-03 synthesizer + structuredContent → 84-04 SqliteConnector). Next is 84-05-PLAN.md — the per-backend Postgres crate (`pmcp-toolkit-postgres`), following the `SqliteConnector` real-driver shape (Arc<Mutex<Connection>> + spawn_blocking) shipped in 84-04. Plans 05/06/07 ship Postgres/MySQL/Athena using the `synthesize_from_config_with_connector` variant; `DatabaseSection.url` (84-03) feeds their URL constructors (D-08). `SqliteConnector` lives at `crates/pmcp-server-toolkit/src/sql/sqlite.rs` as the reference impl.
