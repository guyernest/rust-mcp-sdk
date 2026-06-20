//! `cell_map.json` emission — the manifest-driven I/O map.
//!
//! A [`CellMap`] gives the served `calculate` tool a CONCRETE, tested
//! input/output mapping: for each `Role::Input` and `Role::Output` cell it records
//! `{json_key, seed_coord, unit}`.
//!
//! - `json_key` — the neutral JSON key the LLM/caller uses for this cell. Derived
//!   via the runtime's shared [`json_key_for_role`] (the manifest `name` when present,
//!   else the `meaning`, else the cell key) so the cell-map key and the served
//!   tools' schema builder cannot drift.
//! - `seed_coord` — the `CellEnv` seed coordinate: the fully-qualified cell key
//!   (`sheet!addr`) the executor seeds/reads.
//! - `unit` — the declared unit (`m2`/`GBP`/…), carried verbatim.
//!
//! # Generalization fix (§5)
//!
//! The lighthouse `CellMap` carried a hardcoded `supply_total_cell` (a customer
//! "headline output" assumption). The SDK [`CellMap`] is the runtime's
//! `{inputs, tools[]}` shape (WBV2-03 §4.1) with NO privileged-headline field: each
//! output Table becomes its own [`Tool`], so no single output is elevated.
//! [`build_tools`] fails loud ONLY when there is no output Table at all (a workbook
//! with zero output Tables cannot serve any tool), never on the absence of a single
//! named "supply total."
//!
//! [`build_cell_map`] is the single-tool transitional path (one tool wraps all
//! outputs) the existing named-range compile orchestrator still emits;
//! [`build_tools`] is the per-Table multi-tool primitive (WBV2-03/04) the served
//! fan-out + per-tool reconcile ([`reconcile_tools`]) consume. The Plan 03 flat
//! `.outputs()` accessor is RETIRED (Plan 04): every consumer iterates
//! `tools[].outputs` per-tool.
//!
//! Built from the (tier-ratified) [`Manifest`] in `emit_bundle`; serialized
//! through the deterministic [`crate::artifact::serialize`] choke point.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use pmcp_workbook_runtime::{
    json_key_for_role, role_for_cell, sanitize_tool_name, upstream_input_leaves, CellRole,
    CellValue, Dag, LintFinding, Manifest, Role, Severity,
};

use crate::reconcile::within_tol;

// Re-export the runtime-safe artifact shapes (the served loader deserializes the
// SAME `CellMap`/`CellEntry`/`Tool`); never re-declared here.
pub use pmcp_workbook_runtime::{CellEntry, CellMap, Tool};

/// One output Table's identity + membership (WBV2-03 §4.1), supplied by the
/// orchestrator from the harvested `TableRecord`s: which output CELLS belong to
/// which output Table, plus the Table's tool `name` + `description` (caption).
///
/// This is the grouping the manifest alone cannot supply — a `CellRole` does not
/// record its owning Table — so the offline caller (which harvested the Table
/// areas) passes membership explicitly. The unit tests build it synthetically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputTable {
    /// The raw output-Table name → the tool `name` (MCP-charset sanitization in
    /// the served emit, Plan 04).
    pub name: String,
    /// The caption cell above the Table → the tool `description`, when authored.
    pub description: Option<String>,
    /// The fully-qualified `sheet!addr` cell keys of this Table's output cells.
    pub output_cells: Vec<String>,
}

