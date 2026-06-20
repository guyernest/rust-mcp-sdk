---
phase: 100-excel-workbook-built-in-servers-v2
plan: 05
subsystem: workbook-explain-preview-and-ba-docs
tags: [workbook-explain, dry-run-preview, tool-surface, dag-derived-inputs, mdbook, ba-authoring, docs-three-shapes]

# Dependency graph
requires:
  - phase: 100-excel-workbook-built-in-servers-v2
    plan: 04
    provides: "served multi-tool surface (one tool per output Table, DAG-derived per-tool inputSchema + non-empty outputSchema), sanitize_tool_name shared sanitizer, build_tools/Tool model"
provides:
  - "cargo pmcp workbook explain <file.xlsx> — read-only ingest→synth dry-run preview of the served multi-tool surface (text default + --format json), data→stdout / advisory→stderr (Phase-74 D-11), NO bundle written"
  - "explain_surface.rs — the PURE tool-surface projection + render (ToolSurface/InputParam/OutputField + explain_workbook/project_tool_surface/format_tool_surface), mounted into the cargo-pmcp lib via #[path] as crate::workbook_explain (mirrors templates_workbook_server)"
  - "per-tool input DAG derivation by walking each output cell's formula refs back to the input pool (calculate_tax=[filing,income]; estimate_refund=[filing,income,withheld] — disjoint on withheld)"
  - "pmcp_workbook_compiler::sanitize_tool_name re-export at the compiler crate root (single shared source explain reuses)"
  - "pmcp-book Chapter 12.14 + pmcp-course chapter teaching the table-based authoring contract (table model only; retired named-range identifiers proven absent)"
affects: [cargo-pmcp-workbook-cli, pmcp-book, pmcp-course, ba-onboarding]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Pure-leaf lib seam: the read-only projection/render lives in a dependency-light module (compiler ingest/synth + serde/anyhow, NO clap/GlobalFlags) mounted into the lib target via #[path], so the example + integration test reach it WITHOUT the bin-only commands::* tree — the established templates_workbook_server convention"
    - "Deferral-respecting projection: explain harvests the served surface DIRECTLY from the ingested TableRecords + harvest_input_row/harvest_output_row (the template_harvest_e2e path) rather than the deferred named-range manifest+DAG orchestrator, then derives per-tool inputs via a formula-ref reachability walk — honest to what the served binary emits without depending on the Plan-04 Rule-4 deferral"
    - "Orphan-input policy: a variable input no output Table's formulas reach is surfaced workbook-wide (on every tool) so the preview never silently drops an authored caller parameter (e.g. filing, gated by an enum but not referenced by a formula in the template)"

key-files:
  created:
    - cargo-pmcp/src/commands/workbook/explain.rs
    - cargo-pmcp/src/commands/workbook/explain_surface.rs
    - cargo-pmcp/tests/workbook_explain.rs
    - cargo-pmcp/examples/workbook_explain.rs
    - pmcp-book/src/workbook-table-authoring.md
    - pmcp-course/src/workbook-table-authoring.md
  modified:
    - cargo-pmcp/src/commands/workbook/mod.rs
    - cargo-pmcp/src/lib.rs
    - crates/pmcp-workbook-compiler/src/lib.rs
    - pmcp-book/src/SUMMARY.md
    - pmcp-course/src/SUMMARY.md

key-decisions:
  - "explain projects the served surface from the ingest records DIRECTLY (harvest_input_row/harvest_output_row over TableRecords) rather than build_tools(manifest, dag, output_tables) — build_tools needs the named-range manifest+formula DAG the Plan-04 Rule-4 deferral left un-wired for the raw-xlsx table path; the direct harvest matches the template_harvest_e2e proof and the served emit, and writes NO bundle"
  - "Per-tool inputs derived by a formula-ref reachability walk (extract_a1_refs over each output cell's formula, transitively) — gives the disjoint surface (withheld reaches estimate_refund only) the spec §4.2 worked example specifies, without the runtime Dag (which the table path does not build here)"
  - "A variable input reached by NO output formula is surfaced on EVERY tool (workbook-wide), so the template's `filing` (enum-gated but not formula-referenced) appears on both tools, matching the §4.2 example [filing,income] / [filing,income,withheld] — nothing authored is silently dropped"
  - "Split the pure projection into explain_surface.rs (lib-mountable, no clap/GlobalFlags) and kept explain.rs as the thin CLI arm (ExplainArgs + execute) — the commands::* tree is bin-only, so the lib seam is required for the example + integration test (the templates_workbook_server precedent)"
  - "Re-export sanitize_tool_name at the compiler crate root rather than adding pmcp-workbook-runtime as a new cargo-pmcp dependency — single shared source, one dep edge"

