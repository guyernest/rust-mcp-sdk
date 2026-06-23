# Phase 100: Workbook Accuracy-Verification Surface - Pattern Map

**Mapped:** 2026-06-22
**Files analyzed:** 11 (9 modify, 1 create, 1 fixture-bundle)
**Analogs found:** 11 / 11 (all in-tree; this phase is ADDITIVE — every change extends an existing pattern)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/pmcp-workbook-runtime/src/render/mod.rs` (Item 1: text/bool formula) | utility (writer) | transform | `write_number_cell` (same file, lines 414-443) | exact (same file, sibling fn) |
| `crates/pmcp-workbook-runtime/src/render/mod.rs` (Item 2: `RenderMode` param) | utility (writer) | transform | `render_xlsx` (same file, line 227) | exact (extend signature) |
| `crates/pmcp-workbook-runtime/src/reconcile.rs` (NEW) | service (pure diff) | batch/transform | `executor::run` (executor.rs:92) + `project_tool_outputs` (handler.rs:70) | role-match (compose 2 existing) |
| `crates/pmcp-workbook-runtime/src/artifact_model.rs` (consume `Tool.oracle`/`seed_coord`) | model | — | `Tool`/`CellEntry` (same file, lines 33-73) | exact (read-only consumer) |
| `crates/pmcp-workbook-runtime/src/lib.rs` (re-exports) | config (barrel) | — | existing `pub use render::{...}` (lib.rs:104) | exact |
| `crates/pmcp-server-toolkit/src/workbook/render_uri.rs` (`mode` field) | utility (codec) | request-response | `DecodedRender`/`RenderPayload`/`RenderPayloadRef` (same file, 74-105) | exact (add field) |
| `crates/pmcp-server-toolkit/src/workbook/render_resource.rs` (thread `mode`) | service | request-response | `regenerate` (same file, line 85) | exact (extend call) |
| `crates/pmcp-server-toolkit/src/workbook/handler.rs` (`mode` parse + `VerifyAccuracyHandler`) | controller (tool handler) | request-response | `RenderWorkbookHandler` (handler.rs:582) + `ExplainHandler` (handler.rs:257) | exact |
| `crates/pmcp-server-toolkit/src/workbook/schema.rs` (`mode` prop + verify output schema) | config (schema) | — | `assemble_input_schema` (schema.rs:392) + `render_workbook_output_schema` | exact |
| `crates/pmcp-server-toolkit/src/workbook/mod.rs` (register 6th tool + count) | config (registration) | — | meta-tool block (mod.rs:265-286) | exact |
| `crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/*` (text+bool output, D-07) | test (fixture) | file-I/O | `build_bundle_lock`/`fold_evidence_hash` (artifact_model.rs:149,168) | role-match |

## Pattern Assignments

### `render/mod.rs` Item 1 — text/bool formula-with-cached-result (WBVER-01)

**Analog:** `write_number_cell`, same file, lines 414-443. This is THE template: it already
branches on `(cell.formula, fmt)` and writes `Formula::new(...).set_result(...)` for the
formula arms.

**The 4-arm formula-or-literal template to generalize** (lines 422-441):
```rust
match (&cell.formula, fmt) {
    (Some(f), Some(fmt)) => {
        let formula = Formula::new(normalize_formula_for_writer(f)).set_result(format_number(n));
        ws.write_formula_with_format(row, col, formula, fmt).map_err(writer_err)?;
    },
    (Some(f), None) => {
        let formula = Formula::new(normalize_formula_for_writer(f)).set_result(format_number(n));
        ws.write_formula(row, col, formula).map_err(writer_err)?;
    },
    (None, Some(fmt)) => { ws.write_number_with_format(row, col, n, fmt).map_err(writer_err)?; },
    (None, None) => { ws.write_number(row, col, n).map_err(writer_err)?; },
}
```

**The formula-BLIND arms Item 1 must fix** (lines 389-390 — `write_computed_value`):
```rust
Some(CellValue::Text(s)) => write_string_cell(ws, row, col, s, fmt)?,                 // ignores cell.formula!
Some(CellValue::Bool(b)) => write_string_cell(ws, row, col, &b.to_string(), fmt)?,    // ignores cell.formula!
```

**Pattern to copy:** Factor a `write_formula_or_value` helper (design §4.2, Claude's discretion)
that takes the cached-result string + `cell.formula` + `fmt` + `RenderMode` and mirrors the
4-arm match above. `set_result` takes `impl Into<String>`, so:
- Number → `set_result(format_number(n))` (unchanged, already proven)
- Text → `set_result(s)` (the string verbatim)
- Bool → `set_result(if b { "TRUE" } else { "FALSE" })`

**Complexity guard (Pitfall 3):** keep `write_computed_value` a thin dispatcher; push the
formula/format/mode combinatorics into the ONE shared helper — exactly how `write_number_cell`
and `write_string_cell` are already split out (lines 414, 446). PMAT cog-25 is enforced in CI.

**Determinism / non-finite guards UNCHANGED:** the `n.is_finite()` check (lines 384-386) and the
`_ => cell.value` fallback (lines 394-398) stay as-is.

---

### `render/mod.rs` Item 2 — `RenderMode { Filled, InputsOnly }` + `render_xlsx(mode)` (WBVER-02)

**Analog:** `render_xlsx` signature + threading, same file, line 227:
```rust
pub fn render_xlsx(layout: &LayoutDescriptor, run: &RunResult) -> Result<Vec<u8>, RenderError> {
    let mut wb = init_workbook()?;
    for sheet in &layout.sheets {
        let ws = wb.add_worksheet();
        render_sheet(ws, sheet, run)?;     // ← mode threads down through here
    }
    wb.save_to_buffer().map_err(writer_err)
}
```

**Pattern to copy:** Add `mode: RenderMode` as the 3rd param; thread it `render_sheet` →
`write_cell` → `write_computed_value` → the new `write_formula_or_value` helper, mirroring how
`run: &RunResult` is already threaded through all four. `InputsOnly` = the same `Formula::new(...)`
WITHOUT `.set_result(...)` (a bare formula); non-formula cells write their value unchanged in BOTH
modes (so input-cell seeding falls out for free — the fixture's input cells have `"formula": null`,
research-verified).

**Enum location (Claude's discretion, research recommends here):** define `RenderMode` in
`render/mod.rs` beside `LayoutDescriptor` and re-export at the crate root, mirroring the existing
`pub use render::{CellLayout, LayoutDescriptor, SheetLayout, ...}` (lib.rs:104). It crosses the
runtime→toolkit boundary the same way `LayoutDescriptor` does.

**Determinism note (Pitfall 3 / design §4.3):** `init_workbook` already pins doc properties to a
FIXED datetime (lines 239-245), so each mode stays byte-stable. The two modes differ by design —
property tests assert PER-MODE determinism, NEVER cross-mode equality.

---

### `reconcile.rs` (NEW) — `reconcile_reference(...) -> ReconcileReport` (WBVER-03)

**Analog A (re-run the engine):** `executor::run` (executor.rs:92), already the serve-time path:
```rust
pub fn run(ir: &HashMap<String, Cell>, dag: &Dag, seed: &CellEnv)
    -> Result<RunResult, Box<LintFinding>> { ... }
```

**Analog B (project + look up server values):** `project_tool_outputs` (handler.rs:70-89) — the
EXACT lookup `reconcile_reference` mirrors per output, and the same `entry.seed_coord` is D-01's
`cell` A1 address:
```rust
for entry in &tool.outputs {
    let Some(value) = run.computed.get(&entry.seed_coord) else {
        return Err(WorkbookToolError::invalid_input(/* fail closed */));
    };
    let projected = finite_output_value(value, &entry.seed_coord, &entry.json_key)?;
    // ...
}
```

**Analog C (reference inputs = tier defaults):** `seed_tier_defaults(manifest)` (input.rs:123) —
VERIFIED the oracle was computed at exactly these defaults. Seed identically:
```rust
fn seed_tier_defaults(manifest: &Manifest) -> BTreeMap<String, Value> {
    let mut seeds = BTreeMap::new();
    for role in &manifest.cells {
        if matches!(role.role, Role::Input) {
            if let Some(default) = tier_default(role) { seeds.insert(role.cell.clone(), default); }
        }
    }
    seeds
}
```
(`reconcile.rs` lives in the runtime, so seed the `CellEnv` directly — see `run_bundle` at
handler.rs:54-57 for the `env.with_value(key, value)` loop the seeding mirrors.)

**Analog D (`ReconcileReport` DTO derives):** `RunResult` (executor.rs:70) and `Tool`
(artifact_model.rs:54) — copy the derive set:
```rust
#[derive(Debug, Clone, Default, Serialize, schemars::JsonSchema)]  // RunResult — for outputSchema feed
// OR (carries f64 → drop Eq, like Tool):
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
```
Use `schemars::JsonSchema` so `verify_accuracy`'s `outputSchema` is derivable, matching `RunResult`.

**Oracle source of truth (model, read-only):** `Tool.oracle: BTreeMap<String, CellValue>`
(artifact_model.rs:72) keyed by output json_key; `Tool.outputs: Vec<CellEntry>` (line 68) where each
`CellEntry { json_key, seed_coord, unit }` (lines 33-40) gives both the server-value lookup key and
the D-01 `cell` address.

**D-01/D-02 cell field:** `cell = Some(entry.seed_coord.clone())` (already sheet-qualified A1, e.g.
`"3_Outputs!B3"`). Type stays `Option<String>`; `None` only when an `oracle` key has no matching
`outputs` entry (malformed bundle) — row still reports deltas. No layout walk needed.

**D-03/D-04 (panic-free, `?`/`get`):** mirror handler.rs's `Result` discipline — unknown tool
filter → `Err` listing tool names (NOT panic, NOT empty); empty oracle → `outputs: []`,
`all_within_tol = true`, contributes 0 to `cells_checked`.

**Negative-path test mandate (Pitfall 5):** a conforming bundle always passes, so ADD a
perturbed-oracle unit test (construct a `Tool` with a wrong `oracle` value, assert
`within_tol == false`). Test the diff, not just the golden.

---

### `render_uri.rs` — `mode` in the URI payload (WBVER-02)

**Analog:** `DecodedRender` / `RenderPayload` / `RenderPayloadRef`, same file, lines 74-105.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedRender { pub dto: Value, pub provenance: ProvStamp }   // + pub mode: RenderMode

#[derive(Debug, Deserialize)]
struct RenderPayload { dto: Value, provenance: ProvStamp }              // + #[serde(default)] mode

#[derive(Serialize)]
struct RenderPayloadRef<'a> { dto: &'a Value, provenance: &'a ProvStamp }  // + mode: &RenderMode
```

