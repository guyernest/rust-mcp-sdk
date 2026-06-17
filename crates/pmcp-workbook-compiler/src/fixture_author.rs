//! Reusable `#[cfg(test)]` `.xlsx` fixture author with GENUINE Excel identity
//! (WBEX-01 critical-path landmine retirement).
//!
//! # Why this module exists (Pitfall 1 — the .xlsx authoring gap)
//!
//! There is NO in-repo `.xlsx` generator. The committed `tax-calc.xlsx` is an
//! externally-authored binary blob; every fixture the WBEX gates need (the loan
//! workbook in Plan 04, the Excel-quirk corpus in Plan 05) must be authored
//! through ONE proven helper rather than re-discovering the freshness-gate recipe
//! per fixture. This module is that helper.
//!
//! # Genuine Excel identity comes for free from `rust_xlsxwriter` (verified)
//!
//! The provenance gate ([`crate::provenance::gate::classify`]) admits ONLY a
//! workbook whose `docProps/app.xml` carries an anchored
//! `<Application>Microsoft Excel</Application>` AND a positive Excel marker (an
//! `<AppVersion>` build string AND a non-sentinel calcId). `rust_xlsxwriter`
//! 0.95 hard-codes EXACTLY this identity on every save:
//!
//! - `app.rs` writes `<Application>Microsoft Excel</Application>`,
//! - `app.rs` writes `<AppVersion>12.0000</AppVersion>` (a positive marker),
//! - `workbook.rs` writes `<calcPr calcId="124519" fullCalcOnLoad="1"/>`
//!   (`124519` is NOT the umya sentinel `122211`).
//!
//! So an authored workbook classifies [`ProvenanceClass::ExcelTrusted`]
//! WITHOUT any extra [`rust_xlsxwriter::DocProperties`] call — the only residual
//! freshness problem is the `fullCalcOnLoad=1` staleness signal, which the
//! `#[cfg(test)]` trusted-fixture override DEMOTES to a Warning. A umya-authored
//! fixture would classify [`ProvenanceClass::UmyaFabricated`] and be REFUSED
//! (that is the whole point of using `rust_xlsxwriter`, not umya).
//!
//! # The cached `<v>` IS the reconcile oracle
//!
//! Every formula is written via [`rust_xlsxwriter::Formula::set_result`] so the
//! emitted `<v>` carries the AUTHORED cached value. The compiler's reconcile
//! stage grades the executor's recomputation against that cached value — so the
//! cached result is the test oracle (the same mechanism the golden gate uses).
//!
//! # Reproducible, NON-MUTATING generation (Codex HIGH)
//!
//! A normal `cargo test` run NEVER writes into `tests/fixtures/`: the self-tests
//! author into a [`tempfile::TempDir`] only. Committed `.xlsx` fixtures (the
//! loan workbook, the quirk corpus, the leap probe) are written ONLY by the
//! `#[ignore]`d, env-gated [`regenerate_fixtures`] generator below — run
//! intentionally with `PMCP_REGEN_FIXTURES=1 cargo test -p pmcp-workbook-compiler
//! regenerate_fixtures -- --ignored`. Alongside every committed fixture the
//! generator emits a `*.gen.json` metadata sidecar (generator fn, input cells,
//! expected output cells, override reason) so the binary `.xlsx` stays
//! reproducible and traceable.

#![cfg(test)]

use std::path::Path;

use rust_xlsxwriter::{Color, Format, Formula, Workbook, XlsxError};

use crate::provenance::gate::{classify, ProvenanceClass};
use crate::provenance::raw_parts::{read_app_props, read_calc_pr};

/// The dialect colour-role palette ARGBs (mirrors
/// `pmcp-workbook-dialect/src/lib.rs:57-58`): a blue FONT marks an INPUT, a
/// green FILL marks a governed CONSTANT. The manifest synthesis classifies a
/// cell's [`crate::Role`] from these colours alone, so an authored fixture MUST
/// paint its inputs/constants with them to be roled correctly.
pub(crate) const INPUT_FONT_ARGB: u32 = 0x0000_00FF; // FF0000FF blue font → input
pub(crate) const CONSTANT_FILL_ARGB: u32 = 0x00E2_EFDA; // FFE2EFDA green fill → constant

/// The colour role to paint an authored cell with (drives the synth
/// classification — see [`INPUT_FONT_ARGB`] / [`CONSTANT_FILL_ARGB`]).
///
// Why: `Plain`/`Constant` (and `AuthoredCell::Text` below) are the reusable
// author surface the WBEX gates consume next — the loan workbook (Plan 04) paints
// governed constants + text labels, the quirk corpus (Plan 05) uses plain cells.
// This module is the ONE proven author both gates ride; the variants are part of
// its contract, exercised by Plan 04/05's fixtures, not dead.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CellPaint {
    /// No fill / no font colour — a plain label, a formula, or an output cell.
    Plain,
    /// Blue font → the synth classifies this cell `Role::Input`.
    Input,
    /// Green fill → the synth classifies this cell `Role::Constant` (governed).
    Constant,
}

