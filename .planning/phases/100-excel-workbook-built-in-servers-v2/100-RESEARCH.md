# Phase 100: Excel Workbook Built-in servers v2 — Research

**Researched:** 2026-06-20
**Domain:** Excel→MCP workbook compiler; table-based authoring contract; multi-tool emission; DAG-derived per-tool input schemas
**Confidence:** HIGH (the design is locked; this research is a codebase-mapping exercise — every symbol below was located with file:line via grep against the working tree)

## Summary

The design is **already locked** in `docs/design/workbook-table-authoring-contract.md`. This phase is an *engineering lift* onto an existing, mature 4-crate workbook subsystem (`pmcp-workbook-compiler`, `pmcp-workbook-runtime`, `pmcp-workbook-dialect`, `pmcp-server-toolkit/src/workbook`) plus the `cargo-pmcp workbook` CLI. The current model is rigidly **single-tool**: one `CellMap { inputs[], outputs[] }` drives one `calculate` tool covering all named outputs. The new model is **multi-tool**: each named Excel Table becomes its own MCP tool with a DAG-derived input schema.

The three load-bearing facts that de-risk the phase:

1. **umya 3.0.0 reads Tables but the ingest layer does not harvest them yet.** `SheetRecord.tables` is `Vec<RangeRef>` — only table *areas*, NOT table *names* or *column names* `[CITED: docs/design §2; VERIFIED: cell_map.rs:163-164, ingest/mod.rs:310-323]`. The umya API the spec needs (`Table::get_name`, `get_area`, `get_columns`, `TableColumn::get_name`) is confirmed present in the pinned crate `[VERIFIED: ~/.cargo umya-spreadsheet-3.0.0/src/structs/table.rs:65,104,138 + table_column.rs:453]`. So harvest is *additive ingest work*, not a new dependency — the purity boundary is untouched.

2. **A provenance-valid `.xlsx` author already exists in-repo, via `rust_xlsxwriter` (NOT umya).** `fixture_author.rs` (test-only) produces `ProvenanceClass::ExcelTrusted` workbooks for free `[VERIFIED: fixture_author.rs:55-58, 16-39]`. `rust_xlsxwriter` 0.95 supports `Worksheet::add_table`, `Table::set_name/set_columns`, AND `add_data_validation` (the tier dropdown) `[VERIFIED: ~/.cargo rust_xlsxwriter-0.95.0/src/table.rs + worksheet.rs:8968,9388]`. This **unblocks Success Criterion 4 and phasing step 1** — but the template must be promoted out of `#[cfg(test)]` to a shippable artifact, and the author extended to emit Tables (today it emits cells + `out_*`/`in_*` defined names only).

3. **The DAG exists but lacks transitive reachability.** `Dag` exposes forward (`dependencies_of`) and reverse (`dependents`) one-hop adjacency plus `toposort`, but NO "upstream input leaves reachable from cell X" helper `[VERIFIED: runtime/src/dag.rs:65-83]`. §4.2's per-tool input derivation needs exactly that traversal — a new pure function over the existing `Dag`.

**Primary recommendation:** Sequence the phase as the spec's 7 steps. The biggest single lift is the **manifest model extension** (§4.1: `CellMap{inputs,outputs}` → `{ inputs[], tools[] }`) which fans out into the toolkit's schema builders (`schema.rs`), the handler registration (one `CalculateHandler` → N per-tool handlers), and the cell-map emitter. Retire F1/F3-input cleanly (pre-1.0, no compat). Keep F2 untouched.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Read Excel Tables (name/area/columns) | Compiler (umya-owning) | — | umya MUST stay confined to the compiler (purity gate); runtime never reads `.xlsx` |
| Harvest type/unit/enum/tier per row | Compiler (ingest + synth) | — | Compile-time projection; runtime consumes the manifest |
| Build cross-sheet formula DAG | Compiler (`dag/`, `formula/`) | Runtime (`Dag` container) | Build links umya; the `Dag` container is runtime-side for re-execution |
| Per-tool input derivation (reachability) | Runtime or Compiler (`Dag` traversal) | — | Pure `Dag` algorithm — can live runtime-side beside `toposort`; consumed by both compiler emit + served schema |
| Multi-tool manifest model | Runtime (`manifest_model.rs`) | Compiler (synth) | Model types are runtime-owned (reader-free leaf, WBRT-01); synthesis is compiler-side |
| Emit N named MCP tools w/ I/O schema | Served toolkit (`workbook/handler.rs`, `schema.rs`, `mod.rs`) | — | Tool registration + schema projection is the served boundary |
| Provenance-valid template `.xlsx` | Compiler-side author (`rust_xlsxwriter`) | CLI/templates dir | `rust_xlsxwriter` is the ONLY provenance-clean writer; umya-authored = `UmyaFabricated` (refused) |
| `workbook explain` dry-run preview | CLI (`cargo-pmcp/.../workbook/`) | Compiler (ingest+synth) | Read-only ingest→render; model on the existing `lint` subcommand |
| Fail-helpful row linting | Compiler (stage-1 / `dialect/linter.rs`) | — | Reshape F1 into row-level `LintFinding`s in the existing collect-all gate |
| Provenance gate (calcPr/app.xml) | Compiler (`provenance/`, quick-xml-quarantined) | — | **Orthogonal — untouched** (§6) |

