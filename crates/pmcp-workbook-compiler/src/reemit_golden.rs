//! Producer/consumer proof (WBCO-05): compiling the neutral `tax-calc.xlsx`
//! fixture through the GENERIC `compile_workbook` driver reproduces the committed
//! `tax-calc@1.1.0` golden by STRUCTURAL EQUIVALENCE (the O-2 pre-decided default;
//! byte-identical is a non-blocking stretch goal only).
//!
//! # Why this lives in `src/` under `#[cfg(test)]` (CR-01)
//!
//! The fixture override (`compile_workbook_with_fixture_override`) is
//! `#[cfg(test)]`-only — there is NO publishable Cargo feature that arms it, so a
//! downstream consumer's default/`--all-features` build can never reach it. An
//! integration test in `tests/` compiles as an EXTERNAL crate, where `#[cfg(test)]`
//! items are invisible; so the proof MUST live INSIDE the crate as a `#[cfg(test)]`
//! module to reach the override. It runs via plain `cargo test
//! -p pmcp-workbook-compiler` — NO features, NO special cfgs.
//!
//! The committed neutral fixture (`tests/fixtures/tax-calc.xlsx`) is authored by
//! `rust_xlsxwriter` (a pure writer) and carries a GENUINE Excel identity
//! (`<Application>Microsoft Excel</Application>` + an `<AppVersion>` build string +
//! a non-sentinel calcId) so it classifies as `ProvenanceClass::ExcelTrusted`. Its
//! ONLY freshness problem is `fullCalcOnLoad=1` (a staleness signal), which the
//! `#[cfg(test)]` override DEMOTES to a Warning. The override CANNOT soften the
//! fabricated-/non-Excel-identity refusal (`oracle/non-excel-app` is no longer in
//! `SOFTENABLE_FRESHNESS_RULES`); trusted identity comes the legitimate way.
//!
//! The proof asserts the five structural-equivalence dimensions defined in
//! 93-07-PLAN `<interfaces>`:
//!
//!   1. Normalized-JSON equality on the LOAD-BEARING semantic members the synth
//!      path reproduces (executable.ir.json, cell_map.json).
//!   2. All seven members present.
//!   3. BUNDLE.lock combined hash recomputes (via the runtime helper) + bundle_id.
//!   4. The emitted bundle loads via pmcp-server-toolkit::workbook.
//!   5. Named-output names/dtypes/roles match the golden's.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::{build_bundle_lock, compile_workbook, compile_workbook_with_fixture_override};

/// The committed golden bundle dir (workspace sibling, relative to THIS crate).
fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0")
}

/// The committed neutral fixture (`tests/fixtures/tax-calc.xlsx`).
fn committed_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tax-calc.xlsx")
}

/// Copy the COMMITTED neutral fixture into `dir/tax-calc.xlsx`. Returns the path.
fn place_fixture(dir: &Path) -> PathBuf {
    let xlsx = dir.join("tax-calc.xlsx");
    std::fs::copy(committed_fixture(), &xlsx).expect("copy committed fixture");
    xlsx
}

/// Place the neutral fixture into a fresh scratch dir, compile it via the
/// `#[cfg(test)]` trusted-fixture override, and return the emitted bundle dir.
fn compile_fixture() -> (tempfile::TempDir, PathBuf) {
    let scratch = tempfile::TempDir::new().expect("scratch dir");
    let xlsx = place_fixture(scratch.path());

    let out_root = scratch.path().join("out");
    std::fs::create_dir_all(&out_root).expect("out root");
    compile_workbook_with_fixture_override(&xlsx, &out_root, "tax-calc", "1.1.0", "proof-approver")
        .expect("compile the neutral fixture via the trusted-fixture override");
    let bundle = out_root.join("tax-calc@1.1.0");
    (scratch, bundle)
}

/// Parse a bundle member to a `serde_json::Value` (normalized: key order / format
/// ignored).
fn read_json(dir: &Path, member: &str) -> Value {
    let bytes = std::fs::read(dir.join(member)).unwrap_or_else(|e| panic!("read {member}: {e}"));
    serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("parse {member}: {e}"))
}

