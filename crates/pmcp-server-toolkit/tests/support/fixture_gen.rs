//! The synthetic `tax-calc@1.1.0` golden bundle generator (Phase 92 Plan 02,
//! Task 1 — D-01..D-05).
//!
//! [`generate_tax_calc_bundle`] writes the SEVEN bundle members the runtime
//! [`pmcp_workbook_runtime::load_bundle`] verifier expects, built from the
//! runtime's OWN Serialize types + [`pmcp_workbook_runtime::build_bundle_lock`]
//! so the golden `BUNDLE.lock` is byte-identical to what `BundleLoader` recomputes
//! (no integrity false-positive). The workbook is a SYNTHETIC progressive-tax
//! calculator (D-02): enum + numeric inputs across tiers, a governed bracket rate
//! table, MULTIPLE named outputs (no privileged headline — WBSV-01), manifest
//! bracket-boundary annotations (D-18), a full layout descriptor, and a recorded
//! `1.0.0 → 1.1.0` changelog (so `diff_version` serves a real prev→current pair —
//! Codex HIGH #5).
//!
//! # Determinism (Codex MEDIUM #8)
//!
//! Every map-valued artifact field uses a sorted `BTreeMap` (never an
//! unordered-iteration map) — the IR is serialized as a `BTreeMap<String, Cell>`
//! so no iteration order leaks into the bytes. Every artifact is serialized with
//! the SAME `serde_json::to_string_pretty` config, so regeneration is
//! byte-reproducible (the Task-2 byte-stability check enforces this in CI).
//!
//! # Customer-data scrub (D-01 / S-4)
//!
//! Only tax-domain identifiers appear here — zero lighthouse / customer names.
//! The neutral `json_key` field (the post-92-01 rename) is used on every cell
//! entry.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use pmcp_workbook_runtime::bundle_loader::{
    MEMBER_CELL_MAP, MEMBER_CHANGELOG, MEMBER_IR, MEMBER_LAYOUT, MEMBER_LOCK, MEMBER_MANIFEST,
    MEMBER_PARSER_EQUIV,
};
use pmcp_workbook_runtime::changelog::{
    ChangeClass, OutputDelta, OutputMeta, Severity, VersionChangelog,
};
use pmcp_workbook_runtime::manifest_model::{
    AnnotationDecl, CellRole, Dtype, GovernedDatum, InputTier, Manifest, Role,
};
use pmcp_workbook_runtime::render::{CellLayout, LayoutDescriptor, SheetLayout};
use pmcp_workbook_runtime::sheet_ir::value::CellValue;
use pmcp_workbook_runtime::{
    build_bundle_lock, fold_evidence_hash, sha256_hex, BinOp, Cell, CellEntry, CellExpr, CellMap,
    Expr, LAYOUT_DESCRIPTOR_VERSION,
};

/// The neutral bundle identifier (D-17). The lock's `bundle_id` and the
/// manifest's `workflow` MUST agree (the loader's stamp-binding gate).
pub const BUNDLE_ID: &str = "tax-calc";
/// The committed golden version (Codex HIGH #5 — the `@1.1.0` golden carries the
/// recorded `1.0.0 → 1.1.0` changelog).
pub const VERSION: &str = "1.1.0";
/// The version the recorded changelog transitions FROM.
pub const PREV_VERSION: &str = "1.0.0";

/// The input-sheet cell keys.
const CELL_GROSS_INCOME: &str = "1_Inputs!B2";
const CELL_FILING_STATUS: &str = "1_Inputs!B3";
const CELL_DEDUCTIONS: &str = "1_Inputs!B4";
/// The governed bracket rate-table cells.
const CELL_BRACKET1_RATE: &str = "2_Brackets!B2";
const CELL_BRACKET2_RATE: &str = "2_Brackets!B3";
const CELL_BRACKET1_BOUND: &str = "2_Brackets!A2";
const CELL_BRACKET2_BOUND: &str = "2_Brackets!A3";
/// The output-sheet cell keys (multiple named outputs — no headline).
const CELL_TAXABLE_INCOME: &str = "3_Outputs!B2";
const CELL_TAX_OWED: &str = "3_Outputs!B3";
const CELL_EFFECTIVE_RATE: &str = "3_Outputs!B4";
const CELL_MARGINAL_RATE: &str = "3_Outputs!B5";