## Standard Stack

This phase adds **no new external dependencies** — the purity boundary forbids a second reader, and every capability is reachable with crates already in the workspace.

### Core (already present, verified)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `umya-spreadsheet` | 3.0.0 | Read Excel Tables (compiler-only) | The compiler's existing reader; Table API confirmed reachable `[VERIFIED]` |
| `rust_xlsxwriter` | 0.95 | Author provenance-valid template `.xlsx` (Tables + data-validation dropdowns) | The ONLY writer that yields `ProvenanceClass::ExcelTrusted`; umya-authored books are refused `[VERIFIED]` |
| `quick-xml` | (workspace) | calcPr/app.xml provenance read (quarantined in `provenance/raw_parts.rs`) | Orthogonal to this phase — do NOT touch |
| `serde` / `schemars` | (workspace) | Manifest/CellMap model + JSON-Schema emission | Existing model convention (`rename_all`, `#[serde(default)]` additive precedent) |
| `serde_json` | (workspace) | Tool input/output schema `Value` construction | Existing `schema.rs` pattern |

**Installation:** none — all crates are workspace members or existing deps.

**Version verification:**
```bash
grep umya crates/pmcp-workbook-compiler/Cargo.toml      # umya-spreadsheet "3.0.0" path-pinned (verified)
grep rust_xlsxwriter crates/pmcp-workbook-compiler/Cargo.toml  # "0.95", default-features=false (verified)
```

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| umya for Table read | calamine | **FORBIDDEN** — adds a reader → purity-gate failure. Do not consider. |
| rust_xlsxwriter for template | umya writer | umya-authored books classify `UmyaFabricated` → provenance gate REFUSES them (`fixture_author.rs:36-39`). Must use rust_xlsxwriter. |

## Package Legitimacy Audit

> No external packages are installed in this phase (purity boundary forbids new readers; everything is a workspace dep). Audit not applicable — all crates are already vendored and version-pinned in the workspace. slopcheck step SKIPPED (no `npm/pip/cargo add` of any non-workspace crate).

## Architecture Patterns

### System Architecture Diagram

```
            template.xlsx (shipped, rust_xlsxwriter-authored, provenance-valid)
                    │  (= BA starting point = training artifact = reference fixture)
                    ▼
┌─────────────────────────── COMPILER (umya-owning, offline) ──────────────────────────┐
│  ingest::ingest()  ──► WorkbookMap                                                     │
│     • collect_cells / data_validations / table_ranges  [EXISTING]                      │
│     • HARVEST TABLES: get_name + get_columns per Table  [NEW — additive ingest]        │
│                    │                                                                   │
│                    ▼                                                                   │
│  stage1::run_stage1 (lint+synth+freshness)                                             │
│     • synth → manifest cells (type/unit/enum harvested from value cell)  [RESHAPE]     │
│     • ROW LINTING: blank name / dup key / value-less / no-caption / bad charset [NEW]  │
│                    │                                                                   │
│   RETIRE: promote_named_outputs(out_*) / name_named_inputs(in_*) / refuse_uncallable   │
│   REPLACE with: harvest_tables() → input pool + per-table tool decls                   │
│                    │                                                                   │
│                    ▼                                                                   │
│  build_ir_and_dag ──► Dag (forward+reverse adjacency)  [EXISTING]                      │
│     • NEW: upstream_input_leaves(output_cell) traversal → per-tool input_keys (§4.2)   │
│                    │                                                                   │
│                    ▼                                                                   │
│  reconcile (per-tool oracle = output table `value` cell)  [EXISTING mechanism, per-tool]│
│  provenance::gate::classify (calcPr/app.xml)  [ORTHOGONAL — untouched]                 │
│                    │                                                                   │
│                    ▼  emits bundle: manifest.json (now {inputs[],tools[]}) + IR + DAG  │
└────────────────────────────────────────────────────────────────────────────────────┘
                    │  (reader-free bundle — NO umya crosses)
                    ▼
┌────────────────── SERVED TOOLKIT (pmcp-server-toolkit, reader-free) ─────────────────┐
│  with_workbook_bundle()                                                                │
│     • TODAY: 1× CalculateHandler + ExplainHandler + 3 meta tools                       │
│     • NEW:   N× per-tool handler (one per output table) w/ DAG-derived inputSchema     │
│              + outputSchema (structuredContent) — F2 override advertising RETAINED     │
└────────────────────────────────────────────────────────────────────────────────────┘
                    │
                    ▼     cargo pmcp workbook explain <file>  [NEW — model on `lint`]
              "here is the tool surface an AI will see" (text first)
```