/// Check (1) — the re-emitted IR's formula DAG is a SUBSET of the golden's.
///
/// The committed served golden was regenerated (Plan 04, WBV2-04) into the
/// two-Table shape — it carries an EXTRA `Estimate_Refund` tool (the `4_Refund!B2`
/// refund cell + the `1_Inputs!B5` withheld input) the legacy named-range
/// `tax-calc.xlsx` source does NOT declare. So the named-range compile output is
/// the golden's TAX SUBSET: every cell it emits must match the golden byte-for-byte
/// (the formula DAG is the load-bearing semantic member), but the golden may carry
/// additional refund-tool cells.
#[test]
fn structural_eq_check1_executable_ir_is_subset_of_golden() {
    let (_scratch, bundle) = compile_fixture();
    let emitted = read_json(&bundle, "executable.ir.json");
    let golden = read_json(&golden_dir(), "executable.ir.json");
    let emitted_obj = emitted.as_object().expect("emitted IR is an object");
    let golden_obj = golden.as_object().expect("golden IR is an object");
    for (cell, expr) in emitted_obj {
        assert_eq!(
            Some(expr),
            golden_obj.get(cell),
            "re-emitted IR cell {cell} must match the golden (the formula DAG is the \
             load-bearing semantic member)"
        );
    }
}

/// Check (1, cont.) — the re-emitted cell_map's seed coordinates are a SUBSET of
/// the golden's (the served I/O contract: the executor seeds/reads each cell by its
/// `seed_coord`). The regenerated served golden (WBV2-04 two-Table shape) carries an
/// EXTRA refund input/output the legacy named-range source does not declare, so the
/// compile output is the golden's TAX SUBSET.
#[test]
fn structural_eq_check1_cell_map_seed_coords_subset_of_golden() {
    let (_scratch, bundle) = compile_fixture();
    let emitted = read_json(&bundle, "cell_map.json");
    let golden = read_json(&golden_dir(), "cell_map.json");

    use std::collections::BTreeSet;
    let input_coords = |v: &Value| -> BTreeSet<String> {
        v["inputs"]
            .as_array()
            .expect("inputs array")
            .iter()
            .map(|e| e["seed_coord"].as_str().expect("seed_coord").to_string())
            .collect()
    };
    // WBV2-03/04: output seed_coords live under `tools[].outputs[]` (the multi-tool
    // model); union across tools by iterating each tool's outputs.
    let output_coords = |v: &Value| -> BTreeSet<String> {
        v["tools"]
            .as_array()
            .expect("tools array")
            .iter()
            .flat_map(|t| t["outputs"].as_array().expect("tool.outputs array").iter())
            .map(|e| e["seed_coord"].as_str().expect("seed_coord").to_string())
            .collect()
    };
    assert!(
        input_coords(&emitted).is_subset(&input_coords(&golden)),
        "every re-emitted input seed_coord is present in the golden"
    );
    assert!(
        output_coords(&emitted).is_subset(&output_coords(&golden)),
        "every re-emitted output seed_coord is present in the golden"
    );
}

/// Check (2) — all seven members present.
#[test]
fn structural_eq_check2_seven_members_present() {
    let (_scratch, bundle) = compile_fixture();
    for member in [
        "manifest.json",
        "executable.ir.json",
        "cell_map.json",
        "layout.json",
        "BUNDLE.lock",
        "evidence/changelog.json",
        "evidence/parser_equivalence.json",
    ] {
        assert!(
            bundle.join(member).exists(),
            "the re-emitted bundle carries the seven-member contract: missing {member}"
        );
    }
}

/// Check (3) — BUNDLE.lock combined hash recomputes via the runtime helper, and
/// bundle_id is present (D-17).
#[test]
fn structural_eq_check3_bundle_lock_recomputes() {
    let (_scratch, bundle) = compile_fixture();
    let lock: Value = read_json(&bundle, "BUNDLE.lock");
    assert!(
        lock.get("bundle_id").is_some(),
        "BUNDLE.lock carries bundle_id (D-17)"
    );

    let ir_json = std::fs::read_to_string(bundle.join("executable.ir.json")).expect("ir");
    let manifest_json = std::fs::read_to_string(bundle.join("manifest.json")).expect("manifest");
    let recomputed = build_bundle_lock(
        lock["bundle_id"].as_str().expect("bundle_id"),
        lock["version"].as_str().expect("version"),
        lock["workbook_hash"]
            .as_str()
            .expect("workbook_hash")
            .to_string(),
        &ir_json,
        &manifest_json,
        lock["artifacts"]["evidence"]
            .as_str()
            .expect("evidence hash"),
    );
    assert_eq!(
        recomputed.combined,
        lock["combined"].as_str().expect("combined"),
        "the BUNDLE.lock combined hash recomputes via the runtime build_bundle_lock helper"
    );
}