/// A single authored cell: its A1 address plus its kind. The kind carries the
/// value/formula so the author writes the right `rust_xlsxwriter` call AND so the
/// generated metadata can record what each cell is.
///
// Why: the `Text` variant is reusable author surface for Plan 04's loan workbook
// (header/label cells) — part of this module's contract, not dead.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) enum AuthoredCell {
    /// A numeric value cell (`write_number`), painted per [`CellPaint`].
    Number {
        /// A1 address (e.g. `"A1"`).
        addr: &'static str,
        /// The numeric value to write.
        value: f64,
        /// The colour role to paint.
        paint: CellPaint,
    },
    /// A text/label cell (`write_string`), always [`CellPaint::Plain`].
    Text {
        /// A1 address.
        addr: &'static str,
        /// The label text.
        text: &'static str,
    },
    /// A formula cell with an AUTHORED cached result (the reconcile oracle): the
    /// `<v>` carries `cached`, set via [`Formula::set_result`].
    Formula {
        /// A1 address.
        addr: &'static str,
        /// The Excel formula text WITHOUT the leading `=` (rust_xlsxwriter adds
        /// it), e.g. `"A1*2"`.
        formula: &'static str,
        /// The authored cached result string written into `<v>` — the oracle the
        /// reconcile stage grades the recomputation against.
        cached: &'static str,
    },
}

impl AuthoredCell {
    /// The A1 address of this cell.
    fn addr(&self) -> &'static str {
        match self {
            AuthoredCell::Number { addr, .. }
            | AuthoredCell::Text { addr, .. }
            | AuthoredCell::Formula { addr, .. } => addr,
        }
    }
}

/// A `name → A1-target` defined-name (named range). The `out_*` convention drives
/// [`crate::promote_named_outputs`]; a `pmcp_dialect_version` name exercises the
/// WBDL-02 present-path. `target` is the FULLY-QUALIFIED `'Sheet'!$A$1` form
/// `rust_xlsxwriter::Workbook::define_name` expects.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DefinedNameSpec {
    /// The defined-name identifier (e.g. `"out_result"`).
    pub(crate) name: &'static str,
    /// The fully-qualified single-cell target (e.g. `"'Calc'!$B$1"`).
    pub(crate) target: &'static str,
}

/// A reusable workbook author spec consumed by [`author_xlsx`]. One sheet of
/// authored cells plus its workbook-global defined names. (A single sheet covers
/// every fixture this phase needs; a multi-sheet author is a trivial extension if
/// a later plan needs cross-sheet refs.)
#[derive(Debug, Clone)]
pub(crate) struct WorkbookSpec {
    /// The single worksheet name.
    pub(crate) sheet: &'static str,
    /// The authored cells on that sheet.
    pub(crate) cells: Vec<AuthoredCell>,
    /// The workbook-global defined names (named ranges).
    pub(crate) defined_names: Vec<DefinedNameSpec>,
}

/// Author a `.xlsx` at `path` from `spec` using `rust_xlsxwriter` (a pure
/// writer). The saved workbook carries a GENUINE Excel identity (so it classifies
/// [`ProvenanceClass::ExcelTrusted`]) and every formula carries its authored
/// cached `<v>` (the reconcile oracle).
///
/// # Errors
/// Returns the underlying [`XlsxError`] on any write/save failure (test path —
/// the caller `.expect`s it).
pub(crate) fn author_xlsx(path: &Path, spec: &WorkbookSpec) -> Result<(), XlsxError> {
    let palette = CellFormats::new();

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet.set_name(spec.sheet)?;

    for cell in &spec.cells {
        write_cell(worksheet, cell, &palette)?;
    }

    for dn in &spec.defined_names {
        workbook.define_name(dn.name, dn.target)?;
    }

    workbook.save(path)?;
    Ok(())
}

/// The two paint-driven cell formats authored fixtures use: the INPUT font
/// colour and the CONSTANT fill colour. Built once per [`author_xlsx`] call.
struct CellFormats {
    input: Format,
    constant: Format,
}

impl CellFormats {
    fn new() -> Self {
        Self {
            input: Format::new().set_font_color(Color::RGB(INPUT_FONT_ARGB)),
            constant: Format::new().set_background_color(Color::RGB(CONSTANT_FILL_ARGB)),
        }
    }
}

/// Write a single authored cell to `worksheet` at its A1 address, dispatching on
/// the cell kind. Numbers carry their paint-driven format; formulas carry their
/// authored cached `<v>` oracle. Surfaces any underlying [`XlsxError`].
fn write_cell(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    cell: &AuthoredCell,
    palette: &CellFormats,
) -> Result<(), XlsxError> {
    let (row, col) = parse_a1(cell.addr());
    match cell {
        AuthoredCell::Number { value, paint, .. } => {
            write_number_cell(worksheet, row, col, *value, *paint, palette)
        },
        AuthoredCell::Text { text, .. } => worksheet.write_string(row, col, *text).map(|_| ()),
        AuthoredCell::Formula {
            formula, cached, ..
        } => {
            let f = Formula::new(*formula).set_result(*cached);
            worksheet.write_formula(row, col, f).map(|_| ())
        },
    }
}

/// Write a numeric cell, selecting the paint-driven format (INPUT font colour,
/// CONSTANT fill, or PLAIN no-format).
fn write_number_cell(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    col: u16,
    value: f64,
    paint: CellPaint,
    palette: &CellFormats,
) -> Result<(), XlsxError> {
    match paint {
        CellPaint::Input => worksheet
            .write_number_with_format(row, col, value, &palette.input)
            .map(|_| ()),
        CellPaint::Constant => worksheet
            .write_number_with_format(row, col, value, &palette.constant)
            .map(|_| ()),
        CellPaint::Plain => worksheet.write_number(row, col, value).map(|_| ()),
    }
}

/// Parse a simple `A1`-style address into zero-based `(row, col)` for
/// `rust_xlsxwriter`. Supports single/multi-letter columns and 1-based rows
/// (the only forms the authored fixtures use). Test-only: a malformed address is
/// a fixture-author bug and panics.
fn parse_a1(addr: &str) -> (u32, u16) {
    let split = addr
        .find(|c: char| c.is_ascii_digit())
        .expect("authored A1 address has a row digit");
    let (col_letters, row_digits) = addr.split_at(split);
    let mut col: u32 = 0;
    for ch in col_letters.chars() {
        let v = (ch.to_ascii_uppercase() as u32) - ('A' as u32) + 1;
        col = col * 26 + v;
    }
    let col = u16::try_from(col - 1).expect("authored column fits a u16");
    let row: u32 = row_digits.parse::<u32>().expect("authored row is numeric") - 1;
    (row, col)
}