patterns-established:
  - "Pattern: read-only CLI preview modelled on lint.rs — dual --format text|json, a PURE format_tool_surface(&[ToolSurface], format) String renderer (JSON testable without stdout capture), advisory header → stderr gated on should_output()/PMCP_QUIET, data → stdout"
  - "Pattern: docs in three shapes led by the cargo pmcp CLI — the book + course chapters open with `cargo pmcp workbook explain` and return to it as the deploy-time habit"

requirements-completed: [WBV2-06, WBV2-07]

# Metrics
duration: ~55min
completed: 2026-06-20
---

# Phase 100 Plan 05: Workbook Explain Preview + BA Authoring Docs Summary

**`cargo pmcp workbook explain <file.xlsx>` now renders the exact served tool surface an AI will see BEFORE deploy — one tool per output Table, each with a DAG-derived per-tool `inputSchema` (the template yields disjoint inputs: `calculate_tax`=[filing,income], `estimate_refund`=[filing,income,withheld]) and a non-empty `outputSchema` — text by default and `--format json` for tooling, writing NO bundle (a pure read-only ingest→synth projection). A runnable `cargo run --example workbook_explain` demonstrates it on the shipped `template.xlsx`, a 5-test snapshot integration test pins the render, and the table-based authoring contract is now taught in both a pmcp-book chapter and a pmcp-course chapter (seeded from the spec + template, retired named-range identifiers proven absent).**

## Performance

- **Duration:** ~55 min
- **Tasks:** 2 (both `type=auto`; Task 1 `tdd=true`)
- **Files:** 6 created, 5 modified

## Accomplishments

- **Task 1 — workbook explain subcommand + example** (`da8cff38`): Added the `Explain` variant + dispatch arm to `WorkbookCommand` and `pub mod explain;`/`pub mod explain_surface;`. `explain_surface.rs` is the PURE projection: `explain_workbook(path)` runs a read-only `ingest::ingest` (NO bundle), `project_tool_surface(&WorkbookMap)` harvests the Inputs Table into a shared input pool (via `harvest_input_row`, strict→constant excluded, variable→exposed with unit + list-DV enum) and projects ONE `ToolSurface` per output Table (name sanitized via the shared `sanitize_tool_name`, description = caption above the Table, outputs via `harvest_output_row`), and derives each tool's minimal inputs by walking its output cells' formula references transitively back to the input pool. `format_tool_surface(&[ToolSurface], format)` is a PURE dual-format renderer (text block per tool: `tool <name>` / `description` / `inputs: key: type [unit] [enum]` / `outputs: key: type [unit]`; JSON serializes the surface). `explain.rs` is the thin CLI arm (`ExplainArgs { workbook_path, --format }` + `execute` with advisory→stderr / data→stdout, Phase-74 D-11). Mounted the pure module into the lib via `#[path]` as `crate::workbook_explain` so the example + test reach it without the bin-only command tree. Re-exported `sanitize_tool_name` at the compiler crate root. Snapshot integration test (`tests/workbook_explain.rs`, 5 tests) over the real `template.xlsx` + 10 surface unit tests + the runnable `examples/workbook_explain.rs` (the CLAUDE.md ALWAYS arm).
- **Task 2 — BA-facing book + course chapters** (`97124c6b`): Wrote `pmcp-book/src/workbook-table-authoring.md` (Chapter 12.14) and `pmcp-course/src/workbook-table-authoring.md` teaching the four region types (§3.1), the standard columns + what `value` does (§3.2–§3.3), output Table → MCP tool with caption description (§4), DAG-derived per-tool inputs (§4.2 calculate_tax/estimate_refund worked example), governance via the tier dropdown (§6), and the `cargo pmcp workbook explain` preview + fail-helpful lint workflow (§8). Both reproduce the §7 annotated reference diagram and use the shipped `template.xlsx` as the running example; both lead with the `cargo pmcp` CLI workflow. The course chapter carries exercises/checkpoints in the course voice. Wired both into their `SUMMARY.md` TOCs. Teach ONLY the table model — a prose migration note ("the table model **replaces** the old named-range model") is included for upgraders, while the retired `in_*`/`out_*`/`define_name` MECHANISM identifiers are proven absent by the scoped negative grep.