/// A deterministic pretty-print of any artifact (the SINGLE serializer config —
/// Codex MEDIUM #8). `serde_json::to_string_pretty` is fixed two-space indent,
/// and every map field we build is a `BTreeMap`, so the output is reproducible.
fn to_canonical_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).expect("artifact serializes")
}

/// A literal-number cell (a constant / governed-datum cell carrying no formula).
fn literal_num(key: &str, n: f64) -> (String, Cell) {
    (
        key.to_string(),
        Cell {
            key: key.to_string(),
            expr: CellExpr::Literal(CellValue::Number(n)),
        },
    )
}

/// `left op right` over two cell references.
fn binop_refs(key: &str, left: &str, op: BinOp, right: &str) -> (String, Cell) {
    (
        key.to_string(),
        Cell {
            key: key.to_string(),
            expr: CellExpr::Formula(Expr::BinaryOp {
                left: Box::new(Expr::Ref(left.to_string())),
                op,
                right: Box::new(Expr::Ref(right.to_string())),
            }),
        },
    )
}

/// Build the executable IR as a SORTED `BTreeMap<String, Cell>` (Codex MEDIUM #8 —
/// deterministic key order). The formulas are pure-cell arithmetic computing the
/// outputs from the inputs + the governed bracket rate table:
///
/// - `taxable_income = gross_income - deductions`
/// - `tax_owed       = taxable_income * bracket1_rate`
/// - `effective_rate = tax_owed / gross_income`
/// - `marginal_rate  = bracket2_rate`  (the top applicable bracket rate)
fn build_ir() -> BTreeMap<String, Cell> {
    let mut ir: BTreeMap<String, Cell> = BTreeMap::new();
    // The three Role::Input cells are deliberately ABSENT from the IR (CR-01):
    // validate_input pre-seeds them, and an IR literal would re-seed the bundle's
    // baked-in defaults at walk time, clobbering the caller's values (see the seed
    // contract on `executor::run`). They remain declared in the manifest and
    // listed in cell_map.inputs so validate_input seeds and dtype/enum-gates them.
    // Governed bracket rate table (literal governed-data cells).
    for (k, c) in [
        literal_num(CELL_BRACKET1_BOUND, 0.0),
        literal_num(CELL_BRACKET1_RATE, 0.10),
        literal_num(CELL_BRACKET2_BOUND, 40_000.0),
        literal_num(CELL_BRACKET2_RATE, 0.22),
    ] {
        ir.insert(k, c);
    }
    // Outputs (pure-cell formulas).
    for (k, c) in [
        binop_refs(
            CELL_TAXABLE_INCOME,
            CELL_GROSS_INCOME,
            BinOp::Sub,
            CELL_DEDUCTIONS,
        ),
        binop_refs(
            CELL_TAX_OWED,
            CELL_TAXABLE_INCOME,
            BinOp::Mul,
            CELL_BRACKET1_RATE,
        ),
        binop_refs(
            CELL_EFFECTIVE_RATE,
            CELL_TAX_OWED,
            BinOp::Div,
            CELL_GROSS_INCOME,
        ),
    ] {
        ir.insert(k, c);
    }
    // marginal_rate is a direct reference to the top bracket rate.
    ir.insert(
        CELL_MARGINAL_RATE.to_string(),
        Cell {
            key: CELL_MARGINAL_RATE.to_string(),
            expr: CellExpr::Formula(Expr::Ref(CELL_BRACKET2_RATE.to_string())),
        },
    );
    ir
}

/// One input `CellRole` row, with the given tier and optional enum domain.
fn input_role(
    cell: &str,
    name: &str,
    unit: Option<&str>,
    meaning: &str,
    dtype: Dtype,
    tier: InputTier,
    allowed_values: Option<Vec<String>>,
) -> CellRole {
    CellRole {
        cell: cell.to_string(),
        role: Role::Input,
        name: Some(name.to_string()),
        unit: unit.map(str::to_string),
        meaning: Some(meaning.to_string()),
        dtype,
        colour_evidence: None,
        source: "synthetic-fixture".to_string(),
        notes: None,
        tier: Some(tier),
        allowed_values,
    }
}

/// One output `CellRole` row.
fn output_role(cell: &str, name: &str, unit: &str, meaning: &str) -> CellRole {
    CellRole {
        cell: cell.to_string(),
        role: Role::Output,
        name: Some(name.to_string()),
        unit: Some(unit.to_string()),
        meaning: Some(meaning.to_string()),
        dtype: Dtype::Number,
        colour_evidence: None,
        source: "synthetic-fixture".to_string(),
        notes: None,
        tier: None,
        allowed_values: None,
    }
}

