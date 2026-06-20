---
phase: 100-excel-workbook-built-in-servers-v2
plan: 06
subsystem: phase-wide-quality-purity-gate
tags: [quality-gate, purity-check, pmat-complexity, retired-symbol-sweep, gate-verdict, wbv2-08]

# Dependency graph
requires:
  - phase: 100-excel-workbook-built-in-servers-v2
    plan: 04
    provides: "served multi-tool fan-out + CalculateHandler retirement + Plan-03 .outputs() shim removal (the symbols this sweep confirms gone) + the documented Rule-4 named-range orchestrator deferral"
  - phase: 100-excel-workbook-built-in-servers-v2
    plan: 05
    provides: "workbook explain preview + BA docs (the last code wave landed before this gate)"
provides:
  - "Recorded phase-wide gate verdict: make lint (CI clippy) GREEN, make purity-check GREEN, PMAT cog<=25 GREEN in CI scope (src/) and CLEAN in all phase-100 workbook paths, workbook crate-cone + served-toolkit + cargo-pmcp workbook tests + both ALWAYS examples GREEN"
  - "Retired-symbol sweep: CalculateHandler GONE, Plan-03 CellMap::outputs() shim GONE, no in_*/out_* injection sites — proven by grep; strip_governance_prefix/name_named_inputs/promote_named_outputs SURVIVE per the documented Plan-04 Rule-4 deferral (recorded, not rewritten)"
  - "Honest exclusion of 3 pre-existing out-of-scope failures (mysql E0277, auth-cache proptest, Phase-93 fuzz_provenance_reader wrapper drift), all verified untouched by phase 100"
affects: [phase-100-close, roadmap-success-criterion-5]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Gate verdict scoping: the binding CI gate is make lint (root pmcp clippy, generous allow-list) + the PMAT src/-prefixed cognitive gate — both GREEN; the workspace-wide make quality-gate build/test step is dominated by the pre-existing pmcp-toolkit-mysql E0277 (SQL-connector milestone, not phase 100), so the verdict is recorded against the workbook crate cone + the CI-binding subset, with the full picture reported honestly"
    - "rust_xlsxwriter in the served runtime is the WRITER, not a reader: make purity-check positively asserts it PRESENT in pmcp-workbook-runtime (PURITY_WRITER_CRATES); the load-bearing invariant is no-READER (umya/calamine/quick-xml), which IS held in every served tree"

key-files:
  created:
    - .planning/phases/100-excel-workbook-built-in-servers-v2/100-06-SUMMARY.md
  modified:
    - .planning/phases/100-excel-workbook-built-in-servers-v2/deferred-items.md

key-decisions:
  - "Recorded the Plan-04 Rule-4 deferral as the sweep verdict for strip_governance_prefix/name_named_inputs/promote_named_outputs rather than attempting the architectural orchestrator rewrite (gated by the named-range fixture corpus; explicitly out of scope per the orchestrator directive)"
  - "Did NOT remove rust_xlsxwriter from pmcp-workbook-runtime to satisfy the plan's Task-2 acceptance criterion #4 — it is the deliberate writer the purity gate's positive assertion requires; removing it would BREAK make purity-check. The authoritative gate is make purity-check (GREEN), which bans readers (umya/calamine/quick-xml), not the writer"
  - "Excluded 3 pre-existing out-of-scope failures (verified at/before the phase base, untouched by any phase-100 commit) instead of fixing them: pmcp-toolkit-mysql sqlx E0277, the auth-cache normalize_round_trip_idempotent proptest, and the Phase-93 fuzz_provenance_reader stable-compile wrapper drift"

patterns-established:
  - "Pattern: phase-close gate verdict separates the binding CI gate (make lint + PMAT src/) from the workspace-wide aggregate (blocked by a pre-existing sibling-milestone error), and records both honestly with cargo-tree negative assertions + grep sweep output verbatim"

requirements-completed: [WBV2-08]

# Metrics
duration: ~6min
completed: 2026-06-20
---