**Pattern to copy:** add a `mode` field to all three. CRITICAL back-compat (Pitfall 1): the DECODE
side `RenderPayload.mode` MUST be `#[serde(default)]` (absent → `Filled`) so URIs minted before this
phase still decode. Keep `RenderPayloadRef` field ORDER stable so `encode_is_deterministic`
(render_uri.rs:205) stays byte-identical.

**Size guard UNCHANGED:** `MAX_ENCODED_URI_LEN` = 64 KiB (line 66), checked FIRST in `decode`
(line 146). `mode` adds ~32 base64 bytes — ~0.05% of cap, no risk.

**Proptests to extend** (lines 248-288): `prop_encode_decode_identity` (add a `mode`-carrying
round-trip per mode) + keep `prop_decode_total` (the fuzz surface — already feeds arbitrary input;
`#[serde(default)]` keeps it total). Add an assertion that a `mode`-carrying URI stays
`< MAX_ENCODED_URI_LEN`.

---

### `render_resource.rs` — thread `mode` into `render_xlsx` (WBVER-02)

**Analog:** `regenerate`, same file, lines 85-108 — the 5-step stateless regen pipeline:
```rust
let decoded = render_uri::decode(uri).map_err(...)?;   // decoded.mode now available
// provenance check (lines 90-96) UNCHANGED
let validated = validate_input(decoded.dto, ...).map_err(...)?;
let run = super::handler::run_bundle(&self.bundle, validated.seeds).map_err(...)?;
let bytes = render_xlsx(&self.bundle.layout, &run)          // ← add decoded.mode
    .map_err(|e| RegenError::Render(e.to_string()))?;
```

