# Phase 100: Excel Workbook Built-in Servers v2 - Pattern Map

**Mapped:** 2026-06-20
**Files analyzed:** 12 (created or substantially modified)
**Analogs found:** 12 / 12 (this is a redesign of a mature subsystem — every new file has a strong in-repo analog)

> This is a REDESIGN, not green-field. The single-tool `CellMap{inputs,outputs}` → single `calculate` pipeline is being fanned out to a multi-tool `{ inputs[], tools[] }` model where each named Excel Table becomes one MCP tool with a DAG-derived input schema. Almost every primitive already exists; the work is *re-wiring single→multi* and *re-keying named-range→table-name*. Every excerpt below carries a file:line so a planner task can say "model X on Y."

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/pmcp-workbook-runtime/src/manifest_model.rs` (extend → `Tool`/`OutputField`/`InputField`) | model | transform | same file (`CellRole`/`InputTier` additive-serde precedent) | **exact** (in-file precedent) |
| `crates/pmcp-workbook-runtime/src/artifact_model.rs` (extend `CellMap` → `{inputs[],tools[]}`) | model | transform | same file (`CellMap`/`CellEntry`) | **exact** (in-file precedent) |
| `crates/pmcp-workbook-runtime/src/dag.rs` (add `upstream_input_leaves`) | utility | transform | same file (`toposort`, `dependencies_of`) | **exact** (in-file precedent) |
| `crates/pmcp-workbook-compiler/src/ingest/mod.rs` (`table_records()` harvest) | service (ingest) | file-I/O (umya read) | `table_ranges` + `data_validations` (same file) | **exact** (extends sibling fn) |
| `crates/pmcp-workbook-compiler/src/ingest/cell_map.rs` (add `TableRecord` to `SheetRecord`) | model | transform | `DataValidationRecord`/`SheetRecord.tables` (same file) | **exact** (in-file precedent) |
| `crates/pmcp-workbook-compiler/src/manifest/synth.rs` (per-row harvest type/unit/enum/tier) | service (synth) | transform | `apply_dv_fork`/`freeze_or_reason` (same file) | **exact** (extends sibling) |
| `crates/pmcp-workbook-compiler/src/lib.rs` (retire F1/promote/name fns → table-row build) | service (orchestrator) | transform | `promote_named_outputs`/`name_named_inputs`/`refuse_uncallable_inputs` (same file) | **exact** (reshape in place) |
| `crates/pmcp-workbook-compiler/src/artifact/cell_map.rs` (`build_tools` per-table) | service (emit) | transform | `build_cell_map` (same file) | **exact** (in-file precedent) |
| `crates/pmcp-server-toolkit/src/workbook/schema.rs` (per-tool I/O schema) | utility (schema) | transform | `input_schema_for_manifest`/`output_schema_for_manifest` (same file) | **exact** (parameterize per-tool) |
| `crates/pmcp-server-toolkit/src/workbook/handler.rs` (per-tool handler) | controller (tool) | request-response | `CalculateHandler` (same file) | **exact** (one→N) |
| `crates/pmcp-server-toolkit/src/workbook/mod.rs` (N-handler registration loop) | config (registration) | event-driven | `with_workbook_bundle` `.tool_arc` chain (same file) | **exact** (chain→loop) |
| `cargo-pmcp/src/commands/workbook/explain.rs` (NEW subcommand) | controller (CLI) | request-response | `cargo-pmcp/src/commands/workbook/lint.rs` | **exact** (clone the read-only shape) |
| `crates/pmcp-workbook-compiler/src/fixture_author.rs` (Table-emit + `template.xlsx`) | utility (author) | file-I/O (xlsx write) | `author_xlsx`/`WorkbookSpec`/`DefinedNameSpec` (same file) | **role-match** (extend writer surface) |

---

## Pattern Assignments

### 1. `manifest_model.rs` — additive-serde model extension (multi-tool model types)

**Analog:** `crates/pmcp-workbook-runtime/src/manifest_model.rs` (the file itself — `CellRole.tier`/`allowed_values` are the verbatim additive-serde precedent the new `Tool`/`OutputField` types follow).

**Additive-serde precedent** (manifest_model.rs:119-132): new fields land as `#[serde(default)]` (+ `skip_serializing_if` for `Option`/`Vec`) so old bundles deserialize unchanged. Pre-1.0 with no legacy bundles also permits a clean structural break — but match this style for any field added to an existing struct:
```rust
#[serde(default)]
pub tier: Option<InputTier>,
#[serde(default, skip_serializing_if = "Option::is_none")]
pub allowed_values: Option<Vec<String>>,
```

