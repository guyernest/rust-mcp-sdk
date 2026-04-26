---
gsd_state_version: 1.0
milestone: v2.0
milestone_name: Protocol Modernization
status: Ready to execute
stopped_at: Phase 77 Plan 04 complete (configure add + use)
last_updated: "2026-04-26T21:00:00.000Z"
progress:
  total_phases: 40
  completed_phases: 34
  total_plans: 83
  completed_plans: 83
  percent: 100
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-10)

**Core value:** Close credibility and DX gaps where rmcp outshines PMCP -- documentation accuracy, feature gate presentation, macro documentation, example index, repo hygiene.
**Current focus:** Phase 77 — cargo-pmcp-configure-commands

## Current Position

Phase: 77 (cargo-pmcp-configure-commands) — EXECUTING
Plan: 4 of 9
Next: Phase 74 (cargo pmcp auth subcommand, multi-server OAuth token cache) — reordered ahead of Phase 73 per operator direction 2026-04-21
After: Phase 73 (Typed client helpers + list_all pagination, PARITY-CLIENT-01)
Operator follow-ups (deferred from Phase 75 Wave 5, not blocking Phase 74): (a) merge Phase 75 Wave 5 + 75.5 to paiml/rust-mcp-sdk:main; (b) post-merge run `gh workflow run quality-badges.yml -R paiml/rust-mcp-sdk` and append observation to `.planning/phases/75-fix-pmat-issues/75-05-GATE-VERIFICATION.md` "## Badge flip observation" section.

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

- Total plans completed: 98 (v1.0: 9, v1.1: 10, v1.2: 9, v1.3: 12, v1.4: 10, v1.5: 6, v1.6: 5, v1.7: 4, v2.0: 11)
- Total phases completed: 29

## Accumulated Context

### Decisions

See PROJECT.md Key Decisions table for full history.

v2.1 decisions:

