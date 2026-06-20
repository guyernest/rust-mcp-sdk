---
phase: 100-excel-workbook-built-in-servers-v2
reviewed: 2026-06-20T00:00:00Z
depth: standard
files_reviewed: 28
files_reviewed_list:
  - cargo-pmcp/src/commands/workbook/explain.rs
  - cargo-pmcp/src/commands/workbook/explain_surface.rs
  - cargo-pmcp/src/commands/workbook/mod.rs
  - crates/pmcp-server-toolkit/src/workbook/error.rs
  - crates/pmcp-server-toolkit/src/workbook/handler.rs
  - crates/pmcp-server-toolkit/src/workbook/input.rs
  - crates/pmcp-server-toolkit/src/workbook/mod.rs
  - crates/pmcp-server-toolkit/src/workbook/schema.rs
  - crates/pmcp-workbook-compiler/src/artifact/cell_map.rs
  - crates/pmcp-workbook-compiler/src/artifact/layout.rs
  - crates/pmcp-workbook-compiler/src/artifact/mod.rs
  - crates/pmcp-workbook-compiler/src/fixture_author.rs
  - crates/pmcp-workbook-compiler/src/ingest/cell_map.rs
  - crates/pmcp-workbook-compiler/src/ingest/mod.rs
  - crates/pmcp-workbook-compiler/src/lib.rs
  - crates/pmcp-workbook-compiler/src/manifest/mod.rs
  - crates/pmcp-workbook-compiler/src/manifest/synth.rs
  - crates/pmcp-workbook-compiler/src/provenance/gate.rs
  - crates/pmcp-workbook-compiler/src/provenance/mod.rs
  - crates/pmcp-workbook-compiler/src/provenance/region_hash.rs
  - crates/pmcp-workbook-compiler/src/reemit_golden.rs
  - crates/pmcp-workbook-compiler/src/stage1.rs
  - crates/pmcp-workbook-runtime/src/artifact_model.rs
  - crates/pmcp-workbook-runtime/src/bundle_loader.rs
  - crates/pmcp-workbook-runtime/src/dag.rs
  - crates/pmcp-workbook-runtime/src/lib.rs
  - crates/pmcp-workbook-runtime/src/manifest_model.rs
findings:
  critical: 2
  warning: 6
  info: 4
  total: 12
status: issues_found
---

# Phase 100: Code Review Report

**Reviewed:** 2026-06-20T00:00:00Z
**Depth:** standard
**Files Reviewed:** 28 (3 not in the explicit list — `fixture_author.rs` and `provenance/mod.rs` were skimmed; the remainder read in full)
**Status:** issues_found

## Summary

This is the Phase 100 table-based workbook authoring feature: ingest Excel Tables → harvest input/output structure → build a DAG → fan out one MCP tool per output Table with DAG-derived per-tool input schemas. The umya purity boundary, the `catch_unwind` containment seam, the fail-closed bundle loader, the input validation gates, and the region-hash provenance machinery are all well-engineered, panic-conscious, and heavily tested. No umya/calamine reader type leaks across the runtime boundary — the purity invariant holds in the code I read.

However, the **headline multi-tool fan-out is not actually wired into the production compile pipeline.** `build_tools` / `tool_name_collision_findings` / `reconcile_tools` — the entire WBV2-03/04 per-Table fan-out plus its collision lint — are called ONLY from `#[cfg(test)]` code. The production `compile_workbook` → `emit_bundle` path calls `build_cell_map`, which emits a SINGLE workflow-named tool wrapping all outputs with an EMPTY `input_keys`. Two correctness consequences follow (CR-01, CR-02). The committed golden fixture was regenerated into the two-Table shape out-of-band, and the producer/consumer proof only asserts SUBSET relations, so this gap is invisible to the test suite.

Secondary concerns: the `explain` preview's A1-reference walker silently treats cross-sheet references as same-sheet (WR-01), and a fifth undocumented error code leaked into the "four stable codes" table.

## Critical Issues

### CR-01: Multi-tool fan-out + collision lint are dead on the production compile path

