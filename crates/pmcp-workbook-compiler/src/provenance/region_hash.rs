//! The four region hashes.
//!
//! Computes [`RegionHashes`] over a [`WorkbookMap`] partitioned by the
//! [`Manifest`] roles. It REUSES the single length-prefixed-sha256
//! canonicalization from the runtime ([`pmcp_workbook_runtime::update_field`]) —
//! NOT a second hand-rolled digest — so the tab/newline boundary-forgery hazard
//! is solved once.
//!
//! # Region map
//!
//! | Region     | Source cells              | Hash covers           |
//! |------------|---------------------------|-----------------------|
//! | `inputs`   | manifest role = input     | **value only**        |
//! | `data`     | manifest role = constant  | **value only**        |
//! | `outputs`  | manifest role = output    | **value only**        |
//! | `formulas` | every `is_formula` cell   | **formula text only** |
//!
//! A formula cell appears in BOTH its role region (value) AND `formulas`
//! (formula text) — orthogonal projections. Within each region cells are SORTED
//! by `cell_key` before feeding; each field is fed length-prefixed via
//! `update_field` with a region-specific domain tag.
//!
//! # No-roles is a `Result::Err`, never a panic
//!
//! A manifest carrying ZERO `CellRole` rows yields
//! `Err(LintFinding{ rule: "oracle/missing-manifest", … })` — region hashing
//! requires roles, so a role-less manifest is refused at this boundary (no
//! hidden empty-hash path, no panic). An EMPTY *region* (roles present but zero
//! cells partition into it — or zero formula cells exist) is DIFFERENT again: it
//! is recorded EXPLICITLY as `None`, never as the SHA-256 of empty input.

use pmcp_workbook_runtime::update_field;
use sha2::{Digest, Sha256};

use crate::ingest::{cell_key, WorkbookMap};
use crate::provenance::RegionHashes;
use crate::{LintFinding, Manifest, Role, Severity};

/// Sentinel fed for a manifest role-cell whose `cell_key` is ABSENT from the
/// workbook map. A real cached `<v>` is the cell's literal value and can never
/// be this domain-tagged marker, so an absent role cell and a present-but-empty
/// (`""`) one always hash differently. The `\u{0}`-fenced tag cannot appear in a
/// spreadsheet value.
const ABSENT_ROLE_CELL: &str = "\u{0}oracle:absent-role-cell\u{0}";