/// Write the paired `*.provenance-override.json` trusted-fixture marker beside
/// `xlsx_path` (clone of `tax-calc.provenance-override.json`, retargeting
/// `fixture` to the authored file's name). Returns the marker path.
///
/// # Errors
/// Returns the underlying I/O / serialization error on failure.
pub(crate) fn write_override_marker(xlsx_path: &Path, reason: &str) -> Result<(), std::io::Error> {
    let file_name = xlsx_path
        .file_name()
        .and_then(|s| s.to_str())
        .expect("xlsx path has a UTF-8 file name");
    let marker_path = xlsx_path.with_extension("provenance-override.json");
    let marker = serde_json::json!({
        "kind": "trusted-fixture",
        "fixture": file_name,
        "reason": reason,
        "authored_by": "rust_xlsxwriter",
        "scope": "test-path-only",
    });
    let body = serde_json::to_string_pretty(&marker)?;
    std::fs::write(marker_path, body)
}

/// Write the GENERATION METADATA sidecar (`*.gen.json`) beside `xlsx_path`
/// (Codex suggestion): the generator fn name, the input cells, the expected
/// output cells, and the override reason — so a committed binary `.xlsx` is
/// reproducible and traceable to the exact author call that produced it.
///
/// # Errors
/// Returns the underlying I/O / serialization error on failure.
pub(crate) fn write_gen_metadata(
    xlsx_path: &Path,
    generator_fn: &str,
    spec: &WorkbookSpec,
    override_reason: &str,
) -> Result<(), std::io::Error> {
    let inputs: Vec<&str> = spec
        .cells
        .iter()
        .filter_map(|c| match c {
            AuthoredCell::Number {
                addr,
                paint: CellPaint::Input,
                ..
            } => Some(*addr),
            _ => None,
        })
        .collect();
    let expected_outputs: Vec<serde_json::Value> = spec
        .defined_names
        .iter()
        .filter(|d| d.name.starts_with("out_"))
        .map(|d| serde_json::json!({ "name": d.name, "target": d.target }))
        .collect();
    let formulas: Vec<serde_json::Value> = spec
        .cells
        .iter()
        .filter_map(|c| match c {
            AuthoredCell::Formula {
                addr,
                formula,
                cached,
            } => Some(serde_json::json!({
                "cell": addr, "formula": formula, "cached_oracle": cached,
            })),
            _ => None,
        })
        .collect();
    let meta = serde_json::json!({
        "generator_fn": generator_fn,
        "authored_by": "rust_xlsxwriter",
        "sheet": spec.sheet,
        "input_cells": inputs,
        "formula_oracles": formulas,
        "expected_outputs": expected_outputs,
        "override_reason": override_reason,
    });
    let gen_path = xlsx_path.with_extension("gen.json");
    let body = serde_json::to_string_pretty(&meta)?;
    std::fs::write(gen_path, body)
}

/// The provenance class of an authored `.xlsx`, read the SAME way the production
/// gate reads it: parse `docProps/app.xml` + `xl/workbook.xml` from the ORIGINAL
/// on-disk bytes via the quarantined raw reader, then [`classify`]. This is the
/// DIRECT provenance assertion target (Codex HIGH) — a self-test calls this and
/// asserts `== ExcelTrusted`, never inferring trust from compile success.
pub(crate) fn classify_authored(bytes: &[u8]) -> ProvenanceClass {
    let app = read_app_props(bytes).expect("authored .xlsx has a readable app.xml");
    let calc = read_calc_pr(bytes).expect("authored .xlsx has a readable calcPr");
    classify(
        app.application.as_deref(),
        app.app_version.as_deref(),
        calc.calc_id,
    )
}

/// A trivial 1-formula spec: a blue-font input `A1`, a formula `B1 = A1*2` with an
/// authored cached oracle, and an `out_result` named range targeting `B1`. The
/// canonical smoke-test workbook the self-tests author into a TempDir.
fn trivial_spec() -> WorkbookSpec {
    WorkbookSpec {
        sheet: "Calc",
        cells: vec![
            AuthoredCell::Number {
                addr: "A1",
                value: 10.0,
                paint: CellPaint::Input,
            },
            AuthoredCell::Formula {
                addr: "B1",
                formula: "A1*2",
                cached: "20",
            },
        ],
        defined_names: vec![DefinedNameSpec {
            name: "out_result",
            target: "'Calc'!$B$1",
        }],
    }
}