/// Check (4) — the emitted bundle loads via the Phase 92 fail-closed toolkit loader.
#[test]
fn structural_eq_check4_loads_via_toolkit() {
    use pmcp_server_toolkit::workbook::{load_bundle, LocalDirSource};
    let (_scratch, bundle) = compile_fixture();
    let source = LocalDirSource::new(&bundle);
    let loaded = load_bundle(&source).expect("the re-emitted bundle loads via the toolkit loader");
    assert_eq!(loaded.stamp.bundle_id, "tax-calc");
    assert_eq!(loaded.stamp.version, "1.1.0");
    assert_eq!(loaded.cell_map.inputs.len(), 3, "three inputs served");
    let output_count: usize = loaded.cell_map.tools.iter().map(|t| t.outputs.len()).sum();
    assert_eq!(output_count, 4, "four named outputs served");
}

/// Check (5) — named-output names/dtypes/roles match the golden's.
#[test]
fn structural_eq_check5_named_outputs_match() {
    let (_scratch, bundle) = compile_fixture();
    let emitted = read_json(&bundle, "manifest.json");
    let golden = read_json(&golden_dir(), "manifest.json");

    // Project each manifest to {cell -> (role, dtype, name)} for the OUTPUT cells.
    let outputs = |m: &Value| -> BTreeMap<String, (String, String, Value)> {
        m["cells"]
            .as_array()
            .expect("cells array")
            .iter()
            .filter(|c| c["role"] == Value::String("output".to_string()))
            .map(|c| {
                (
                    c["cell"].as_str().expect("cell").to_string(),
                    (
                        c["role"].as_str().expect("role").to_string(),
                        c["dtype"].as_str().expect("dtype").to_string(),
                        c["name"].clone(),
                    ),
                )
            })
            .collect()
    };
    let emitted_outputs = outputs(&emitted);
    let golden_outputs = outputs(&golden);
    // The regenerated served golden (WBV2-04 two-Table shape) declares an EXTRA
    // refund output the legacy named-range source does not, so the compile output's
    // outputs are a SUBSET of the golden's — but every shared output cell's
    // role/dtype/name must match exactly.
    for (cell, (e_role, e_dtype, e_name)) in &emitted_outputs {
        let (g_role, g_dtype, g_name) = golden_outputs
            .get(cell)
            .unwrap_or_else(|| panic!("re-emitted output cell {cell} present in the golden"));
        assert_eq!(e_role, g_role, "output {cell} role matches");
        assert_eq!(e_dtype, g_dtype, "output {cell} dtype matches");
        assert_eq!(e_name, g_name, "output {cell} named-range name matches");
    }
}

/// The override CANNOT weaken production refusal: the SAME fixture bytes compiled
/// on the PRODUCTION path ([`compile_workbook`], which always enforces the
/// provenance + freshness refuse path) are REFUSED — the trusted-fixture override
/// is reachable only on the `#[cfg(test)]` path and never relaxes production.
#[test]
fn override_does_not_weaken_production_refusal() {
    let scratch = tempfile::TempDir::new().expect("scratch");
    let xlsx = place_fixture(scratch.path());
    let out_root = scratch.path().join("out");
    std::fs::create_dir_all(&out_root).expect("out root");

    let result = compile_workbook(&xlsx, &out_root, "tax-calc", "1.1.0", "prod-approver");
    assert!(
        result.is_err(),
        "the production compile_workbook MUST refuse the fixture's staleness signal \
         (the trusted-fixture override is test-only and never weakens production)"
    );
}

/// OPTIONAL non-blocking STRETCH (O-2): byte-identical re-emit. The golden was
/// hand-authored with provenance metadata a real compile cannot reproduce, so
/// byte-identity is NOT expected — this is logged, never asserted.
#[test]
fn stretch_byte_identical_is_logged_not_required() {
    let (_scratch, bundle) = compile_fixture();
    let mut identical = 0u32;
    let mut total = 0u32;
    for member in ["executable.ir.json", "cell_map.json"] {
        total += 1;
        let a = std::fs::read(bundle.join(member)).expect("emitted");
        let b = std::fs::read(golden_dir().join(member)).expect("golden");
        if a == b {
            identical += 1;
        }
    }
    eprintln!(
        "[stretch] byte-identical members: {identical}/{total} (structural equivalence is the \
         pre-decided default; byte-identity is never required)"
    );
}