**File:** `crates/pmcp-workbook-compiler/src/artifact/mod.rs:180`, `crates/pmcp-workbook-compiler/src/artifact/cell_map.rs:86,321,347`
**Issue:** `emit_bundle` (the only production emit path, reached via `compile_workbook_inner` → `gate::accept::promote`) builds its `cell_map.json` via `build_cell_map(&ratified)`:

```rust
// artifact/mod.rs:180
let cell_map = build_cell_map(&ratified).map_err(EmitError::CellMap)?;
```

`build_cell_map` (cell_map.rs:460) is documented as "the TRANSITIONAL single-tool path": it wraps EVERY `Role::Output` cell in ONE `Tool` named `manifest.workflow`, with `input_keys: Vec::new()` (cell_map.rs:485-491). The per-Table primitives that implement the actual Phase 100 feature —

- `build_tools` (cell_map.rs:86) — DAG-derived per-Table `input_keys`,
- `tool_name_collision_findings` (cell_map.rs:347) — the T-100-17 post-sanitize collision lint,
- `reconcile_tools` (cell_map.rs:321) — per-tool oracle grading,

are reachable ONLY from `#[cfg(test)]` callers (verified by grep: every non-test reference is the `pub fn` definition itself or the re-export shim). Therefore:
1. A real `cargo pmcp workbook compile` NEVER produces the multi-tool fan-out described throughout the docs/handlers — it always emits one workflow-named tool.
2. The tool-name **collision lint never runs at compile time**. Two output Tables whose names sanitize to the same MCP name (e.g. `Calculate Tax` and `calculate_tax`) are not caught by the compiler; `mod.rs:252-263` then registers both under the same `tool_arc` name at served boot — a silent last-writer-wins, exactly the failure `tool_name_collision_findings` was written to prevent.

The producer/consumer proof (`reemit_golden.rs`) hides this: it asserts only `is_subset` relations (lines 138-145) and its own comment (lines 82-89, 107-111) admits "the committed served golden was REGENERATED (Plan 04) into the two-Table shape" — i.e. the golden was produced by something other than `compile_workbook`. The golden `cell_map.json` carries two tools with populated `input_keys`; a fresh compile of the same fixture would carry one tool with empty `input_keys`.

**Fix:** Wire the multi-tool path into `emit_bundle`. The compiler already harvests `TableRecord`s (areas + names + columns) in ingest, so it can construct `OutputTable` membership and call `build_tools` + `tool_name_collision_findings` (folding the collision findings into the stage-1 Error gate) instead of `build_cell_map`. Concretely:

```rust
// in emit_bundle / the driver, before emit:
let output_tables = output_tables_from_harvest(&map, &manifest); // group Role::Output by Table area
let collisions = tool_name_collision_findings(&output_tables);
if collisions.iter().any(|f| f.severity == Severity::Error) {
    return Err(CompileError::Lint(stage1::render_aggregate(&collisions.iter().collect::<Vec<_>>())));
}
let (tools, lints) = build_tools(&manifest, &dag, &output_tables)
    .map_err(EmitError::CellMap)?;
let cell_map = CellMap { inputs, tools };
```

If single-tool emission is genuinely intended for this phase, then the docs, handler comments, and `reemit_golden` golden must be corrected to stop advertising a fan-out that does not happen.

### CR-02: Served per-tool input schema is empty for a production-compiled bundle, but the runtime accepts inputs anyway (schema/runtime divergence)

**File:** `crates/pmcp-server-toolkit/src/workbook/schema.rs:336-348`, `crates/pmcp-server-toolkit/src/workbook/input.rs:142-167`
**Issue:** This is the direct served-side consequence of CR-01. `input_schema_for_tool` projects only the inputs whose `json_key` is in `tool.input_keys`:

```rust
// schema.rs:338-346
for entry in &cell_map.inputs {
    if tool.input_keys.iter().any(|k| k == &entry.json_key) {
        input_props.insert(entry.json_key.clone(), input_prop_for_entry(manifest, entry));
    }
}
```

