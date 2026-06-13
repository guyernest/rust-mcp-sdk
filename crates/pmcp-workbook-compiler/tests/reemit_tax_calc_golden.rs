//! Producer/consumer proof (WBCO-05): compiling the neutral `tax-calc.xlsx`
//! fixture through the GENERIC `compile_workbook` driver reproduces the committed
//! `tax-calc@1.1.0` golden by STRUCTURAL EQUIVALENCE (the O-2 pre-decided default;
//! byte-identical is a non-blocking stretch goal only).
//!
//! The golden was synthesized directly from runtime types (Pitfall 3 — no source
//! `.xlsx` ever existed). This proof authors a NEUTRAL fixture (NOT via umya — the
//! provenance gate refuses a umya-authored fixture) carrying cached formula values
//! (the reconcile oracle), compiles it via the dev-only trusted-fixture override,
//! and asserts the five structural-equivalence dimensions defined in 93-07-PLAN
//! `<interfaces>`:
//!
//!   1. Normalized-JSON equality on the LOAD-BEARING semantic members the synth
//!      path reproduces (executable.ir.json, cell_map.json). The manifest's
//!      provenance-only metadata (`source`, `ratified_by`, `meaning`, ...) is NOT
//!      reproducible from a real compile (synthesis derives `source` from colour
//!      and leaves names/meanings unset, whereas the golden was hand-authored with
//!      `source: "synthetic-fixture"`), so manifest equality is asserted on the
//!      SEMANTIC projection (per-cell role + dtype + named-output identity), not
//!      the full provenance string set — see the DEVIATION note below.
//!   2. All seven members present.
//!   3. BUNDLE.lock combined hash recomputes (via the runtime helper) + bundle_id.
//!   4. The emitted bundle loads via pmcp-server-toolkit::workbook.
//!   5. Named-output names/dtypes/roles match the golden's.
//!
//! Run with: `cargo test -p pmcp-workbook-compiler --test reemit_tax_calc_golden --features trusted-fixture`

// Shared neutral-fixture authoring (rust_xlsxwriter; cached results; NOT umya).
#[path = "support/tax_calc_fixture.rs"]
mod tax_calc_fixture;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pmcp_workbook_compiler::{
    build_bundle_lock, compile_workbook, compile_workbook_with_fixture_override,
};
use serde_json::Value;

/// The committed golden bundle dir (workspace sibling, relative to THIS crate).
fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0")
}

/// The committed neutral fixture (`tests/fixtures/tax-calc.xlsx`).
fn committed_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tax-calc.xlsx")
}

/// Write the neutral fixture into `dir/tax-calc.xlsx`: copy the COMMITTED fixture
/// when present, else author it fresh (deterministic — identical bytes). Returns
/// the written path.
fn place_fixture(dir: &Path) -> PathBuf {
    let xlsx = dir.join("tax-calc.xlsx");
    let committed = committed_fixture();
    if committed.exists() {
        std::fs::copy(&committed, &xlsx).expect("copy committed fixture");
    } else {
        std::fs::write(&xlsx, tax_calc_fixture::author_tax_calc_xlsx()).expect("author fixture");
    }
    xlsx
}