### Pattern 1: Additive serde model extension (the `tier`/`allowed_values` precedent)
**What:** New manifest fields land as `#[serde(default, skip_serializing_if=...)]` so old bundles deserialize unchanged.
**When to use:** Extending `Manifest`/`CellMap` to the `{ inputs[], tools[] }` shape (§4.1).
**Example:**
```rust
// Source: crates/pmcp-workbook-runtime/src/manifest_model.rs:119-132 (the verbatim precedent)
#[serde(default)]
pub tier: Option<InputTier>,
#[serde(default, skip_serializing_if = "Option::is_none")]
pub allowed_values: Option<Vec<String>>,
```
> Note: pre-1.0 with no legacy bundles means a clean structural break is *also* acceptable for `CellMap`→`tools[]`; the additive precedent is the conservative option if the team wants existing fixtures to keep deserializing during transition.

### Pattern 2: Per-tool input derivation via DAG reachability (§4.2 — the new algorithm)
**What:** A pure traversal over `Dag` collecting the `Role::Input` leaf cells transitively reachable upstream of an output table's value cells.
**When to use:** Computing each tool's minimal `input_keys`.
**Example:**
```rust
// NEW — pure fn over the existing Dag (model beside toposort in runtime/src/dag.rs)
// dependencies_of(cell) is the one-hop "depends on" edge (dag.rs:65).
fn upstream_input_leaves(dag: &Dag, output_cell: &str, input_cells: &HashSet<String>) -> BTreeSet<String> {
    let mut seen = HashSet::new();
    let mut leaves = BTreeSet::new();
    let mut stack = vec![output_cell.to_string()];
    while let Some(c) = stack.pop() {
        if !seen.insert(c.clone()) { continue; }
        if input_cells.contains(&c) { leaves.insert(c); continue; } // leaf
        for dep in dag.dependencies_of(&c) { stack.push(dep.clone()); }
    }
    leaves
}
// Edge cases (§4.2): input reachable only via a constant path → not in input_cells → excluded.
// input feeding NO tool → lint "feeds no tool". shared intermediate → each tool gets union of its own leaves.
```

### Pattern 3: Read-only CLI preview (model `workbook explain` on `workbook lint`)
**What:** Ingest → synth → render a human report to stdout; advisory to stderr. No bundle written.
**When to use:** `cargo pmcp workbook explain` (§8, phasing step 6).
**Example:**
```rust
// Source: cargo-pmcp/src/commands/workbook/lint.rs:46-65 — the proven read-only shape
let (map, _findings) = pmcp_workbook_compiler::ingest::ingest(path)?;  // ingest only
// then synth → project tool surface → render text (data→stdout, status→stderr, Phase-74 D-11)
```
Register a new `WorkbookCommand::Explain(ExplainArgs)` variant at `cargo-pmcp/src/commands/workbook/mod.rs:75-90` (the existing `Compile|Lint|Emit` enum + dispatch).

### Anti-Patterns to Avoid
- **Adding a second reader (calamine) to harvest Table metadata** — purity-gate failure. umya already exposes `get_name`/`get_columns`; extend ingest.
- **Authoring the template with umya** — classifies `UmyaFabricated`, refused by the provenance gate. Use `rust_xlsxwriter`.
- **Mutating `role.name` to strip prefixes** — the prefix model is being RETIRED; keying moves to the table `name` column. Do not port `strip_governance_prefix`.
- **One generic `calculate` with a `mode` enum** — explicitly rejected by §4: N named tools are strictly better for LLM tool-selection.
- **Touching `provenance/` or quick-xml** — orthogonal (§6); any change there is out of scope.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Read Table name/columns | Manual XML parse of `xl/tables/tableN.xml` | `Worksheet::get_tables()` + `Table::get_name/get_columns` | umya already walks the relationships (`reader/xlsx.rs:225`) |
| Author provenance-valid `.xlsx` | Hand-craft OOXML zip | `rust_xlsxwriter` + the `fixture_author.rs` recipe | `ExcelTrusted` identity comes for free (`fixture_author.rs:16-39`) |
| Topo-order / cycle detection | New graph code | `pmcp_workbook_runtime::toposort` | Kahn's already implemented + tested (`dag.rs:91`) |
| Located lint findings | Bespoke error strings | `LintFinding::new(Severity, code, sheet, addr, msg, repair)` + `LintReport` collect-all | Existing fail-helpful machinery (`lib.rs:707-759`) |
| Per-tool oracle reconcile | New comparator | Existing reconcile stage keyed per output table | `lib.rs:354-365`, `comparison_from_outputs` (`lib.rs:567`) |
| JSON-Schema emission | Hand-written schema JSON | The `schema.rs` builders (extend per-tool) | `dtype_json_type`, `result_envelope_schema`, enum-from-`allowed_values` all exist |

**Key insight:** Almost every primitive this phase needs already exists; the work is *re-wiring single→multi* and *re-keying named-range→table-name*, not green-field construction.

## Runtime State Inventory

