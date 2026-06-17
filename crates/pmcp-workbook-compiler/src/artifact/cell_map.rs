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
//! `{inputs, outputs}` shape with NO privileged-headline field: the served
//! all-outputs path iterates [`CellMap::outputs`] independently, so no single
//! output is elevated. `build_cell_map` therefore fails loud ONLY when there is no
//! output at all (a workbook with zero outputs cannot serve a `calculate`), never
//! on the absence of a single named "supply total."
//!
//! Built from the (tier-ratified) [`Manifest`] in `emit_bundle`; serialized
//! through the deterministic [`crate::artifact::serialize`] choke point.

use pmcp_workbook_runtime::{json_key_for_role, CellRole, Manifest, Role};

// Re-export the runtime-safe artifact shapes (the served loader deserializes the
// SAME `CellMap`/`CellEntry`); never re-declared here.
pub use pmcp_workbook_runtime::{CellEntry, CellMap};

/// Build the [`CellMap`] from a (tier-ratified) [`Manifest`].
///
/// For each `Role::Input`/`Role::Output` [`CellRole`] derives a [`CellEntry`]
/// (`json_key` via [`json_key_for_role`], `seed_coord` = the cell key, `unit`). Fails loud
/// (returns `Err`) ONLY if the manifest declares NO `Role::Output` cell — a served
/// workbook with no output cannot answer a `calculate`.
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

    Ok(CellMap { inputs, outputs })
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
        assert_eq!(map.inputs[0].json_key, "in_gross_income");
        assert_eq!(map.inputs[0].seed_coord, "1_Inputs!B2");
        assert_eq!(map.inputs[0].unit.as_deref(), Some("USD"));
        assert_eq!(map.outputs.len(), 1, "one Role::Output entry");
        assert_eq!(map.outputs[0].json_key, "out_tax_owed");
        assert_eq!(map.outputs[0].seed_coord, "3_Outputs!B3");
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
        assert_eq!(map.outputs[0].json_key, "3_Outputs!B2");
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
        assert_eq!(map.outputs.len(), 2, "both outputs are first-class");
    }
}