**Pattern to copy:** single-line change — pass `decoded.mode` as the 3rd `render_xlsx` arg. The
provenance verification (lines 90-96) and re-validation (line 98) are unchanged — `mode` is a render
parameter, not an input, so it is NOT re-validated against the manifest.

---

### `handler.rs` — `mode` parse (WBVER-02) + `VerifyAccuracyHandler` (WBVER-03)

**Item 2 analog:** `RenderWorkbookHandler::compute` (handler.rs:602-610):
```rust
fn compute(&self, args: Value) -> Result<Value, WorkbookToolError> {
    let validated = validate_input(args, &self.bundle.manifest, &self.bundle.cell_map)?;
    let uri = render_uri::encode(&validated.canonical_dto, &self.stamp)?;
    let payload = json!({ "resource_uri": uri, "mime_type": render_uri::WORKBOOK_XLSX_MIME });
    Ok(with_provenance(payload, &self.stamp))
}
```

**GOTCHA (research-verified):** `validate_input` deserializes into `CalculateInput` which is
`#[serde(deny_unknown_fields)]` (input.rs:46-58) — a `mode` key would be REJECTED. The handler must
lift `args["mode"]` out FIRST (default `"filled"`, unknown → `Err`, never panic), then pass the
remaining `{inputs, overrides}` to `validate_input`, then `render_uri::encode(dto, stamp, mode)`.