# Phase 100 Plan 06: Phase-wide Quality / Purity / Retired-symbol Gate Summary

**The breaking table-based-authoring surface ships defect-free against the binding gates: `make lint` (the CI-exact root-`pmcp` clippy gate, pedantic+nursery) is GREEN, `make purity-check` (the WBRT-04 three-layer reader-ban) is GREEN with `umya`/`calamine` proven absent from `pmcp-workbook-runtime` and no reader in any served tree, the PMAT cognitive gate reports ZERO `cog>25` violations in CI scope (`src/`-prefixed) AND zero in every phase-100 workbook path, and the workbook crate-cone + served-toolkit + cargo-pmcp workbook tests + both ALWAYS examples all pass. The retired-symbol sweep proves `CalculateHandler` and the Plan-03 `CellMap::outputs()` shim are GONE workspace-wide and no `in_*/out_*` governance-injection site survives; the three named-range orchestrator symbols (`strip_governance_prefix`/`name_named_inputs`/`promote_named_outputs`) survive exactly as the documented Plan-04 Rule-4 architectural deferral — recorded, not rewritten.**

## Performance

- **Duration:** ~6 min
- **Tasks:** 3 (all `type=auto` gate runs)
- **Files:** 1 created (this SUMMARY), 1 modified (deferred-items ledger)

## Gate Results (verbatim)

### Task 1 — make quality-gate + PMAT complexity gate

| Gate | Command | Result |
|------|---------|--------|
| fmt | `cargo fmt --all -- --check` | **exit 0** (clean) |
| clippy (CI-binding) | `make lint` (root `pmcp` `--features full`, pedantic+nursery, `--lib --tests` + `--examples` check) | **GREEN — "✓ No lint issues"** |
| PMAT cognitive (CI scope) | `pmat analyze complexity --max-cognitive 25` filtered to `src/`-prefixed | **ZERO `src/` cognitive violations → CI PMAT gate GREEN** |
| PMAT cognitive (phase-100 scope) | filtered to `crates/pmcp-workbook*` / `crates/pmcp-server-toolkit/src/workbook` / `cargo-pmcp/src/commands/workbook` | **ZERO violations — CLEAN** |
| served binary build | `cargo build -p pmcp-workbook-server` | **exit 0** |
| workbook crate-cone tests | `cargo test -p pmcp-workbook-runtime -p pmcp-workbook-compiler -p pmcp-workbook-dialect` | **all GREEN** (incl. the runtime doctest) |
| served-toolkit workbook tests | `cargo test -p pmcp-server-toolkit --no-default-features --features workbook,workbook-embedded` | **all GREEN** (7+3+1+1+1+5+32 + the 15s integration test — zero failures) |
| cargo-pmcp workbook explain | `cargo test -p cargo-pmcp --test workbook_explain` | **5 passed** |
| cargo-pmcp embedded mirror | `cargo test -p cargo-pmcp --lib templates_workbook_server` | **8 passed** |
| ALWAYS example 1 | `cargo run --example workbook_table_authoring --features workbook-embedded -p pmcp-server-toolkit` | **exit 0** (prints `calculate_tax` / `estimate_refund` disjoint DAG-derived inputs) |
| ALWAYS example 2 | `cargo run -p cargo-pmcp --example workbook_explain` | **exit 0** |

**PMAT verdict detail (honest full picture):** the two workspace-wide `cog>25` violations are BOTH in `tests/` files of UNRELATED subsystems and are therefore outside both the CI gate (`startswith("src/")`) and phase-100 scope:
- `crates/mcp-tester/tests/property_tests.rs:53` `prop_g3_handler_detection_independent_of_sdk` (cog 29) — mcp-tester.
- `crates/pmcp-server-toolkit/tests/sql_server_http_example.rs:158` `example_body_is_at_most_15_lines` (cog 28) — SQL-server example test, NOT workbook.

No SATD introduced; no bare `#[allow(clippy::cognitive_complexity)]` added.

### Task 2 — make purity-check (the umya-isolation boundary)