/// Build the per-Table [`Tool`]s with DAG-derived minimal `input_keys` (WBV2-03 §4.2).
///
/// Groups the manifest's `Role::Output` cells by their owning output Table (via the
/// caller-supplied `output_tables` membership). For each Table, the tool's
/// `input_keys` is the UNION over that Table's output cells of
/// [`upstream_input_leaves`] — the minimal subset of the manifest's `Role::Input`
/// cells transitively reachable upstream — mapped from cell key to its served
/// `json_key` via the shared [`json_key_for_role`]. The tool's `outputs` reuse
/// [`entry`] per output cell; its `oracle` carries each output cell's authored
/// expected value.
///
/// Edge cases (§4.2): a constant-only upstream path contributes NO input (excluded
/// by [`upstream_input_leaves`], no lint); an input cell reachable by NO tool yields
/// a "feeds no tool" `WARNING` [`LintFinding`].
///
/// Returns the tools (in `output_tables` order) plus the collected edge-case lints.
///
/// # Errors
/// Returns `Err` if `output_tables` is empty (a served workbook with zero output
/// Tables cannot answer any tool — the fail-loud-on-zero-outputs check generalized).
pub fn build_tools(
    manifest: &Manifest,
    dag: &Dag,
    output_tables: &[OutputTable],
) -> Result<(Vec<Tool>, Vec<LintFinding>), String> {
    if output_tables.is_empty() {
        return Err(
            "the manifest declares no output Table — a served workbook must have at \
             least one output Table to expose a tool"
                .to_string(),
        );
    }

    // The shared input pool: the cell keys of every Role::Input, plus a lookup from
    // a cell key to its served json_key (so input_keys carry semantic keys).
    let input_cells: HashSet<String> = manifest
        .cells
        .iter()
        .filter(|c| c.role == Role::Input)
        .map(|c| c.cell.clone())
        .collect();
    let input_key_of: BTreeMap<String, String> = manifest
        .cells
        .iter()
        .filter(|c| c.role == Role::Input)
        .map(|c| (c.cell.clone(), json_key_for_role(c)))
        .collect();

    let mut tools = Vec::with_capacity(output_tables.len());
    // Track which input cells feed at least one tool (the "feeds no tool" lint).
    let mut fed_inputs: HashSet<String> = HashSet::new();

    for table in output_tables {
        // build_one_tool already walks each output's upstream input leaves to derive
        // input_keys; it returns the reached input CELLS so we mark them fed without a
        // second DAG traversal.
        let (tool, reached) = build_one_tool(manifest, dag, table, &input_cells, &input_key_of);
        fed_inputs.extend(reached);
        tools.push(tool);
    }

    let findings = feeds_no_tool_findings(manifest, &input_cells, &fed_inputs);
    Ok((tools, findings))
}

/// Build ONE [`Tool`] for an output Table: its outputs, DAG-derived `input_keys`,
/// and reconcile oracle. Returns the tool plus the set of input CELLS it reached
/// (so `build_tools` can mark them fed without re-walking the DAG). Kept separate so
/// `build_tools` stays a thin loop (cog ≤25).
fn build_one_tool(
    manifest: &Manifest,
    dag: &Dag,
    table: &OutputTable,
    input_cells: &HashSet<String>,
    input_key_of: &BTreeMap<String, String>,
) -> (Tool, BTreeSet<String>) {
    let mut input_keys: BTreeSet<String> = BTreeSet::new();
    let mut reached_cells: BTreeSet<String> = BTreeSet::new();
    let mut outputs = Vec::new();
    let mut oracle = BTreeMap::new();

    for cell_key in &table.output_cells {
        // input_keys = union of this output's upstream input leaves (mapped to keys);
        // reached_cells carries the same leaves at cell granularity for the fed-inputs lint.
        for leaf in upstream_input_leaves(dag, cell_key, input_cells) {
            if let Some(json_key) = input_key_of.get(&leaf) {
                input_keys.insert(json_key.clone());
            }
            reached_cells.insert(leaf);
        }
        // outputs + oracle from the manifest CellRole for this output cell.
        if let Some(role) = role_for_cell(manifest, cell_key) {
            outputs.push(entry(role));
            if let Some(value) = oracle_value(role) {
                oracle.insert(json_key_for_role(role), value);
            }
        }
    }

    let tool = Tool {
        name: table.name.clone(),
        description: table.description.clone(),
        input_keys: input_keys.into_iter().collect(),
        outputs,
        oracle,
    };
    (tool, reached_cells)
}

/// Emit one `WARNING` "feeds no tool" [`LintFinding`] per `Role::Input` cell that is
/// NOT upstream of any tool (§4.2). A constant-only path is NOT flagged (constants
/// are not in `input_cells`). Located on the input cell for BA-actionable repair.
fn feeds_no_tool_findings(
    manifest: &Manifest,
    input_cells: &HashSet<String>,
    fed_inputs: &HashSet<String>,
) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    for cell in manifest.cells.iter().filter(|c| c.role == Role::Input) {
        if input_cells.contains(&cell.cell) && !fed_inputs.contains(&cell.cell) {
            let (sheet, addr) = split_cell_key(&cell.cell);
            findings.push(LintFinding::new(
                Severity::Warning,
                "manifest/input-feeds-no-tool",
                sheet,
                addr,
                format!(
                    "input cell {} is not upstream of any output Table — no served \
                     tool consumes it",
                    cell.cell
                ),
                "remove the unused input row, or reference it from an output Table's \
                 formula so a tool consumes it",
            ));
        }
    }
    findings
}

// ---- per-tool reconcile (WBV2-05, Open-Q2) -----------------------------------

/// One graded output row in a tool's [`Comparison`]: the output `json_key`, the
/// computed value, the authored oracle value, and whether they reconcile within
/// the penny tolerance.
#[derive(Debug, Clone)]
pub struct ComparisonRow {
    /// The output column's served `json_key`.
    pub json_key: String,
    /// The value the bundle IR computed for this output cell.
    pub computed: CellValue,
    /// The authored expected value (the cached `<v>` oracle).
    pub oracle: CellValue,
    /// `true` iff `computed` reconciles with `oracle` within tolerance.
    pub reconciled: bool,
}

