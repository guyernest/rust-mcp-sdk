//! The REDEFINITION predicate + changelog projection (GATE-02, D-14).
//!
//! [`diff_outputs`] diffs the named `Role::Output` regions of a prior vs current
//! manifest and emits the [`VersionChangelog`] records. For each changed output it
//! computes the redefinition predicate (D-14):
//!
//! - a change to the declared `meaning`, `unit`, `source` (provenance), OR the
//!   CANONICAL IR sub-DAG identity hash ([`super::ir_subdag_hash`]) ⇒
//!   [`Severity::Redefinition`]
//! - a pure value change with identical schema AND identical identity hash is NOT
//!   emitted here (it lives in the governed-data table / numeric gate)
//!
//! It MIRRORS a schema-diff structure — compute added/removed/changed sets, then
//! render a human-readable summary — WITHOUT adding any external dependency. Each
//! changed output carries the SHARED [`ChangeClass`] enum value (NOT a String).

use std::collections::HashMap;

use pmcp_workbook_runtime::changelog::Severity;
use pmcp_workbook_runtime::manifest_model::{CellRole, Manifest, Role};
use pmcp_workbook_runtime::sheet_ir::Cell;
use pmcp_workbook_runtime::{ChangeClass, OutputDelta, OutputMeta, VersionChangelog};

use super::ir_identity::ir_subdag_hash;

/// Project a [`CellRole`]'s declared schema into an [`OutputMeta`] triple
/// (`meaning`/`unit`/`provenance`) — the redefinition predicate's comparison shape.
fn output_meta(role: &CellRole) -> OutputMeta {
    OutputMeta {
        meaning: role.meaning.clone(),
        unit: role.unit.clone(),
        provenance: Some(role.source.clone()),
    }
}

/// Whether two output metas declare the SAME schema (meaning + unit + provenance).
fn same_schema(a: &OutputMeta, b: &OutputMeta) -> bool {
    a.meaning == b.meaning && a.unit == b.unit && a.provenance == b.provenance
}

/// Diff the named `Role::Output` regions and emit the prev→current
/// [`VersionChangelog`] (D-13/D-15).
#[must_use]
pub fn diff_outputs(
    prev: &Manifest,
    current: &Manifest,
    prev_ir: &HashMap<String, Cell>,
    current_ir: &HashMap<String, Cell>,
    from_version: &str,
    to_version: &str,
) -> VersionChangelog {
    let prev_outputs = index_outputs(prev);
    let cur_outputs = index_outputs(current);

    let mut regions: Vec<&str> = prev_outputs
        .keys()
        .chain(cur_outputs.keys())
        .copied()
        .collect();
    regions.sort_unstable();
    regions.dedup();

    let mut deltas: Vec<OutputDelta> = Vec::new();
    for region in regions {
        let prev_role = prev_outputs.get(region).copied();
        let cur_role = cur_outputs.get(region).copied();

        match (prev_role, cur_role) {
            (Some(p), Some(c)) => {
                let old = output_meta(p);
                let new = output_meta(c);
                let schema_changed = !same_schema(&old, &new);
                let identity_changed =
                    ir_subdag_hash(region, prev_ir) != ir_subdag_hash(region, current_ir);

                if !schema_changed && !identity_changed {
                    // No schema/identity change — a pure value drift lives in the
                    // governed-data table / numeric gate, not here.
                    continue;
                }

                deltas.push(OutputDelta {
                    region: region.to_string(),
                    change_class: ChangeClass::OutputSchema,
                    old,
                    new,
                    severity: Severity::Redefinition,
                });
            },
            (Some(p), None) => {
                deltas.push(OutputDelta {
                    region: region.to_string(),
                    change_class: ChangeClass::OutputSchema,
                    old: output_meta(p),
                    new: OutputMeta {
                        meaning: None,
                        unit: None,
                        provenance: None,
                    },
                    severity: Severity::Redefinition,
                });
            },
            (None, Some(c)) => {
                deltas.push(OutputDelta {
                    region: region.to_string(),
                    change_class: ChangeClass::OutputSchema,
                    old: OutputMeta {
                        meaning: None,
                        unit: None,
                        provenance: None,
                    },
                    new: output_meta(c),
                    severity: Severity::Redefinition,
                });
            },
            (None, None) => {},
        }
    }

    let summary = render_summary(&deltas, from_version, to_version);
    VersionChangelog {
        from_version: from_version.to_string(),
        to_version: to_version.to_string(),
        deltas,
        summary,
    }
}

/// Index a manifest's `Role::Output` cells by their fully-qualified region key.
fn index_outputs(manifest: &Manifest) -> HashMap<&str, &CellRole> {
    manifest
        .cells
        .iter()
        .filter(|c| matches!(c.role, Role::Output))
        .map(|c| (c.cell.as_str(), c))
        .collect()
}

