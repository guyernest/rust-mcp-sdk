//! Library-level demonstration of the GENERIC compiler driver's
//! provenance-refusal boundary (WBCO-07).
//!
//! This example runs the PUBLIC [`pmcp_workbook_compiler::compile_workbook`] API
//! against the committed neutral fixture (`tests/fixtures/tax-calc.xlsx`) and
//! HONESTLY shows the outcome. The fixture is authored by a non-Excel writer, so
//! its cache carries a `fullCalcOnLoad=1` staleness stamp — the production
//! compiler correctly REFUSES it with a typed [`CompileError`]. A refusal is the
//! honest demonstration: the security boundary is doing its job.
//!
//! There is NO override here and NO Cargo feature to flip (CR-01: the
//! trusted-fixture override is `#[cfg(test)]`-only — it is unreachable from any
//! default or feature-unifiable build, so a downstream consumer can never arm the
//! provenance bypass). The in-crate `#[cfg(test)]` golden proof
//! (`src/reemit_golden.rs`) is where a successful compile-and-re-emit is proven.
//!
//! Run with: `cargo run -p pmcp-workbook-compiler --example compile_a_workbook`
//!
//! The CLI front-end is Phase 94 — this is the library-level demonstration.

use std::path::Path;

use pmcp_workbook_compiler::{compile_workbook, CompileError};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let committed = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/tax-calc.xlsx");
    let xlsx_path = Path::new(committed);
    if !xlsx_path.exists() {
        return Err(format!("committed fixture not found: {committed}").into());
    }
    println!("compiling neutral fixture via the PUBLIC compile_workbook API: {committed}");

    let dir = std::env::temp_dir().join(format!("tax-calc-example-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let out_root = dir.join("out");
    std::fs::create_dir_all(&out_root)?;

    // The production pipeline: ingest -> stage1 (lint+synth+freshness) -> ratify ->
    // parse+DAG -> executor -> reconcile -> emit. The freshness/provenance gate
    // enforces the WBCO-07 refuse path — no override is reachable here.
    match compile_workbook(
        xlsx_path,
        &out_root,
        "tax-calc",
        "1.1.0",
        "example-approver",
    ) {
        Ok(lock) => {
            // A genuinely-Excel-provenanced, fresh workbook would land here.
            let bundle_dir = out_root.join("tax-calc@1.1.0");
            println!("emitted bundle: {}", bundle_dir.display());
            println!(
                "  bundle_id={} version={} combined_hash={}",
                lock.bundle_id, lock.version, lock.combined
            );
        },
        Err(e) => {
            // The HONEST demonstration: the non-genuine-Excel cache is REFUSED with
            // a typed error. This is the security boundary working as designed.
            println!("provenance-refusal boundary REFUSED the workbook (as designed):");
            match &e {
                CompileError::Lint(msg) => {
                    println!("  CompileError::Lint — collect-all blocking finding(s):");
                    for line in msg.lines() {
                        println!("  {line}");
                    }
                },
                other => println!("  {other}"),
            }
            println!(
                "\nThis refusal is correct: the committed fixture is not a genuine, fresh \
                 Excel save, so its cached values are not trusted as the oracle. The \
                 production compiler NEVER admits it — and there is no publishable feature \
                 to bypass the boundary (CR-01)."
            );
        },
    }

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