**Item 3 analog:** `ExplainHandler` (handler.rs:257-331) is the closest meta-tool shape — a
no-required-input, validate→run→project→stamp handler. Mirror its skeleton:
```rust
pub struct ExplainHandler { bundle: Arc<WorkbookBundle>, stamp: ProvStamp }
impl ExplainHandler {
    pub const NAME: &str = "explain";
    pub fn new(bundle: Arc<WorkbookBundle>) -> Self { let stamp = ProvStamp::from_bundle(&bundle); Self { bundle, stamp } }
    fn compute(&self, args: Value) -> Result<Value, WorkbookToolError> {
        let validated = validate_input(args, ...)?;
        let run = run_bundle(&self.bundle, validated.seeds)?;
        let payload = json!({ ... });
        Ok(with_provenance(payload, &self.stamp))
    }
}
#[async_trait] impl ToolHandler for ExplainHandler {
    async fn handle(&self, args, _extra) -> pmcp::Result<Value> { Ok(render_at_boundary(self.compute(args), &self.stamp)) }
    fn metadata(&self) -> Option<ToolInfo> { Some(ToolInfo::with_ui(Self::NAME, Some(...), <input_schema>, WORKBOOK_TOOL_UI).with_output_schema(<out>)) }
}
```

**`VerifyAccuracyHandler` differences:** no inputs except an optional tool-name filter (D-03 unknown
filter → `Err` listing names — use the `WorkbookToolError::invalid_input` style at handler.rs:77);
`compute` calls `pmcp_workbook_runtime::reconcile_reference(...)` instead of `run_bundle`+project;
empty oracle → vacuous (D-04). Shared helpers to reuse verbatim: `with_provenance` (line 114),
`render_at_boundary` (line 125), `to_iserror_result` boundary discipline.