/// Build the logical manifest. `workflow == BUNDLE_ID` (the loader's stamp gate),
/// declares THREE inputs (numeric + enum + numeric across tiers), TWO governed
/// bracket-rate constants, FOUR outputs (no headline), and TWO bracket-boundary
/// annotations (D-18 — prove the generic annotations path).
fn build_manifest(workbook_hash: &str) -> Manifest {
    Manifest {
        schema_version: 1,
        workflow: BUNDLE_ID.to_string(),
        workbook_hash: Some(workbook_hash.to_string()),
        ratified: true,
        ratified_by: Some("synthetic-fixture".to_string()),
        ratified_at: Some("2026-06-10".to_string()),
        cells: vec![
            input_role(
                CELL_GROSS_INCOME,
                "in_gross_income",
                Some("USD"),
                "Gross annual income before deductions",
                Dtype::Number,
                InputTier::Variable {
                    default: CellValue::Number(60_000.0),
                },
                None,
            ),
            input_role(
                CELL_FILING_STATUS,
                "in_filing_status",
                None,
                "Tax filing status",
                Dtype::Text,
                InputTier::Variable {
                    default: CellValue::Text("single".to_string()),
                },
                Some(vec![
                    "single".to_string(),
                    "married_joint".to_string(),
                    "head_of_household".to_string(),
                ]),
            ),
            input_role(
                CELL_DEDUCTIONS,
                "in_deductions",
                Some("USD"),
                "Total itemized or standard deductions",
                Dtype::Number,
                InputTier::Variable {
                    default: CellValue::Number(12_000.0),
                },
                None,
            ),
            output_role(
                CELL_TAXABLE_INCOME,
                "out_taxable_income",
                "USD",
                "Income subject to tax after deductions",
            ),
            output_role(
                CELL_TAX_OWED,
                "out_tax_owed",
                "USD",
                "Total tax owed for the period",
            ),
            output_role(
                CELL_EFFECTIVE_RATE,
                "out_effective_rate",
                "ratio",
                "Tax owed as a fraction of gross income",
            ),
            output_role(
                CELL_MARGINAL_RATE,
                "out_marginal_rate",
                "ratio",
                "The rate applied to the next dollar of income",
            ),
        ],
        loop_block: None,
        governed_data: vec![
            GovernedDatum {
                key: "const_bracket1_rate".to_string(),
                value: CellValue::Number(0.10),
                effective_date: Some("2026-01-01".to_string()),
                approved_by: Some("synthetic-fixture".to_string()),
                provenance: Some("synthetic progressive-tax schedule, bracket 1".to_string()),
            },
            GovernedDatum {
                key: "const_bracket2_rate".to_string(),
                value: CellValue::Number(0.22),
                effective_date: Some("2026-01-01".to_string()),
                approved_by: Some("synthetic-fixture".to_string()),
                provenance: Some("synthetic progressive-tax schedule, bracket 2".to_string()),
            },
        ],
        changelog: vec![],
        capability_calls: vec![],
        annotations: vec![
            AnnotationDecl {
                name: "bracket_boundary_1".to_string(),
                target: CELL_BRACKET1_BOUND.to_string(),
                meaning: "Lower bound of tax bracket 1 (USD); income at/above is taxed at rate 1"
                    .to_string(),
            },
            AnnotationDecl {
                name: "bracket_boundary_2".to_string(),
                target: CELL_BRACKET2_BOUND.to_string(),
                meaning: "Lower bound of tax bracket 2 (USD); income at/above is taxed at rate 2"
                    .to_string(),
            },
        ],
    }
}