/// Compute the four region hashes over `map`, partitioned by the `manifest`
/// roles.
///
/// Returns `Err(LintFinding{ rule: "oracle/missing-manifest" })` when `manifest`
/// carries NO `CellRole` rows (region hashing requires roles — a role-less
/// manifest is refused here, never panicked, never an empty-hash path).
/// Otherwise partitions cells by [`Role`] (`Input → inputs`, `Constant → data`,
/// `Output → outputs`, all VALUE only) plus every `is_formula` cell into
/// `formulas` (FORMULA TEXT only). Each region is sorted by `cell_key` and fed
/// length-prefixed via the reused [`pmcp_workbook_runtime::update_field`].
///
/// The `Err` variant is `LintFinding` BY CONTRACT (the gate COLLECTS this
/// finding into its fail-closed `Vec<LintFinding>` so a role-less manifest is a
/// refusal, not a panic). `LintFinding` is a flat located finding, not an error
/// chain; `clippy::result_large_err` is allowed here because the contract is the
/// located finding itself, never a boxed indirection.
#[allow(clippy::result_large_err)]
pub fn compute_region_hashes(
    map: &WorkbookMap,
    manifest: &Manifest,
) -> Result<RegionHashes, LintFinding> {
    // A role-less manifest is REFUSED here (no panic, no empty-hash).
    if manifest.cells.is_empty() {
        let sheet = super::gate::workbook_sheet(map);
        return Err(LintFinding::new(
            Severity::Error,
            "oracle/missing-manifest",
            sheet,
            None,
            "no ratified manifest roles — region hashing requires a ratified \
             manifest (cell roles) before oracle ingest",
            "Synthesize a manifest and ratify it before running the oracle \
             freshness gate.",
        ));
    }

    // Build a cell_key → &CellRecord lookup so role-cells join the cell map by
    // their `sheet!addr` key (CellRole.cell is already that key).
    let mut by_key: std::collections::HashMap<String, &crate::ingest::CellRecord> =
        std::collections::HashMap::new();
    for sheet in &map.sheets {
        for cell in &sheet.cells {
            by_key.insert(cell_key(&sheet.name, &cell.addr), cell);
        }
    }

    // Partition role-cells by Role; collect (cell_key, field-bytes) for the three
    // value regions. A role-cell whose key is ABSENT from the map must NOT hash
    // identically to a present-but-EMPTY-valued cell — the absent case feeds a
    // distinct, non-forgeable sentinel; present-but-empty stays `""`.
    let mut inputs: Vec<(String, String)> = Vec::new();
    let mut data: Vec<(String, String)> = Vec::new();
    let mut outputs: Vec<(String, String)> = Vec::new();
    for role_cell in &manifest.cells {
        let value = match by_key.get(&role_cell.cell) {
            Some(c) => c.value.clone().unwrap_or_default(),
            None => ABSENT_ROLE_CELL.to_string(),
        };
        let entry = (role_cell.cell.clone(), value);
        match role_cell.role {
            Role::Input => inputs.push(entry),
            Role::Constant => data.push(entry),
            Role::Output => outputs.push(entry),
            // A Role::Formula role cell is hashed via the `formulas` region below
            // (its formula text), not as a value region.
            Role::Formula => {},
        }
    }

    // The `formulas` region is EVERY is_formula cell (independent of manifest
    // role): formula TEXT only, never the cached `<v>`.
    let mut formulas: Vec<(String, String)> = Vec::new();
    for sheet in &map.sheets {
        for cell in &sheet.cells {
            if cell.is_formula {
                let text = cell.formula.clone().unwrap_or_default();
                formulas.push((cell_key(&sheet.name, &cell.addr), text));
            }
        }
    }

    // An EMPTY partition is recorded EXPLICITLY as None. Hashing it would produce
    // the SHA-256 of empty input — a constant that never flips on any cell change
    // yet reads as "a real hash".
    Ok(RegionHashes {
        inputs: hash_nonempty_region(b"inputs.value", &mut inputs),
        data: hash_nonempty_region(b"data.value", &mut data),
        outputs: hash_nonempty_region(b"outputs.value", &mut outputs),
        formulas: hash_nonempty_region(b"formulas.f", &mut formulas),
    })
}

/// Hash one region, recording an EMPTY partition as `None`: a zero-cell region
/// must never fold to the empty-input SHA-256. A non-empty region yields
/// `Some(hash_region(..))`.
fn hash_nonempty_region(domain_tag: &[u8], cells: &mut [(String, String)]) -> Option<String> {
    if cells.is_empty() {
        None
    } else {
        Some(hash_region(domain_tag, cells))
    }
}