/// REPRODUCIBLE, NON-MUTATING fixture generator (Codex HIGH). This is the ONLY
/// path that writes committed `.xlsx` fixtures into the source tree, and it is
/// `#[ignore]`d + env-gated so a normal `cargo test` run NEVER mutates
/// `tests/fixtures/`. Run intentionally with:
///
/// ```text
/// PMCP_REGEN_FIXTURES=1 cargo test -p pmcp-workbook-compiler \
///     regenerate_fixtures -- --ignored
/// ```
///
/// Without the `PMCP_REGEN_FIXTURES` env var the body is a no-op even if invoked
/// directly with `--ignored`, so it can never silently rewrite tracked fixtures.
/// Plans 04 (loan workbook) and 05 (quirk corpus + the leap probe) extend the
/// `targets` list below; each committed fixture is paired with its
/// `*.provenance-override.json` marker and `*.gen.json` metadata sidecar.
#[test]
#[ignore = "writes committed fixtures into the source tree; run only with PMCP_REGEN_FIXTURES=1"]
fn regenerate_fixtures() {
    if std::env::var("PMCP_REGEN_FIXTURES").is_err() {
        eprintln!(
            "[regenerate_fixtures] PMCP_REGEN_FIXTURES not set — no-op (the generator never \
             mutates tests/fixtures/ on a normal run)"
        );
        return;
    }
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    std::fs::create_dir_all(&fixtures).expect("fixtures dir");

    // The leap-year probe (Plan 96-03 Task 2): serial-arithmetic over f64 with
    // whitelisted ops only — see SPIKE-1900-leap.md for the disposition.
    let probe = leap1900_probe_spec();
    let probe_path = fixtures.join("leap1900-probe.xlsx");
    let probe_reason = "authored by rust_xlsxwriter; the 1900-leap serial offset encoded as \
                        whitelisted f64 arithmetic (NO date function) — honoured ONLY on the \
                        test/dev path. Production still refuses non-fresh provenance.";
    author_xlsx(&probe_path, &probe).expect("author leap1900 probe");
    write_override_marker(&probe_path, probe_reason).expect("probe marker");
    write_gen_metadata(&probe_path, "leap1900_probe_spec", &probe, probe_reason)
        .expect("probe gen metadata");
    eprintln!("[regenerate_fixtures] wrote {}", probe_path.display());

    // The loan/mortgage rate-tier calculator (Plan 96-04 Task 1 — the WBEX-01
    // generalization fixture): a fixed-cell DAG whose divergence from tax-calc
    // comes ENTIRELY from whitelist-legal lookup families (VLOOKUP +
    // INDEX/MATCH against a rate-tier table, IFERROR guards, nested-IF tiering,
    // ROUND/CEILING to currency) — NO PMT/POWER/exponentiation (D-02). Fully
    // synthetic (zero customer/TowelRads material).
    let loan = loan_calc_spec();
    let loan_path = fixtures.join("loan-calc.xlsx");
    let loan_reason = "authored by rust_xlsxwriter; a synthetic loan/mortgage rate-tier \
                       calculator (the WBEX-01 second-workbook generalization fixture). \
                       Whitelist-legal (VLOOKUP/INDEX-MATCH/IFERROR/nested-IF/ROUND/CEILING, \
                       NO PMT/POWER) — honoured ONLY on the test/dev path. Production still \
                       refuses non-fresh provenance.";
    author_xlsx(&loan_path, &loan).expect("author loan-calc fixture");
    write_override_marker(&loan_path, loan_reason).expect("loan marker");
    write_gen_metadata(&loan_path, "loan_calc_spec", &loan, loan_reason)
        .expect("loan gen metadata");
    eprintln!("[regenerate_fixtures] wrote {}", loan_path.display());

    // The WBEX-02 Excel-quirk reconcile corpus (Plan 96-05 Task 2): each tiny
    // workbook encodes ONE numerically-expressible Excel quirk as a whitelisted
    // formula DAG with a cached `<v>` oracle, so the quirks_reconcile harness can
    // grade the executor's recomputation against the oracle through the REAL
    // penny-reconcile path. (The 1900-leap reconcile fixture is the committed
    // `leap1900-probe.xlsx` above — Plan 96-03 disposition A — so it is NOT
    // re-authored here; this generator emits only the six NON-leap reconcile
    // fixtures.) Authored into the quirks/ subdir, each with its override marker
    // and gen-metadata sidecar.
    let quirks_dir = fixtures.join("quirks");
    std::fs::create_dir_all(&quirks_dir).expect("quirks fixtures dir");
    for (file_stem, generator_fn, spec) in quirk_reconcile_specs() {
        let path = quirks_dir.join(format!("{file_stem}.xlsx"));
        let reason = "authored by rust_xlsxwriter; a WBEX-02 Excel-quirk reconcile fixture \
                      (one quirk, whitelisted formula DAG, cached <v> oracle) — honoured ONLY \
                      on the test/dev path. Production still refuses non-fresh provenance.";
        author_xlsx(&path, &spec).expect("author quirk fixture");
        write_override_marker(&path, reason).expect("quirk marker");
        write_gen_metadata(&path, generator_fn, &spec, reason).expect("quirk gen metadata");
        eprintln!("[regenerate_fixtures] wrote {}", path.display());
    }
}

/// The WBEX-02 quirk reconcile-fixture specs (Plan 96-05 Task 2): each is a tiny
/// single-formula DAG encoding ONE numerically-expressible Excel quirk, with the
/// cached `<v>` carrying the Excel oracle the penny-reconcile harness grades the
/// recomputation against. The returned tuples are `(file_stem, generator_fn_name,
/// spec)` for the env-gated generator above. (The 1900-leap reconcile fixture is
/// the committed `leap1900-probe.xlsx` — Plan 96-03 disposition A — so it is not
/// in this list; the four NAMED roadmap quirks are covered as: 1900-leap = the
/// probe; half-rounding = `quirk-half-rounding`; empty-cell coercion =
/// `quirk-empty-coercion`; error/`#DIV/0!` propagation = the scalar_eval layer +
/// the SPIKE-documented runtime limitation, see `quirks_reconcile.rs`.)
pub(crate) fn quirk_reconcile_specs() -> Vec<(&'static str, &'static str, WorkbookSpec)> {
    vec![
        (
            "quirk-half-rounding",
            "quirk_half_rounding_spec",
            quirk_half_rounding_spec(),
        ),
        (
            "quirk-negative-rounding",
            "quirk_negative_rounding_spec",
            quirk_negative_rounding_spec(),
        ),
        (
            "quirk-empty-coercion",
            "quirk_empty_coercion_spec",
            quirk_empty_coercion_spec(),
        ),
        (
            "quirk-float-boundary",
            "quirk_float_boundary_spec",
            quirk_float_boundary_spec(),
        ),
        (
            "quirk-text-coercion",
            "quirk_text_coercion_spec",
            quirk_text_coercion_spec(),
        ),
    ]
}