/// Build the I/O cell map (NO `supply_total_cell` after 92-01). Uses the neutral
/// `json_key` field on every entry (S-4).
fn build_cell_map() -> CellMap {
    CellMap {
        inputs: vec![
            CellEntry {
                json_key: "gross_income".to_string(),
                seed_coord: CELL_GROSS_INCOME.to_string(),
                unit: Some("USD".to_string()),
            },
            CellEntry {
                json_key: "filing_status".to_string(),
                seed_coord: CELL_FILING_STATUS.to_string(),
                unit: None,
            },
            CellEntry {
                json_key: "deductions".to_string(),
                seed_coord: CELL_DEDUCTIONS.to_string(),
                unit: Some("USD".to_string()),
            },
        ],
        outputs: vec![
            CellEntry {
                json_key: "taxable_income".to_string(),
                seed_coord: CELL_TAXABLE_INCOME.to_string(),
                unit: Some("USD".to_string()),
            },
            CellEntry {
                json_key: "tax_owed".to_string(),
                seed_coord: CELL_TAX_OWED.to_string(),
                unit: Some("USD".to_string()),
            },
            CellEntry {
                json_key: "effective_rate".to_string(),
                seed_coord: CELL_EFFECTIVE_RATE.to_string(),
                unit: Some("ratio".to_string()),
            },
            CellEntry {
                json_key: "marginal_rate".to_string(),
                seed_coord: CELL_MARGINAL_RATE.to_string(),
                unit: Some("ratio".to_string()),
            },
        ],
    }
}

/// One captured layout cell.
fn layout_cell(
    addr: &str,
    formula: Option<&str>,
    value: &str,
    number_format: Option<&str>,
) -> CellLayout {
    CellLayout {
        addr: addr.to_string(),
        formula: formula.map(str::to_string),
        value: Some(value.to_string()),
        number_format: number_format.map(str::to_string),
        fill_argb: None,
        font_argb: None,
    }
}

/// Build the full captured layout descriptor (D-05). `source_workbook_hash` MUST
/// equal the lock's `workbook_hash` (the loader's stamp-binding gate). The three
/// sheets (Inputs / Brackets / Outputs) mirror the IR cells.
fn build_layout(workbook_hash: &str) -> LayoutDescriptor {
    LayoutDescriptor {
        descriptor_version: LAYOUT_DESCRIPTOR_VERSION,
        source_workbook_hash: Some(workbook_hash.to_string()),
        sheets: vec![
            SheetLayout {
                name: "1_Inputs".to_string(),
                hidden: false,
                cells: vec![
                    layout_cell("B2", None, "60000", Some("#,##0.00")),
                    layout_cell("B3", None, "single", None),
                    layout_cell("B4", None, "12000", Some("#,##0.00")),
                ],
                merges: vec![],
                col_widths: vec![(1, 18.0), (2, 14.0)],
                hidden_cols: vec![],
            },
            SheetLayout {
                name: "2_Brackets".to_string(),
                hidden: false,
                cells: vec![
                    layout_cell("A2", None, "0", Some("#,##0.00")),
                    layout_cell("B2", None, "0.1", Some("0.00%")),
                    layout_cell("A3", None, "40000", Some("#,##0.00")),
                    layout_cell("B3", None, "0.22", Some("0.00%")),
                ],
                merges: vec![],
                col_widths: vec![(1, 14.0), (2, 10.0)],
                hidden_cols: vec![],
            },
            SheetLayout {
                name: "3_Outputs".to_string(),
                hidden: false,
                cells: vec![
                    layout_cell(
                        "B2",
                        Some("1_Inputs!B2-1_Inputs!B4"),
                        "48000",
                        Some("#,##0.00"),
                    ),
                    layout_cell(
                        "B3",
                        Some("3_Outputs!B2*2_Brackets!B2"),
                        "4800",
                        Some("#,##0.00"),
                    ),
                    layout_cell(
                        "B4",
                        Some("3_Outputs!B3/1_Inputs!B2"),
                        "0.08",
                        Some("0.00%"),
                    ),
                    layout_cell("B5", Some("2_Brackets!B3"), "0.22", Some("0.00%")),
                ],
                merges: vec![],
                col_widths: vec![(1, 22.0), (2, 14.0)],
                hidden_cols: vec![],
            },
        ],
    }
}

/// Build the recorded `1.0.0 → 1.1.0` changelog (D-15 / Codex HIGH #5). `to_version`
/// MUST equal the lock's `version` (the loader's stamp gate). Records a real
/// per-output delta so `diff_version` serves a meaningful prev→current pair.
fn build_changelog() -> VersionChangelog {
    VersionChangelog {
        from_version: PREV_VERSION.to_string(),
        to_version: VERSION.to_string(),
        deltas: vec![OutputDelta {
            region: "out_marginal_rate".to_string(),
            change_class: ChangeClass::OutputSchema,
            old: OutputMeta {
                meaning: Some("The top bracket rate".to_string()),
                unit: Some("ratio".to_string()),
                provenance: Some("synthetic-fixture".to_string()),
            },
            new: OutputMeta {
                meaning: Some("The rate applied to the next dollar of income".to_string()),
                unit: Some("ratio".to_string()),
                provenance: Some("synthetic-fixture".to_string()),
            },
            severity: Severity::Redefinition,
        }],
        summary: "1.1.0: clarified out_marginal_rate meaning (1 redefinition)".to_string(),
    }
}