> This phase includes a rename/retire dimension (the `in_*`/`out_*` model). Inventory of what carries the old convention beyond source files:

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | **None** — bundles are recompiled from source `.xlsx`; no persisted datastore keyed on `in_*`/`out_*`. Verified: no DB/KV in the workbook served path (`mod.rs` registers handlers over an in-memory `WorkbookBundle`). | Recompile fixtures from the new template; no data migration. |
| Live service config | **None** — workbook servers are stateless (Shape A `pmcp-workbook-server` binary embeds a bundle at build). No UI-resident config. | None. |
| OS-registered state | **None** — no scheduler/daemon registrations reference the convention. | None. |
| Secrets/env vars | `PMCP_REGEN_FIXTURES=1` gates fixture regeneration (`fixture_author.rs:45`) — a test toggle, not a secret; name unchanged. | None (re-run regen after the template lands). |
| Build artifacts / committed fixtures | `tests/fixtures/{tax-calc,loan-calc,leap1900-probe}.xlsx` + `*.gen.json` sidecars + `cargo-pmcp/src/templates/workbook_bundle/tax-calc.xlsx` all carry `in_*`/`out_*` defined names (the misleading hand-authored set, §7). `fixture_author.rs:388-935` injects `in_a1`/`in_value`/`in_loan_amount`/… defined names. | **REPLACE** with the table-based template + regenerated reference fixtures (phasing step 1). Delete stale `.xlsx` + `.gen.json` once superseded. |

**The canonical question — what still has the old string after source is updated?** Only the committed `.xlsx` fixtures + their `.gen.json` sidecars + the CLI template copy. All are regenerated from the new `rust_xlsxwriter` author; none is live runtime state.

## Cleanup-Ledger File:Line Map (§9 — retire / reshape / keep)

| §9 Item | Symbol | Location | Fate | Notes |
|---------|--------|----------|------|-------|
| F2 — advertise override keys | `input_schema_for_manifest` overrides block | `pmcp-server-toolkit/src/workbook/schema.rs:298-310` (+ tests 533-608) | **KEEP** | Independent of input model; `variable_tier_keys` is the single source. Survives the rewrite. |
| F3 (outputs) — strip `out_` | `strip_governance_prefix` `"out_"` branch | `runtime/src/manifest_model.rs:190` | **SUBSUMED** | Output keys come from the table `name` column. Remove once tables land. |
| F3 (inputs) — strip `in_` | `strip_governance_prefix` `"in_"` branch + `json_key_for_role` strip call | `runtime/src/manifest_model.rs:175-198` | **RETIRE** | No `in_*` named ranges anymore. Delete `strip_governance_prefix`; simplify `json_key_for_role` to `name → meaning → cell` with NO strip. |
| F1 — hard-error on missing `in_*` | `refuse_uncallable_inputs` + `validate_input_keys` + `unnamed_input_finding`/`duplicate_input_key_finding`/`empty_input_key_finding` | `compiler/src/lib.rs:675-780` (called at `lib.rs:331`) | **RESHAPE** | Re-target the same `LintFinding` codes at table ROWS (blank `name`, dup key, value-less row) instead of named ranges. Intent preserved; the messages must drop "define a named range `in_<name>`" repair text (`lib.rs:715-721`). |
| `name_named_inputs` (in_*) | `name_named_inputs` | `compiler/src/lib.rs:637-656` (called `lib.rs:325`) | **RETIRE** | Replaced by table harvest assigning `name` from the table `name` column. |
| `promote_named_outputs` (out_*) | `promote_named_outputs` | `compiler/src/lib.rs:603-620` (called `lib.rs:321`) | **RETIRE/RESHAPE** | Output detection moves to "named output Table" instead of `out_*` defined name. The Role::Output promotion logic is still needed — re-source it from tables. |
| `json_key_for_role` strip | `json_key_for_role` | `runtime/src/manifest_model.rs:175-180` | **RESHAPE** | Keep the fn (cell_map.rs:70, schema.rs use it) but remove the prefix strip; key = table `name` verbatim. |
| `in_*`/`out_*` ingestion path | `DefinedNameRecord` consumers via `map.defined_names` + `Role::from_name_prefix` | `runtime/manifest_model.rs:58-68`; `cell_map.rs:65-72`; `synth.rs:852` | **RETIRE for input/output keying** | `Role::from_name_prefix` (`const_` too) may still be used by the overlap/Guide check — confirm during plan; keep only if a non-named-range consumer remains. |
| Fixture `in_*` injections | `DefinedNameSpec { name: "in_*" }` × 11 | `fixture_author.rs:388,543,575,622,660,664,702,865,869,873,935` | **REPLACE** | Extend `fixture_author` to emit Tables; regenerate fixtures from the new template. |
| Misleading committed fixtures | `tax-calc.xlsx` / `leap1900-probe.xlsx` / synthetic | `tests/fixtures/*.xlsx` + `cargo-pmcp/src/templates/workbook_bundle/tax-calc.xlsx` | **REPLACE** | New table-based template doubles as the honest reference fixture (§7). |
| Provenance gate | `provenance::gate::classify`, `raw_parts` (quick-xml) | `compiler/src/provenance/gate.rs`, `raw_parts.rs:41-49` | **KEEP — UNTOUCHED** | Orthogonal (§6). Do not modify. |

## Common Pitfalls