**Struct-shape precedent** (manifest_model.rs:90-133, the full `CellRole`): every model struct derives `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]` (drop `Eq` when any field is `f64`-bearing via `CellValue`/`InputTier`), `#[serde(rename_all = "...")]` where applicable, and carries doc comments on every field (the crate's documentation gate). The new `Tool { name, description, input_keys: Vec<String>, outputs: Vec<OutputField>, oracle: ... }` (spec §4.1) mirrors this exactly.

**Enum-with-data precedent** (manifest_model.rs:141-159, `InputTier`): `#[serde(rename_all = "snake_case", tag = "kind")]` internally-tagged enum carrying typed payloads. Model the `{variable, strict}` tier dropdown ingestion on this.

**RETIRE in this file** (per §9 cleanup ledger):
- `json_key_for_role` (manifest_model.rs:175-180) — **RESHAPE**: keep the fn (consumed at `artifact/cell_map.rs:70` and `schema.rs:35`) but DELETE the `strip_governance_prefix` call; key = table `name` verbatim. New body collapses to `role.name.or(role.meaning).or(role.cell)` with NO strip.
- `strip_governance_prefix` (manifest_model.rs:182-198) — **DELETE** entirely.
- `Role::from_name_prefix` (manifest_model.rs:53-68) — **AUDIT before delete** (Assumption A1 / Open Q1): the D-04 overlap/Guide check may still consume it for `const_`. Grep call sites in the plan; keep only the `const_` branch if a non-table consumer remains.
- The `from_name_prefix` test (manifest_model.rs:371-377) follows the fate of the fn.

---

### 2. `artifact_model.rs` — `CellMap{inputs,outputs}` → `{inputs[], tools[]}` (THE CORE LIFT)

**Analog:** `crates/pmcp-workbook-runtime/src/artifact_model.rs` (the file itself).

**Current single-tool shape** (artifact_model.rs:27-49) — this is what is being fanned out:
```rust
pub struct CellEntry {
    pub json_key: String,    // LLM-facing key
    pub seed_coord: String,  // sheet!addr cell key
    pub unit: Option<String>,
}
pub struct CellMap {
    pub inputs: Vec<CellEntry>,   // one per Role::Input
    pub outputs: Vec<CellEntry>,  // one per Role::Output — the single-tool projection
}
```

**The lift** (spec §4.1): `outputs: Vec<CellEntry>` becomes `tools: Vec<Tool>`, where each `Tool` owns its own `outputs` + DAG-derived `input_keys`. `inputs` stays as the shared pool. Keep `CellEntry` as-is (it already carries `json_key`/`seed_coord`/`unit`) — reuse it inside each `Tool`. Both compiler (emitter) and served binary deserialize this SAME type (the umya-free boundary, artifact_model.rs:10-22), so the new `Tool` type also lives HERE, not re-declared per side.

**Every call site that must fan out** (Pitfall 3 — plan as ONE wave):
- `crates/pmcp-workbook-compiler/src/artifact/cell_map.rs:43` `build_cell_map` → emits the new `tools[]`.
- `crates/pmcp-server-toolkit/src/workbook/schema.rs:76,275` `output_schema_for_manifest`/`input_schema_for_manifest` (iterate `cell_map.outputs`/`cell_map.inputs`).
- `crates/pmcp-server-toolkit/src/workbook/handler.rs:144-201` `CalculateHandler` (single `compute` over `cell_map`).
- `crates/pmcp-server-toolkit/src/workbook/mod.rs:248-271` the `.tool_arc` registration chain.
- `crates/pmcp-workbook-compiler/src/lib.rs:354` `comparison_from_outputs` (per-tool oracle partition — Assumption A5).

---

### 3. `dag.rs` — `Dag::upstream_input_leaves` reachability (§4.2 new algorithm)

**Analog:** `crates/pmcp-workbook-runtime/src/dag.rs` (model the new pure fn beside `toposort`).

**Existing one-hop accessor to build on** (dag.rs:63-67) — `dependencies_of` is the "depends on" edge the traversal walks:
```rust
pub fn dependencies_of(&self, key: &str) -> &[String] {
    self.dependencies.get(key).map_or(&[], |v| v.as_slice())
}
```

**Existing algorithm style to match** (dag.rs:91-146, `toposort`): a free `pub fn` over `&Dag`, `HashMap`/`VecDeque`/`HashSet` work-set, **sorts intermediate collections for deterministic output** (dag.rs:107,125 — `ready.sort()` / `newly_ready.sort()`), returns owned `String`s (no foreign type crosses). The new traversal returns `BTreeSet<String>` (sorted-by-construction) — same determinism discipline.

**New fn (from RESEARCH §Pattern 2, verified against this DAG API):**
```rust
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
```
**Edge cases (§4.2):** input reachable only via a constant path → not in `input_cells` → excluded; input feeding NO tool → lint "feeds no tool"; shared intermediate → each tool gets the union of its own upstream leaves.

**Test precedent** (dag.rs:148-214): `#[cfg(test)] mod tests` with `Dag::new()`/`add_edge` fixtures; assert ordering/membership. Add the property-test harness (random DAG generator) per the Wave-0 gap.

---

### 4. `ingest/mod.rs` — harvest Table name + columns (`table_records()`)

**Analog:** `crates/pmcp-workbook-compiler/src/ingest/mod.rs` (`table_ranges` reads areas; `data_validations` is the FLAT-MAP idiom + umya-isolation discipline to copy).

**Current table harvest — areas ONLY, metadata dropped** (ingest/mod.rs:310-323) — THE EXTENSION POINT (Pitfall 1):
```rust
fn table_ranges(ws: &umya_spreadsheet::Worksheet, sheet_name: &str) -> Vec<RangeRef> {
    ws.tables()
        .iter()
        .map(|t| {
            let (start, end) = t.area();   // ← reads area; never calls get_name()/get_columns()
            RangeRef { sheet: sheet_name.to_string(), start: start.get_coordinate(), end: end.get_coordinate() }
        })
        .collect()
}
```

**New `table_records()`** calls `t.get_name()` + `t.get_columns().iter().map(|c| c.get_name())` (umya 3.0.0 API verified in RESEARCH §2) and produces a `Vec<TableRecord>`. Expected `cols == ["name","value","description"]` (+ `"tier"` for input tables).

**umya-isolation discipline to preserve** (ingest/mod.rs:332-356, `data_validations`): NEVER `.unwrap()` on a umya accessor (the crate deny gate — `unwrap_or(&[])` for absent collections). umya `.unwrap()`s on malformed table XML (§2 caveat 1 / Pitfall 2) — keep the read inside `ingest::ingest`, which maps failure to `CompileError::Ingest` (lib.rs:293-294). Add a fuzz target feeding malformed table bytes (Wave-0 gap).

**DV harvest already wired (enum + tier dropdown source)** (ingest/mod.rs:332-356): `data_validations()` emits one `DataValidationRecord` per (DV × range) with `dv_type "list"` + raw `formula1`. This IS the enum/tier dropdown source the per-row harvest reads (`synth.rs:apply_dv_fork`).

---

### 5. `ingest/cell_map.rs` — `TableRecord` struct on `SheetRecord`

**Analog:** `crates/pmcp-workbook-compiler/src/ingest/cell_map.rs` (`DataValidationRecord` at :98-108 is the owned-record shape; `SheetRecord` at :143-174 is the host).

**Owned-record precedent** (cell_map.rs:98-108): `#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]`, owned `RangeRef`/`String` only ("no `umya` type crosses — the module-doc quarantine invariant"), doc comment per field. New:
```rust
pub struct TableRecord {
    pub name: String,          // t.get_name() — the TOOL name candidate
    pub area: RangeRef,        // t.get_area()
    pub columns: Vec<String>,  // t.get_columns().map(get_name)
}
```

**Host field to ADD** (cell_map.rs:163-164) — `tables: Vec<RangeRef>` becomes (or gains a sibling) `tables: Vec<TableRecord>` (additive; `SheetRecord` is already `PartialEq`-only due to `col_widths: f64`, so no derive change):
```rust
/// Excel table ranges as owned [`RangeRef`]s.   ← TODAY (areas only)
pub tables: Vec<RangeRef>,
```

---

### 6. `manifest/synth.rs` — per-row harvest (type/unit/enum/tier from the `value` cell)

**Analog:** `crates/pmcp-workbook-compiler/src/manifest/synth.rs` (`apply_dv_fork` + `freeze_or_reason` are the verbatim "project a cell's metadata into a `CellRole`" pattern).

**DV → enum harvest precedent** (synth.rs:206-224): finds the covering DV by range, freezes an eligible inline list to `allowed_values`, else emits a reason-coded WARNING (never blocks). The table harvest reads the same DV machinery for BOTH the enum domain AND the `tier` dropdown (§3.3 — the tier dropdown dogfoods the enum-from-dropdown mechanism):
```rust
match freeze_or_reason(dv, &sheet.name, wb, cell_role.dtype) {
    Ok(values) => cell_role.allowed_values = Some(values),
    Err(reason) => findings.push(dv_dynamic_finding(&sheet.name, &cell.addr, reason)),
}
```

**Type witness from the value cell** (synth.rs:441-445): dtype derives from the value cell's parseability — `f64::parse → Dtype::Number`, else `Dtype::Text`. The new per-row harvest reads the `value` column cell the same way (§3.3 "type ← value cell type").

**Unit from number format** (§3.3 "unit ← value cell number format"): the source field is `CellRecord.number_format` (cell_map.rs:126-129 — `style.number_format().map(|nf| nf.format_code())`); currency→USD, `%`→rate, date→date. There is no existing number-format→unit projector — this is NEW logic, but reads an EXISTING harvested field.

---

### 7. `lib.rs` — retire F1/promote/name; build tools from tables (orchestration)

**Analog:** `crates/pmcp-workbook-compiler/src/lib.rs` (`compile_workbook_inner` is the orchestrator; the three named-range fns are reshaped/retired in place).

**Orchestration sequence to re-wire** (lib.rs:309-365) — steps 3a/3b/3c are replaced by table harvest:
```rust
let mut manifest = stage1.synth_manifest;
promote_named_outputs(&mut manifest, &map);   // (3a) RETIRE/RESHAPE → output from named Table
name_named_inputs(&mut manifest, &map);       // (3b) RETIRE → name from table `name` column
refuse_uncallable_inputs(&manifest)?;         // (3c) RESHAPE → row lints
...
let comparison = comparison_from_outputs(&map, &manifest);   // (7) → per-tool oracle partition
```

**§9 cleanup ledger — exact fates:**
| Symbol | Location | Fate |
|--------|----------|------|
| `promote_named_outputs` | lib.rs:603-620 (called :321) | **RETIRE/RESHAPE** — the `Role::Output` promotion logic is still needed; re-source it from the named output Table, not the `out_*` defined name. |
| `name_named_inputs` | lib.rs:637-656 (called :325) | **RETIRE** — replaced by table harvest assigning `name` from the `name` column. |
| `refuse_uncallable_inputs` / `validate_input_keys` | lib.rs:768-780 / :675-695 (called :331) | **RESHAPE** — re-target the SAME `LintFinding` codes at table ROWS. |
| `unnamed_input_finding` / `duplicate_input_key_finding` / `empty_input_key_finding` | lib.rs:707-759 | **RESHAPE** — keep the finding shape; DROP the "define a named range `in_<name>`" repair text (lib.rs:715-721,738-741,757). New repair text points at the table row/cell. |

**Fail-helpful finding precedent to KEEP** (lib.rs:707-723): the located, repair-bearing finding shape is exactly the message style new row lints (blank `name`, dup key, value-less row, no caption, bad MCP charset) must match:
```rust
fn unnamed_input_finding(cell: &str) -> LintFinding {
    let (sheet, addr) = split_cell_key(cell);
    LintFinding::new(
        Severity::Error,
        "manifest/input-no-semantic-key",   // ← reuse code namespace; re-target message at the row
        sheet, addr,
        format!("input cell {cell} has no in_* named range; ..."),   // ← reword for table rows
        format!("in Excel, define a single-cell named range `in_<name>` ..."),  // ← DROP named-range repair
    )
}
```
`split_cell_key` (lib.rs:699-704) is the `sheet!addr` → located-finding splitter — KEEP, reuse for row-cell locations.

---

### 8. `artifact/cell_map.rs` — `build_tools` per output Table

**Analog:** `crates/pmcp-workbook-compiler/src/artifact/cell_map.rs` (`build_cell_map` is the manifest→artifact projector).

**Current single-output build** (cell_map.rs:43-74) — partitions cells by `Role`, fails loud on zero outputs:
```rust
pub fn build_cell_map(manifest: &Manifest) -> Result<CellMap, String> {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for role in &manifest.cells {
        match role.role {
            Role::Input => inputs.push(entry(role)),
            Role::Output => outputs.push(entry(role)),
            Role::Constant | Role::Formula => {},
        }
    }
    if outputs.is_empty() { return Err("... no Role::Output cell ...".to_string()); }
    Ok(CellMap { inputs, outputs })
}
fn entry(role: &CellRole) -> CellEntry {
    CellEntry { json_key: json_key_for_role(role), seed_coord: role.cell.clone(), unit: role.unit.clone() }
}
```

**The lift:** instead of one flat `outputs`, group output cells by their owning Table → one `Tool` per group; for each Tool call `upstream_input_leaves` to derive `input_keys`. The fail-loud-on-zero-outputs check generalizes to "fail loud if zero Tables." Reuse `entry()` verbatim for each Tool's outputs. The "feeds no tool" / "constant-only path" edge-case lints (§4.2) live here or beside the harvest.

---

### 9. `schema.rs` — per-tool input + output schema (TypedToolWithOutput)

**Analog:** `crates/pmcp-server-toolkit/src/workbook/schema.rs` (`input_schema_for_manifest` + `output_schema_for_manifest` — parameterize per-tool instead of whole-cell-map).

**Output schema builder** (schema.rs:66-100) — iterate `cell_map.outputs` → per-tool iterate `tool.outputs`:
```rust
pub fn output_schema_for_manifest(manifest: &Manifest, cell_map: &CellMap) -> Value {
    let mut output_props = Map::new();
    for entry in &cell_map.outputs {        // ← becomes `for entry in &tool.outputs`
        let role = role_for_seed(manifest, &entry.seed_coord);
        output_props.insert(entry.json_key.clone(), output_column_schema(entry.unit.as_deref(), role));
    }
    ... result_envelope_schema(success)
}
```

**Input schema builder + the strict envelope** (schema.rs:274-330) — `additionalProperties:false` mirrors the runtime DTO gate (V5 input-validation, must keep). Per-tool: iterate the tool's DAG-derived `input_keys` instead of all `cell_map.inputs`:
```rust
pub fn input_schema_for_manifest(manifest: &Manifest, cell_map: &CellMap) -> Value {
    let mut input_props = Map::new();
    for entry in &cell_map.inputs {         // ← becomes the tool's DAG-derived input subset
        let role = role_for_seed(manifest, &entry.seed_coord);
        let dtype = role.map_or(Dtype::Number, |r| r.dtype);
        ...
        if let Some(allowed) = role.and_then(|r| r.allowed_values.as_ref()) {
            prop.insert("enum".to_string(), json!(allowed));   // closed-enum from workbook DV
        }
        input_props.insert(entry.json_key.clone(), Value::Object(prop));
    }
    // F2 — KEEP UNTOUCHED (§9): advertise variable_tier_keys override props
    ...
}
```

**F2 — KEEP** (schema.rs:297-310 + tests :533-608): the `variable_tier_keys` override-advertising block survives the rewrite verbatim (it is independent of the input model). `crate::workbook::input::variable_tier_keys` stays the single source.

**Dtype→JSON mapping** (schema.rs:24-30, `dtype_json_type`) and the shared `result_envelope_schema` (schema.rs:114-140 — folds `isError`/`provenance`) are reused unchanged per tool.

---

### 10. `handler.rs` — one tool handler per output Table

**Analog:** `crates/pmcp-server-toolkit/src/workbook/handler.rs` (`CalculateHandler` is the exact one→N template).

**Current single handler** (handler.rs:144-201) — clone this shape, parameterizing by a `Tool`:
```rust
pub struct CalculateHandler { bundle: Arc<WorkbookBundle>, stamp: ProvStamp }
impl CalculateHandler {
    pub const NAME: &str = "calculate";   // ← becomes tool.name (sanitized to MCP charset)
    pub fn new(bundle: Arc<WorkbookBundle>) -> Self { ... }
    fn compute(&self, args: Value) -> Result<Value, WorkbookToolError> {
        let validated = validate_input(args, &self.bundle.manifest, &self.bundle.cell_map)?;
        let run = run_bundle(&self.bundle, validated.seeds)?;
        let outputs = project_outputs(&self.bundle, &run)?;   // ← project only THIS tool's outputs
        ...
    }
}
```

**The `with_output_schema` (TypedToolWithOutput) pattern to replicate per tool** (handler.rs:180-200) — emits BOTH inputSchema AND outputSchema (→ `structuredContent`):
```rust
fn metadata(&self) -> Option<ToolInfo> {
    Some(
        ToolInfo::with_ui(
            Self::NAME,
            Some("Compute the workbook outputs ...".into()),   // ← becomes tool.description (caption)
            input_schema_for_manifest(&self.bundle.manifest, &self.bundle.cell_map),   // ← per-tool inputs
            WORKBOOK_TOOL_UI,
        )
        .with_output_schema(output_schema_for_manifest(&self.bundle.manifest, &self.bundle.cell_map)),  // ← per-tool outputs
    )
}
```
The new `WorkbookToolHandler::new(bundle, tool)` holds an `Arc<WorkbookBundle>` + a `Tool`; `metadata()` returns `tool.name`/`tool.description`/per-tool schemas. `render_at_boundary` (handler.rs:132-138 — error → `isError` envelope) and the `#[async_trait] impl ToolHandler` (handler.rs:174-178) are reused unchanged.

---

### 11. `mod.rs` — N-handler registration loop

**Analog:** `crates/pmcp-server-toolkit/src/workbook/mod.rs` (`with_workbook_bundle`'s `.tool_arc` chain).

**Current fixed 5-tool chain** (mod.rs:248-271) — the `CalculateHandler` registration becomes a loop:
```rust
let builder = self
    .tool_arc(CalculateHandler::NAME, Arc::new(CalculateHandler::new(bundle.clone())))  // ← LOOP over tools
    .tool_arc(ExplainHandler::NAME, Arc::new(ExplainHandler::new(bundle.clone())))      // ← KEEP (meta tools)
    .tool_arc(GetManifestHandler::NAME, Arc::new(GetManifestHandler::new(bundle.clone())))
    .tool_arc(DiffVersionHandler::NAME, Arc::new(DiffVersionHandler::new(bundle.clone())))
    .tool_arc(RenderWorkbookHandler::NAME, Arc::new(RenderWorkbookHandler::new(bundle.clone())))
    .resources_arc(Arc::new(RenderWorkbookResource::new(bundle)));
```

**The loop target** (RESEARCH §"Tool registration loop"):
```rust
let mut builder = self;
for tool in &bundle.tools {
    builder = builder.tool_arc(&tool.name, Arc::new(WorkbookToolHandler::new(bundle.clone(), tool.clone())));
}
// then chain the 4 meta tools (Explain/GetManifest/DiffVersion/RenderWorkbook) + the resource unchanged
```
**Zero-output warning to generalize** (mod.rs:235-244): the "bundle declares zero outputs" `tracing::warn!` becomes "bundle declares zero tools."

---

### 12. `cargo-pmcp/.../workbook/explain.rs` — NEW `workbook explain` subcommand

**Analog:** `cargo-pmcp/src/commands/workbook/lint.rs` (clone its read-only ingest→render shape end to end).

**Subcommand registration** (mod.rs:74-93) — add an `Explain` variant + dispatch arm to the existing enum:
```rust
pub enum WorkbookCommand {
    Compile(compile::CompileArgs),
    Lint(lint::LintArgs),
    Emit(emit::EmitArgs),
    // ADD: Explain(explain::ExplainArgs),
}
impl WorkbookCommand {
    pub fn execute(self, gf: &GlobalFlags) -> Result<()> {
        match self {
            WorkbookCommand::Compile(args) => compile::execute(args, gf),
            WorkbookCommand::Lint(args) => lint::execute(args, gf),
            WorkbookCommand::Emit(args) => emit::execute(args, gf),
            // ADD: WorkbookCommand::Explain(args) => explain::execute(args, gf),
        }
    }
}
```
Also add `pub mod explain;` at mod.rs:21-25.

**Args + dual-format renderer to mirror** (lint.rs:28-37, 46-60, 100-110): `--format text|json` (default `text`), data→stdout / advisory→stderr (Phase-74 D-11), pure `format_*` String fn so JSON is testable without stdout capture:
```rust
#[derive(Debug, Args)]
pub struct LintArgs {
    pub workbook_path: PathBuf,
    #[arg(long, default_value = "text")]
    pub format: String,
}
pub fn execute(args: LintArgs, gf: &GlobalFlags) -> Result<()> {
    let report = lint_workbook(&args.workbook_path)?;
    let not_quiet = gf.should_output() && std::env::var("PMCP_QUIET").is_err();
    print_lint_report(&report, &args.format, not_quiet)?;
    ...
}
```

**Read-only ingest shape** (lint.rs:63-68) — `explain` ingests + synthesizes (no bundle written) then renders the projected tool surface:
```rust
fn lint_workbook(path: &std::path::Path) -> Result<LintReport> {
    let (map, _ingest_findings) = pmcp_workbook_compiler::ingest::ingest(path)
        .with_context(|| format!("failed to ingest workbook {}", path.display()))?;
    let src = WorkbookCellSource::new(&map);
    Ok(dialect_lint(&src, &DialectRules::default()))
}
```
`explain` extends this: ingest → synth → project tools → render per-tool `name`/`description`/inputSchema (key: type [unit] [enum]) / outputSchema (RESEARCH OQ-3). Recommend TEXT first + a thin `--format json` add-on, exactly like `format_lint_report` (lint.rs:100-110).

**Text-render style to match** (lint.rs:114-134, `render_text` + `severity_label`): one located line per item; snapshot-test the output (Wave-0 gap).

---

### 13. `fixture_author.rs` — Table-emitting author + shipped `template.xlsx`

**Analog:** `crates/pmcp-workbook-compiler/src/fixture_author.rs` (`author_xlsx`/`WorkbookSpec`/`DefinedNameSpec` — extend the writer surface; ADD `add_table`/`set_columns`/`add_data_validation`).

**Provenance-valid writer recipe (WHY rust_xlsxwriter)** (fixture_author.rs:12-30): `rust_xlsxwriter` hard-codes `<Application>Microsoft Excel</Application>` + `<AppVersion>12.0000</AppVersion>` + `calcPr calcId="124519"` → classifies `ProvenanceClass::ExcelTrusted` for free. umya-authored books classify `UmyaFabricated` and are REFUSED. **The template MUST use rust_xlsxwriter** (anti-pattern: authoring with umya).

**Cached `<v>` IS the reconcile oracle** (fixture_author.rs:32-37, 220-225): every formula via `Formula::new(f).set_result(cached)` so the `<v>` carries the authored expected value — this IS the §3.2 output `value` oracle the gate checks:
```rust
AuthoredCell::Formula { formula, cached, .. } => {
    let f = Formula::new(*formula).set_result(*cached);
    worksheet.write_formula(row, col, f).map(|_| ())
}
```

**Author entry point to extend** (fixture_author.rs:171-188, `author_xlsx`) — TODAY writes cells + `define_name` (named ranges). The new template author adds, per Table: `worksheet.add_table(...)` + `Table::set_name(...)` + `Table::set_columns(...)`, and `worksheet.add_data_validation(...)` for the `{variable, strict}` tier dropdown + the sample enum dropdown (rust_xlsxwriter 0.95 APIs verified in RESEARCH §2):
```rust
pub(crate) fn author_xlsx(path: &Path, spec: &WorkbookSpec) -> Result<(), XlsxError> {
    let palette = CellFormats::new();
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet.set_name(spec.sheet)?;
    for cell in &spec.cells { write_cell(worksheet, cell, &palette)?; }
    for dn in &spec.defined_names { workbook.define_name(dn.name, dn.target)?; }  // ← `out_*`/`in_*` RETIRE
    workbook.save(path)?;
    Ok(())
}
```

**RETIRE the `in_*`/`out_*` defined-name injection** (fixture_author.rs:137-160 `DefinedNameSpec`, :182-184 `define_name` loop, :384-392 the `in_a1`/`out_result` specs) — §9: output keys come from the Table `name` column; no `in_*`/`out_*` named ranges. **REPLACE** with Table specs.

**Reproducible-fixture discipline to match** (fixture_author.rs:39-49, 416 `regenerate_fixtures`): commit `template.xlsx` via the `#[ignore]`d, env-gated path (`PMCP_REGEN_FIXTURES=1 ... -- --ignored`) with a `*.gen.json` sidecar. **Pitfall 4:** the module is `#![cfg(test)]` (fixture_author.rs:51) — for a SHIPPED template, either promote the table-authoring logic to a non-test module / xtask generator, OR commit a generated `template.xlsx` once (RESEARCH recommends the latter for v1). Decide in the plan (Assumption A2). Location: `cargo-pmcp/src/templates/workbook_bundle/` (replacing `tax-calc.xlsx`) + copy into `tests/fixtures/` (Open Q3 — one source, two consumers, §7).

---

## Shared Patterns

### Located, repair-bearing lint findings (fail-helpful)
**Source:** `crates/pmcp-workbook-runtime/src/finding.rs:44-80` (`LintFinding::new(severity, rule, sheet, cell, message, repair)`) + `LintReport` collect-all (:86-113, `has_errors` is the gate, only `Error` blocks).
**Apply to:** ALL new row lints (blank `name`, dup key, value-less row, no caption, unmappable tool name) + the harvest/cell_map edge-case lints (§4.2). Match the message format: `<severity> <sheet>!<cell> [<rule>]: <message> — fix: <repair>` (rendered at lint.rs:114-134). Reuse the `manifest/...` rule namespace from the F1 codes (lib.rs:711,731,751); only re-target the message + repair text at table rows.

### Additive serde on shared model types
**Source:** `manifest_model.rs:119-132` (`#[serde(default)]` + `skip_serializing_if`).
**Apply to:** any field added to an existing `Manifest`/`CellRole`/`SheetRecord` struct so old bundles/fixtures deserialize unchanged. New top-level types (`Tool`, `TableRecord`) follow the full derive set (`Debug,Clone,PartialEq,Serialize,Deserialize,schemars::JsonSchema`; drop `Eq` if `f64`-bearing).

### Strict input envelope (V5 input validation)
**Source:** `schema.rs:312-329` (`additionalProperties:false` at root + `inputs` object) mirroring the runtime DTO `deny_unknown_fields` gate (`workbook/input.rs`).
**Apply to:** every per-tool `inputSchema`. A client trusting the schema must never send a key the runtime then rejects — each new per-tool schema keeps the strict envelope.

### umya-isolation / purity boundary
**Source:** `ingest/mod.rs:332-356` (no `.unwrap()` on umya accessors; `unwrap_or(&[])`); `cell_map.rs` module-doc "no `umya` type crosses." `Makefile:506+` `make purity-check` cargo-tree ban.
**Apply to:** the new `table_records()` harvest (umya confined to the compiler; owned `String`/`RangeRef` only cross into the manifest). NEVER add calamine; NEVER let umya into the served tree.

### Provenance gate — DO NOT TOUCH
**Source:** `crates/pmcp-workbook-compiler/src/provenance/gate.rs`, `raw_parts.rs` (quick-xml quarantined).
**Apply to:** nothing — orthogonal (§6). Any change here is out of scope.

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| (none) | — | — | Every new/modified file in this phase has a strong in-repo analog. The two genuinely-new pure functions — `Dag::upstream_input_leaves` and a number-format→unit projector — model their STYLE on `toposort` (dag.rs:91) and `apply_dv_fork` (synth.rs:206) respectively, but their core logic is new (RESEARCH §Pattern 2 supplies the verified `upstream_input_leaves` body). |

---

## Metadata

**Analog search scope:**
- `crates/pmcp-workbook-runtime/src/` (manifest_model.rs, artifact_model.rs, dag.rs, finding.rs)
- `crates/pmcp-workbook-compiler/src/` (ingest/mod.rs, ingest/cell_map.rs, manifest/synth.rs, artifact/cell_map.rs, lib.rs, fixture_author.rs)
- `crates/pmcp-server-toolkit/src/workbook/` (schema.rs, handler.rs, mod.rs)
- `cargo-pmcp/src/commands/workbook/` (mod.rs, lint.rs)

**Files scanned (read in full or targeted):** 13 source files; cross-referenced against the cleanup-ledger file:line map in 100-RESEARCH.md (all verified).

**Pattern extraction date:** 2026-06-20