/// QUIRK: half-rounding boundary. `ROUND(1594.925, 2)` is `1594.93` (half away
/// from zero), NOT the naive binary-f64 `1594.92` — the executor's `excel_round`
/// applies the boundary epsilon. {formula `ROUND(A1,2)`, context: ROUND of a
/// decimal-half input; oracle `1594.93`; reconcile key `out_rounded` → B1}.
fn quirk_half_rounding_spec() -> WorkbookSpec {
    WorkbookSpec {
        sheet: "Quirk",
        cells: vec![
            AuthoredCell::Number {
                addr: "A1",
                value: 1594.925,
                paint: CellPaint::Input,
            },
            AuthoredCell::Formula {
                addr: "B1",
                formula: "ROUND(A1,2)",
                cached: "1594.93",
            },
        ],
        defined_names: vec![
            DefinedNameSpec {
                name: "in_value",
                target: "'Quirk'!$A$1",
            },
            DefinedNameSpec {
                name: "out_rounded",
                target: "'Quirk'!$B$1",
            },
        ],
    }
}

/// QUIRK: negative-value rounding sign. `ROUND(-2.5, 0)` is `-3` (Excel rounds
/// half AWAY FROM ZERO, sign preserved), NOT `-2`. {formula `ROUND(A1,0)`,
/// context: ROUND of a negative half; oracle `-3`; reconcile key `out_rounded`
/// → B1}.
fn quirk_negative_rounding_spec() -> WorkbookSpec {
    WorkbookSpec {
        sheet: "Quirk",
        cells: vec![
            AuthoredCell::Number {
                addr: "A1",
                value: -2.5,
                paint: CellPaint::Input,
            },
            AuthoredCell::Formula {
                addr: "B1",
                formula: "ROUND(A1,0)",
                cached: "-3",
            },
        ],
        defined_names: vec![
            DefinedNameSpec {
                name: "in_value",
                target: "'Quirk'!$A$1",
            },
            DefinedNameSpec {
                name: "out_rounded",
                target: "'Quirk'!$B$1",
            },
        ],
    }
}

/// QUIRK: empty-cell coercion (named roadmap quirk). An EMPTY cell coerces to 0
/// in additive arithmetic (Excel's empty-cell-as-0). The empty cell is PRODUCED
/// by a 2-arg `IF` whose condition is false — `IF(A2>9999, 1)` has no else
/// branch, so Excel (and the runtime semantics layer) returns EMPTY (not a hard
/// `#REF!` the way an absent range member would). Then `A2 + A1` adds the
/// number to the empty cell: `5 + (empty->0) = 5`. {formula `A2+A1` where
/// `A1 = IF(A2>9999,1)` is empty; context: empty cell in additive `+`; oracle
/// `5`; reconcile key `out_sum` → B1}. The ONLY input is A2.
///
/// (Why not a blank cell in a SUM range: on this runtime an ABSENT range member
/// is a hard `#REF!`, not Empty — empty-cell-as-0 applies to a cell that RESOLVES
/// to Empty, which the 2-arg `IF` produces deterministically.)
fn quirk_empty_coercion_spec() -> WorkbookSpec {
    WorkbookSpec {
        sheet: "Quirk",
        cells: vec![
            AuthoredCell::Number {
                addr: "A2",
                value: 5.0,
                paint: CellPaint::Input,
            },
            // A1: a deterministically-EMPTY helper (false 2-arg IF, no else).
            AuthoredCell::Formula {
                addr: "A1",
                formula: "IF(A2>9999,1)",
                cached: "",
            },
            // B1: the named output — adds the number to the empty cell (->0).
            AuthoredCell::Formula {
                addr: "B1",
                formula: "A2+A1",
                cached: "5",
            },
        ],
        defined_names: vec![
            DefinedNameSpec {
                name: "in_value",
                target: "'Quirk'!$A$2",
            },
            DefinedNameSpec {
                name: "out_sum",
                target: "'Quirk'!$B$1",
            },
        ],
    }
}

/// QUIRK: float boundary. `0.1 + 0.2` is stored in binary-f64 as
/// `0.30000000000000004`, not exactly `0.3` — which is WHY money compares go
/// through `within_tol` (±0.01), never exact-float `==`. {formula `A1+A2`,
/// context: binary-f64 additive boundary; oracle `0.3` (graded within_tol);
/// reconcile key `out_sum` → B1}.
fn quirk_float_boundary_spec() -> WorkbookSpec {
    WorkbookSpec {
        sheet: "Quirk",
        cells: vec![
            AuthoredCell::Number {
                addr: "A1",
                value: 0.1,
                paint: CellPaint::Input,
            },
            AuthoredCell::Number {
                addr: "A2",
                value: 0.2,
                paint: CellPaint::Input,
            },
            AuthoredCell::Formula {
                addr: "B1",
                formula: "A1+A2",
                cached: "0.3",
            },
        ],
        defined_names: vec![
            DefinedNameSpec {
                name: "in_a",
                target: "'Quirk'!$A$1",
            },
            DefinedNameSpec {
                name: "in_b",
                target: "'Quirk'!$A$2",
            },
            DefinedNameSpec {
                name: "out_sum",
                target: "'Quirk'!$B$1",
            },
        ],
    }
}