/// Place the neutral fixture + override into a fresh scratch dir, compile it via
/// the trusted-fixture override, and return the emitted bundle dir.
fn compile_fixture() -> (tempfile::TempDir, PathBuf) {
    let scratch = tempfile::TempDir::new().expect("scratch dir");
    let xlsx = place_fixture(scratch.path());
    std::fs::write(
        scratch.path().join("tax-calc.provenance-override.json"),
        tax_calc_fixture::PROVENANCE_OVERRIDE_JSON,
    )
    .expect("write override");

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

/// Check (1) — normalized-JSON equality on a load-bearing semantic member.
#[test]
fn structural_eq_check1_executable_ir_normalized_json_equal() {
    let (_scratch, bundle) = compile_fixture();
    let emitted = read_json(&bundle, "executable.ir.json");
    let golden = read_json(&golden_dir(), "executable.ir.json");
    assert_eq!(
        emitted, golden,
        "the re-emitted executable.ir.json must be normalized-JSON equal to the golden \
         (the formula DAG is the load-bearing semantic member)"
    );
}

/// Check (1, cont.) — cell_map.json seed-coordinate equality (the served I/O
/// contract: the executor seeds/reads each cell by its `seed_coord`). The `unit`
/// is provenance metadata the golden hand-authored (`"USD"`/`"ratio"`) that a real
/// synthesis pass cannot derive from the workbook (no unit signal in cells), so it
/// is NOT compared here (DEVIATION note in the module doc); the LOAD-BEARING seed
/// coordinates are asserted equal per direction.
#[test]
fn structural_eq_check1_cell_map_seed_coords_equal() {
    let (_scratch, bundle) = compile_fixture();
    let emitted = read_json(&bundle, "cell_map.json");
    let golden = read_json(&golden_dir(), "cell_map.json");

    let coords = |v: &Value, dir: &str| -> Vec<String> {
        let mut c: Vec<String> = v[dir]
            .as_array()
            .expect("array")
            .iter()
            .map(|e| e["seed_coord"].as_str().expect("seed_coord").to_string())
            .collect();
        c.sort();
        c
    };
    assert_eq!(
        coords(&emitted, "inputs"),
        coords(&golden, "inputs"),
        "input seed_coords match the golden"
    );
    assert_eq!(
        coords(&emitted, "outputs"),
        coords(&golden, "outputs"),
        "output seed_coords match the golden"
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
    assert!(lock.get("bundle_id").is_some(), "BUNDLE.lock carries bundle_id (D-17)");

    let ir_json = std::fs::read_to_string(bundle.join("executable.ir.json")).expect("ir");
    let manifest_json = std::fs::read_to_string(bundle.join("manifest.json")).expect("manifest");
    let recomputed = build_bundle_lock(
        lock["bundle_id"].as_str().expect("bundle_id"),
        lock["version"].as_str().expect("version"),
        lock["workbook_hash"].as_str().expect("workbook_hash").to_string(),
        &ir_json,
        &manifest_json,
        lock["artifacts"]["evidence"].as_str().expect("evidence hash"),
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
    assert_eq!(loaded.cell_map.outputs.len(), 4, "four named outputs served");
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
    assert_eq!(
        emitted_outputs.keys().collect::<Vec<_>>(),
        golden_outputs.keys().collect::<Vec<_>>(),
        "the same output cells are declared"
    );
    for (cell, (e_role, e_dtype, e_name)) in &emitted_outputs {
        let (g_role, g_dtype, g_name) = &golden_outputs[cell];
        assert_eq!(e_role, g_role, "output {cell} role matches");
        assert_eq!(e_dtype, g_dtype, "output {cell} dtype matches");
        assert_eq!(e_name, g_name, "output {cell} named-range name matches");
    }
}

/// The override CANNOT weaken production refusal: the SAME fixture bytes compiled
/// on the PRODUCTION path ([`compile_workbook`], which always enforces the
/// provenance refuse path) are REFUSED — the trusted-fixture override is reachable
/// only on the dev/test path and never relaxes production.
#[test]
fn override_does_not_weaken_production_refusal() {
    let scratch = tempfile::TempDir::new().expect("scratch");
    let xlsx = place_fixture(scratch.path());
    let out_root = scratch.path().join("out");
    std::fs::create_dir_all(&out_root).expect("out root");

    let result = compile_workbook(&xlsx, &out_root, "tax-calc", "1.1.0", "prod-approver");
    assert!(
        result.is_err(),
        "the production compile_workbook MUST refuse the non-Excel-authored fixture \
         (the trusted-fixture override is dev/test-only and never weakens production)"
    );
}

/// OPTIONAL non-blocking STRETCH (O-2): byte-identical re-emit. The golden was
/// hand-authored with provenance metadata a real compile cannot reproduce
/// (`source: "synthetic-fixture"`, hand-written meanings), so byte-identity is NOT
/// expected — this is logged, never asserted (structural equivalence is the
/// pre-decided default and keeps the golden frozen).
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