/// Render the BA-readable summary string (D-13).
fn render_summary(deltas: &[OutputDelta], from_version: &str, to_version: &str) -> String {
    if deltas.is_empty() {
        return format!("{from_version} → {to_version}: no output schema or identity changes.");
    }
    let redefinitions = deltas
        .iter()
        .filter(|d| matches!(d.severity, Severity::Redefinition))
        .count();
    let drifts = deltas.len() - redefinitions;

    let mut lines = vec![format!(
        "{from_version} → {to_version}: {} output change(s) — {redefinitions} redefinition(s), \
         {drifts} drift(s).",
        deltas.len()
    )];
    for d in deltas {
        let kind = match d.severity {
            Severity::Redefinition => "REDEFINITION (BA review required)",
            Severity::Drift => "drift",
        };
        lines.push(format!(
            "  - {}: {kind} [meaning {:?}→{:?}, unit {:?}→{:?}, provenance {:?}→{:?}]",
            d.region,
            d.old.meaning,
            d.new.meaning,
            d.old.unit,
            d.new.unit,
            d.old.provenance,
            d.new.provenance
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_workbook_runtime::formula::{BinOp, Expr};
    use pmcp_workbook_runtime::manifest_model::Dtype;
    use pmcp_workbook_runtime::sheet_ir::CellExpr;
    use pmcp_workbook_runtime::CellValue;

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

    fn output(cell: &str, meaning: &str, unit: &str, source: &str) -> CellRole {
        CellRole {
            cell: cell.to_string(),
            role: Role::Output,
            name: Some("out_tax_owed".to_string()),
            unit: Some(unit.to_string()),
            meaning: Some(meaning.to_string()),
            dtype: Dtype::Number,
            colour_evidence: None,
            source: source.to_string(),
            notes: None,
            tier: None,
            allowed_values: None,
        }
    }

    fn ir_for(region: &str, op: BinOp) -> HashMap<String, Cell> {
        let mut ir = HashMap::new();
        ir.insert(
            "S!A1".to_string(),
            Cell {
                key: "S!A1".to_string(),
                expr: CellExpr::Literal(CellValue::Number(10.0)),
            },
        );
        ir.insert(
            region.to_string(),
            Cell {
                key: region.to_string(),
                expr: CellExpr::Formula(Expr::BinaryOp {
                    left: Box::new(Expr::Ref("S!A1".to_string())),
                    op,
                    right: Box::new(Expr::Number(1.05)),
                }),
            },
        );
        ir
    }

    #[test]
    fn meaning_change_is_redefinition_value_change_is_not_emitted() {
        let prev = manifest_with(vec![output(
            "3_Outputs!B3",
            "supply only",
            "USD",
            "colour+guide",
        )]);
        let cur = manifest_with(vec![output(
            "3_Outputs!B3",
            "supply + install",
            "USD",
            "colour+guide",
        )]);
        let ir = ir_for("3_Outputs!B3", BinOp::Mul);
        let cl = diff_outputs(&prev, &cur, &ir, &ir, "1.0.0", "1.1.0");
        assert_eq!(cl.deltas.len(), 1);
        assert_eq!(cl.deltas[0].severity, Severity::Redefinition);
        assert_eq!(cl.deltas[0].change_class, ChangeClass::OutputSchema);

        let same = manifest_with(vec![output(
            "3_Outputs!B3",
            "supply only",
            "USD",
            "colour+guide",
        )]);
        let cl2 = diff_outputs(&prev, &same, &ir, &ir, "1.0.0", "1.1.0");
        assert!(cl2.deltas.is_empty(), "an unchanged output emits no delta");
    }

    #[test]
    fn unit_change_is_redefinition() {
        let prev = manifest_with(vec![output("3_Outputs!B3", "tax", "USD", "colour+guide")]);
        let cur = manifest_with(vec![output("3_Outputs!B3", "tax", "cents", "colour+guide")]);
        let ir = ir_for("3_Outputs!B3", BinOp::Mul);
        let cl = diff_outputs(&prev, &cur, &ir, &ir, "1.0.0", "1.1.0");
        assert_eq!(cl.deltas.len(), 1);
        assert_eq!(cl.deltas[0].severity, Severity::Redefinition);
    }

    #[test]
    fn ir_identity_change_is_redefinition_with_identical_schema() {
        let prev = manifest_with(vec![output("3_Outputs!B3", "tax", "USD", "colour+guide")]);
        let cur = manifest_with(vec![output("3_Outputs!B3", "tax", "USD", "colour+guide")]);
        let prev_ir = ir_for("3_Outputs!B3", BinOp::Mul);
        let cur_ir = ir_for("3_Outputs!B3", BinOp::Add); // different operator
        let cl = diff_outputs(&prev, &cur, &prev_ir, &cur_ir, "1.0.0", "1.1.0");
        assert_eq!(cl.deltas.len(), 1, "an identity change is a delta");
        assert_eq!(
            cl.deltas[0].severity,
            Severity::Redefinition,
            "a formula-identity change with identical schema is a REDEFINITION"
        );
    }

    #[test]
    fn summary_is_a_non_empty_human_readable_string() {
        let prev = manifest_with(vec![output(
            "3_Outputs!B3",
            "supply only",
            "USD",
            "colour+guide",
        )]);
        let cur = manifest_with(vec![output(
            "3_Outputs!B3",
            "supply + install",
            "USD",
            "colour+guide",
        )]);
        let ir = ir_for("3_Outputs!B3", BinOp::Mul);
        let cl = diff_outputs(&prev, &cur, &ir, &ir, "1.0.0", "1.1.0");
        assert!(!cl.summary.is_empty());
        assert!(cl.summary.contains("1.0.0"));
        assert!(cl.summary.contains("1.1.0"));
        assert!(cl.summary.to_lowercase().contains("redefinition"));
    }

    #[test]
    fn added_and_removed_outputs_are_redefinitions() {
        let prev = manifest_with(vec![output("3_Outputs!B3", "tax", "USD", "colour+guide")]);
        let cur = manifest_with(vec![output(
            "3_Outputs!B4",
            "rate",
            "ratio",
            "colour+guide",
        )]);
        let ir = HashMap::new();
        let cl = diff_outputs(&prev, &cur, &ir, &ir, "1.0.0", "1.1.0");
        assert_eq!(cl.deltas.len(), 2);
        assert!(cl
            .deltas
            .iter()
            .all(|d| matches!(d.severity, Severity::Redefinition)));
    }
}