- 4 phases derived from 5 requirement categories following research-recommended dependency order: examples+protocol -> macros -> docs.rs pipeline -> polish
- EXMP and PROT combined into Phase 65 (both are credibility fixes, no dependency between them, co-deliverable)
- Phase ordering follows the docs.rs build pipeline dependency: content accuracy first, then rendering pipeline, then polish
- No new runtime dependencies for this milestone -- all fixes are config, content, and attribute changes
- [Phase 65]: All 17 orphan examples compile successfully -- registered all with import-derived feature flags (no deletions needed)
- [Phase 65]: examples/README.md replaced with PMCP example index — 63 examples categorized by Role/Capability/Complexity + migration reference
- [Phase 69.1]: Pinned rmcp comparison baseline at 1.5.0 (latest stable on crates.io as of 2026-04-16, tag rmcp-v1.5.0); pmcp baseline is v2.3.0 + feat/sql-code-mode at commit dbaee6cc
- [Phase 69.1]: 4 High-severity ergonomics gaps identified — MACRO-02 (rustdoc fallback for tool description), HANDLER-02 (Extensions typemap on RequestHandlerExtra), HANDLER-05 (peer handle in RequestHandlerExtra), CLIENT-02 (typed call_tool + list_all_* pagination helpers). Plan 02 will derive one follow-on phase proposal per High row.
- [Phase 69.2]: 3 follow-on phase proposals drafted in 69-PROPOSALS.md — PARITY-HANDLER-01 (bundling HANDLER-02 + HANDLER-05 on shared RequestHandlerExtra edit site, target v2.2, 4 plans), PARITY-CLIENT-01 (CLIENT-02, target late v2.1, 3 plans), PARITY-MACRO-01 (MACRO-02, target late v2.1, 3 plans). Row-ID bijection verified: all 4 High Row IDs cited in Derived-from + Rationale subsections. Flagged regex bug in Task 2 verify block for Plan 03 correction (pipe-table trailing `|` not matched).
- [Phase 69]: rmcp parity research complete — 69-RESEARCH.md (gap matrix, 32 rows total, 4 High-severity) + 69-PROPOSALS.md (3 proposals). 3 PARITY-* requirement IDs landed in REQUIREMENTS.md (one per proposal); follow-on phases not yet scheduled.
- [Phase 72]: [Phase 72 Plan 02]: PoC Slice 1 spike EXECUTED on throwaway branch — T4_compile_errors=0, T4_loc_delta=537, ~15 min wall-clock under 4-hour hard time-box; serde-shape divergence found (rmcp requires params: {}; rejects null/missing) downgrades INVENTORY row 1 EXACT→compatible-via-adapter
- [Phase 72]: [Phase 72 Plan 02]: Rubric shipped 9 thresholds T1..T9; T8 historical churn + T9 enterprise-feature-preservation added; T2 expanded with PR merge latency; gh fallback URL codified verbatim; default-to-B logic removed per HIGH-1
- [Phase 72.1]: CR-03 rev-2 shipped — cargo-pmcp 0.8.0 -> 0.8.1 (patch, additive). Landing Next.js template now uses runtime `fetch('/landing-config')` via shared `useLandingConfig` hook; 4 consumers (signup, callback, connect [server->client flip], Header [conditional button]) routed through the hook; 4 NEXT_PUBLIC_* reads deleted; 3 stale rustdoc refs in `src/landing/config.rs` rewritten. MCP_SERVER_NAME branding preserved (CR-03 §0). 12/12 CR-03 ACs PASS; G1..G8 guardrails green (G7 scaffold smoke added per Codex M3; G8 77-line LOC delta ≤100 budget). AC-11 manual offline gate approved by operator guy 2026-04-20. Unblocks pmcp.run Phase 71 UAT Test 7 + Cost Coach prod launch. crates.io release is a separate follow-up (git tag v0.8.1 triggers .github/workflows/release.yml).
- [Phase ?]: Phase 74 release state landed — pmcp 2.5.0 + cargo-pmcp 0.9.0 + mcp-tester 0.5.2; 8 pins bumped; CHANGELOG dated 2026-04-21; quality-gate green. Tagging is operator-driven.
- [Phase ?]: [Phase 77 Plan 03]: TargetConfigV1 + per-variant named-struct serde-tagged enum; find_project_root lifted to configure/workspace.rs::find_workspace_root (pub); test_support_configure #[path] bridge in lib.rs; serial_test=3 dev-dep added; 12 tests pass; Default derive added on GlobalFlags.
- [Phase ?]: [Phase 77 Plan 04]: configure add + configure use shipped — 6-pattern raw-credential validator (AKIA/ASIA/ghp_/github_pat_/sk_live_/AIza), name regex `[A-Za-z0-9_-]+` (T-77-03 path-traversal mitigation), GEM-1 escape-hatch in error msg, GEM-2 stderr switching note on overwrite, BOM-tolerant `pub fn read_active_marker` for downstream resolver. 16 unit tests pass; 2 commits (03908c2f + 0d8bb8ee).

### Roadmap Evolution

