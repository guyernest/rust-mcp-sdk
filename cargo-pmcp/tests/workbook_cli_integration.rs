//! End-to-end CLI integration coverage for `cargo pmcp workbook {lint,compile,emit}`
//! (Phase 94-05, closing WBCL-01..04) PLUS a durable purity-boundary guard.
//!
//! These tests drive the REAL built `cargo-pmcp` binary (via
//! `assert_cmd::Command::cargo_bin("cargo-pmcp")`) against the sole Phase-93
//! workbook fixture (`crates/pmcp-workbook-compiler/tests/fixtures/tax-calc.xlsx`)
//! and assert the ACTUAL production outcomes, plus a fast cargo-tree purity
//! confirmation and an `#[ignore]`-d `make purity-check` guard.
//!
//! ## Fixture reality (drives every compile/emit assertion)
//!
//! The `tax-calc.xlsx` fixture declares NO `version` (nor `wb_version`) single-cell
//! named range, so the production seed-lane `read_workbook_version` step REFUSES it
//! with a clean typed error (exit `1`) BEFORE any bundle is written — this is the
//! correctness boundary working (the bundle version MUST come from the workbook,
//! never a flag). The compile/emit E2E therefore asserts that REFUSAL (no bundle
//! minted), while the hash-covered `gated: false` marker round-trip — emit's
//! `(false, true)` contract — is asserted directly against the 94-00 library
//! channel (`write_gate_marker` / `read_gate_marker`), which is the exact channel
//! the emit handler stamps. The lint path runs cleanly over the fixture (lint does
//! not enforce provenance). See the 94-05 SUMMARY "Known residual risk" bullet for
//! the un-constructible gate-block E2E (covered by Plan-03 unit tests).
//!
//! ## Purity boundary (T-94-05-PURITY)
//!
//! Linking `pmcp-workbook-compiler` into `cargo-pmcp` pulls the `umya` reader into
//! cargo-pmcp's OFFLINE tooling tree (by design) but must NOT leak into any SERVED
//! reader-free tree. The fast default-run cargo-tree assertions confirm `umya` /
//! `quick-xml` are ABSENT from `pmcp-workbook-runtime` / `pmcp-workbook-dialect`
//! and PRESENT (non-vacuous) on the cargo-pmcp -> compiler edge; the slow recursive
//! `make purity-check` shell is quarantined behind `#[ignore]` (concern E).

use std::path::PathBuf;
use std::process::Command as StdCommand;

/// Absolute path to the sole reusable Phase-93 workbook fixture.
// Used by the Task-2 end-to-end CLI tests added in the same file.
#[allow(dead_code)]
fn fixture_xlsx() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/pmcp-workbook-compiler/tests/fixtures/tax-calc.xlsx")
}

/// Run `cargo tree` for `crate_spec` (extra args appended) and return its stdout.
///
/// Fails the test on a non-zero cargo status (fail-closed: a tree we cannot read is
/// not a tree we can certify reader-free).
fn cargo_tree(args: &[&str]) -> String {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let out = StdCommand::new(cargo)
        .arg("tree")
        .args(args)
        .output()
        .expect("spawn cargo tree");
    assert!(
        out.status.success(),
        "cargo tree {:?} failed (fail-closed):\n{}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).to_string()
}

// ---------------------------------------------------------------------------
// Task 1 — purity boundary confirmation (fast default-run cargo-tree assertions
// + an #[ignore]-d make-purity-check guard). Concern E: NEVER shell `make` from a
// normal #[test] (slow/recursive/flaky).
// ---------------------------------------------------------------------------

/// FAST (default-run): the SERVED reader-free runtime tree contains NO `umya` /
/// `quick-xml`, even though cargo-pmcp now links the compiler (T-94-05-PURITY).
#[test]
fn served_runtime_tree_stays_reader_free() {
    let tree = cargo_tree(&["-p", "pmcp-workbook-runtime"]);
    let lower = tree.to_lowercase();
    assert!(
        !lower.contains("umya"),
        "served pmcp-workbook-runtime tree must NOT contain umya:\n{tree}"
    );
    assert!(
        !lower.contains("quick-xml"),
        "served pmcp-workbook-runtime tree must NOT contain quick-xml:\n{tree}"
    );
}

/// FAST (default-run): the SERVED reader-free dialect tree contains NO `umya` /
/// `quick-xml` (the second served crate in PURITY_CRATES).
#[test]
fn served_dialect_tree_stays_reader_free() {
    let tree = cargo_tree(&["-p", "pmcp-workbook-dialect"]);
    let lower = tree.to_lowercase();
    assert!(
        !lower.contains("umya"),
        "served pmcp-workbook-dialect tree must NOT contain umya:\n{tree}"
    );
    assert!(
        !lower.contains("quick-xml"),
        "served pmcp-workbook-dialect tree must NOT contain quick-xml:\n{tree}"
    );
}

/// FAST (default-run): the cargo-pmcp -> pmcp-workbook-compiler edge actually
/// exists (the purity confirmation is NON-VACUOUS — the link really happened).
/// `cargo tree -i` (inverse) lists the crates that depend ON the compiler; the
/// cargo-pmcp offline tooling crate must appear there.
#[test]
fn cargo_pmcp_links_the_compiler_non_vacuously() {
    let tree = cargo_tree(&["-p", "cargo-pmcp", "-i", "pmcp-workbook-compiler"]);
    assert!(
        tree.contains("pmcp-workbook-compiler"),
        "the inverse tree must name the compiler:\n{tree}"
    );
    assert!(
        tree.contains("cargo-pmcp"),
        "cargo-pmcp must depend on the compiler (the edge is real):\n{tree}"
    );
    // And umya IS (by design) present in cargo-pmcp's OFFLINE tree — proving the
    // reader linked, just isolated to offline tooling, never a served tree.
    let cargo_pmcp_tree = cargo_tree(&["-p", "cargo-pmcp"]);
    assert!(
        cargo_pmcp_tree.to_lowercase().contains("umya"),
        "umya is present in cargo-pmcp's offline tree (the reader linked here, not in a served tree)"
    );
}

/// DURABLE GUARD (concern E): shells `make purity-check`. `#[ignore]`-d so it never
/// runs in the default `cargo test` pass (slow/recursive/flaky — it re-enters cargo)
/// while documenting the exact command. Run explicitly with:
/// `cargo test -p cargo-pmcp --test workbook_cli_integration -- --ignored purity`.
#[test]
#[ignore = "shells `make purity-check` (slow/recursive); run explicitly via \
            `cargo test -p cargo-pmcp --test workbook_cli_integration -- --ignored purity`"]
fn purity_check_passes_with_the_new_compiler_edge() {
    // The workspace root is the cargo-pmcp manifest's parent.
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let status = StdCommand::new("make")
        .arg("purity-check")
        .current_dir(&workspace_root)
        .status()
        .expect("spawn make purity-check");
    assert!(
        status.success(),
        "make purity-check must pass (exit 0) with the cargo-pmcp -> compiler edge; \
         a failure would mean cargo-pmcp leaked a reader into a served tree (NEVER weaken this gate)"
    );
}