/// QUIRK: text→number coercion. In a MULTIPLICATIVE context Excel coerces a
/// numeric-text operand to its number: `"5.5" * 2` is `11`. (Context is
/// load-bearing: in an ADDITIVE `&`/concat context the same text would NOT be
/// summed — see the scalar_eval layer, which pins the `+`-vs-`*` distinction.)
/// {formula `A1*A2`, context: numeric-text in `*`; oracle `11`; reconcile key
/// `out_product` → B1}. A1 is authored as TEXT `"5.5"`.
fn quirk_text_coercion_spec() -> WorkbookSpec {
    WorkbookSpec {
        sheet: "Quirk",
        cells: vec![
            AuthoredCell::Text {
                addr: "A1",
                text: "5.5",
            },
            AuthoredCell::Number {
                addr: "A2",
                value: 2.0,
                paint: CellPaint::Input,
            },
            AuthoredCell::Formula {
                addr: "B1",
                formula: "A1*A2",
                cached: "11",
            },
        ],
        defined_names: vec![
            DefinedNameSpec {
                name: "in_factor",
                target: "'Quirk'!$A$2",
            },
            DefinedNameSpec {
                name: "out_product",
                target: "'Quirk'!$B$1",
            },
        ],
    }
}

/// The synthetic loan/mortgage rate-tier calculator spec (Plan 96-04 Task 1 —
/// the WBEX-01 second-workbook generalization fixture).
///
/// # Why this workbook is deliberately divergent from `tax-calc` (D-01/D-02)
///
/// It is a fixed-cell formula DAG whose divergence is sourced ENTIRELY from
/// whitelist-legal LOOKUP families — proving the manifest-driven serve path
/// generalizes to a workbook whose shape `tax-calc` never exercised:
///
/// - a rate-tier TABLE (governed constants `D2:E4`, painted green) mapping a
///   credit-tier index → an annual rate,
/// - `VLOOKUP(...)` AND `INDEX(.., MATCH(.., 0))` both resolving the tier→rate
///   (two lookup families, cross-checked),
/// - `IFERROR(..)` guarding a lookup miss,
/// - a nested `IF(.., IF(..))` tiering the credit score into a band index,
/// - `ROUND`/`CEILING` to currency (the executor's `excel_round`/`excel_ceiling`
///   source of truth — the cached `<v>` oracle below is computed against them).
///
/// It is whitelist-legal: NO `PMT`, NO `POWER`, NO exponentiation (an
/// arbitrary-term `(1+r)^n` amortization is NOT expressible and is deferred);
/// the first-period interest model below stays inside the 13-fn whitelist.
///
/// # Named inputs / outputs (with units)
///
/// Inputs (blue font → `Role::Input`): `loan_amount` (A2, USD), `term_months`
/// (A3), `credit_score` (A4).
///
/// Outputs (`out_*` named ranges → `Role::Output`, MULTIPLE — no privileged
/// single headline, the Anti-Pattern this fixture guards against):
/// - `out_credit_tier` (B2) — the resolved tier index,
/// - `out_applied_rate` (B3, unit `percent`) — the VLOOKUP'd annual rate (the
///   custom-unit output that exercises the served schema's unit projection),
/// - `out_monthly_interest` (B5, unit `USD`) — first-period interest,
/// - `out_total_interest` (B6, unit `USD`) — `CEILING`'d simple-interest total,
/// - `out_tier_rate` (B8, unit `percent`) — the INDEX/MATCH cross-check of the
///   same tier→rate (proves both lookup families resolve identically).
///
/// # The reconcile oracle (cached `<v>`)
///
/// Worked for `loan_amount=240000, term_months=360, credit_score=700`:
/// - B2 tier  = `IF(700>=740,3,IF(700>=680,2,1))`                     → `2`
/// - B3 rate  = `IFERROR(VLOOKUP(2,D2:E4,2,FALSE),0.08)`             → `0.06`
/// - B4 m.rate= `ROUND(0.06/12,6)`                                    → `0.005`
/// - B5 m.int = `ROUND(240000*0.005,2)`                              → `1200`
/// - B6 t.int = `CEILING(1200*360,1)`                               → `432000`
/// - B8 tier  = `IFERROR(INDEX(E2:E4,MATCH(2,D2:D4,0)),0.08)`        → `0.06`
fn loan_calc_spec() -> WorkbookSpec {
    WorkbookSpec {
        sheet: "Loan",
        cells: vec![
            // ---- labels (documentation, not role-bearing) ----
            AuthoredCell::Text {
                addr: "A1",
                text: "Loan rate-tier calculator (synthetic)",
            },
            AuthoredCell::Text {
                addr: "C1",
                text: "tier",
            },
            AuthoredCell::Text {
                addr: "D1",
                text: "rate",
            },
            // ---- inputs (blue font → Role::Input) ----
            AuthoredCell::Number {
                addr: "A2",
                value: 240_000.0,
                paint: CellPaint::Input,
            },
            AuthoredCell::Number {
                addr: "A3",
                value: 360.0,
                paint: CellPaint::Input,
            },
            AuthoredCell::Number {
                addr: "A4",
                value: 700.0,
                paint: CellPaint::Input,
            },
            // ---- rate-tier table (governed constants, green fill) ----
            AuthoredCell::Number {
                addr: "D2",
                value: 1.0,
                paint: CellPaint::Constant,
            },
            AuthoredCell::Number {
                addr: "E2",
                value: 0.08,
                paint: CellPaint::Constant,
            },
            AuthoredCell::Number {
                addr: "D3",
                value: 2.0,
                paint: CellPaint::Constant,
            },
            AuthoredCell::Number {
                addr: "E3",
                value: 0.06,
                paint: CellPaint::Constant,
            },
            AuthoredCell::Number {
                addr: "D4",
                value: 3.0,
                paint: CellPaint::Constant,
            },
            AuthoredCell::Number {
                addr: "E4",
                value: 0.045,
                paint: CellPaint::Constant,
            },
            // ---- the formula DAG (cached <v> = the reconcile oracle) ----
            AuthoredCell::Formula {
                addr: "B2",
                formula: "IF(A4>=740,3,IF(A4>=680,2,1))",
                cached: "2",
            },
            AuthoredCell::Formula {
                addr: "B3",
                formula: "IFERROR(VLOOKUP(B2,D2:E4,2,FALSE),0.08)",
                cached: "0.06",
            },
            AuthoredCell::Formula {
                addr: "B4",
                formula: "ROUND(B3/12,6)",
                cached: "0.005",
            },
            AuthoredCell::Formula {
                addr: "B5",
                formula: "ROUND(A2*B4,2)",
                cached: "1200",
            },
            AuthoredCell::Formula {
                addr: "B6",
                formula: "CEILING(B5*A3,1)",
                cached: "432000",
            },
            AuthoredCell::Formula {
                addr: "B8",
                formula: "IFERROR(INDEX(E2:E4,MATCH(B2,D2:D4,0)),0.08)",
                cached: "0.06",
            },
            // ---- the WBDL-02 present-path declaration (compatible value) ----
            AuthoredCell::Text {
                addr: "A10",
                text: "1.0",
            },
        ],
        defined_names: vec![
            // `in_*` INPUT named ranges (the input analogue of `out_*`): they give
            // each blue-font input a STABLE semantic served key (`loan_amount`)
            // instead of the cell's numeric value. See `name_named_inputs` (lib.rs).
            DefinedNameSpec {
                name: "in_loan_amount",
                target: "'Loan'!$A$2",
            },
            DefinedNameSpec {
                name: "in_term_months",
                target: "'Loan'!$A$3",
            },
            DefinedNameSpec {
                name: "in_credit_score",
                target: "'Loan'!$A$4",
            },
            DefinedNameSpec {
                name: "out_credit_tier",
                target: "'Loan'!$B$2",
            },
            DefinedNameSpec {
                name: "out_applied_rate",
                target: "'Loan'!$B$3",
            },
            DefinedNameSpec {
                name: "out_monthly_interest",
                target: "'Loan'!$B$5",
            },
            DefinedNameSpec {
                name: "out_total_interest",
                target: "'Loan'!$B$6",
            },
            DefinedNameSpec {
                name: "out_tier_rate",
                target: "'Loan'!$B$8",
            },
            DefinedNameSpec {
                name: "pmcp_dialect_version",
                target: "'Loan'!$A$10",
            },
        ],
    }
}