**H3 BINDING (must-not-miss, handler.rs:671-685):** `reserved_tool_names_match_the_registered_meta_tool_names`
asserts `pmcp_workbook_runtime::RESERVED_TOOL_NAMES` == `[Explain, GetManifest, DiffVersion,
RenderWorkbook]`. Adding the 6th served tool (5th meta tool) requires updating BOTH:
- `RESERVED_TOOL_NAMES` in `manifest_model.rs:212` (currently `[&str; 4]`, lives in the runtime LEAF
  so the compiler reads it without a toolkit dep) → add `"verify_accuracy"`, bump to `[&str; 5]`.
- the `registered` array in this test → add `VerifyAccuracyHandler::NAME`.

---

### `schema.rs` — `mode` input prop (WBVER-02) + `verify_accuracy_output_schema()` (WBVER-03)

**Item 2 analog:** `assemble_input_schema` (schema.rs:392-426) — the top-level
`additionalProperties:false` render input envelope. Add a `mode` property so advertise == accept:
```rust
"mode": { "type": "string", "enum": ["filled", "inputs_only"] }   // optional; default "filled"
```
This preserves the "advertise == accept" invariant the codebase enforces (the same reason
`override_props` is advertised, lines 400-406).

**Item 3 analog:** the existing `*_output_schema()` fns (e.g. `render_workbook_output_schema`,
`diff_version_output_schema`, imported at handler.rs:35-39). Add `verify_accuracy_output_schema()`
in the same shape — or derive from `schemars` over `ReconcileReport`.

---

### `mod.rs` — register the 6th tool + fix "five tools" count (WBVER-03)

**Analog:** the meta-tool registration block (mod.rs:267-286):
```rust
let builder = builder
    .tool_arc(ExplainHandler::NAME, Arc::new(ExplainHandler::new(bundle.clone())))
    .tool_arc(GetManifestHandler::NAME, Arc::new(GetManifestHandler::new(bundle.clone())))
    .tool_arc(DiffVersionHandler::NAME, Arc::new(DiffVersionHandler::new(bundle.clone())))
    .tool_arc(RenderWorkbookHandler::NAME, Arc::new(RenderWorkbookHandler::new(bundle.clone())))
    .resources_arc(Arc::new(RenderWorkbookResource::new(bundle)));
```
**Pattern to copy:** add a 5th `.tool_arc(VerifyAccuracyHandler::NAME, Arc::new(VerifyAccuracyHandler::new(bundle.clone())))`
before `.resources_arc(...)`. Update the doc/count strings "five served tools" / "all FIVE" /
"register all five" → six (mod.rs:4-5, 152-153, 165-166, 265-266 per research).

---

### Tax fixture `tax-calc@1.1.0` — text + bool formula output (D-06/D-07)

**Analog (re-fold the integrity lock):** `build_bundle_lock` (artifact_model.rs:168) +
`fold_evidence_hash` (artifact_model.rs:149) + `sha256_hex` (line 126). The bundle is integrity-
locked (loader verifies at boot); after editing the 4 JSON artifacts the `BUNDLE.lock` MUST be
re-folded or `load_bundle` fails (`"golden bundle boots"` expect panics, handler.rs:652).

```rust
pub fn build_bundle_lock(bundle_id, version, workbook_hash, ir_json, manifest_json, evidence_hash) -> BundleLock {
    let h_exec = sha256_hex(ir_json.as_bytes());
    let h_manifest = sha256_hex(manifest_json.as_bytes());
    let combined = sha256_hex(format!("{h_exec}{h_manifest}{h_evidence}").as_bytes());
    // ...
}
```
(`evidence_hash` folds `cell_map.json` via `fold_evidence_hash` — sort-by-path, length-prefixed.)