```
$ make purity-check
purity-check: Layer 1 clean — no umya/calamine/quick-xml/swc_/pmcp-code-mode in
  pmcp-workbook-runtime pmcp-workbook-dialect (all feature combos);
  rust_xlsxwriter present in pmcp-workbook-runtime (zip permitted via the writer)
purity-check: pmcp-server-toolkit workbook + workbook-embedded are reader-free
purity-check: pmcp-workbook-compiler reader-present (umya-spreadsheet found),
  single quick-xml version, zip versions bounded to writer+reader (2)
purity-check: pmcp-workbook-server reader-free (umya/calamine/quick-xml/swc_/pmcp-code-mode absent)
bans ok / bans ok
purity-check PASSED
```
**exit 0.**

Negative cargo-tree assertions (verbatim):
```
$ cargo tree -p pmcp-workbook-runtime -i umya-spreadsheet
error: package ID specification `umya-spreadsheet` did not match any packages   ← not in tree

$ cargo tree -p pmcp-workbook-runtime -i calamine
error: package ID specification `calamine` did not match any packages           ← not in tree
```

`grep -rln calamine crates/ cargo-pmcp/` returns ONLY comment lines and cargo-deny BAN declarations (`pmcp-workbook-runtime/deny.toml:43`, `pmcp-workbook-dialect/deny.toml:36` — `{ name = "calamine" }` ban entries) and explanatory comments in `Cargo.toml` — **no `[dependencies]` entry**: the anti-pattern second reader was never added.

