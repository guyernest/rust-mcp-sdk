//! The D-03 named-output vs helper-cell severity split (WBCO-04).
//!
//! A penny-reconcile mismatch is graded by WHAT the diverging cell is, not by how
//! big the gap is:
//!
//! - a mismatch on a **NAMED OUTPUT** is an **ERROR** that blocks emit (a published
//!   answer is wrong);
//! - a mismatch on an **intermediate / helper cell** is a located **WARNING** (a
//!   helper divergence that actually matters propagates to a named output, where it
//!   is caught as an error there).
//!
//! "Named output" = a manifest cell whose [`Role`] is [`Role::Output`] (the
//! `out_*` named-range convention resolved at synthesis). Everything else — a
//! `Formula` helper, a `Constant`, an `Input`, or a cell with no manifest row — is a
//! helper for severity purposes.

use pmcp_workbook_runtime::{Manifest, Role, Severity};

/// Grade a reconcile mismatch at `cell_key` into the D-03 severity: [`Severity::Error`]
/// for a NAMED OUTPUT, [`Severity::Warning`] for an intermediate/helper cell.
///
/// The decision keys on the manifest [`Role`] at the cell: only [`Role::Output`] is
/// a named output. A cell with no manifest row is a helper (a Warning) — an absent
/// row never silently upgrades to a blocking error.
#[must_use]
pub fn mismatch_severity(cell_key: &str, manifest: &Manifest) -> Severity {
    if is_named_output(cell_key, manifest) {
        Severity::Error
    } else {
        Severity::Warning
    }
}

/// Whether `cell_key` is a NAMED OUTPUT (a manifest cell with [`Role::Output`]).
#[must_use]
pub fn is_named_output(cell_key: &str, manifest: &Manifest) -> bool {
    manifest
        .cells
        .iter()
        .any(|c| c.cell == cell_key && c.role == Role::Output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_workbook_runtime::{CellRole, Dtype};

    fn manifest_with(cell: &str, role: Role) -> Manifest {
        Manifest {
            schema_version: 1,
            workflow: "wf".to_string(),
            workbook_hash: None,
            ratified: true,
            ratified_by: None,
            ratified_at: None,
            cells: vec![CellRole {
                cell: cell.to_string(),
                role,
                name: None,
                unit: None,
                meaning: None,
                dtype: Dtype::Number,
                colour_evidence: None,
                source: "test".to_string(),
                notes: None,
                tier: None,
                allowed_values: None,
            }],
            loop_block: None,
            governed_data: vec![],
            changelog: vec![],
            capability_calls: vec![],
            annotations: vec![],
        }
    }

    #[test]
    fn a_named_output_mismatch_is_an_error() {
        let m = manifest_with("S!C11", Role::Output);
        assert_eq!(mismatch_severity("S!C11", &m), Severity::Error);
        assert!(is_named_output("S!C11", &m));
    }

    #[test]
    fn a_helper_formula_mismatch_is_a_warning() {
        let m = manifest_with("S!C7", Role::Formula);
        assert_eq!(mismatch_severity("S!C7", &m), Severity::Warning);
        assert!(!is_named_output("S!C7", &m));
    }

    #[test]
    fn a_cell_with_no_manifest_row_is_a_warning_never_a_silent_error() {
        let m = manifest_with("S!C11", Role::Output);
        // A different, un-roled cell is a helper (Warning) — never upgraded.
        assert_eq!(mismatch_severity("S!Z99", &m), Severity::Warning);
    }
}