/// ONE tool's reconcile grade: its per-output [`ComparisonRow`]s. A tool reconciles
/// iff EVERY graded row reconciles (`is_match`).
#[derive(Debug, Clone, Default)]
pub struct Comparison {
    /// The per-output graded rows (one per oracle-bearing output cell).
    pub rows: Vec<ComparisonRow>,
}

impl Comparison {
    /// `true` iff every graded output row reconciled within tolerance (a tool with
    /// no oracle rows trivially matches).
    #[must_use]
    pub fn is_match(&self) -> bool {
        self.rows.iter().all(|r| r.reconciled)
    }
}

/// Grade ONE tool's computed outputs against ITS OWN output-Table value oracle
/// (Open-Q2): partition the run's `computed` map to ONLY this tool's
/// `outputs[].seed_coord` keys, look up each output's authored oracle from
/// `tool.oracle` (keyed by `json_key`), and grade them with the shared
/// [`within_tol`] penny comparator. An output with no oracle entry contributes no
/// graded row (nothing to reconcile against).
#[must_use]
pub fn comparison_from_outputs_for_tool(
    computed: &BTreeMap<String, CellValue>,
    tool: &Tool,
) -> Comparison {
    let mut rows = Vec::new();
    for entry in &tool.outputs {
        let Some(oracle) = tool.oracle.get(&entry.json_key) else {
            continue;
        };
        let computed_value = computed
            .get(&entry.seed_coord)
            .cloned()
            .unwrap_or(CellValue::Empty);
        let reconciled = within_tol(&computed_value, oracle);
        rows.push(ComparisonRow {
            json_key: entry.json_key.clone(),
            computed: computed_value,
            oracle: oracle.clone(),
            reconciled,
        });
    }
    Comparison { rows }
}

/// The aggregated per-tool reconcile report (WBV2-05): one [`Comparison`] per tool,
/// keyed by the sanitized tool name. The gate's non-zero exit derives from
/// [`ToolReconcileReport::any_mismatch`] (ANY tool mismatch blocks); [`render`] emits
/// one human-readable section per tool with FAILING tools first.
///
/// [`render`]: ToolReconcileReport::render
#[derive(Debug, Clone, Default)]
pub struct ToolReconcileReport {
    /// `(sanitized tool name, that tool's Comparison)`, in build order.
    pub per_tool: Vec<(String, Comparison)>,
}

impl ToolReconcileReport {
    /// `true` iff ANY tool failed to reconcile (the gate's non-zero-exit signal).
    #[must_use]
    pub fn any_mismatch(&self) -> bool {
        self.per_tool.iter().any(|(_, c)| !c.is_match())
    }

    /// Render one section per tool (a tool-name header + its graded rows), FAILING
    /// tools listed first so the operator sees the blocking mismatches at the top.
    #[must_use]
    pub fn render(&self) -> String {
        let mut ordered: Vec<&(String, Comparison)> = self.per_tool.iter().collect();
        // Failing tools (is_match == false) sort before passing tools.
        ordered.sort_by_key(|(_, c)| c.is_match());
        let mut out = String::new();
        for (name, comparison) in ordered {
            let status = if comparison.is_match() {
                "OK"
            } else {
                "MISMATCH"
            };
            out.push_str(&format!("tool `{name}`: {status}\n"));
            for row in &comparison.rows {
                let mark = if row.reconciled { "  ok " } else { "  XX " };
                out.push_str(&format!(
                    "{mark}{}: computed {:?} vs oracle {:?}\n",
                    row.json_key, row.computed, row.oracle
                ));
            }
        }
        out
    }
}

/// Build the aggregated [`ToolReconcileReport`] over every tool in `tools` (Open-Q2):
/// grade each tool's outputs against its own oracle and key the result by the
/// SANITIZED tool name (so a collision/charset issue surfaces here too). The gate
/// derives its non-zero exit from [`ToolReconcileReport::any_mismatch`].
///
/// # Errors
/// Returns `Err` if any tool's raw name is unmappable to the MCP tool-name charset
/// (the same fail-closed reject the served registration applies).
pub fn reconcile_tools(
    computed: &BTreeMap<String, CellValue>,
    tools: &[Tool],
) -> Result<ToolReconcileReport, String> {
    let mut per_tool = Vec::with_capacity(tools.len());
    for tool in tools {
        let name = sanitize_tool_name(&tool.name)
            .map_err(|raw| format!("output Table '{raw}' has no MCP-mappable tool name"))?;
        per_tool.push((name, comparison_from_outputs_for_tool(computed, tool)));
    }
    Ok(ToolReconcileReport { per_tool })
}