- Phases 65-68 added: v2.1 rmcp Upgrades milestone (examples cleanup, macros rewrite, docs.rs pipeline, documentation polish)
- Phase 67.1 inserted after Phase 67: Code Mode Support (URGENT) — external developer support for code mode pattern (validation + execution) based on pmcp-run/built-in/shared/pmcp-code-mode SDK_DESIGN_SPEC.md
- Phase 67.2 inserted after Phase 67.1: Code Mode Derive Hardening (URGENT) — fix 3 critical derive macro issues from pmcp.run team review: policy_evaluator not called, static ValidationContext, hardcoded "graphql" code type
- Phase 69 added (initially added as duplicate Phase 68, renumbered 2026-04-16 to avoid collision with existing "Phase 68: General Documentation Polish"): rmcp parity research — scope narrowed to ergonomics-only + follow-on phase proposals; transports/examples/docs-coverage intentionally excluded to eliminate overlap with Phase 68. Deliverables: 69-RESEARCH.md (gap matrix) + 69-PROPOSALS.md (2–5 phase proposals seeded from High-severity gaps).
- Phase 70 added: Add Extensions typemap and peer back-channel to RequestHandlerExtra (PARITY-HANDLER-01) — bundles HANDLER-02 (Extensions typemap) + HANDLER-05 (peer handle) on the shared RequestHandlerExtra edit site per 69-PROPOSALS.md.
- Phase 71 added: Rustdoc fallback for #[mcp_tool] tool descriptions (PARITY-MACRO-01) — rustdoc-harvest fallback in pmcp-macros when `description = "..."` attribute is omitted, per 69-PROPOSALS.md Proposal 3 (MACRO-02).
- Phase 71 planned + replanned (2026-04-17): initial 3 plans → replanned to **4 plans / 12 tasks / 4 waves** after Codex cross-AI review surfaced 2 HIGH findings. HIGH-1 resolved via new `crates/pmcp-macros-support/` sibling crate (proc-macro crates cannot export public items; Option A adopted). HIGH-2 resolved via explicit `^pmcp = ` ripple audit + concurrent `cargo-pmcp 0.6.0→0.6.1` + `mcp-tester 0.5.0→0.5.1` patch bumps per CLAUDE.md §"Version Bump Rules". Semver posture revised: **pmcp 2.3.0→2.4.0 (minor, not patch)** — rustdoc-only macro source form is additive feature. Final VERIFICATION PASSED after 2 revision iterations.
- Phase 72 added (2026-04-19): Investigate rmcp as foundations for pmcp — evaluate using rmcp for the protocol layer while repositioning pmcp + tooling as the pragmatic, batteries-included SDK for enterprise use cases. Goal is a research/decision phase to reduce protocol-spec maintenance burden and focus pmcp on higher-level DX.
- Phase 72 planned + replanned (2026-04-19): initial 3 plans → replanned to **3 plans / 12 tasks / 3 waves** after Gemini + Codex cross-AI review (72-REVIEWS.md) surfaced 3 HIGH findings. HIGH-1 (consensus, default-to-B bias) resolved by removing the default-to-B rule and replacing it with an explicit decision tree (N<3→DEFER, N=3..4→D, N≥5→highest-scoring); valid recommendation set tightened to {A, B, C1, C2, D, DEFER} — E is prohibited as an outcome (contingency-only footnote). HIGH-2 (consensus, PoC/threshold resolution gap) resolved by adding Plan 02 Task 1b which EXECUTES PoC Slice 1 on a throwaway branch `spike/72-poc-slice-1` with a 4-hour hard time-box, producing 72-POC-RESULTS.md with real T4_compile_errors + T4_loc_delta, then deletes the branch + scratch dir. HIGH-3 (Codex-only, weak inventory evidence) resolved by upgrading 72-01 Task 2 row schema from 5 to 9 columns (adding exact symbols, public API surface, owned impls/macros, serde compat risk, feature flags, downstream crates). Also: strategy matrix rows changed {A,B,C,D,E}→{A,B,C1,C2,D} + E as footnote; rubric expanded T1..T9 including T8 (historical churn, 180d git log on src/types/+src/shared/) and T9 (enterprise-feature-preservation checklist for TypedTool/workflow/mcp_apps/auth/middleware/mcp-preview/cargo-pmcp); T2 expanded to include PR merge latency + codified gh fallback URL; Plan 01 Task 0 creates 72-CONTEXT.md locking T6/T7; Plan 03 Task 1b runs an awk semantic audit that auto-downgrades to DEFER if any `### Criterion` subsection fails to cite a T-ID + inventory/matrix row. 7 deliverables total (was 5). Final VERIFICATION PASSED on first revision iteration.
- Phase 72 executed + verified (2026-04-20): 3 waves shipped; final **Recommendation: D** (Maintain pmcp as authoritative Rust MCP SDK). N=7/9 resolved thresholds (T6/T7 remain UNKNOWN). Slice 1 spike executed on throwaway branch — T4_compile_errors=0, T4_loc_delta=537, serde `params: null` round-trip FAILS against rmcp 1.5.0, downgrading inventory row 1 from EXACT to compatible-via-adapter (strongest counterargument to A/B). 72-REVIEWS.md HIGH findings all resolved in final artifacts. Verification PASSED 15/15 after 1 gap-closure iteration (C3/C5 matrix-citation regex fixes + REQUIREMENTS.md -01/-02 ledger sync). Phase 69's parity phases remain the forward path.
- Phase 73 added (2026-04-20): Typed client helpers + list_all pagination (PARITY-CLIENT-01) — implements 69-PROPOSALS.md Proposal 2 (CLIENT-02). Adds `call_tool_typed<T>` / `call_prompt_typed<T>` (typed-arg serialization) and `list_all_tools` / `list_all_prompts` / `list_all_resources` (auto-paginating on next_cursor with max-iteration safety cap) to `Client`. Additive, non-breaking; 3 plans expected; minor semver bump.
- Phase 72.1 inserted after Phase 72 (2026-04-20): **Finalize landing support (URGENT)** — implements pmcp-run team CR-03 rev-2 (runtime `/landing-config` fetch replacing the four `NEXT_PUBLIC_*` build-time env vars in `cargo-pmcp/templates/landing/nextjs/`). HIGH priority; blocks pmcp.run Phase 71 UAT Test 7 and Cost Coach prod launch. Scope: template-only + required `useLandingConfig` shared hook + 3-line rustdoc fix in `src/landing/config.rs`. `MCP_SERVER_NAME` branding stays. Target: `cargo-pmcp 0.8.1` patch. CR source copied to `72.1-CR-03-SOURCE.md`.
- ROADMAP.md headings `## Phase Details (v2.1)` / `## Progress (v2.1)` renamed to `## Phase Details — Current Milestone` / `## Progress — Current Milestone` (2026-04-20) to prevent `extractCurrentMilestone` regex from clipping the v2.1 section at the Phase Details sub-heading; STATE.md frontmatter `milestone` also corrected from `v2.0` → `v2.1` (v2.0 shipped phases 54-59 per ROADMAP line 13; Phase 72+ is v2.1 territory).
- Phase 74 added (2026-04-21): Add cargo pmcp auth subcommand with multi-server OAuth token management — consolidate the existing OAuth PKCE flow (currently nested inside every server-connecting command via `AuthFlags`) into a dedicated `cargo pmcp auth` command group: `login <url>`, `logout [<url> | --all]`, `status [<url>]`, `token <url>`, `refresh <url>`. Evolve `~/.pmcp/oauth-tokens.json` into a per-server-keyed cache so multiple OAuth-protected servers can be logged in concurrently (e.g. dev + staging + prod). Additive; `cargo-pmcp` patch/minor bump. Motivation: remove the per-command browser prompt friction for developers running `test conformance`, `test run`, `loadtest`, `preview` against OAuth-protected servers.
- Phase 73 ↔ 74 order swapped (2026-04-21): Phase 74 (cargo pmcp auth) promoted ahead of Phase 73 (typed client helpers) per operator direction. Phase numbers preserved (no directory/reference churn); only ROADMAP.md ordering + "Depends on" chain flipped: Phase 74 now depends on Phase 72.1, Phase 73 now depends on Phase 74. Rationale: developer-facing DX unblocker (OAuth friction while testing OAuth-protected servers) takes priority over additive client ergonomics.
- Phase 75 added (2026-04-22): Fix PMAT issues — close accumulated quality-gate debt surfaced after the v2.6.0 release when `cargo install pmat-cli` was corrected to `cargo install pmat` (PR #246) and the workflow finally ran real analysis. Current baseline (local `pmat quality-gate`, worktrees excluded): **94 complexity** violations (cognitive complexity > 25), **439 duplicate** code blocks, **33 SATD** TODO/FIXME comments, **4 entropy** violations, **2 README sections** missing (Installation, Usage). Target: restore "Quality Gate: passing" on the auto-generated top-of-README badge. Hotspots in `crates/pmcp-code-mode/`, `cargo-pmcp/src/pentest/`, `cargo-pmcp/src/deployment/`, `src/server/streamable_http_server.rs`, `pmcp-macros/`. Expected as a multi-wave phase.
- Phase 76 added (2026-04-22): cargo-pmcp IAM declarations — implement pmcp.run platform CR `/Users/guy/Development/mcp/sdk/pmcp-run/docs/CLI_IAM_CHANGE_REQUEST.md` (filed 2026-04 after cost-coach prod incident 2026-04-23). Two parts land together in one phase: **Part 1** adds stable `McpRoleArn` CfnOutput + `pmcp-${ServerName}-McpRoleArn` export to both template branches in `cargo-pmcp/src/commands/deploy/init.rs:485-747` (pmcp-run + aws-lambda), unblocking existing bolt-on stacks via `Fn::ImportValue`. **Part 2** adds a new optional `[iam]` section to `.pmcp/deploy.toml` (`[[iam.tables]]`, `[[iam.buckets]]`, `[[iam.statements]]`) that translates to `addToRolePolicy` calls on `McpRole`; CLI validates footguns (Allow `*:*` on `*` → hard error, unknown service prefix → warn, cross-account ARN → warn, action regex enforced). Mirrors platform's existing `TablePermission` construct one-to-one. Backward compatible (empty default). CR explicitly rejected env-var-name auto-inference and `${serverName}-*` prefix auto-grant — do not re-propose. Full brief at `.planning/phases/76-cargo-pmcp-iam-declarations-servers-declare-iam-needs-in-dep/76-BRIEF.md`.
- [Phase 75 Wave 0]: D-09 resolved empirically — PMAT 3.15.0 `quality-gate` has NO `--include`/`--exclude` flag; `.pmatignore` (gitignore-style globs) is the only gate-honored path-filter mechanism. Wave 5 must use `.pmatignore` for fuzz/+packages/+examples/ (defensive) plus bare `--checks complexity`.
- [Phase 75 Wave 0]: **D-10 resolved D-10-B (UNFAVORABLY)** — PMAT 3.15.0 IGNORES `#[allow(clippy::cognitive_complexity)]`. Empirical fixture (cog 41 function with project-template `// Why:` annotation) STILL flagged. P5 (allow-with-Why suppression) is REMOVED from Phase 75 toolkit. Every flagged complexity hotspot must reduce ≤25 by real refactor. **SCOPE EXPANSION ALERT** — Wave 1 is BLOCKED awaiting operator decision (split phase 75/75.5 vs accept additional refactor effort).
- [Phase 75 Wave 0]: **D-11 resolved D-11-B (UNFAVORABLY)** — bare `pmat quality-gate --fail-on-violation` (the BADGE command) currently fails on 5 dimensions: complexity 94, duplicate 1545, satd 33, entropy 13, sections 2. Even after Waves 1-4 reduce complexity to 0, the bare gate will STILL exit 1 — meaning the README badge will stay RED. Wave 5 MUST patch `quality-badges.yml` line ~72 with `--checks complexity` (alongside the new `ci.yml` gate job).
- [Phase 75 Wave 0]: Inventory snapshot committed at `.planning/phases/75-fix-pmat-issues/pmat-inventory-2026-04-22.json` (166 violations, 91 cog + 75 cyc). All later wave deltas derive from this file via `jq`; CONTEXT.md prose counts (94/73/21/3) are explicitly superseded.
- [Phase 75 Wave 0]: Phase 76 dependency inversion — Phase 76 shipped to main BEFORE Phase 75 despite logically depending on it. Material divergences (vs CONTEXT.md baseline): complexity gate count UNCHANGED at 94 (Phase 76 added cargo-pmcp branchy code but gate-relevant count is identical); duplicates tripled (439 → 1545); entropy tripled (4 → 13); in-scope src/ count grew (73 → 86); examples/ count dropped 21 → 0 under the gate. Detailed reconciliation in `pmat-inventory-summary.md`.
- Phase 77 added (2026-04-25): Add cargo pmcp configure commands — design and implement `cargo pmcp configure add|use|list|remove|show` (modeled after `aws configure`) so developers can define named deployment targets (dev/prod/staging) carrying pmcp.run discovery endpoint URLs (PMCP_API_URL like https://ipwojemcm6.execute-api.us-west-2.amazonaws.com or its `/.well-known/pmcp-config` variant), AWS CLI profile, region, and target-specific credentials. Per-workspace target selection so sibling servers in one monorepo can deploy to different environments simultaneously. Extensible to aws-lambda direct deploy and Google Cloud Run targets. Integrates with existing `cargo pmcp deploy` and pmcp.run upload flows so they read the active target rather than hardcoded URLs/profiles. Scope: TOML config schema (workspace `.pmcp/` + user `~/.config/pmcp/`), env var override (PMCP_TARGET=name), explicit precedence rules (workspace > user > env). Phase directory normalized to `.planning/phases/77-cargo-pmcp-configure-commands/` (SDK-generated default slug was unworkable). To plan: `/gsd-plan-phase 77`.

### Pending Todos

- **OPERATOR DECISION REQUIRED before Wave 1**: D-10-B scope-expansion. Pick one of (1) split Phase 75 into 75 + 75.5 — recommended; (2) accept additional refactor effort in single phase; (3) raise cog threshold (rejected per CONTEXT.md). See `.planning/phases/75-fix-pmat-issues/75-00-SUMMARY.md` "SCOPE EXPANSION DETECTED" section.

### Blockers/Concerns

- Wave 5 must patch `quality-badges.yml` per D-11-B — without that, no amount of complexity reduction flips the badge.

### Phase 75 Wave 2 Decisions (2026-04-24)

- [Phase 75 Wave 2]: All 40 cargo-pmcp/ hotspots refactored to cog ≤25 via P1-P4 extraction alone. No P5 invocations. No escapees logged to 75.5-ESCAPEES.md. Both monsters (check.rs::execute cog 105, handle_oauth_action cog 91) decomposed to ≤25 via P1 per-stage pipeline + P4 per-variant dispatch.
- [Phase 75 Wave 2]: PMAT complexity-gate count dropped 75 → 29 (delta −46) after Wave 2. Aggregate Phase 75 delta so far: baseline 94 → 29 (−65). `make quality-gate` exits 0 end-to-end.
- [Phase 75 Wave 2]: Shared scan_for_package helper established in cloudflare/init.rs — 3-bird kill (reduces find_core_package + find_any_package + removes duplicated scan loop). Pattern reusable in Wave 3.
- [Phase 75 Wave 2]: 7 new in-file unit tests added to commands/test/check.rs for pure predicate helpers (detect_transport_error + print_test_results). Full E2E mock-HTTP tests deferred — out-of-scope for structural refactor.
- [Phase 75 Wave 2]: Phase 76 dependency inversion reconciled — deploy_to_pmcp_run measured cog 66 at Wave 2 start (was 65 in plan's RESEARCH.md); other 5 named hotspots matched the plan exactly. No task rework.

### Phase 75 Wave 3 Decisions (2026-04-24)

- [Phase 75 Wave 3]: All 5 named pmcp-code-mode hotspots refactored to cog ≤25 via P6 + P1 extraction alone. No P5 invocations. No escapees logged to 75.5-ESCAPEES.md. Both eval-monsters (evaluate_with_scope cog 123→17, evaluate_array_method_with_scope cog 117→≤25) decomposed via per-ValueExpr-variant / per-ArrayMethodCall-variant dispatch tables. evaluate_string_method (50→≤25), parse_policy_annotations (35→≤25), pattern_matches (34→≤25) cleared via P1.
- [Phase 75 Wave 3]: PMAT complexity-gate count dropped 29 → 22 (delta −7) after Wave 3. Aggregate Phase 75 delta: baseline 94 → 22 (−72). `make quality-gate` exits 0 end-to-end.
- [Phase 75 Wave 3]: Pre-existing pmcp-code-mode lint debt (18 lib + 28 test clippy errors + 3 dead-code warnings logged in deferred-items.md 2026-04-23) cleared in opening sweep before any cog refactor. Imports trimmed in cedar_validation.rs test mod, manual `if let Some` → `.flatten()`, `assert_eq!(_, true)` → `assert!(_)`, redundant `Ok(?)` removed, etc. No P5/dead-code annotations added beyond two `#[allow(dead_code)]` on retained-but-unused fields/methods (MockHttpExecutor.mode, PlanCompiler.max_api_calls, PlanExecutor::evaluate_with_binding/_with_two_bindings — all extension surfaces or future-diagnostics).
- [Phase 75 Wave 3]: EvalContext struct allowance from plan body NOT triggered — the per-helper signature spam never materialised because each evaluate_*-variant helper takes only ≤4 args (`expr_subparts`, `&V`, `&HashMap`, sometimes `&mut HashMap`). Dispatcher remains a flat match on `&ValueExpr` / `&ArrayMethodCall`.
- [Phase 75 Wave 3]: One out-of-scope pmcp-code-mode warning remains in the gate (find_blocked_fields_recursive cog 24 in executor.rs — warning-level severity, not in this plan's hotspot list). Deferred to Wave 4 or later per scope-boundary rule.
- [Phase 75 Wave 3]: Wave 0 semantic-regression baseline (eval_semantic_regression.rs, 34 tests) byte-identical across all 5 commits — no JsonValue output drift; no `assert_eq!` payload changes.

### Phase 75 Wave 4 Decisions (2026-04-25)

- [Phase 75 Wave 4]: All 5 plan-named scattered hotspots refactored to cog ≤25 via P1 + P4 extraction (run_diagnostics_internal 55→16, mcp-tester::main 40→≤25, handle_socket 37→≤25, list_resources 31→≤25, lambda::handler 26→≤25). No P5 invocations. No escapees logged.
- [Phase 75 Wave 4]: Plan body underspecified the residual gate count — 22 violations at start, 5 plan-named accounted for 6 (with one same-file warning), 8 fuzz/+packages/ handled by `.pmatignore` per Wave 0 chosen_path: (a), and 8 outstanding cog 24-25 warnings in cargo-pmcp/, src/, crates/pmcp-code-mode/ refactored under Rule 3 (auto-fix blocking issue) since gate-exit-0 was the acceptance criterion. All 8 cleared to ≤23.
- [Phase 75 Wave 4]: PMAT complexity-gate count dropped 22 → 0 (delta −22). **Aggregate Phase 75 delta: baseline 94 → 0 (−94, ALL violations cleared).** `make quality-gate` exits 0 end-to-end. `pmat quality-gate --fail-on-violation --checks complexity` exits 0.
- [Phase 75 Wave 4]: `.pmatignore` (gitignore-style globs) added at repo root excluding fuzz/, packages/, and examples/ (defensively — count is currently 0 but a future branchy example shouldn't silently regress the gate). Per Wave 0 spike Mechanism 6 — the only path-filter mechanism PMAT 3.15.0 honors on `quality-gate`.
- [Phase 75 Wave 4]: fuzz/auth_flows::test_auth_flow cog 122 NOT refactored (plan body assumed mandatory refactor under D-03; Wave 0 chosen_path: (a) supersedes that — fuzz harnesses excluded via `.pmatignore` per D-09 framing).
- [Phase 75 Wave 4]: SATD triage per D-04 — 25 inventoried, 11 in-scope (b) migrated to `// See #NNN` refs against 3 umbrella issues filed at paiml/rust-mcp-sdk (#247 aws-sdk-secretsmanager wiring, #248 cargo-pmcp commands roadmap, #249 pmcp-code-mode misc); 14 classified as out-of-D-04-scope scaffold/template content (json! literal values in scenario_generator.rs + r#"..."# template-literal contents in validate.rs and cloudflare/init.rs that are written to user-generated files).
- [Phase 75 Wave 4]: Pre-existing build error in crates/pmcp-code-mode/src/cedar_validation.rs (missing get_sql_baseline_policies import in the --all-features test build) fixed under Rule 3 — required for the plan's `cargo test --workspace --all-features` verification step. Reproducible against pre-Wave-4 HEAD.
- [Phase 75 Wave 4]: Wave 5 ready — needs (a) `--checks complexity` job in .github/workflows/ci.yml and (b) patch `quality-badges.yml:~72` per D-11-B.

### Phase 75.5 Plan 01 Decisions (2026-04-25)

- [Phase 75.5 Plan 01]: All 12 Category-A bare `#[allow(clippy::cognitive_complexity)]` attributes removed from `src/` via single-line attribute deletion (10 files: src/server/elicitation.rs, src/server/notification_debouncer.rs, src/server/resource_watcher.rs, src/server/mod.rs, src/server/transport/websocket_enhanced.rs ×2, src/shared/sse_optimized.rs, src/shared/connection_pool.rs, src/shared/logging.rs, src/client/mod.rs, src/client/http_logging_middleware.rs ×2). No refactor triggered — clippy pedantic+nursery on `--features full` raised zero `cognitive_complexity` warnings post-removal, empirically confirming that all 12 underlying functions sit at cog ≤25 (consistent with main-was-already-green pre-condition).
- [Phase 75.5 Plan 01]: `make quality-gate` exit 0 + `pmat quality-gate --fail-on-violation --checks complexity` exit 0 (PMAT 3.15.0, matches CI command per Phase 75 Wave 5 D-07) end-to-end. `grep -rn '#[allow(clippy::cognitive_complexity)]' src/` returns 0 matches — Phase 75-ADDENDUM-D10B Rule 1 (no new bare allows) compliance confirmed.
- [Phase 75.5 Plan 01]: ESCAPEES.md (Category B) unchanged at 0 entries — zero P5-residual handoffs across Phase 75 Waves 1-4 + zero new escapees from this plan (no refactor branch was triggered).
- [Phase 75.5 Plan 01]: Plan-verify `cargo test --workspace --all-features` surfaced two pre-existing environmental failure clusters classified out-of-scope per deviation-rules SCOPE BOUNDARY: (a) `mcp-e2e-tests::chess` (10 failures) — chromiumoxide v0.9 fetcher cannot find a Chrome browser archive in the runner cache (`~/.cache/ms-playwright` missing); (b) `pmcp-tasks::store::redis` (24 failures) + `pmcp-tasks::store::dynamodb` (12 failures) — TCP "Connection refused (os error 61)" against required external services not running locally. Neither cluster touches src/server/, src/shared/, or src/client/; both fail identically on the parent commit (29dc0a8b) pre-changes. CI-matching gate `make quality-gate` (the authoritative pre-merge check) exits 0.
- [Phase 75.5 Plan 01]: Phase 75 + 75.5 complexity-debt program **closed**. Aggregate state: 0 PMAT complexity violations gate-wide, 0 bare `#[allow(clippy::cognitive_complexity)]` attributes in src/, ESCAPEES.md empty, CI gate live (D-07), badge command aligned (D-11-B). Pending operator follow-up: merge Wave 5 + 75.5 to paiml/main and run `gh workflow run quality-badges.yml -R paiml/rust-mcp-sdk` to record badge-flip observation.

### Phase 75 Wave 5 Decisions (2026-04-25)

- [Phase 75 Wave 5]: PMAT quality-gate complexity-only check landed in `.github/workflows/ci.yml` `quality-gate` job (3 new steps: install pinned PMAT 3.15.0, verify version, run `pmat quality-gate --fail-on-violation --checks complexity`). The `gate` aggregate job already lists `quality-gate` in `needs:`, so a PMAT failure now propagates to the org-required `gate` status check and PR-blocks merge.
- [Phase 75 Wave 5]: D-11-B badge alignment landed in `.github/workflows/quality-badges.yml` line 92 — added `--checks complexity` so the badge command matches the new ci.yml gate command. Without this, the README badge would stay red on duplicate/SATD/entropy/sections dimensions even after Phase 75's complexity work brings complexity to 0.
- [Phase 75 Wave 5]: CLAUDE.md gained `### CI Quality Gates (PR-blocking, added Phase 75 Wave 5)` subsection documenting the gate, the PMAT 3.15.0 pin, the debug recipe (P1–P6 refactor catalog + `// Why:` `#[allow]` template), and the "do not weaken" warning.
- [Phase 75 Wave 5]: **Task 5-02 replanned mid-execution (option A → option B per user)**: original plan was a regression-PR fail-closed test on upstream; user challenged the value (the bare PMAT exit-code-on-violations was already established by every prior red-badge run). Tried fork-internal PR (#3 on guyernest/rust-mcp-sdk) — GitHub did NOT trigger the CI workflow, likely because fork main is 21+ commits behind upstream/local main (phase 64 unmerged) yielding `mergeable: CONFLICTING`. Switched to local-pmat evidence: `pmat quality-gate --fail-on-violation --checks complexity` against a deliberate-complexity fixture (cog 77) exits 1 by name. PR #3 closed; throwaway branch deleted local + remote. Full audit in `75-05-GATE-VERIFICATION.md`.
- [Phase 75 Wave 5]: **Task 5-03 (badge flip observation) deferred** — requires Wave 5 to land on `paiml/rust-mcp-sdk:main`. Operator follow-up: trigger `gh workflow run quality-badges.yml -R paiml/rust-mcp-sdk` post-merge and append observation to `75-05-GATE-VERIFICATION.md` "## Badge flip observation" section.
- [Phase 75 Wave 5]: Aggregate Phase 75 status: complexity violations 94 → 0 (clean); CI gate live; badge command aligned. Phase complete pending the deferred badge-flip observation post-merge.

## Session Continuity

Last session: 2026-04-26T21:00:00.000Z
Stopped at: Phase 77 Plan 04 complete (configure add + use)
Resume: Execute Phase 77 Plan 05 (configure list + show). Plans 04 and 05 are sibling plans in Wave 3; 05 unblocks Plan 06 (resolver + banner). Plan 04 left `pub fn read_active_marker` in `configure/use_cmd.rs` ready for Plan 06's resolver to consume. Plan 04 also documented `validate_target_name` duplication for Plan 09 to consolidate during quality-gate cleanup.