For a bundle emitted by `build_cell_map`, every tool has `input_keys: Vec::new()`, so this loop inserts NOTHING — the served tool advertises an input schema with an empty `inputs.properties` and `additionalProperties:false`. But the runtime gate `validate_input` → `seed_supplied_inputs` (input.rs:148-165) validates against the FULL `cell_map.inputs` pool, accepting any known input key. A client that trusts the advertised (empty) schema would believe the tool takes no inputs, while the runtime silently accepts and seeds them — the exact "client trusting the schema never sends a key the runtime then rejects" invariant the schema module claims to uphold (schema.rs:8-10), inverted: the schema is now STRICTER than the runtime, hiding every real input from discovery.

The module-doc and the V5 invariant ("a client trusting the advertised schema must never be able to send a key the runtime then rejects", schema.rs:332-334) are only satisfied when `input_keys` is correctly populated — which only happens via `build_tools` (CR-01). The handler/schema unit tests pass because they use the hand-authored golden whose `input_keys` are populated; they never exercise a `build_cell_map`-produced tool.

**Fix:** Resolving CR-01 (populate `input_keys` via `build_tools`) fixes this. As defense-in-depth, `input_schema_for_tool` should fall back to the full input pool when `tool.input_keys` is empty (treating "no DAG derivation" as "all shared inputs"), so a single-tool bundle still advertises its inputs:

```rust
let project_all = tool.input_keys.is_empty();
for entry in &cell_map.inputs {
    if project_all || tool.input_keys.iter().any(|k| k == &entry.json_key) {
        input_props.insert(entry.json_key.clone(), input_prop_for_entry(manifest, entry));
    }
}
```

## Warnings

### WR-01: `extract_a1_refs` treats cross-sheet references as same-sheet, corrupting the explain DAG derivation

**File:** `cargo-pmcp/src/commands/workbook/explain_surface.rs:397-425` (with `reachable_addrs` at 376-392)
**Issue:** `extract_a1_refs` is documented (line 396) to recognize "single-cell same-sheet references" and ignore "cross-sheet refs". It does NOT. For a formula `1_Inputs!B5`, the scanner skips the leading digit, consumes `Inputs` (no trailing digits → not a ref), hits `!`, then consumes `B5` and emits it as a bare `"B5"`. The `!` only guards the token BEFORE it (via `trailing_ident`), never the token AFTER it. So a cross-sheet reference contributes the foreign cell's local address as if it were on the current sheet. `reachable_addrs` then walks `cell_formula(sheet, "B5")` on the WRONG sheet — pulling in an unrelated same-named cell's formula or missing the real upstream input. The DAG-derived per-tool input set in the `explain` preview can therefore be wrong (over- or under-reporting inputs) for any workbook that uses cross-sheet formula references — which the table model explicitly supports (Inputs Table on `1_Inputs`, outputs on another sheet). The existing tests only cover single-sheet formulas (`ROUND(B4*G3-1759,0)`, `B11/B4`), so this is untested.

**Fix:** Detect and skip the sheet-qualified address as a unit, or capture cross-sheet refs as `sheet!addr` keys and resolve them against the right sheet. Minimal fix — when the char immediately following the consumed digits is `!`, the token was a sheet name, not a ref; and when the char immediately PRECEDING the captured letters is `!`, the captured `addr` belongs to the qualifying sheet, not the current one. Track the qualifier and route `reachable_addrs` accordingly (or drop cross-sheet refs entirely if same-sheet is truly the only supported shape, but then the preview must not claim to derive inputs for a multi-sheet workbook).

### WR-02: No served-boot guard against tool-name collisions or duplicate registrations

**File:** `crates/pmcp-server-toolkit/src/workbook/mod.rs:251-263`
**Issue:** The boot loop sanitizes each tool name and registers via `builder.tool_arc(&name, ...)`. If two tools in `cell_map.tools` sanitize to the same MCP name, the second silently overwrites the first (or duplicate-registers, depending on `tool_arc` semantics) — no error, no warning. The compiler-side `tool_name_collision_findings` is the intended guard, but it never runs in production (CR-01), so the served boot is the last line of defense and it has none. A tampered or hand-built bundle with colliding tool names boots a server that silently drops a tool.

**Fix:** In the registration loop, track the set of already-registered sanitized names and fail closed (`ToolkitError::Synth`) on a duplicate, mirroring the unmappable-name reject already present at line 253-258.