## Task Commits

1. **Task 1: workbook explain preview + pure surface lib seam + snapshot test + example** — `da8cff38` (feat)
2. **Task 2: pmcp-book + pmcp-course table-authoring chapters + SUMMARY wiring** — `97124c6b` (docs)

## Decisions Made

- **Direct ingest-record projection (not `build_tools`+DAG).** The Plan-04 Rule-4 deferral left the named-range manifest+formula-DAG orchestrator un-wired for the raw-xlsx table path, so `build_tools(manifest, dag, output_tables)` is not reachable from a raw `template.xlsx`. `explain` instead harvests the served surface DIRECTLY from the ingested `TableRecord`s using the same `harvest_input_row`/`harvest_output_row` projectors the `template_harvest_e2e` proof exercises, then derives per-tool inputs via a formula-reference reachability walk — honest to what the served binary emits (Plan 04), writing NO bundle, and independent of the deferral.
- **Orphan inputs surfaced workbook-wide.** The template's `filing` is a variable, enum-gated input that no output formula references; rather than drop it (a DAG-strict reading would), it is surfaced on every tool so the preview never hides an authored caller parameter — yielding the spec §4.2 surface exactly (`[filing, income]` / `[filing, income, withheld]`).
- **Pure-leaf lib seam.** Split `explain_surface.rs` (lib-mountable, dependency-light) from `explain.rs` (the bin CLI arm with `clap`/`GlobalFlags`), because `commands::*` is bin-only — the established `templates_workbook_server` `#[path]` convention.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `explain` projects the tool surface from the ingest records directly instead of `synth → build_tools`**
- **Found during:** Task 1 (the plan's `<action>` said "runs synth → `build_tools` to project the tool surface").
- **Issue:** `build_tools` requires `(manifest, dag, output_tables)`. Producing a `Manifest` + runtime `Dag` + `OutputTable` membership from a raw `template.xlsx` is exactly the named-range compile orchestrator the Plan-04 Rule-4 deferral left un-wired for the table path; `ingest::ingest` returns a raw `WorkbookMap`, not a `CellMap`/`Manifest`. Calling `build_tools` from `explain` was therefore not reachable without the deferred rewrite (which the plan's carry-forward explicitly says explain must not depend on).
- **Fix:** `explain` harvests the served surface directly from the ingested `TableRecord`s via the SAME `harvest_input_row`/`harvest_output_row`/`harvest_allowed_values` projectors the `template_harvest_e2e` integration proof uses, and derives per-tool inputs by walking each output cell's formula references (`extract_a1_refs`, transitive) back to the input pool. This reproduces the served multi-tool surface (Plan 04) on the real template (disjoint on `withheld`) without depending on the deferral.
- **Files modified:** `cargo-pmcp/src/commands/workbook/explain_surface.rs`.
- **Verification:** the snapshot test + `cargo pmcp workbook explain template.xlsx` print `calculate_tax`=[filing,income] / `estimate_refund`=[filing,income,withheld].
- **Committed in:** `da8cff38`.

**2. [Rule 3 - Blocking] The pure projection was split into a lib-mountable `explain_surface.rs` (the plan placed everything in `explain.rs`)**
- **Found during:** Task 1 (the snapshot test + example import `explain_workbook`/`format_tool_surface`/`ToolSurface`).
- **Issue:** The `commands::*` tree is bin-only — it cross-depends on the CLI subsystem and is NOT exposed by the `cargo_pmcp` lib crate, so a `tests/`/`examples/` file cannot reach `commands::workbook::explain::*`. `explain.rs`'s `execute` also uses `GlobalFlags` (bin-only), so the whole module is not lib-mountable as-is.
- **Fix:** Moved the PURE projection + render + types into `explain_surface.rs` (dependency-light: compiler `ingest`/`synth` + serde/anyhow, NO clap/GlobalFlags) and mounted it into the lib via `#[path]` as `crate::workbook_explain` (the established `templates_workbook_server` convention). `explain.rs` keeps `ExplainArgs` + `execute` and calls the surface module. The test + example import `cargo_pmcp::workbook_explain::*`.
- **Files modified:** `cargo-pmcp/src/lib.rs`, `cargo-pmcp/src/commands/workbook/{explain.rs,explain_surface.rs,mod.rs}`.
- **Verification:** `cargo test -p cargo-pmcp --test workbook_explain` (5 pass) + `cargo run --example workbook_explain` (exit 0).
- **Committed in:** `da8cff38`.

**3. [Rule 3 - Blocking] Re-exported `sanitize_tool_name` at the compiler crate root**
- **Found during:** Task 1 (`explain` must sanitize output-Table names identically to the served registration).
- **Issue:** `sanitize_tool_name` lives in `pmcp-workbook-runtime` but is NOT re-exported at the `pmcp-workbook-compiler` crate root, and `pmcp-workbook-runtime` is not a direct `cargo-pmcp` dependency. Adding the runtime as a new dep OR duplicating the sanitizer would either widen the dep graph or let the preview charset drift from registration.
- **Fix:** Added `pub use pmcp_workbook_runtime::sanitize_tool_name;` at the compiler crate root (the single shared source the served registration + the compiler collision lint already call); `explain` reaches it through its existing compiler dep.
- **Files modified:** `crates/pmcp-workbook-compiler/src/lib.rs`.
- **Committed in:** `da8cff38`.

---

**Total deviations:** 3 auto-fixed (all Rule 3 blocking). No architectural (Rule 4) changes; no authentication gates.

## Threat Model Outcome

- **T-100-12 (malformed .xlsx → DoS):** mitigated. `explain` calls the SAME `pmcp_workbook_compiler::ingest::ingest` boundary as `lint`/`compile` — the Plan-02 `catch_unwind` seam maps a malformed-table panic to a clean `IngestError` (a typed `anyhow` error, exit 1), never a process abort.
- **T-100-13 (strict-constant rendered as a callable input):** mitigated. `harvest_input_pool` marks a `strict`-tier row as `Role::Constant` (`exposed = false`) and excludes it from every tool's `inputs` — the preview advertises the SAME caller-exposed set the server emits (`rate` is strict in the template and never appears as an input). Preview and runtime cannot diverge on which cells are callable.
- **T-100-SC (package installs):** accept — no new crate; mdbook is the existing doc toolchain.

## Known Stubs

None. The preview is a complete, runnable projection over the real `template.xlsx`; the two chapters are complete and built.

## Threat Flags

None — `explain` introduces no new network endpoint, auth path, or trust boundary beyond the read-only ingest already covered by the `<threat_model>`.

## Self-Check: PASSED

- Created files verified present: `cargo-pmcp/src/commands/workbook/explain.rs`, `cargo-pmcp/src/commands/workbook/explain_surface.rs`, `cargo-pmcp/tests/workbook_explain.rs`, `cargo-pmcp/examples/workbook_explain.rs`, `pmcp-book/src/workbook-table-authoring.md`, `pmcp-course/src/workbook-table-authoring.md`.
- Commits verified in git log: `da8cff38` (feat), `97124c6b` (docs).
- `cargo test -p cargo-pmcp --test workbook_explain`: 5 passed.
- `cargo test -p cargo-pmcp --bin cargo-pmcp explain_surface`: 10 passed.
- `cargo run -p cargo-pmcp --example workbook_explain`: exit 0 (prints the disjoint two-tool surface).
- `cargo pmcp workbook explain template.xlsx` (+ `--format json`): exit 0.
- `cargo clippy -p cargo-pmcp --all-targets`: no warnings on the explain files (pre-existing unrelated warnings out of scope).
- `mdbook build` (pmcp-book + pmcp-course): exit 0; both new HTML pages render.
- Negative grep for retired `in_*`/`out_*`/`define_name` identifiers: clean (exit non-zero / nothing found), migration prose note present.

---
*Phase: 100-excel-workbook-built-in-servers-v2*
*Completed: 2026-06-20*