/// Hash one NON-EMPTY region: SORT `(cell_key, field)` pairs by `cell_key` (so
/// the digest is independent of iteration order), then feed each `cell_key` +
/// `field` length-prefixed via the reused `update_field` with `domain_tag`.
fn hash_region(domain_tag: &[u8], cells: &mut [(String, String)]) -> String {
    cells.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hasher = Sha256::new();
    for (key, field) in cells.iter() {
        // Domain-separate the region (so a value in `inputs` cannot collide with
        // the same bytes in `data`), then feed the located key + the field.
        update_field(&mut hasher, domain_tag, key.as_bytes());
        update_field(&mut hasher, domain_tag, field.as_bytes());
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::{CellRecord, FormulaKind, SheetRecord};
    use crate::{CellRole, Dtype};

    fn cell(addr: &str, value: Option<&str>, formula: Option<&str>) -> CellRecord {
        CellRecord {
            addr: addr.to_string(),
            formula: formula.map(|s| s.to_string()),
            value: value.map(|s| s.to_string()),
            fill_argb: None,
            font_argb: None,
            number_format: None,
            is_formula: formula.is_some(),
            formula_kind: FormulaKind::Normal,
        }
    }

    fn sheet(name: &str, cells: Vec<CellRecord>) -> SheetRecord {
        SheetRecord {
            name: name.to_string(),
            state: "visible".to_string(),
            hidden_rows: vec![],
            hidden_cols: vec![],
            col_widths: vec![],
            merges: vec![],
            cf_ranges: vec![],
            tables: vec![],
            data_validations: vec![],
            notes: vec![],
            cells,
        }
    }

    fn map_with(cells: Vec<CellRecord>) -> WorkbookMap {
        WorkbookMap {
            sheets: vec![sheet("S", cells)],
            defined_names: vec![],
            external_links: vec![],
            has_macros: false,
            source_extension: "xlsx".to_string(),
            save_timestamp: None,
        }
    }

    fn role(cell: &str, role: Role) -> CellRole {
        CellRole {
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
        }
    }

    pub(super) fn manifest_with(cells: Vec<CellRole>) -> Manifest {
        Manifest {
            schema_version: 1,
            workflow: "tax-calc".to_string(),
            workbook_hash: None,
            ratified: true,
            ratified_by: None,
            ratified_at: None,
            cells,
            loop_block: None,
            governed_data: Vec::new(),
            changelog: vec![],
            capability_calls: vec![],
            annotations: vec![],
        }
    }

    #[test]
    fn region_hashes_are_stable_across_runs() {
        let map = map_with(vec![
            cell("A1", Some("10"), None),
            cell("B1", Some("0.37"), None),
            cell("C1", Some("99"), None),
            cell("D1", Some("=A1*B1"), Some("A1*B1")),
        ]);
        let manifest = manifest_with(vec![
            role("S!A1", Role::Input),
            role("S!B1", Role::Constant),
            role("S!C1", Role::Output),
        ]);
        let a = compute_region_hashes(&map, &manifest).expect("hash a");
        let b = compute_region_hashes(&map, &manifest).expect("hash b");
        assert_eq!(a, b, "region hashes must be stable across runs");
        for (region, h) in [
            ("inputs", a.inputs.as_deref()),
            ("data", a.data.as_deref()),
            ("outputs", a.outputs.as_deref()),
            ("formulas", a.formulas.as_deref()),
        ] {
            let h = h.unwrap_or_else(|| panic!("{region} is non-empty => Some"));
            assert_eq!(h.len(), 64, "each region hash is a 64-char sha256 hex");
        }
    }

    #[test]
    fn changing_an_input_value_flips_inputs_not_formulas() {
        let manifest = manifest_with(vec![role("S!A1", Role::Input)]);
        let base = compute_region_hashes(
            &map_with(vec![
                cell("A1", Some("10"), None),
                cell("D1", Some("=A1"), Some("A1")),
            ]),
            &manifest,
        )
        .expect("base");
        let changed = compute_region_hashes(
            &map_with(vec![
                cell("A1", Some("11"), None),
                cell("D1", Some("=A1"), Some("A1")),
            ]),
            &manifest,
        )
        .expect("changed");
        assert_ne!(
            base.inputs, changed.inputs,
            "input value change flips inputs"
        );
        assert_eq!(
            base.formulas, changed.formulas,
            "input value change does NOT touch formulas"
        );
    }

    #[test]
    fn changing_a_formula_text_flips_formulas_not_value_regions() {
        let manifest = manifest_with(vec![role("S!A1", Role::Input)]);
        let base = compute_region_hashes(
            &map_with(vec![
                cell("A1", Some("10"), None),
                cell("D1", Some("=A1"), Some("A1")),
            ]),
            &manifest,
        )
        .expect("base");
        let changed = compute_region_hashes(
            &map_with(vec![
                cell("A1", Some("10"), None),
                cell("D1", Some("=A1*2"), Some("A1*2")),
            ]),
            &manifest,
        )
        .expect("changed");
        assert_ne!(
            base.formulas, changed.formulas,
            "formula text change flips formulas"
        );
        assert_eq!(
            base.inputs, changed.inputs,
            "formula text change does NOT touch inputs"
        );
    }

    #[test]
    fn role_less_manifest_returns_missing_manifest_finding_not_panic() {
        let map = map_with(vec![cell("A1", Some("10"), None)]);
        let manifest = manifest_with(vec![]); // NO CellRole rows
        let err = compute_region_hashes(&map, &manifest).expect_err("role-less => Err");
        assert_eq!(err.rule, "oracle/missing-manifest");
        assert_eq!(err.severity, Severity::Error);
        assert_eq!(err.cell, None, "workbook-level finding");
    }

    #[test]
    fn absent_role_cell_hashes_differently_from_present_but_empty() {
        let manifest = manifest_with(vec![role("S!A1", Role::Input)]);
        let present_empty =
            compute_region_hashes(&map_with(vec![cell("A1", Some(""), None)]), &manifest)
                .expect("present-but-empty");
        let absent = compute_region_hashes(&map_with(vec![cell("B2", Some("x"), None)]), &manifest)
            .expect("absent");
        assert_ne!(
            present_empty.inputs, absent.inputs,
            "an absent role cell must not hash like a present-but-empty one"
        );
    }

    #[test]
    fn an_empty_outputs_region_is_an_explicit_none_not_a_vacuous_hash() {
        let map = map_with(vec![cell("A1", Some("10"), None)]);
        let manifest = manifest_with(vec![role("S!A1", Role::Input)]);
        let hashes = compute_region_hashes(&map, &manifest).expect("roles present => Ok");
        assert_eq!(
            hashes.outputs, None,
            "an empty outputs partition is the explicit None record"
        );
    }

    #[test]
    fn empty_partitions_are_explicit_none_not_vacuous_hashes() {
        const EMPTY_SHA256: &str =
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let map = map_with(vec![cell("C1", Some("99"), None)]);
        let manifest = manifest_with(vec![role("S!C1", Role::Output)]);
        let hashes = compute_region_hashes(&map, &manifest).expect("roles present => Ok");
        assert_eq!(hashes.inputs, None);
        assert_eq!(hashes.data, None);
        assert_eq!(hashes.formulas, None);
        assert!(
            hashes.outputs.is_some(),
            "the declared output stays covered"
        );

        let full_map = map_with(vec![
            cell("A1", Some("10"), None),
            cell("B1", Some("0.37"), None),
            cell("D1", Some("=A1"), Some("A1")),
        ]);
        let full_manifest = manifest_with(vec![
            role("S!A1", Role::Input),
            role("S!B1", Role::Constant),
        ]);
        let full = compute_region_hashes(&full_map, &full_manifest).expect("ok");
        for (region, h) in [
            ("inputs", full.inputs.as_deref()),
            ("data", full.data.as_deref()),
            ("formulas", full.formulas.as_deref()),
        ] {
            let h = h.unwrap_or_else(|| panic!("{region} declared => Some"));
            assert_eq!(h.len(), 64);
            assert_ne!(h, EMPTY_SHA256, "{region} is never the empty fold");
        }
    }

    #[test]
    fn a_declared_output_cell_yields_a_real_outputs_hash_that_flips_on_change() {
        let manifest = manifest_with(vec![role("S!C1", Role::Output)]);
        let base = compute_region_hashes(&map_with(vec![cell("C1", Some("99"), None)]), &manifest)
            .expect("base");
        let changed =
            compute_region_hashes(&map_with(vec![cell("C1", Some("100"), None)]), &manifest)
                .expect("changed");
        let base_hash = base.outputs.expect("declared output => Some");
        let changed_hash = changed.outputs.expect("declared output => Some");
        assert_eq!(base_hash.len(), 64);
        assert_ne!(
            base_hash, changed_hash,
            "an output-cell value change must flip the outputs hash"
        );
    }
}