### Pitfall 1: Tables are ingested as ranges only — the metadata is silently dropped
**What goes wrong:** Assuming `SheetRecord.tables` already carries names/columns. It does NOT — `table_ranges` only reads `t.area()` (`ingest/mod.rs:310-323`), and `SheetRecord.tables: Vec<RangeRef>` (`cell_map.rs:163-164`) has no name field.
**Why it happens:** umya's Table type *has* `get_name`/`get_columns` but the existing ingest path never calls them (verified: zero `get_columns()` calls in `ingest/`).
**How to avoid:** Add a `TableRecord { name, area: RangeRef, columns: Vec<String> }` to `SheetRecord` (additive); call `t.get_name()`/`t.get_columns().map(get_name)` in a new `table_records()` ingest fn.
**Warning signs:** A harvested manifest with `name: None` on every input — the symptom the F1 lint was built to catch.

### Pitfall 2: umya `.unwrap()`s on malformed table XML (§2 caveat 1)
**What goes wrong:** A bad `tableN.xml` panics inside umya instead of returning a clean compile error.
**Why it happens:** umya's table reader is not panic-free.
**How to avoid:** Keep table reads inside the existing umya-isolation boundary in `ingest::ingest` (which already maps ingest failures to `CompileError::Ingest` at `lib.rs:294`). Do NOT let a umya panic cross into the served path. Add a fuzz target feeding malformed table bytes to confirm clean-error (not panic).
**Warning signs:** A panic in a compile test rather than a `CompileError`.

### Pitfall 3: The fan-out from single→multi-tool touches three layers at once
**What goes wrong:** Changing only the manifest model leaves `schema.rs` (`input_schema_for_manifest`/`output_schema_for_manifest` take one `CellMap`) and `handler.rs` (one `CalculateHandler`) emitting the old single tool.
**Why it happens:** The single-tool assumption is baked into the served toolkit at `schema.rs:76,275`, `handler.rs:144-201`, `mod.rs:249-265`.
**How to avoid:** Plan the model change + schema-per-tool + handler-per-tool + registration loop as ONE coherent wave so `make quality-gate` is only run on a consistent tree. Each tool's schema reuses the existing builders parameterized by that tool's `input_keys`/`outputs`.
**Warning signs:** `tools/list` showing one `calculate` after the model change.

### Pitfall 4: Template must be a SHIPPED artifact, but the author is `#[cfg(test)]`
**What goes wrong:** Reusing `fixture_author.rs` directly fails — it is `#![cfg(test)]` (`fixture_author.rs:51`) and not part of any built artifact.
**Why it happens:** It was scoped as a test helper.
**How to avoid:** Either (a) promote the table-authoring logic to a non-test module / a small `xtask`-style generator the build can invoke, or (b) generate `template.xlsx` once via the env-gated `regenerate_fixtures` path and commit it (the `.gen.json` sidecar discipline). Recommend (b) for v1: commit a generated `template.xlsx` + sidecar, matching the existing reproducible-fixture pattern (`fixture_author.rs:42-49`).
**Warning signs:** A linker error pulling `rust_xlsxwriter` into a non-test build, or the template diverging from its generator.

### Pitfall 5: Cognitive complexity (CLAUDE.md / Phase 99 just shipped a cog-reduction)
**What goes wrong:** A multi-tool harvest + DAG-traversal + per-tool emit easily exceeds cog 25 in one function.
**Why it happens:** Naturally branchy loops over tables × rows × columns.
**How to avoid:** Decompose per the P1–P6 techniques in `75-RESEARCH.md`; one fn per concern (harvest, derive-inputs, emit-schema). PMAT runs in CI only (`pmat quality-gate --fail-on-violation --checks complexity`, cog ≤25, hard cap 50). Phase 99 just did this for the same crates — match that style.
**Warning signs:** Local PMAT `analyze complexity` flags `src/` functions > cog 25.

## Code Examples

### umya Table read (the new harvest — confirmed API)
```rust
// Source: VERIFIED ~/.cargo umya-spreadsheet-3.0.0/src/structs/table.rs:65,104,138; table_column.rs:453
for ws in book.get_sheet_collection() {
    for t in ws.get_tables() {                 // -> &[Table]
        let name = t.get_name();               // ListObject name = TOOL name candidate
        let (start, end) = t.get_area();        // (Coordinate, Coordinate)
        let cols: Vec<&str> = t.get_columns().iter().map(|c| c.get_name()).collect();
        // expect cols == ["name","value","description"] (+ "tier" for input tables)
    }
}
```

### Data-validation harvest already wired (enum + tier dropdown source)
```rust
// Source: crates/pmcp-workbook-compiler/src/ingest/mod.rs:332-356
// data_validations() already emits one DataValidationRecord per (DV × range) with
// dv_type "list" + raw formula1 — this is the enum/tier dropdown source the harvest needs.
```

