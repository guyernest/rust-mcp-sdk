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
//! TRANSITIONAL (Plan 03→04): [`build_cell_map`] still wraps all outputs in ONE tool
//! (the served call sites read them via the deprecated `.outputs()` accessor) until
//! Plan 04 wires the per-Table [`build_tools`] fan-out into the orchestrator.
//!
//! Built from the (tier-ratified) [`Manifest`] in `emit_bundle`; serialized
//! through the deterministic [`crate::artifact::serialize`] choke point.

use std::collections::{BTreeMap, HashSet};

use pmcp_workbook_runtime::{
    json_key_for_role, upstream_input_leaves, CellRole, CellValue, Dag, LintFinding, Manifest,
    Role, Severity,
};

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
        let tool = build_one_tool(manifest, dag, table, &input_cells, &input_key_of);
        // Re-derive this tool's reached input CELLS to mark them fed (the tool only
        // stores the mapped json_keys, so recompute the cell-level union here).
        for cell in &table.output_cells {
            for leaf in upstream_input_leaves(dag, cell, &input_cells) {
                fed_inputs.insert(leaf);
            }
        }
        tools.push(tool);
    }

    let findings = feeds_no_tool_findings(manifest, &input_cells, &fed_inputs);
    Ok((tools, findings))
}

/// Build ONE [`Tool`] for an output Table: its outputs, DAG-derived `input_keys`,
/// and reconcile oracle. Kept separate so `build_tools` stays a thin loop (cog ≤25).
fn build_one_tool(
    manifest: &Manifest,
    dag: &Dag,
    table: &OutputTable,
    input_cells: &HashSet<String>,
    input_key_of: &BTreeMap<String, String>,
) -> Tool {
    let mut input_keys: BTreeMap<String, ()> = BTreeMap::new();
    let mut outputs = Vec::new();
    let mut oracle = BTreeMap::new();

    for cell_key in &table.output_cells {
        // input_keys = union of this output's upstream input leaves (mapped to keys).
        for leaf in upstream_input_leaves(dag, cell_key, input_cells) {
            if let Some(json_key) = input_key_of.get(&leaf) {
                input_keys.insert(json_key.clone(), ());
            }
        }
        // outputs + oracle from the manifest CellRole for this output cell.
        if let Some(role) = role_for(manifest, cell_key) {
            outputs.push(entry(role));
            if let Some(value) = oracle_value(role) {
                oracle.insert(json_key_for_role(role), value);
            }
        }
    }

    Tool {
        name: table.name.clone(),
        description: table.description.clone(),
        input_keys: input_keys.into_keys().collect(),
        outputs,
        oracle,
    }
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

/// Find the manifest [`CellRole`] for a fully-qualified cell key.
fn role_for<'a>(manifest: &'a Manifest, cell_key: &str) -> Option<&'a CellRole> {
    manifest.cells.iter().find(|c| c.cell == cell_key)
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
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();

    for role in &manifest.cells {
        match role.role {
            Role::Input => inputs.push(entry(role)),
            Role::Output => outputs.push(entry(role)),
            Role::Constant | Role::Formula => {},
        }
    }

    if outputs.is_empty() {
        return Err(
            "the manifest declares no Role::Output cell — a served workbook must \
             have at least one output to answer a calculate"
                .to_string(),
        );
    }

    // TRANSITIONAL (Plan 03→04): wrap all outputs in ONE tool. Plan 04 replaces this
    // single-tool projection with the per-Table `build_tools` fan-out + the harvested
    // tool name/description, and retires the `.outputs()` accessor the served side reads.
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
fn entry(role: &CellRole) -> CellEntry {
    CellEntry {
        json_key: json_key_for_role(role),
        seed_coord: role.cell.clone(),
        unit: role.unit.clone(),
    }
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
        #[allow(deprecated)]
        let outputs = map.outputs();
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
        #[allow(deprecated)]
        let outputs = map.outputs();
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
        #[allow(deprecated)]
        let outputs = map.outputs();
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
}