/// The 1900-leap-year probe spec (Plan 96-03 Task 2 disposition A).
///
/// Excel treats 1900 as a leap year, so the date serial for any day on/after
/// 1900-03-01 is ONE GREATER than the astronomically-correct count from the
/// 1900-01-01 epoch (the phantom serial 60 = 1900-02-29). The probe encodes this
/// as PURE f64 serial arithmetic with whitelisted ops only (`IF` + `+`/`-`/
/// comparison) — NO `DATE`/`DATEVALUE`, NO new dialect function:
///
/// - `A1` (input): a raw day-count from the 1900-01-01 epoch (here `61`, which is
///   1900-03-01 counting 1900-01-01 as serial 1).
/// - `B1` (formula `IF(A1>59, A1+1, A1)` cached `62`): adds the phantom-leap
///   offset for serials past 1900-02-28 — exactly Excel's serial behaviour.
/// - `out_excel_serial` → `B1`: the Excel serial the reconcile oracle grades.
fn leap1900_probe_spec() -> WorkbookSpec {
    WorkbookSpec {
        sheet: "Serial",
        cells: vec![
            AuthoredCell::Number {
                addr: "A1",
                value: 61.0,
                paint: CellPaint::Input,
            },
            AuthoredCell::Formula {
                addr: "B1",
                formula: "IF(A1>59,A1+1,A1)",
                cached: "62",
            },
        ],
        defined_names: vec![DefinedNameSpec {
            name: "out_excel_serial",
            target: "'Serial'!$B$1",
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compile_workbook, compile_workbook_with_fixture_override};

    /// (Direct provenance assertion #1) An authored workbook classifies
    /// `ProvenanceClass::ExcelTrusted` — read DIRECTLY from the authored bytes via
    /// the same raw-reader + classify path the production gate uses, NOT inferred
    /// from a successful compile. A umya-authored fixture would classify
    /// `UmyaFabricated` and fail this assertion.
    #[test]
    fn authored_xlsx_classifies_excel_trusted_directly() {
        let dir = tempfile::TempDir::new().expect("scratch dir");
        let xlsx = dir.path().join("trivial.xlsx");
        author_xlsx(&xlsx, &trivial_spec()).expect("author the trivial workbook");

        let bytes = std::fs::read(&xlsx).expect("read authored bytes");
        assert_eq!(
            classify_authored(&bytes),
            ProvenanceClass::ExcelTrusted,
            "a rust_xlsxwriter-authored workbook carries genuine Excel identity \
             (<Application>Microsoft Excel</Application> + an <AppVersion> build + a \
             non-sentinel calcId) and MUST classify ExcelTrusted directly"
        );
    }

    /// (End-to-end #2) The authored workbook compiles through the `#[cfg(test)]`
    /// trusted-fixture override (the recipe passes the freshness gate end-to-end:
    /// genuine identity + demoted `fullCalcOnLoad` staleness), and the executor's
    /// recomputation reconciles against the authored cached `<v>` oracle.
    #[test]
    fn authored_xlsx_compiles_via_fixture_override() {
        let dir = tempfile::TempDir::new().expect("scratch dir");
        let xlsx = dir.path().join("trivial.xlsx");
        author_xlsx(&xlsx, &trivial_spec()).expect("author the trivial workbook");

        let out_root = dir.path().join("out");
        std::fs::create_dir_all(&out_root).expect("out root");
        let lock = compile_workbook_with_fixture_override(
            &xlsx,
            &out_root,
            "trivial",
            "1.0.0",
            "fixture-author-proof",
        )
        .expect("the authored workbook compiles via the trusted-fixture override");
        assert_eq!(lock.version, "1.0.0");
    }

    /// (Production-refusal guard #3 — T-96-07) The SAME authored bytes are REFUSED
    /// on the production `compile_workbook` (Enforce) path: only the `#[cfg(test)]`
    /// override accepts the `fullCalcOnLoad=1` staleness; production never weakens.
    #[test]
    fn production_compile_refuses_authored_fixture() {
        let dir = tempfile::TempDir::new().expect("scratch dir");
        let xlsx = dir.path().join("trivial.xlsx");
        author_xlsx(&xlsx, &trivial_spec()).expect("author the trivial workbook");

        let out_root = dir.path().join("out");
        std::fs::create_dir_all(&out_root).expect("out root");
        let result = compile_workbook(&xlsx, &out_root, "trivial", "1.0.0", "prod-approver");
        assert!(
            result.is_err(),
            "production compile_workbook (Enforce) MUST refuse the authored fixture's \
             fullCalcOnLoad staleness — the trusted-fixture override is test-only and \
             never weakens production (T-96-07)"
        );
    }

    /// The override marker + generation-metadata sidecars are written beside the
    /// fixture with the cloned trusted-fixture shape and the recorded provenance
    /// (generator fn, input cells, formula oracles, expected outputs). Authored
    /// into a TempDir — a normal test run writes NOTHING into `tests/fixtures/`.
    #[test]
    fn author_emits_override_and_gen_metadata() {
        let dir = tempfile::TempDir::new().expect("scratch dir");
        let xlsx = dir.path().join("trivial.xlsx");
        let spec = trivial_spec();
        author_xlsx(&xlsx, &spec).expect("author");
        write_override_marker(&xlsx, "smoke-test fixture").expect("marker");
        write_gen_metadata(&xlsx, "trivial_spec", &spec, "smoke-test fixture").expect("gen meta");

        let marker = std::fs::read_to_string(dir.path().join("trivial.provenance-override.json"))
            .expect("marker exists");
        let marker_json: serde_json::Value =
            serde_json::from_str(&marker).expect("marker is valid json");
        assert_eq!(marker_json["kind"], "trusted-fixture");
        assert_eq!(marker_json["fixture"], "trivial.xlsx");
        assert_eq!(marker_json["authored_by"], "rust_xlsxwriter");

        let gen = std::fs::read_to_string(dir.path().join("trivial.gen.json")).expect("gen exists");
        let gen_json: serde_json::Value = serde_json::from_str(&gen).expect("gen is valid json");
        assert_eq!(gen_json["generator_fn"], "trivial_spec");
        assert_eq!(gen_json["input_cells"][0], "A1");
        assert_eq!(gen_json["formula_oracles"][0]["cached_oracle"], "20");
        assert_eq!(gen_json["expected_outputs"][0]["name"], "out_result");
    }

    /// `parse_a1` maps A1-notation to zero-based `(row, col)` for the cells the
    /// fixtures use (single/multi-letter columns, 1-based rows).
    #[test]
    fn parse_a1_maps_addresses() {
        assert_eq!(parse_a1("A1"), (0, 0));
        assert_eq!(parse_a1("B2"), (1, 1));
        assert_eq!(parse_a1("Z1"), (0, 25));
        assert_eq!(parse_a1("AA1"), (0, 26));
    }

    /// The committed 1900-leap probe fixture (`tests/fixtures/leap1900-probe.xlsx`).
    fn committed_leap_probe() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/leap1900-probe.xlsx")
    }

    /// (SPIKE disposition A — Plan 96-03 Task 2) The COMMITTED leap-year probe
    /// compiles + RECONCILES via the trusted-fixture override: the executor
    /// recomputes `IF(A1>59, A1+1, A1)` (the Excel-serial phantom-leap offset over
    /// bare `f64`, whitelisted ops only) and reconciles against the authored cached
    /// `<v>` oracle (`62`). This PROVES the 1900-leap quirk is DAG-expressible
    /// without any date function — see SPIKE-1900-leap.md.
    #[test]
    fn leap1900_probe_compiles_and_reconciles() {
        let dir = tempfile::TempDir::new().expect("scratch dir");
        let xlsx = dir.path().join("leap1900-probe.xlsx");
        std::fs::copy(committed_leap_probe(), &xlsx).expect("copy committed probe");

        // Direct provenance assertion: the committed probe is ExcelTrusted.
        let bytes = std::fs::read(&xlsx).expect("read probe bytes");
        assert_eq!(classify_authored(&bytes), ProvenanceClass::ExcelTrusted);

        let out_root = dir.path().join("out");
        std::fs::create_dir_all(&out_root).expect("out root");
        // A clean compile means the reconcile gate matched the executor's
        // recomputation (the serial offset) to the cached oracle — disposition A.
        compile_workbook_with_fixture_override(
            &xlsx,
            &out_root,
            "leap1900-probe",
            "1.0.0",
            "spike-1900-leap",
        )
        .expect("the leap1900 probe compiles + reconciles via the override (disposition A)");
    }
}