### Tool registration loop (the multi-tool target)
```rust
// Source: crates/pmcp-server-toolkit/src/workbook/mod.rs:249-265 (CURRENT single-tool)
// NEW: iterate bundle.manifest tools, register one handler each:
let mut builder = self;
for tool in &bundle.tools {
    builder = builder.tool_arc(&tool.name, Arc::new(WorkbookToolHandler::new(bundle.clone(), tool.clone())));
}
// each handler.metadata() returns ToolInfo::with_ui(name, desc, input_schema(tool), ui)
//   .with_output_schema(output_schema(tool))  -- the existing TypedToolWithOutput pattern (handler.rs:180-200)
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Per-cell `in_*`/`out_*` named ranges + colour-role synth | Named Excel Tables, columns `name\|value\|description\|tier`, row iteration | This phase (Phase 100) | BA authors in visible standard Excel; no invisible named-range failure mode |
| One `calculate` tool, all outputs | One MCP tool per output Table, DAG-derived inputs | This phase | Better LLM tool-selection; minimal per-tool schemas |
| Hand-authored misleading fixtures | One `rust_xlsxwriter` template = starting point + training + reference fixture | This phase | Honest fixtures; provenance-valid by construction |
| Single `CellMap{inputs,outputs}` | `{ inputs[], tools[] }` manifest | This phase | The core engineering lift (§4.1) |

**Deprecated/outdated (retire this phase):** `strip_governance_prefix`, `name_named_inputs`, the `in_*` half of `promote_named_outputs`/F1, the `in_*`-injecting fixtures.

## Open Questions Resolved (§10 — recommendations)

1. **`0_meta` key set + optionality.** **Recommendation: fully optional; minimal v1 key set = `{ server, version }`** (server-name hint + workbook version). Governance *defaults* deferred (tier already lives per-row in the input table per §3.2, so a meta-level default is redundant for v1). Co-location principle (§3.2/§7) means meta carries NO per-field/per-tool data. If absent, derive server name from the workbook filename and version `"1"`. Rationale: the spec's diagram shows only `server`+`version`; adding more invites drift.

2. **v1 per-tool input override vs DAG-derived-only.** **Recommendation: DAG-derived ONLY for v1** (the spec leans this way at §4.2: "Out of scope for v1 unless a real case appears"). Ship the reachability algorithm + the three edge-case lints; do NOT build the explicit `inputs:` caption/column override surface. Leave a documented seam (a future column/caption) so adding it later is additive. Rationale: the motivating examples (`calculate_tax`, `estimate_refund`) are fully covered by DAG derivation; the override is speculative complexity.

3. **`cargo pmcp workbook explain` output format.** **Recommendation: human-readable TEXT first** (data→stdout, advisory→stderr per Phase-74 D-11, matching `lint.rs:114-153`); add a `--format json` flag as a thin add-on in the same plan (the renderer already takes a `format` param in `lint.rs`). Text shows: per-tool `name`, `description`, input schema (key: type [unit] [enum]), output schema (key: type [unit]). Rationale: the human dry-run is the primary guard against silent-broken-deploy (§8); JSON is cheap to add given the existing dual-format renderer pattern.

## Proposed Requirement Breakdown (REQ-style — planner can lock)

Mapped 1:1 to the spec's 7 phasing steps (§11). Suggested ID prefix `WBV2-` (workbook v2).

| Req ID | Maps to step | Description | Success Criterion |
|--------|-------------|-------------|-------------------|
| **WBV2-01** | 1 | Ship a provenance-valid `template.xlsx` (rust_xlsxwriter-authored, `ExcelTrusted`) carrying an Inputs Table (`name\|value\|description\|tier` + tier dropdown + a sample enum dropdown), calc + reference regions, and ≥1 named output Table with a caption description. Doubles as the honest reference fixture. | SC4 |
| **WBV2-02** | 2 | Ingest harvests Excel Tables: add `TableRecord{name,area,columns}` to `SheetRecord` via `get_tables()/get_name()/get_columns()`; harvest per-row type (value-cell type), unit (number format), enum (data-validation list), tier (tier column) into the manifest. umya panic on malformed table XML is contained as a clean `CompileError`. | SC1 |
| **WBV2-03** | 3 | Extend the manifest model `CellMap{inputs,outputs}` → `{ inputs[], tools[] }` (additive serde). Add `Dag::upstream_input_leaves` reachability; derive each tool's `input_keys` from its output Table's cells. Handle §4.2 edge cases (constant-only path excluded; input feeding no tool lint; shared-intermediate union). | SC1, SC2 |
| **WBV2-04** | 4 | Emit one named MCP tool per output Table: tool name = sanitized table name (`^[a-zA-Z0-9_-]{1,64}$`), description = caption cell, per-tool `inputSchema` (DAG-derived) + non-empty `outputSchema` → `structuredContent` (TypedToolWithOutput). Register N handlers in `with_workbook_bundle`. **F2 override advertising retained.** | SC2, SC5 |
| **WBV2-05** | 5 | Per-tool reconciliation: grade each tool's computed outputs against its output-Table `value` oracle. Reshape F1 into fail-helpful, cell-precise ROW lints (blank `name`, duplicate key, value-less row, output Table with no caption, tool name unmappable to MCP charset). **Retire F1/F3-input + `strip_governance_prefix` + `name_named_inputs`.** | SC3, SC5 |
| **WBV2-06** | 6 | `cargo pmcp workbook explain <file>`: read-only ingest→synth→render the emitted tool surface (tool names/descriptions/per-tool I/O schemas) to stdout BEFORE deploy. Text first; `--format json` thin add-on. Modeled on `workbook lint`. | SC3 |
| **WBV2-07** | 7 | Docs/training: pmcp-book + pmcp-course chapters seeded from the spec + the template; the BA "your Excel process becomes a governed AI-callable tool" story. | SC4 (training arm) |
| **WBV2-08** | (cross) | `make quality-gate` + PMAT (cog ≤25) + `make purity-check` all green; no umya/calamine/quick-xml in any served tree; rust_xlsxwriter confined to compiler/author. | SC5 |

**ALWAYS-requirements (CLAUDE.md, per-feature):** each of WBV2-02..06 ships fuzz + property + unit tests + a `cargo run --example` demonstration.

## Validation Architecture

> nyquist_validation is enabled (config.json `workflow.nyquist_validation: true`). Section included.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `proptest`/`quickcheck` (property), `cargo fuzz` (fuzz), `cargo run --example` (demonstration) — per CLAUDE.md ALWAYS-requirements |
| Config file | none — cargo workspace; `make`/`justfile` drive gates |
| Quick run command | `cargo test -p pmcp-workbook-compiler -p pmcp-workbook-runtime -p pmcp-server-toolkit --lib` |
| Full suite command | `make quality-gate` (fmt --all, clippy pedantic+nursery, build, test, audit) **+** `make purity-check` |

### Success-Criterion → Test Map
| SC | Behavior | Test Type | Command / Artifact |
|----|----------|-----------|--------------------|
| SC1 | Tables harvested; rows → typed/unit/enum/tier fields | unit + property | `cargo test -p pmcp-workbook-compiler harvest`; property: round-trip a generated Table spec → manifest fields ❌ Wave 0 |
| SC1 | malformed table XML → clean `CompileError` not panic | fuzz | `cargo fuzz run workbook_table_ingest` ❌ Wave 0 (new target) |
| SC2 | one tool per output Table; DAG-derived inputs; output schema | unit + integration | `cargo test -p pmcp-server-toolkit multi_tool`; integration: `tools/list` returns N tools w/ I/O schemas ❌ Wave 0 |
| SC2 | reachability correctness (constant-only excluded; union for shared) | property | proptest over random DAGs: derived leaves ⊆ inputs ∧ minimal ❌ Wave 0 |
| SC3 | broken workbook → cell-precise lint | unit | `cargo test -p pmcp-workbook-compiler row_lint` (assert finding code + sheet!addr) — reshape existing `lib.rs:1037,1119` tests |
| SC3 | `workbook explain` previews surface | integration + example | `cargo run -p cargo-pmcp -- workbook explain template.xlsx`; snapshot the text render ❌ Wave 0 |
| SC4 | template is provenance-valid + doubles as fixture | unit | assert `classify(template) == ExcelTrusted`; compile the template green (reuse `fixture_author` provenance assertion `fixture_author.rs:57`) ❌ Wave 0 |
| SC5 | F2 retained; F1/F3-input retired | unit | KEEP `schema.rs` F2 tests (533-608); DELETE `strip_governance_prefix`/`json_key_does_not_strip_prefix_only_name` tests (manifest_model.rs:820) |
| SC5 | purity + PMAT + quality green | gate | `make quality-gate && make purity-check` (CI: `pmat quality-gate --fail-on-violation --checks complexity`) |
| all | per-feature demonstration | example | `cargo run --example workbook_table_authoring` (NEW) ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p <touched crate> --lib`
- **Per wave merge:** full `cargo test` across the 3 workbook crates + cargo-pmcp
- **Phase gate:** `make quality-gate && make purity-check` green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `fuzz/fuzz_targets/workbook_table_ingest.rs` — malformed table XML → clean error (Pitfall 2)
- [ ] property test harness for `Dag::upstream_input_leaves` (random DAG generator)
- [ ] integration test asserting `tools/list` returns one tool per output Table
- [ ] `examples/workbook_table_authoring.rs` — author template → compile → list tools
- [ ] snapshot fixture for `workbook explain` text output
- [ ] extend `fixture_author.rs` with Table-authoring (or a promoted generator) for `template.xlsx`

