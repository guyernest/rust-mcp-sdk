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

use assert_cmd::Command;
use pmcp_workbook_compiler::{read_gate_marker, write_gate_marker};

/// Absolute path to the sole reusable Phase-93 workbook fixture.
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

// ---------------------------------------------------------------------------
// Task 2 — end-to-end CLI integration driving the REAL `cargo pmcp workbook`
// binary over the reused Phase-93 fixture (WBCL-01..04, D-05/D-06/D-08/D-09/D-10).
//
// Every compile/emit assertion reflects the fixture's ACTUAL production outcome:
// the fixture declares no `version` named range, so the seed-lane
// `read_workbook_version` step REFUSES it (clean typed error, exit 1, NO bundle
// written) BEFORE the gate — the correctness boundary working. The emit
// hash-covered `(false, true)` marker contract is asserted directly against the
// 94-00 library channel (the exact channel the emit handler stamps), which is
// reachable independent of the provenance-refused fixture.
// ---------------------------------------------------------------------------

/// A `cargo pmcp` binary handle with deterministic, color-free, quiet-less env so
/// the asserted substrings are stable across machines/CI.
fn workbook_cmd() -> Command {
    let mut cmd = Command::cargo_bin("cargo-pmcp").expect("cargo-pmcp binary must be available");
    // NO_COLOR keeps the UNGATED banner substring ANSI-free; clearing PMCP_QUIET so
    // the test environment cannot suppress a safety banner.
    cmd.env("NO_COLOR", "1").env_remove("PMCP_QUIET");
    cmd
}

/// The fixture's well-known no-`version` refusal message (D-02/D-11 — the bundle
/// version MUST come from the workbook). Asserted on the compile/emit paths.
const NO_VERSION_REFUSAL: &str = "declares no `version`";

/// WBCL-02 / D-10: `workbook lint <fixture>` over the clean fixture exits 0 and
/// reports no dialect findings (lint does not enforce provenance).
#[test]
fn lint_over_the_fixture_exits_zero_with_no_findings() {
    let assert = workbook_cmd()
        .args(["workbook", "lint"])
        .arg(fixture_xlsx())
        .assert()
        .success();
    let out = assert.get_output();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("no dialect findings"),
        "a clean fixture lint reports no findings (D-10):\nstdout={stdout}"
    );
}

/// WBCL-02 / D-09: `workbook lint --format json` emits parseable JSON on stdout
/// (the library `LintReport` serde type — no parallel DTO). A clean fixture yields
/// an empty `findings` array.
#[test]
fn lint_format_json_emits_parseable_json_on_stdout() {
    let assert = workbook_cmd()
        .args(["workbook", "lint"])
        .arg(fixture_xlsx())
        .args(["--format", "json"])
        .assert()
        .success();
    let out = assert.get_output();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
        .expect("lint --format json must be valid JSON on stdout");
    assert!(
        parsed.get("findings").and_then(|f| f.as_array()).is_some(),
        "the JSON carries a `findings` array (D-09): {stdout}"
    );
    assert_eq!(
        parsed["findings"].as_array().expect("findings array").len(),
        0,
        "the clean fixture has zero findings"
    );
}

/// WBCL-01 / seed lane: `workbook compile <fixture> --workflow .. --approver ..`
/// over the available fixture produces the ACTUAL production outcome — a clean
/// typed REFUSAL (exit 1, the correctness boundary), NOT a written bundle, because
/// the fixture declares no `version` named range. No `{id}@{version}/` dir is
/// minted into the out dir.
#[test]
fn compile_seed_lane_over_the_fixture_refuses_a_versionless_workbook() {
    let out_dir = tempfile::tempdir().expect("tempdir for the compile out-root");
    let assert = workbook_cmd()
        .args(["workbook", "compile"])
        .arg(fixture_xlsx())
        .args(["--workflow", "tax-calc", "--approver", "alice"])
        .arg("--out")
        .arg(out_dir.path())
        .assert()
        .failure();
    let out = assert.get_output();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains(NO_VERSION_REFUSAL),
        "the seed-lane compile REFUSES the versionless fixture (the boundary works):\n{combined}"
    );
    // gate-before-write: the refusal mints NO bundle dir.
    let minted: Vec<_> = std::fs::read_dir(out_dir.path())
        .expect("read out dir")
        .filter_map(std::result::Result::ok)
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert!(
        minted.is_empty(),
        "a refused compile writes NOTHING into the out dir, found: {minted:?}"
    );
}

/// D-06 (fixture-independent, ALWAYS asserted): `workbook compile` WITHOUT
/// `--approver` is rejected by clap (a required flag) with a non-zero exit — there
/// is NO git-identity fallback.
#[test]
fn compile_without_approver_is_rejected_by_clap() {
    let assert = workbook_cmd()
        .args(["workbook", "compile"])
        .arg(fixture_xlsx())
        .args(["--workflow", "tax-calc"])
        .assert()
        .failure();
    let out = assert.get_output();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--approver"),
        "clap names the missing required --approver flag (D-06):\n{stderr}"
    );
}