// ---- post-sanitize collision lint (WBV2-05, T-100-17) ------------------------

/// Detect output Tables whose names SANITIZE to the same MCP tool name (T-100-17):
/// two distinct source Tables (`Calculate Tax`, `calculate_tax`, `calculate-tax`)
/// that all map to `calculate_tax` would silently collapse into one tool at
/// registration. This runs in the compiler BEFORE tool registration so a collision
/// is a clean, cell-precise compile failure, not a silent last-writer-wins.
///
/// Groups the `output_tables` by their sanitized name; any group with ≥2 source
/// Tables emits ONE `Severity::Error` [`LintFinding`] naming ALL colliding source
/// Tables and locating at each offender's first output cell. An UNMAPPABLE name
/// (empty/all-illegal) emits a separate `tool-name-unmappable` error finding.
#[must_use]
pub fn tool_name_collision_findings(output_tables: &[OutputTable]) -> Vec<LintFinding> {
    let mut by_sanitized: BTreeMap<String, Vec<&OutputTable>> = BTreeMap::new();
    let mut findings = Vec::new();

    for table in output_tables {
        match sanitize_tool_name(&table.name) {
            Ok(name) => by_sanitized.entry(name).or_default().push(table),
            Err(_) => findings.push(unmappable_tool_name_finding(table)),
        }
    }

    for (sanitized, group) in by_sanitized.iter().filter(|(_, g)| g.len() > 1) {
        findings.push(collision_finding(sanitized, group));
    }
    findings
}

/// The located finding for an unmappable output-Table name (no MCP-charset chars).
fn unmappable_tool_name_finding(table: &OutputTable) -> LintFinding {
    let (sheet, addr) = table
        .output_cells
        .first()
        .map_or(("manifest".to_string(), None), |c| split_cell_key(c));
    LintFinding::new(
        Severity::Error,
        "manifest/tool-name-unmappable",
        sheet,
        addr,
        format!(
            "output Table '{}' has no characters mappable to the MCP tool-name charset \
             [a-z0-9_-]; the tool would be uncallable",
            table.name
        ),
        format!(
            "rename output Table '{}' to include at least one ASCII letter or digit",
            table.name
        ),
    )
}

/// The located finding for ≥2 output Tables colliding on one sanitized MCP name.
fn collision_finding(sanitized: &str, group: &[&OutputTable]) -> LintFinding {
    let names: Vec<&str> = group.iter().map(|t| t.name.as_str()).collect();
    let locations: Vec<String> = group
        .iter()
        .filter_map(|t| t.output_cells.first().cloned())
        .collect();
    let (sheet, addr) = group
        .first()
        .and_then(|t| t.output_cells.first())
        .map_or(("manifest".to_string(), None), |c| split_cell_key(c));
    LintFinding::new(
        Severity::Error,
        "manifest/tool-name-collision",
        sheet,
        addr,
        format!(
            "output Tables [{}] all sanitize to the same MCP tool name `{sanitized}` \
             (at {}); a caller could not address them independently",
            names.join(", "),
            locations.join(", ")
        ),
        format!(
            "rename the output Tables [{}] so each sanitizes to a DISTINCT MCP tool name",
            names.join(", ")
        ),
    )
}

/// The authored expected-result ORACLE value for an output cell, when the manifest
/// carries a typed default (the harvested cached `<v>`). Currently sourced from the
/// `InputTier::Variable` default the harvest stamps on output rows is `None`, so this
/// reads the cell's value via the manifest when present; outputs without a recorded
/// value contribute no oracle entry.
fn oracle_value(role: &CellRole) -> Option<CellValue> {
    // Outputs are never tiered; the harvested expected value rides on the role's
    // tier default ONLY for inputs. Output oracle values are supplied by the
    // orchestrator's reconcile partition (Plan 04 wires the cached <v>); here we
    // surface any tier-carried default for symmetry with inputs.
    match &role.tier {
        Some(pmcp_workbook_runtime::InputTier::Variable { default }) => Some(default.clone()),
        Some(pmcp_workbook_runtime::InputTier::BoundedVariable { default, .. }) => {
            Some(default.clone())
        },
        None => None,
    }
}

/// Split a `sheet!addr` cell key into `(sheet, Some(addr))` for a located finding;
/// a key without `!` locates at the sheet level (`None` addr).
fn split_cell_key(cell: &str) -> (String, Option<String>) {
    match cell.split_once('!') {
        Some((sheet, addr)) => (sheet.to_string(), Some(addr.to_string())),
        None => (cell.to_string(), None),
    }
}