### WR-03: `is_date_format` misclassifies any number format containing `d` as a date unit

**File:** `crates/pmcp-workbook-compiler/src/manifest/synth.rs:560-565`
**Issue:** `is_date_format` returns true if the lowercased format code `contains('y') || contains('d')`. Excel number-format codes legitimately contain `d` in non-date contexts — e.g. the literal text token in `0" units"` (no), but more realistically a format with an embedded color/condition or a custom code like `#,##0" usd"` lowercases to contain `d` (in "usd"). `number_format_to_unit` checks currency BEFORE date, so `"usd"` is caught as USD first; but a format like `0" std"` or any code carrying a stray `d`/`y` outside a date context falls through to `Some("date")`. The codomain is "closed" but the classification is loose enough to stamp a spurious `date` unit on a numeric input. The unit then rides into the served schema and the agent-facing surface.

**Fix:** Match against actual Excel date placeholder runs (`dd`, `mm`, `yy`, `yyyy`, month/day tokens in sequence) rather than a bare substring `contains`, or require at least two distinct date tokens. At minimum, screen out formats that contain a quoted literal section before testing for `d`/`y`.

### WR-04: `references_external_workbook` indexes `formula[i + 1..]` without a UTF-8 boundary guarantee

**File:** `crates/pmcp-workbook-compiler/src/ingest/mod.rs:83-98`
**Issue:** The loop iterates `bytes` (the raw byte slice) and, on finding a `[`, slices `&formula[i + 1..]` and `&formula[i + 1 .. i + 1 + close_rel]`. `i` is a BYTE index from `bytes.iter().enumerate()`. If a multi-byte UTF-8 character precedes a `[`, `i + 1` could land in the middle of... no — `[` is single-byte ASCII so `i` is on the `[` byte and `i+1` is a valid boundary. But `close_rel` comes from `formula[i+1..].find(']')` which is a byte offset into the str slice, so `i + 1 + close_rel` is the `]` byte — also valid. The slicing is actually boundary-safe because `[` and `]` are ASCII. This is NOT a panic, but it is fragile: the code mixes byte-index iteration with str slicing and relies on the ASCII-ness of the bracket bytes for soundness, with no comment asserting it. A future edit that searches for a non-ASCII delimiter the same way would panic on a char-boundary.

**Fix:** Add an assertion/comment documenting the ASCII-bracket invariant, or refactor to iterate `char_indices()` so the boundary safety is structural rather than incidental.

### WR-05: `infer_dtype` / `harvest_dtype` parse with `f64::parse`, accepting `inf`/`nan`/`1e9` as `Number`

**File:** `crates/pmcp-workbook-compiler/src/manifest/synth.rs:462-470, 515-520`
**Issue:** `harvest_dtype`/`infer_dtype` classify a cell value as `Dtype::Number` when `v.trim().parse::<f64>().is_ok()`. Rust's `f64::from_str` accepts `"inf"`, `"infinity"`, `"NaN"` (case-insensitive), and `"-inf"`. A workbook cell whose text is literally `inf` or `nan` would be typed `Number` and lowered by `row_default` to `CellValue::Number(f64::INFINITY)` / `NaN` as a tier default. The served `finite_output_value` (handler.rs:94-111) rejects non-finite OUTPUTS, but a non-finite INPUT default seeded into the executor is not similarly screened at harvest time — it depends on downstream finiteness checks to catch it. This is a latent path to a NaN/Inf seed.

**Fix:** After `parse::<f64>()`, require `n.is_finite()` before classifying as `Number` (and before lowering to a `CellValue::Number` default), so `inf`/`nan` cell text falls through to `Text`.

### WR-06: `metadata()` raw-name fallback can emit an MCP-charset-illegal tool name

**File:** `crates/pmcp-server-toolkit/src/workbook/handler.rs:220-236`
**Issue:** `WorkbookToolHandler::metadata()` falls back to `self.tool.name` (the RAW, unsanitized Table name) when `registered_name()` errors, with the comment "registration is the fail-closed gate." That holds for the `with_workbook_bundle` path (mod.rs:253 rejects unmappable names before registration). But `WorkbookToolHandler::new` is `pub` and `metadata()` is a public trait impl — a consumer constructing a handler directly (or a future registration path) can reach `metadata()` without the boot-time reject, yielding a `ToolInfo` whose `name` violates `^[a-zA-Z0-9_-]{1,64}$`. The "infallible metadata" convenience trades a hard reject for a silently-malformed advertised name.