**Five hash-linked artifacts to edit (all under the fixture dir):** `manifest.json` (add 2 output
`CellRole`s: `dtype:"text"`, `dtype:"bool"`), `executable.ir.json` (2 formula cells, e.g.
`IF(taxable_income>=40000,"bracket_2","bracket_1")` text and `taxable_income>0` bool — VERIFY
`semantics::apply`/`scalar_eval` support `IF`+comparisons first, A1), `cell_map.json` (2 `CellEntry`
in a tool's `outputs` + their `oracle` values), `layout.json` (2 `CellLayout` with `formula` SET —
this is what WBVER-01 proves), `BUNDLE.lock` (re-fold).

**Wave-0 recommendation (Open Q2):** grep `pmcp-workbook-compiler` for a tax-calc fixture emitter;
prefer regeneration over hand-folding 5 hash-linked files. Verify with `load_bundle` before relying
on it (Pitfall 2).

**Example analog (D-06):** extend `crates/pmcp-server-toolkit/examples/workbook_table_authoring.rs`
(exists, 4.3K) to demo `render_workbook(filled)`, `render_workbook(inputs_only)`, and
`verify_accuracy` over the tax bundle.

## Shared Patterns

### Panic-freedom on the value path (`deny(panic/unwrap/expect)`)
**Source:** the module-level lint on render_uri.rs:36-39 and handler.rs:17-20:
```rust
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
```
**Apply to:** the new `reconcile.rs` and `VerifyAccuracyHandler` and the `mode` parse. Use
`?`/`ok_or_else`/`.get(...)` everywhere. D-03 unknown filter → `Err`; D-04 empty oracle → vacuous
report; unknown `mode` → `Err`. NEVER a panic. (Pitfall 4.)

### Domain-failure boundary (`isError:true` envelope, never protocol error)
**Source:** handler.rs:125-130:
```rust
pub(crate) fn render_at_boundary(result: Result<Value, WorkbookToolError>, stamp: &ProvStamp) -> Value {
    result.unwrap_or_else(|e| to_iserror_result(&e, stamp))
}
```
**Apply to:** `VerifyAccuracyHandler::handle` and the render `mode`-parse path — wrap the fallible
`compute` once at the boundary (T-92-10).

### Provenance stamp on every success payload
**Source:** handler.rs:114-119 (`with_provenance`) + `ProvStamp::from_bundle(&bundle)` (every handler
`new`). **Apply to:** the `verify_accuracy` success payload.

### Reader-free / purity boundary
**Source:** `RenderMode` and `reconcile_reference` are writer-only / pure-diff — no `umya`/`quick-xml`/
`calamine`. **Apply to:** keep `reconcile.rs` and the render changes inside the served tree;
`make purity-check` must stay green (WBVER-04). `RenderMode` lives in the runtime leaf so the toolkit
consumes it the way it consumes `LayoutDescriptor`.

### Cross-crate constant (do not hand-copy)
**Source:** `RESERVED_TOOL_NAMES` (manifest_model.rs:212) in the runtime LEAF, bound to the toolkit
handlers by the H3 test (handler.rs:671). **Apply to:** add `"verify_accuracy"` to the const AND the
test array in ONE change — the compiler's reserved-name gate reads this const without a toolkit dep.

## No Analog Found

None. Every change site has a concrete in-tree analog — this phase is purely additive over Phase
92/96 code. The single CREATE (`reconcile.rs`) composes two existing functions (`executor::run` +
the `project_tool_outputs` lookup) rather than introducing a new pattern.

## Metadata

**Analog search scope:**
`crates/pmcp-workbook-runtime/src/{render/mod.rs, artifact_model.rs, sheet_ir/executor.rs, manifest_model.rs, lib.rs}`,
`crates/pmcp-server-toolkit/src/workbook/{handler.rs, render_uri.rs, render_resource.rs, input.rs, schema.rs, mod.rs}`,
`crates/pmcp-server-toolkit/{examples, tests/fixtures/tax-calc@1.1.0}`.
**Files scanned:** 11
**Pattern extraction date:** 2026-06-22