/// Build the [`CellMap`] from a (tier-ratified) [`Manifest`] — the TRANSITIONAL
/// single-tool path (Plan 03→04).
///
/// For each `Role::Input` cell derives a shared-pool [`CellEntry`]; all `Role::Output`
/// cells are wrapped in ONE transitional [`Tool`] (so the existing single-tool emit +
/// served call sites keep working until Plan 04 wires the multi-tool [`build_tools`]
/// fan-out). Fails loud (returns `Err`) ONLY if the manifest declares NO `Role::Output`
/// cell — a served workbook with no output cannot answer a `calculate`.
///
/// # Errors
/// Returns an error string if the manifest declares no `Role::Output` cell.
pub fn build_cell_map(manifest: &Manifest) -> Result<CellMap, String> {
    let inputs = shared_inputs(manifest);
    let mut outputs = Vec::new();

    for role in &manifest.cells {
        if matches!(role.role, Role::Output) {
            outputs.push(entry(role));
        }
    }

    if outputs.is_empty() {
        return Err(
            "the manifest declares no Role::Output cell — a served workbook must \
             have at least one output to answer a calculate"
                .to_string(),
        );
    }

    // Single-tool projection: wrap all outputs in ONE tool (the named-range compile
    // orchestrator's transitional path). The per-Table `build_tools` fan-out (with the
    // harvested tool name/description) is the multi-tool primitive the served side uses.
    Ok(CellMap {
        inputs,
        tools: vec![Tool {
            name: manifest.workflow.clone(),
            description: None,
            input_keys: Vec::new(),
            outputs,
            oracle: BTreeMap::new(),
        }],
    })
}

/// Build a [`CellEntry`] for a role-bearing cell: the JSON key is the runtime's
/// shared [`json_key_for_role`] precedence (name → meaning → cell key).
///
/// `pub(crate)` so the multi-tool [`emit_bundle`](crate::artifact::emit_bundle)
/// branch can derive the shared-input pool the SAME way [`build_cell_map`] does
/// (one definition — the per-input `CellEntry` shape cannot drift between the
/// single-tool fallback and the `build_tools` fan-out).
pub(crate) fn entry(role: &CellRole) -> CellEntry {
    CellEntry {
        json_key: json_key_for_role(role),
        seed_coord: role.cell.clone(),
        unit: role.unit.clone(),
    }
}