/// The parser-equivalence evidence record (a small fixed JSON object). Kept as an
/// owned `BTreeMap` so its key order is deterministic.
fn build_parser_equivalence() -> BTreeMap<String, serde_json::Value> {
    let mut m: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    m.insert("equivalent".to_string(), serde_json::Value::Bool(true));
    m.insert(
        "method".to_string(),
        serde_json::Value::String("synthetic-fixture".to_string()),
    );
    m.insert(
        "checked_cells".to_string(),
        serde_json::Value::Number(11u32.into()),
    );
    m
}

/// Write one member to `out_dir/member`, creating parent dirs as needed.
fn write_member(out_dir: &Path, member: &str, bytes: &str) -> std::io::Result<()> {
    let path = out_dir.join(member);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::File::create(&path)?;
    f.write_all(bytes.as_bytes())?;
    Ok(())
}

/// Generate the full synthetic `tax-calc@1.1.0` golden bundle into `out_dir`.
///
/// Writes the seven members deterministically (`BTreeMap` + a single pretty
/// serializer config — Codex MEDIUM #8) and computes `BUNDLE.lock` via the
/// runtime's own [`build_bundle_lock`] over the EXACT bytes written to disk, so a
/// fresh regeneration is byte-identical and the golden passes `load_bundle`.
///
/// # Errors
///
/// Returns any `std::io` error from creating directories or writing members.
pub fn generate_tax_calc_bundle(out_dir: &Path) -> std::io::Result<()> {
    // The synthetic source-workbook content hash (the provenance anchor; RECORDED,
    // not recomputed from raw workbook bytes — there is no real .xlsx here).
    let workbook_hash = sha256_hex(b"synthetic-tax-calc-source-workbook@1.1.0");

    // Build every artifact via the runtime's OWN Serialize types.
    let ir = build_ir();
    let manifest = build_manifest(&workbook_hash);
    let cell_map = build_cell_map();
    let layout = build_layout(&workbook_hash);
    let changelog = build_changelog();
    let parser_equiv = build_parser_equivalence();

    // Serialize each member ONCE with the single canonical config; these EXACT
    // bytes are what we write AND what we feed the hasher (no re-serialization).
    let ir_json = to_canonical_json(&ir);
    let manifest_json = to_canonical_json(&manifest);
    let cell_map_json = to_canonical_json(&cell_map);
    let layout_json = to_canonical_json(&layout);
    let changelog_json = to_canonical_json(&changelog);
    let parser_equiv_json = to_canonical_json(&parser_equiv);

    // Fold the evidence hash via the runtime's OWN shared fold (Pitfall 2 — the
    // generator and loader fold the identical set byte-for-byte by construction).
    let evidence_hash = fold_evidence_hash(&[
        (MEMBER_CELL_MAP, cell_map_json.as_bytes()),
        (MEMBER_LAYOUT, layout_json.as_bytes()),
        (MEMBER_CHANGELOG, changelog_json.as_bytes()),
        (MEMBER_PARSER_EQUIV, parser_equiv_json.as_bytes()),
    ]);

    // Build the lock via the runtime's own hasher (so the loader's recompute
    // matches byte-for-byte).
    let lock = build_bundle_lock(
        BUNDLE_ID,
        VERSION,
        workbook_hash,
        &ir_json,
        &manifest_json,
        &evidence_hash,
    );
    let lock_json = to_canonical_json(&lock);

    // Write the seven members.
    write_member(out_dir, MEMBER_IR, &ir_json)?;
    write_member(out_dir, MEMBER_MANIFEST, &manifest_json)?;
    write_member(out_dir, MEMBER_CELL_MAP, &cell_map_json)?;
    write_member(out_dir, MEMBER_LAYOUT, &layout_json)?;
    write_member(out_dir, MEMBER_LOCK, &lock_json)?;
    write_member(out_dir, MEMBER_CHANGELOG, &changelog_json)?;
    write_member(out_dir, MEMBER_PARSER_EQUIV, &parser_equiv_json)?;
    Ok(())
}