`umya-spreadsheet` is positively present ONLY in `pmcp-workbook-compiler` (the reader/writer exception, confirmed by the gate's `-i umya-spreadsheet` positive assertion). `quick-xml` is single-version, compiler-confined.

### Task 3 — retired-symbol sweep

```
$ grep -rnE 'strip_governance_prefix|name_named_inputs|promote_named_outputs|CalculateHandler' \
    crates/ cargo-pmcp/ examples/ --include='*.rs'
  → CalculateHandler: ZERO matches (RETIRED — Plan 04 Checkpoint A)
  → strip_governance_prefix / name_named_inputs / promote_named_outputs: STILL PRESENT
    (crates/pmcp-workbook-compiler/src/lib.rs:609/643/901/903, manifest_model.rs:177/189/907/942)
    — the DOCUMENTED Plan-04 Rule-4 architectural deferral (see Exception below)

$ grep -rn 'fn outputs' crates/pmcp-workbook-runtime/src/artifact_model.rs
  → ZERO matches (Plan-03 CellMap::outputs() shim RETIRED — Plan 04)

$ grep -rn '\.outputs()' crates/ --include='*.rs' | grep -v '//'
  → ZERO non-comment callers (only a doc-comment in cell_map.rs:29 noting the accessor is retired)

$ grep -rnE 'DefinedNameSpec|define_name\(' crates/ cargo-pmcp/ examples/ --include='*.rs' | grep -E 'in_|out_'
  → ZERO matches (no in_*/out_* governance-injection site in any fixture/test)
```

**Documented exception (Q1, legitimate, NOT in the banned list):** `Role::from_name_prefix` with its `const_` branch survives at `crates/pmcp-workbook-runtime/src/manifest_model.rs:58` (`in_` → Input, `const_` → Constant, `out_` → Output). This is the still-consumed governance role-mapping the Plan-04 Q1 audit deliberately KEPT — it is explicitly excluded from the banned-symbol list by the plan (`from_name_prefix`/`const_` are NOT banned for this reason). Not deleted.

## Accomplishments

- **Task 1 — quality + PMAT complexity gate:** Ran the CI-exact `make lint` (root `pmcp` clippy, pedantic+nursery, generous allow-list) → GREEN. Ran the PMAT 3.15.0 cognitive gate → zero `src/`-scope violations (CI gate green) and zero in every phase-100 workbook path. Validated the workbook crate-cone (runtime + compiler + dialect), the served-toolkit `workbook`/`workbook-embedded` suites, the cargo-pmcp `workbook_explain` + embedded-mirror tests, the served-binary build, and BOTH ALWAYS `cargo run --example` arms — all green. fmt clean.
- **Task 2 — purity boundary:** `make purity-check` → exit 0 across all three WBRT-04 layers (per-crate/per-feature cargo-tree reader-bans + the compiler positive `umya` assertion + crate-local cargo-deny bans). Captured the `umya`/`calamine` not-in-tree negative assertions on the runtime and proved no `calamine` `[dependencies]` entry exists anywhere. The umya-isolation boundary is intact post-redesign.
- **Task 3 — retired-symbol sweep:** Proved `CalculateHandler` and the Plan-03 `CellMap::outputs()` shim are GONE workspace-wide, and no `in_*/out_*` governance-injection site survives. Recorded the surviving named-range orchestrator trio as the documented Rule-4 deferral and the kept `const_` role branch as the legitimate Q1 exception.

## Task Commits

1. **Tasks 1–3 (combined gate verdict + ledger update):** committed alongside this SUMMARY (the gates produce no source changes; the only artifact is the `deferred-items.md` out-of-scope ledger entry for the Phase-93 fuzz wrapper drift discovered during the Task-1 fuzz-compile sweep).

## Decisions Made

- **The Rule-4 deferral is the sweep verdict, not a defect.** `strip_governance_prefix`/`name_named_inputs`/`promote_named_outputs` survive because the production `compile_workbook`/`prepare_candidate` orchestrator still drives the named-range path the existing fixture corpus (`tax-calc.xlsx`/`loan-calc.xlsx`/`leap1900-probe.xlsx`) + the Phase-96 generalization/quirk proofs depend on. Re-sourcing outputs from harvested Tables is a multi-plan pipeline rewrite (Plan-04 SUMMARY §"Architectural Deferral (Rule 4)") explicitly out of scope here. Recorded, not attempted.
- **rust_xlsxwriter stays in the runtime.** The plan's Task-2 acceptance criterion #4 ("neither umya nor `rust_xlsxwriter` in the served binary tree") is STRICTER than — and contradicts — the Phase-91 runtime-as-writer design that `make purity-check` positively enforces (`PURITY_WRITER_CRATES` requires `rust_xlsxwriter` PRESENT in `pmcp-workbook-runtime`). The load-bearing invariant is **no reader** (umya/calamine/quick-xml), which holds in every served tree. The authoritative gate is `make purity-check` (GREEN); the writer edge through the runtime is by design.
- **Workspace-wide aggregate vs. binding CI gate.** A bare `make quality-gate` build/test step cannot be globally green because of the pre-existing `pmcp-toolkit-mysql` sqlx E0277 (SQL-connector milestone). The binding CI gate (`make lint` + PMAT `src/`) is GREEN; the verdict is recorded against the workbook crate cone + the CI-binding subset, with the full workspace picture reported honestly.

## Deviations from Plan

### Auto-fixed Issues

None — this is a verification/gate plan; the gates passed (or recorded the documented deferral) without requiring source fixes inside scope.

### Recorded Deferrals & Exceptions (not auto-fixed by design)

**1. [Plan-04 Rule-4 deferral — recorded, not rewritten] `strip_governance_prefix`/`name_named_inputs`/`promote_named_outputs` survive.** The retired-symbol sweep finds these three in the production compile orchestrator. Per the orchestrator directive and the Plan-04 SUMMARY, fully retiring them is an architectural pipeline rewrite gated by the named-range fixture corpus — explicitly out of scope. The OBSERVABLE WBV2-04/05 value (served multi-tool fan-out, per-tool reconcile/collision primitives) IS fully live; `CalculateHandler` + the Plan-03 shim ARE retired.

**2. [Q1 exception — legitimate kept symbol] `Role::from_name_prefix` / `const_` branch.** Deliberately excluded from the banned list (still-consumed governance role mapping, Plan-04 Q1 audit). Documented, not deleted.

**3. [Plan Task-2 criterion vs. actual gate] `rust_xlsxwriter` present in the served runtime tree.** The runtime carries the writer by design (Phase 91); `make purity-check` requires it. Recorded as a plan-text-vs-gate discrepancy resolved in favor of the authoritative gate, not a boundary breach (no reader leaked).

## Out-of-scope failures (excluded, verified NOT phase-100 regressions)

| Failure | Verified baseline | Disposition |
|---------|-------------------|-------------|
| `pmcp-toolkit-mysql` sqlx `SqlSafeStr` E0277 (`lib.rs:267`/`653`) | Present at HEAD (`cargo build -p pmcp-toolkit-mysql` reproduces it now) | SQL-connector milestone; blocks the workspace-wide `make quality-gate` build/test step only — not the CI clippy/PMAT gate, not the workbook cone |
| auth-cache `normalize_round_trip_idempotent` proptest red | Pre-existing, unrelated (deferred-items) | Out of scope |
| `pmcp-server-toolkit/src/code_mode.rs:557` `unused_imports` under `--features workbook` | Introduced by v2.9.0 PR #267 (commit `fffa999e`), pre-dates phase 100 | Out of scope |
| `fuzz_provenance_reader` stable-compile `E0425` (`fuzz_targets/fuzz_provenance_reader.rs:35`) | Phase-93 target (commit `785fe601`); `git log 453ce034..HEAD` of the file + `raw_parts.rs` is EMPTY → untouched by phase 100. Phase-100 fuzz targets (`workbook_table_ingest`, `dag_upstream_leaves`) build clean under stable | Phase-93 provenance milestone; logged to deferred-items this plan |

## Threat Model Outcome

- **T-100-14 (reader leaking into served binary → Elevation):** mitigated. `make purity-check` GREEN; `umya`/`calamine` proven not-in-tree on `pmcp-workbook-runtime`; `pmcp-workbook-server`/`pmcp-server-toolkit` `workbook`+`workbook-embedded` reader-free; umya compiler-confined.
- **T-100-15 (cog>25 / SATD / clippy regression shipping silently):** mitigated. `make lint` GREEN; PMAT cog gate GREEN in CI scope and CLEAN in all phase-100 workbook paths; no SATD/bare-allow introduced.
- **T-100-18 (retired named-range symbol or Plan-03 shim surviving as dead compat code):** mitigated for `CalculateHandler` (gone) + the `CellMap::outputs()` shim (gone) + `in_*/out_*` injection (none). The named-range orchestrator trio survives as the EXPLICIT Plan-04 Rule-4 deferral (live, still-consumed by the production compile path — not dead compat code), recorded here per the directive.
- **T-100-SC (package installs):** accept — no installs this phase; `cargo audit` runs inside `make lint`'s sibling `audit` target.

## Known Stubs

None introduced by this plan. (The per-tool reconcile/collision primitives remain tested-but-not-wired into the production orchestrator — that is the Plan-04 Known Stub bound to the same Rule-4 deferral, unchanged here.)

## Threat Flags

None — this plan adds no new network endpoint, auth path, file-access pattern, or trust-boundary schema; it only runs gates and records verdicts.

## Self-Check: PASSED

- Created file verified present: `.planning/phases/100-excel-workbook-built-in-servers-v2/100-06-SUMMARY.md`.
- Modified file verified present: `.planning/phases/100-excel-workbook-built-in-servers-v2/deferred-items.md` (Phase-93 fuzz-wrapper ledger entry added).
- `make lint`: GREEN ("✓ No lint issues").
- `make purity-check`: PASSED (exit 0).
- PMAT cog gate: zero `src/`-scope violations, zero phase-100 workbook-path violations.
- Retired-symbol sweep: `CalculateHandler` gone, `fn outputs`/`.outputs()` shim gone, `in_*/out_*` injection none; named-range trio recorded as Rule-4 deferral; `const_` exception documented.
- Both ALWAYS examples + the workbook crate-cone/served-toolkit/cargo-pmcp workbook tests: GREEN.

---
*Phase: 100-excel-workbook-built-in-servers-v2*
*Completed: 2026-06-20*