/// The shared-input pool [`CellEntry`]s: one [`entry`] per `Role::Input` cell, in
/// manifest order. The SINGLE source both the single-tool [`build_cell_map`]
/// fallback and the multi-tool [`emit_bundle`](crate::artifact::emit_bundle) branch
/// derive `CellMap.inputs` from, so the served shared-input pool is identical on
/// both paths.
pub(crate) fn shared_inputs(manifest: &Manifest) -> Vec<CellEntry> {
    manifest
        .cells
        .iter()
        .filter(|role| matches!(role.role, Role::Input))
        .map(entry)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_workbook_runtime::Dtype;

    fn role(
        cell: &str,
        r: Role,
        name: Option<&str>,
        meaning: Option<&str>,
        unit: Option<&str>,
    ) -> CellRole {
        CellRole {
            cell: cell.to_string(),
            role: r,
            name: name.map(str::to_string),
            unit: unit.map(str::to_string),
            meaning: meaning.map(str::to_string),
            dtype: Dtype::Number,
            colour_evidence: None,
            source: "test".to_string(),
            notes: None,
            tier: None,
            allowed_values: None,
        }
    }

    fn manifest_with(cells: Vec<CellRole>) -> Manifest {
        Manifest {
            schema_version: 1,
            workflow: "tax-calc".to_string(),
            workbook_hash: None,
            ratified: true,
            ratified_by: None,
            ratified_at: None,
            cells,
            loop_block: None,
            governed_data: vec![],
            changelog: vec![],
            capability_calls: vec![],
            annotations: vec![],
        }
    }

    #[test]
    fn cell_map_maps_inputs_and_outputs() {
        let manifest = manifest_with(vec![
            role(
                "1_Inputs!B2",
                Role::Input,
                Some("in_gross_income"),
                Some("Gross income"),
                Some("USD"),
            ),
            role("2_Constants!B2", Role::Constant, None, None, None),
            role(
                "3_Outputs!B3",
                Role::Output,
                Some("out_tax_owed"),
                Some("Tax owed"),
                Some("USD"),
            ),
        ]);
        let map = build_cell_map(&manifest).expect("build cell map");
        assert_eq!(map.inputs.len(), 1, "one Role::Input entry");
        // F3: the served json_key STRIPS the in_/out_ governance prefix that the
        // workbook author uses on the named range (`in_gross_income` →
        // `gross_income`); role.name itself stays prefixed for matching.
        assert_eq!(map.inputs[0].json_key, "gross_income");
        assert_eq!(map.inputs[0].seed_coord, "1_Inputs!B2");
        assert_eq!(map.inputs[0].unit.as_deref(), Some("USD"));
        let outputs = &map.tools[0].outputs;
        assert_eq!(outputs.len(), 1, "one Role::Output entry");
        assert_eq!(outputs[0].json_key, "tax_owed");
        assert_eq!(outputs[0].seed_coord, "3_Outputs!B3");
    }

    #[test]
    fn build_cell_map_fails_loud_without_any_output() {
        let manifest = manifest_with(vec![role(
            "1_Inputs!B2",
            Role::Input,
            Some("in_area"),
            None,
            Some("m2"),
        )]);
        let err = build_cell_map(&manifest).expect_err("no output => Err");
        assert!(
            err.contains("Role::Output"),
            "fail-loud message names the gap: {err}"
        );
    }

    #[test]
    fn json_key_falls_back_to_meaning_then_cell() {
        let manifest = manifest_with(vec![
            // name absent → meaning used.
            role(
                "1_Inputs!B3",
                Role::Input,
                None,
                Some("Filing status"),
                None,
            ),
            // name + meaning absent → cell key used.
            role("3_Outputs!B2", Role::Output, None, None, Some("USD")),
        ]);
        let map = build_cell_map(&manifest).expect("build");
        assert_eq!(map.inputs[0].json_key, "Filing status");
        let outputs = &map.tools[0].outputs;
        assert_eq!(outputs[0].json_key, "3_Outputs!B2");
    }

    #[test]
    fn no_privileged_headline_output() {
        // §5 generalization: TWO outputs both land in `outputs` with no single
        // one elevated (the lighthouse supply_total_cell field is gone).
        let manifest = manifest_with(vec![
            role(
                "3_Outputs!B2",
                Role::Output,
                Some("out_taxable_income"),
                None,
                Some("USD"),
            ),
            role(
                "3_Outputs!B3",
                Role::Output,
                Some("out_tax_owed"),
                None,
                Some("USD"),
            ),
        ]);
        let map = build_cell_map(&manifest).expect("build");
        let outputs = &map.tools[0].outputs;
        assert_eq!(outputs.len(), 2, "both outputs are first-class");
    }

    // ---- build_tools (WBV2-03 §4.2 — DAG-derived per-Table input schemas) ----

    use pmcp_workbook_runtime::{CellValue, Dag, InputTier, Severity};

    /// The §4.2 motivating manifest: three inputs (income, filing, withheld) + two
    /// output Tables (Calculate_Tax over income+filing; Estimate_Refund over
    /// income+filing+withheld via a shared `taxable` intermediate).
    fn motivating_manifest() -> Manifest {
        manifest_with(vec![
            role("In!income", Role::Input, Some("income"), None, Some("USD")),
            role("In!filing", Role::Input, Some("filing"), None, None),
            role(
                "In!withheld",
                Role::Input,
                Some("withheld"),
                None,
                Some("USD"),
            ),
            // outputs carry an authored expected value via a Variable tier default
            // (stand-in for the harvested cached <v> oracle until Plan 04 wiring).
            output_role("Calc!tax", "tax", CellValue::Number(18241.0)),
            output_role("Calc!refund", "refund", CellValue::Number(-3241.0)),
        ])
    }

    fn output_role(cell: &str, name: &str, oracle: CellValue) -> CellRole {
        let mut r = role(cell, Role::Output, Some(name), None, Some("USD"));
        r.tier = Some(InputTier::Variable { default: oracle });
        r
    }

    /// The §4.2 DAG: tax <- taxable <- (income, filing); refund <- (taxable, withheld).
    fn motivating_dag() -> Dag {
        let mut dag = Dag::new();
        dag.add_edge("Calc!tax", "Calc!taxable");
        dag.add_edge("Calc!taxable", "In!income");
        dag.add_edge("Calc!taxable", "In!filing");
        dag.add_edge("Calc!refund", "Calc!taxable");
        dag.add_edge("Calc!refund", "In!withheld");
        dag
    }

    #[test]
    fn build_tools_derives_two_tools_with_minimal_input_keys() {
        let manifest = motivating_manifest();
        let dag = motivating_dag();
        let tables = vec![
            OutputTable {
                name: "Calculate_Tax".to_string(),
                description: Some("Compute tax".to_string()),
                output_cells: vec!["Calc!tax".to_string()],
            },
            OutputTable {
                name: "Estimate_Refund".to_string(),
                description: None,
                output_cells: vec!["Calc!refund".to_string()],
            },
        ];
        let (tools, findings) = build_tools(&manifest, &dag, &tables).expect("build tools");
        assert_eq!(tools.len(), 2, "two output Tables → two Tools");

        let tax = &tools[0];
        assert_eq!(tax.name, "Calculate_Tax");
        assert_eq!(tax.description.as_deref(), Some("Compute tax"));
        assert_eq!(
            tax.input_keys,
            vec!["filing".to_string(), "income".to_string()],
            "Calculate_Tax.input_keys == [filing, income] (sorted; NOT withheld)"
        );

        let refund = &tools[1];
        assert_eq!(refund.name, "Estimate_Refund");
        assert_eq!(
            refund.input_keys,
            vec![
                "filing".to_string(),
                "income".to_string(),
                "withheld".to_string()
            ],
            "Estimate_Refund.input_keys == [filing, income, withheld]"
        );
        // No input feeds no tool here.
        assert!(
            findings.is_empty(),
            "every input feeds a tool: {findings:?}"
        );
    }

    #[test]
    fn build_tools_oracle_carries_authored_expected_per_tool() {
        let manifest = motivating_manifest();
        let dag = motivating_dag();
        let tables = vec![OutputTable {
            name: "Calculate_Tax".to_string(),
            description: None,
            output_cells: vec!["Calc!tax".to_string()],
        }];
        let (tools, _) = build_tools(&manifest, &dag, &tables).expect("build tools");
        assert_eq!(
            tools[0].oracle.get("tax"),
            Some(&CellValue::Number(18241.0)),
            "the tool's oracle carries its output's authored expected value"
        );
    }

    #[test]
    fn build_tools_fails_loud_on_zero_output_tables() {
        let manifest = motivating_manifest();
        let dag = motivating_dag();
        let err = build_tools(&manifest, &dag, &[]).expect_err("zero Tables => Err");
        assert!(
            err.contains("no output Table"),
            "fail-loud message names the gap: {err}"
        );
    }

    #[test]
    fn build_tools_flags_input_feeding_no_tool_but_not_constant_only() {
        // `orphan` is a Role::Input that NO output Table references → "feeds no tool".
        // `Const!base` is a constant on a path → naturally excluded, NO lint.
        let manifest = manifest_with(vec![
            role("In!income", Role::Input, Some("income"), None, None),
            role("In!orphan", Role::Input, Some("orphan"), None, None),
            role("Const!base", Role::Constant, None, None, None),
            output_role("Calc!out", "answer", CellValue::Number(1.0)),
        ]);
        let mut dag = Dag::new();
        dag.add_edge("Calc!out", "In!income");
        dag.add_edge("Calc!out", "Const!base"); // constant-only contributes nothing
                                                // In!orphan is never referenced by any output.
        let tables = vec![OutputTable {
            name: "T".to_string(),
            description: None,
            output_cells: vec!["Calc!out".to_string()],
        }];
        let (tools, findings) = build_tools(&manifest, &dag, &tables).expect("build");
        assert_eq!(
            tools[0].input_keys,
            vec!["income".to_string()],
            "constant-only path excluded; orphan absent (not upstream)"
        );
        assert_eq!(findings.len(), 1, "exactly one feeds-no-tool finding");
        assert_eq!(findings[0].severity, Severity::Warning);
        assert_eq!(findings[0].rule, "manifest/input-feeds-no-tool");
        assert!(
            findings[0].message.contains("In!orphan"),
            "the finding names the orphan input: {}",
            findings[0].message
        );
    }

    // ---- per-tool reconcile (WBV2-05, Open-Q2) ---------------------------

    fn entry(json_key: &str, seed: &str) -> CellEntry {
        CellEntry {
            json_key: json_key.to_string(),
            seed_coord: seed.to_string(),
            unit: None,
        }
    }

    fn tool_with_oracle(name: &str, json_key: &str, seed: &str, oracle: f64) -> Tool {
        let mut o = BTreeMap::new();
        o.insert(json_key.to_string(), CellValue::Number(oracle));
        Tool {
            name: name.to_string(),
            description: None,
            input_keys: vec![],
            outputs: vec![entry(json_key, seed)],
            oracle: o,
        }
    }

    #[test]
    fn comparison_from_outputs_for_tool_grades_against_own_oracle() {
        let tool = tool_with_oracle("Calculate_Tax", "tax", "Calc!B3", 18241.0);
        let mut computed = BTreeMap::new();
        computed.insert("Calc!B3".to_string(), CellValue::Number(18241.0));
        let cmp = comparison_from_outputs_for_tool(&computed, &tool);
        assert!(cmp.is_match(), "an exact oracle match reconciles");
        assert_eq!(cmp.rows.len(), 1);
        assert!(cmp.rows[0].reconciled);
    }

    #[test]
    fn comparison_from_outputs_for_tool_detects_wrong_oracle() {
        let tool = tool_with_oracle("Calculate_Tax", "tax", "Calc!B3", 18241.0);
        let mut computed = BTreeMap::new();
        computed.insert("Calc!B3".to_string(), CellValue::Number(99999.0));
        let cmp = comparison_from_outputs_for_tool(&computed, &tool);
        assert!(!cmp.is_match(), "a wrong computed value fails reconcile");
    }

    #[test]
    fn reconcile_tools_any_mismatch_blocks_on_one_bad_tool() {
        // Two tools; ONE has a wrong oracle → any_mismatch() is true.
        let good = tool_with_oracle("Calculate_Tax", "tax", "Calc!B3", 100.0);
        let bad = tool_with_oracle("Estimate_Refund", "refund", "Calc!B4", 50.0);
        let mut computed = BTreeMap::new();
        computed.insert("Calc!B3".to_string(), CellValue::Number(100.0));
        computed.insert("Calc!B4".to_string(), CellValue::Number(999.0)); // != 50
        let report = reconcile_tools(&computed, &[good, bad]).expect("reconcile");
        assert!(
            report.any_mismatch(),
            "one bad tool makes the aggregated report mismatch (non-zero gate exit)"
        );
        // render() lists the FAILING tool first.
        let rendered = report.render();
        let first_line = rendered.lines().next().unwrap_or("");
        assert!(
            first_line.contains("estimate_refund") && first_line.contains("MISMATCH"),
            "failing tool rendered first: {rendered}"
        );
    }

    #[test]
    fn reconcile_tools_all_match_is_clean() {
        let a = tool_with_oracle("A", "x", "S!A1", 1.0);
        let b = tool_with_oracle("B", "y", "S!A2", 2.0);
        let mut computed = BTreeMap::new();
        computed.insert("S!A1".to_string(), CellValue::Number(1.0));
        computed.insert("S!A2".to_string(), CellValue::Number(2.0));
        let report = reconcile_tools(&computed, &[a, b]).expect("reconcile");
        assert!(!report.any_mismatch(), "all tools reconcile → clean gate");
    }

    // ---- post-sanitize collision lint (WBV2-05, T-100-17) ----------------

    fn out_table(name: &str, cell: &str) -> OutputTable {
        OutputTable {
            name: name.to_string(),
            description: None,
            output_cells: vec![cell.to_string()],
        }
    }

    #[test]
    fn collision_lint_flags_tables_sanitizing_to_same_name() {
        // Per the LOCKED sanitize semantics: a space → `_`, but a literal `-` is a
        // LEGAL char (kept verbatim). So "Calculate Tax" and "calculate_tax" both
        // sanitize to `calculate_tax` (a COLLISION), while "calculate-tax" stays
        // distinct (`calculate-tax`). The collision finding names BOTH underscore
        // offenders and their cell locations.
        let tables = vec![
            out_table("Calculate Tax", "3_Out!B2"),
            out_table("calculate_tax", "3_Out!B3"),
            out_table("calculate-tax", "3_Out!B4"),
        ];
        let findings = tool_name_collision_findings(&tables);
        assert_eq!(
            findings.len(),
            1,
            "exactly one collision (the two underscore-mapping Tables); calculate-tax is distinct"
        );
        assert_eq!(findings[0].rule, "manifest/tool-name-collision");
        assert_eq!(findings[0].severity, Severity::Error);
        for name in ["Calculate Tax", "calculate_tax"] {
            assert!(
                findings[0].message.contains(name),
                "the collision names both offenders ({name}): {}",
                findings[0].message
            );
        }
        // Locates at each offending Table's output cell.
        assert!(findings[0].message.contains("3_Out!B2"));
        assert!(findings[0].message.contains("3_Out!B3"));
    }

    #[test]
    fn collision_lint_passes_distinct_names() {
        let tables = vec![
            out_table("Calculate Tax", "3_Out!B2"),
            out_table("Estimate Refund", "3_Out!B3"),
        ];
        let findings = tool_name_collision_findings(&tables);
        assert!(
            findings.is_empty(),
            "two distinct sanitized names do not collide: {findings:?}"
        );
    }

    #[test]
    fn collision_lint_flags_unmappable_name() {
        let tables = vec![out_table("@@@", "3_Out!B2")];
        let findings = tool_name_collision_findings(&tables);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "manifest/tool-name-unmappable");
        assert_eq!(findings[0].severity, Severity::Error);
    }
}