/// WBCL-03 / D-08: `workbook emit <fixture> --workflow ..` over the available
/// fixture produces the ACTUAL outcome — the same clean refusal (the versionless
/// workbook is rejected before any bundle is written). No bundle is minted.
#[test]
fn emit_over_the_fixture_refuses_a_versionless_workbook() {
    let out_dir = tempfile::tempdir().expect("tempdir for the emit out-root");
    let assert = workbook_cmd()
        .args(["workbook", "emit"])
        .arg(fixture_xlsx())
        .args(["--workflow", "tax-calc"])
        .arg("--out")
        .arg(out_dir.path())
        .assert()
        .failure();
    let out = assert.get_output();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains(NO_VERSION_REFUSAL),
        "the emit path REFUSES the versionless fixture (the boundary works):\n{combined}"
    );
    let minted: Vec<_> = std::fs::read_dir(out_dir.path())
        .expect("read out dir")
        .filter_map(std::result::Result::ok)
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert!(
        minted.is_empty(),
        "a refused emit writes NOTHING into the out dir, found: {minted:?}"
    );
}

/// WBCL-03 / D-08 (94-00 marker contract): the HASH-COVERED `gated: false` marker
/// that the emit handler stamps round-trips through the library channel to
/// `(false, true)` — `gated: false` AND `digest_ok` — and is TAMPER-EVIDENT (an
/// edited marker without an updated digest flips `digest_ok` false). This asserts
/// the exact `read_gate_marker` contract the emit E2E would observe on an accepted
/// fixture; it is fixture-independent because it exercises the marker channel
/// directly (the fixture is provenance-refused, so the CLI never reaches a write).
#[test]
fn emit_ungated_marker_is_hash_covered_and_tamper_evident() {
    let bundle_dir = tempfile::tempdir().expect("tempdir for the bundle dir");
    write_gate_marker(bundle_dir.path(), false).expect("stamp the ungated marker");

    let (gated, digest_ok) = read_gate_marker(bundle_dir.path()).expect("read the marker");
    assert!(!gated, "emit stamps an UNGATED marker (gated: false), D-08");
    assert!(digest_ok, "the marker is hash-covered (digest_ok), 94-00");

    // Tamper: edit gate.json without updating the digest → digest_ok flips false.
    std::fs::write(
        bundle_dir.path().join("evidence/gate.json"),
        "{\n  \"gated\": true\n}",
    )
    .expect("corrupt the marker");
    let (_g, digest_ok_after) = read_gate_marker(bundle_dir.path()).expect("re-read the marker");
    assert!(
        !digest_ok_after,
        "a stripped/edited marker is DETECTABLE (digest_ok == false), T-94-04-UNGATED"
    );
}

/// WBCL-04 / D-05: bare `workbook compile` (no path/bundle-id) over a TWO-entry
/// `pmcp.toml` attempts BOTH declared workbooks with CONTINUE-ON-ERROR — even
/// though the first entry fails (versionless), the second is still attempted, and
/// the run reduces to a worst-status exit 1. Both per-workbook errors surface.
#[test]
fn compile_all_over_two_entry_toml_attempts_both_with_continue_on_error() {
    let proj = tempfile::tempdir().expect("tempdir for the project root");
    // Copy the fixture in as the two declared workbooks.
    std::fs::copy(fixture_xlsx(), proj.path().join("a.xlsx")).expect("copy a.xlsx");
    std::fs::copy(fixture_xlsx(), proj.path().join("b.xlsx")).expect("copy b.xlsx");
    std::fs::write(
        proj.path().join("pmcp.toml"),
        "[[workbook]]\npath = \"a.xlsx\"\nbundle_id = \"a\"\nout_dir = \"dist/a\"\n\n\
         [[workbook]]\npath = \"b.xlsx\"\nbundle_id = \"b\"\nout_dir = \"dist/b\"\n",
    )
    .expect("write the two-entry pmcp.toml");

    let assert = workbook_cmd()
        .current_dir(proj.path())
        .args(["workbook", "compile", "--approver", "alice"])
        .assert()
        .failure();
    let out = assert.get_output();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    // BOTH declared workbooks were attempted (continue-on-error): each surfaces its
    // own per-workbook error line naming its bundle_id.
    assert!(
        combined.contains("error: a "),
        "the FIRST declared workbook (a) was attempted:\n{combined}"
    );
    assert!(
        combined.contains("error: b "),
        "the SECOND declared workbook (b) was attempted despite the first failing \
         (continue-on-error, WBCL-04/D-05):\n{combined}"
    );
}