**Fix:** Either make the raw-name fallback also sanitize-or-redact to a charset-safe placeholder, or have `metadata()` return `None` when the name is unmappable (the tool should not advertise an uncallable name). The boot path already guarantees the name is mappable, so returning `None` here is safe in the wired path and fail-safe in the unwired one.

## Info

### IN-01: Fifth error code `invalid_tool_name` is not in the documented "four stable codes" table

**File:** `crates/pmcp-server-toolkit/src/workbook/error.rs:22-38, 161-179`
**Issue:** The module doc enumerates "The four codes" in a table (`invalid_input`, `missing_field`, `unsupported_option`, `strict_constant_override`), and the struct doc (line 54-56) says `code` is "one of the four stable machine-readable strings." But `unmappable_tool_name` constructs `code: "invalid_tool_name"` — a fifth code absent from the table and the contract. A widget reading the documented table would not recognize it.
**Fix:** Add `invalid_tool_name` to the self-repair table (and its UI meaning), and update the "four codes" prose to "five."

### IN-02: `bundle_loader` doc says "builds the per-cell DAG ONCE" but the runtime never re-checks it for cycles

**File:** `crates/pmcp-workbook-runtime/src/bundle_loader.rs:390-391`, `crates/pmcp-server-toolkit/src/workbook/handler.rs:46-61`
**Issue:** `load()` calls `build_dag(&members.ir)` with no cycle check; `run_bundle` (handler.rs) comments "A DAG cycle (impossible for a conforming bundle) surfaces as an invalid_input error rather than a panic" and relies on `run_executor` to detect it. The loader does not run `toposort` to reject a cyclic IR at boot. A tampered bundle with a cyclic IR passes the integrity/stamp gates (the hashes are self-consistent) and only fails at first tool invocation. That is acceptable (fail-closed at runtime, not a panic), but boot-time rejection would be stronger fail-closed behavior and is cheap given the DAG is already built.
**Fix (optional):** Run `toposort(&dag)` in `load()` and return a `BundleLoadError::Parse`-style cycle error if it fails, so a cyclic bundle never boots.

### IN-03: `oracle_value` doc comment is self-contradictory and the function is misleading for outputs

**File:** `crates/pmcp-workbook-compiler/src/artifact/cell_map.rs:416-433`
**Issue:** The doc comment is garbled ("Currently sourced from the `InputTier::Variable` default the harvest stamps on output rows is `None`, so this reads the cell's value via the manifest when present"). The function actually reads `role.tier` — but `harvest_output_row` always sets `tier: None` for outputs (synth.rs:629-643), so for any real harvested output this returns `None` and the per-tool oracle is empty. The tests only pass because they synthesize outputs with a `Variable` tier as a stand-in (cell_map.rs:668-672). The real oracle wiring is deferred ("Plan 04 wires the cached `<v>`") but the code reads as if it works.
**Fix:** Rewrite the doc to state plainly that output oracles are currently always `None` until the cached-`<v>` wiring lands, and consider returning `None` explicitly with a `// TODO(plan-04)` rather than reading a tier that is structurally never set.

### IN-04: `extract_a1_refs` / `reachable_addrs` have no cycle guard for a self-referential formula

**File:** `cargo-pmcp/src/commands/workbook/explain_surface.rs:376-392`
**Issue:** `reachable_addrs` uses a `reached` set as a visited-guard, so it terminates on cycles — good. But it pushes a ref onto the stack only `if !reached.contains(&r)` (line 387), then re-checks `reached.insert` at pop (line 380); a formula referencing its own cell (`=A1+1` at A1) is handled. This is correct, just worth noting the preview path is cycle-safe by construction (unlike the served path which delegates to `run_executor`). No fix needed — recorded for completeness since the prompt flagged DAG cycle handling.

---

_Reviewed: 2026-06-20T00:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
