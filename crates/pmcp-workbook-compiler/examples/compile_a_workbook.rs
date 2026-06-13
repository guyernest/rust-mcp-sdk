//! Library-level `ingest → emit` demonstration of the generic compiler driver.
//!
//! Authors the neutral `tax-calc.xlsx` fixture (3 inputs incl. an inline-DV enum,
//! a governed bracket table, 4 named outputs, real formulas WITH cached values),
//! then compiles it through [`pmcp_workbook_compiler::compile_workbook_with_fixture_override`]
//! — the SAME synth-driven pipeline the production [`compile_workbook`] runs, but
//! honouring the committed trusted-fixture provenance override so a non-Excel
//! authored fixture is admitted on this DEV path. Production
//! [`compile_workbook`](pmcp_workbook_compiler::compile_workbook) still REFUSES the
//! same bytes.
//!
//! Run with:
//! `cargo run -p pmcp-workbook-compiler --example compile_a_workbook --features trusted-fixture`
//!
//! The CLI front-end is Phase 94 — this is the library-level demonstration.

// The shared fixture authoring (NOT umya; rust_xlsxwriter writer with cached
// results). `include!`d so the example and the proof share ONE authoring path.
#[path = "../tests/support/tax_calc_fixture.rs"]
mod tax_calc_fixture;

use std::path::Path;

use pmcp_workbook_compiler::compile_workbook_with_fixture_override;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("tax-calc-example-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let xlsx_path = dir.join("tax-calc.xlsx");

    // Prefer the COMMITTED neutral fixture (`tests/fixtures/tax-calc.xlsx`); fall
    // back to authoring it fresh (the authoring is deterministic, so both paths
    // produce the same bytes). The committed override marks it as the trusted
    // fixture for the dev/test override path.
    let committed = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/tax-calc.xlsx");
    if Path::new(committed).exists() {
        std::fs::copy(committed, &xlsx_path)?;
        println!("using committed neutral fixture: {committed}");
    } else {
        std::fs::write(&xlsx_path, tax_calc_fixture::author_tax_calc_xlsx())?;
        println!("authored neutral fixture: {}", xlsx_path.display());
    }
    std::fs::write(
        dir.join("tax-calc.provenance-override.json"),
        tax_calc_fixture::PROVENANCE_OVERRIDE_JSON,
    )?;

    // Compile: ingest → stage1 (lint+synth+freshness) → ratify → parse+DAG →
    // executor → reconcile → emit the seven-member bundle.
    let out_root = dir.join("out");
    std::fs::create_dir_all(&out_root)?;
    let lock = compile_workbook_with_fixture_override(
        &xlsx_path,
        &out_root,
        "tax-calc",
        "1.1.0",
        "example-approver",
    )?;

    let bundle_dir = out_root.join("tax-calc@1.1.0");
    println!("emitted bundle: {}", bundle_dir.display());
    println!(
        "  bundle_id={} version={} combined_hash={}",
        lock.bundle_id, lock.version, lock.combined
    );
    for member in [
        "manifest.json",
        "executable.ir.json",
        "cell_map.json",
        "layout.json",
        "BUNDLE.lock",
        "evidence/changelog.json",
        "evidence/parser_equivalence.json",
    ] {
        let present = bundle_dir.join(member).exists();
        println!("  [{}] {member}", if present { "x" } else { " " });
    }

    // Clean up the scratch dir (the demonstration is the printed loop above).
    let _ = std::fs::remove_dir_all(&dir);
    println!("ingest -> emit complete (seven-member bundle produced)");
    Ok(())
}