## Security Domain

> `security_enforcement` not set in config → treated as enabled. Scoped to this phase's surface.

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | Workbook servers are stateless compute; auth is the host/transport's concern (out of scope) |
| V3 Session Management | no | Stateless |
| V4 Access Control | no (this phase) | Strict-constant governance (tier) is a *business* gate, not auth |
| V5 Input Validation | **yes** | Per-tool `inputSchema` (`additionalProperties:false`) + runtime DTO `deny_unknown_fields` mirror (`schema.rs:312-329`, `input.rs`). Each new per-tool schema MUST keep the strict envelope. |
| V6 Cryptography | no | Provenance uses sha256 content hashing (existing, untouched) — not new crypto |

### Known Threat Patterns
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Malformed `.xlsx` (zip-bomb / billion-laughs / malformed table XML) | Denial of Service | Existing `MAX_CELL_COUNT` DoS guard (`ingest/mod.rs:388`); quick-xml entity-expansion disabled (`raw_parts.rs:29`); NEW fuzz target on table XML (Pitfall 2) |
| Caller pins a computed output via override | Tampering | `is_computed` reject gate retained (`manifest_model.rs:215`); per-tool schema must not advertise computed cells as inputs |
| Strict (BA-governed) constant supplied as input | Tampering | `is_strict_constant` reject (`manifest_model.rs:205`) + `strict_constant_override` error (`error.rs:143`) — preserved by the tier column |
| umya reader leaking into served binary | Elevation (boundary breach) | `make purity-check` cargo-tree ban (`Makefile:506+`) — must stay green |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `Role::from_name_prefix` / `const_` handling may have a non-named-range consumer (Guide/overlap check) worth keeping | Cleanup map | LOW — plan must grep its call sites before deleting; could leave a dead fn or remove a still-used one |
| A2 | Committing a generated `template.xlsx` (option b) is acceptable vs. promoting the author to a build-time generator (option a) | Pitfall 4 | LOW — both are viable; team may prefer (a) for drift-resistance |
| A3 | `0_meta` minimal key set `{server,version}` is sufficient for v1 | OQ-1 | MEDIUM — if governance defaults are wanted at meta level, the set grows; needs user confirmation |
| A4 | DAG-derived-only (no explicit override) covers all v1 cases | OQ-2 | MEDIUM — if a real "widen the schema" case exists, the override seam is needed sooner |
| A5 | Existing reconcile stage can be re-keyed per-tool without a rewrite | WBV2-05 | MEDIUM — `comparison_from_outputs` (`lib.rs:567`) assumes one output set; per-tool partition may need restructuring |
| A6 | No persisted datastore keys on `in_*`/`out_*` (Runtime State Inventory all-None except fixtures) | Runtime State | LOW — verified servers are stateless; confirm no downstream pmcp.run cache keys on tool names |

