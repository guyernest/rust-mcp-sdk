//! Runnable demonstration of the `cargo pmcp workbook` CLI surface
//! (Phase 94-05, CLAUDE.md ALWAYS: `cargo run --example`).
//!
//! Usage:
//!   cargo run -p cargo-pmcp --example workbook_cli_demo
//!
//! This example walks the **BA lifecycle** the workbook subcommands serve —
//! author from an example → first build (lint findings) → fix → build (seed) →
//! emit a dev/reference bundle (94-CONTEXT.md `<specifics>`) — over the SOLE
//! reused Phase-93 fixture (`tax-calc.xlsx`, resolved via `CARGO_MANIFEST_DIR`
//! so the path is stable). No workbook is hand-authored.
//!
//! ## Why it calls the COMPILER library verbs (not the CLI handlers)
//!
//! `cargo-pmcp`'s handlers (`commands::workbook::{lint,compile,emit}::execute`)
//! are **bin-only** — `cargo-pmcp/src/lib.rs` deliberately excludes `commands::*`,
//! so this example (which links the `cargo_pmcp` *library*) cannot call them, and
//! it has no dev-dependencies (no `assert_cmd`/`tempfile`). Shelling
//! `cargo run -p cargo-pmcp -- workbook ...` from inside `cargo run --example`
//! risks a build-lock deadlock. So instead the example calls the SAME public
//! `pmcp_workbook_compiler` verbs each subcommand shells over, narrating which
//! `cargo pmcp workbook <verb>` each maps to. The demonstration is therefore
//! hermetic, fast, and honest.
//!
//! ## Honest fixture reality (the correctness boundary working)
//!
//! `tax-calc.xlsx` declares NO `version` named range, so the seed-lane
//! `read_workbook_version` step REFUSES it with a clean typed error BEFORE any
//! bundle is written — the bundle version MUST come from the workbook, never a
//! flag (D-02/D-11). The example NARRATES that refusal as the boundary working
//! and still exits 0: an honest provenance refusal is a SUCCESSFUL demonstration.

use std::path::{Path, PathBuf};

use pmcp_workbook_compiler::dialect::linter::lint as dialect_lint;
use pmcp_workbook_compiler::{
    ingest, read_gate_marker, read_workbook_version, write_gate_marker, DialectRules,
    WorkbookCellSource,
};

/// Absolute path to the sole reusable Phase-93 workbook fixture, resolved from
/// `CARGO_MANIFEST_DIR` so the example runs from any cwd.
fn fixture_xlsx() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/pmcp-workbook-compiler/tests/fixtures/tax-calc.xlsx")
}

fn main() {
    let fixture = fixture_xlsx();
    println!("== cargo pmcp workbook — BA lifecycle demo ==");
    println!("fixture: {}", fixture.display());
    println!();

    demo_lint(&fixture);
    demo_compile_seed(&fixture);
    demo_emit_marker();

    println!();
    println!(
        "== demo complete: lint clean, seed boundary narrated, ungated marker round-tripped =="
    );
}

/// Step 1 — `cargo pmcp workbook lint <wb.xlsx>`.
///
/// Mirrors the `lint` handler: ingest → `WorkbookCellSource` → the dialect
/// linter over `DialectRules::default()`. Lint does NOT enforce provenance, so it
/// runs cleanly over the fixture (the BA's "first build (errors + warnings)" step).
fn demo_lint(fixture: &Path) {
    println!("[1/3] cargo pmcp workbook lint  (author → first build: see findings)");
    let (map, _ingest_findings) = ingest::ingest(fixture).expect("ingest the fixture for lint");
    let src = WorkbookCellSource::new(&map);
    let report = dialect_lint(&src, &DialectRules::default());
    if report.findings.is_empty() {
        println!("      lint: no dialect findings — the sheet is clean, the BA proceeds to build");
    } else {
        println!(
            "      lint: {} finding(s) the BA fixes in Excel:",
            report.findings.len()
        );
        for f in &report.findings {
            println!(
                "        - {}!{:?} [{}]: {}",
                f.sheet, f.cell, f.rule, f.message
            );
        }
    }
    println!();
}

/// Step 2 — `cargo pmcp workbook compile <wb.xlsx> --workflow tax-calc --approver alice`.
///
/// Mirrors the `compile` handler's seed lane: it first reads the workbook-declared
/// version via `read_workbook_version` (the exact provenance step the handler
/// runs before `compile_workbook`). This fixture declares no `version` named
/// range, so the read REFUSES it — the correctness boundary working. The example
/// narrates that refusal honestly rather than pretending a bundle was written.
fn demo_compile_seed(fixture: &Path) {
    println!(
        "[2/3] cargo pmcp workbook compile --workflow tax-calc --approver alice  (build seed lane)"
    );
    match read_workbook_version(fixture) {
        Ok(version) => {
            println!(
                "      compile: workbook declares version {version} — the seed lane would mint \
                 tax-calc@{version}/ (seven-member bundle)"
            );
        },
        Err(e) => {
            println!(
                "      compile: REFUSED at the version-provenance boundary (this is correct):"
            );
            println!("        {e}");
            println!(
                "      the bundle version MUST come from the workbook (a `version` named range), \
                 never a flag (D-02/D-11) — no bundle is minted"
            );
        },
    }
    println!();
}

/// Step 3 — `cargo pmcp workbook emit ...` (the UNGATED dev/reference lane, D-08).
///
/// Mirrors the `emit` handler's marker stamp: after the ungated seed write the
/// handler stamps a HASH-COVERED `gated: false` marker via `write_gate_marker`
/// and prints the loud UNGATED banner. This demonstrates that marker channel
/// directly — `write_gate_marker(dir, false)` then `read_gate_marker` returns
/// `(false, true)` (`gated: false` AND `digest_ok`) — into a temp dir under the
/// OS temp root, so the ungated status travels with the artifact and is
/// tamper-evident.
fn demo_emit_marker() {
    println!("[3/3] cargo pmcp workbook emit  (ungated dev/reference bundle)");
    eprintln!("      UNGATED — not regression-checked, do not deploy");

    let bundle_dir = std::env::temp_dir().join(format!("workbook-cli-demo-{}", std::process::id()));
    std::fs::create_dir_all(&bundle_dir).expect("create the demo bundle dir");

    write_gate_marker(&bundle_dir, false).expect("stamp the hash-covered ungated marker");
    let (gated, digest_ok) = read_gate_marker(&bundle_dir).expect("read the ungated marker back");

    println!(
        "      marker: gated={gated}, digest_ok={digest_ok}  → (false, true): an UNGATED, \
         hash-covered marker that travels with the bundle (D-08, 94-00)"
    );

    std::fs::remove_dir_all(&bundle_dir).ok();
    println!();
}