## Open Questions

1. **Does any consumer still need `Role::from_name_prefix`/`const_` after the input/output convention retires?**
   - What we know: it maps `in_/const_/out_` → Role; used by the D-04 overlap/Guide redundancy check.
   - What's unclear: whether `const_` (governed constant) still uses named ranges under the table model, or whether constants also move to a table column.
   - Recommendation: grep call sites in the plan; if `const_` constants stay named-range-based, KEEP only that branch; else retire the whole fn.

2. **Per-tool reconcile partitioning shape.**
   - What we know: reconcile grades named-output vs cached oracle today over one output set.
   - What's unclear: cleanest way to partition the run's `computed` map per tool (by output-Table membership).
   - Recommendation: each `Tool.outputs[].cell` defines its partition; reuse the existing comparator per partition.

3. **Where does `template.xlsx` live as a shipped artifact** (CLI templates dir vs. compiler tests vs. both)?
   - Recommendation: commit under `cargo-pmcp/src/templates/workbook_bundle/` (replacing the misleading `tax-calc.xlsx`) AND symlink/copy into `tests/fixtures/` as the reference fixture — one source, two consumers (§7's "one artifact, three jobs").

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` / Rust stable | all | ✓ (assumed) | per `rustup` | — |
| `umya-spreadsheet` | compiler ingest | ✓ | 3.0.0 (path-pinned) | none (required) |
| `rust_xlsxwriter` | template author | ✓ | 0.95 | none (only provenance-clean writer) |
| `pmat` | CI complexity gate | CI-only (per CLAUDE.md D-07) | 3.15.0 | local: `pmat analyze complexity` |
| `make` + `just` | quality/purity gates | ✓ | — | — |

No missing dependencies — all are workspace-resolved.

## Sources

### Primary (HIGH confidence)
- `docs/design/workbook-table-authoring-contract.md` — the locked design contract (§1–§11)
- Codebase (working tree, grep-verified file:line throughout) — compiler/runtime/toolkit/CLI crates
- `~/.cargo/.../umya-spreadsheet-3.0.0/src/structs/table.rs:65,104,138` + `table_column.rs:453` — Table API reachability VERIFIED
- `~/.cargo/.../rust_xlsxwriter-0.95.0/src/table.rs` + `worksheet.rs:8968,9388` — `add_table`/`add_data_validation` VERIFIED
- `.planning/REQUIREMENTS.md` (v2.3 milestone, WBRT/WBDL/WBCO requirements)
- `.planning/ROADMAP.md:1331-1356` — Phase 100 goal + Success Criteria
- `CLAUDE.md` — Toyota Way quality gates, ALWAYS-requirements, PMAT cog≤25

### Secondary (MEDIUM confidence)
- `Makefile:500-560` — `purity-check` recipe (cargo-tree ban list)
- `crates/.../75-RESEARCH.md` (referenced by CLAUDE.md for P1–P6 complexity-reduction techniques)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new deps; both writer/reader APIs verified against vendored crate source
- Architecture / cleanup map: HIGH — every symbol located at file:line in the working tree
- Open-question recommendations: MEDIUM — design-discretionary defaults (A3/A4/A5 flagged)
- Reconcile per-tool partitioning: MEDIUM — mechanism exists but partition restructuring is unverified at code level

**Research date:** 2026-06-20
**Valid until:** 2026-07-20 (stable subsystem; the design contract is locked)
