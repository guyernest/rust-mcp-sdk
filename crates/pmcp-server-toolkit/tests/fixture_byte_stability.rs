//! Byte-stability + boot-integrity tests for the committed `tax-calc@1.1.0`
//! golden bundle (Phase 92 Plan 02, Task 2). The tamper negative-path tests are
//! appended by Task 3.
//!
//! - [`golden_regeneration_is_byte_identical`] is the CI mechanism enforcing D-03:
//!   any code change that would alter a golden artifact is caught because the
//!   in-repo generator regenerates the bundle into a tempdir and every committed
//!   golden member must match its freshly-generated counterpart byte-for-byte.
//! - [`golden_passes_boot_integrity`] proves the committed golden passes the same
//!   fail-closed `load_bundle` gate real bundles do (WBSV-08).
//!
//! The whole binary is gated on the `workbook` feature (the generator + the
//! runtime loader only exist there). Run with:
//! `cargo test -p pmcp-server-toolkit --features workbook --test fixture_byte_stability`.

#![cfg(feature = "workbook")]

mod support;

use std::path::{Path, PathBuf};

use pmcp_workbook_runtime::{load_bundle, LocalDirSource};

use support::fixture_gen::{generate_tax_calc_bundle, BUNDLE_ID, VERSION};

/// The committed golden bundle directory (relative to the crate manifest dir).
fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tax-calc@1.1.0")
}

/// The seven members the golden must carry, in the loader's allow-set order.
const MEMBERS: &[&str] = &[
    "executable.ir.json",
    "manifest.json",
    "cell_map.json",
    "layout.json",
    "BUNDLE.lock",
    "evidence/changelog.json",
    "evidence/parser_equivalence.json",
];

/// Regenerate the committed golden in place. Run on demand with
/// `cargo test -p pmcp-server-toolkit --features workbook --test fixture_byte_stability \
///  regenerate_committed_golden -- --ignored --exact`. NOT a normal test (it
/// writes into the source tree); the byte-stability test is the always-on guard.
#[test]
#[ignore = "writes into the committed source tree; run on demand to refresh the golden"]
fn regenerate_committed_golden() {
    let dir = golden_dir();
    std::fs::create_dir_all(&dir).expect("create golden dir");
    generate_tax_calc_bundle(&dir).expect("regenerate committed golden");
}

/// D-03 / T-92-06: the in-repo generator is byte-reproducible, and the committed
/// golden equals a fresh regeneration member-for-member. A code change that alters
/// any artifact fails this test in CI before merge.
#[test]
fn golden_regeneration_is_byte_identical() {
    let tmp = tempfile::tempdir().expect("create regen tempdir");
    generate_tax_calc_bundle(tmp.path()).expect("regenerate golden bundle");

    let golden = golden_dir();
    for member in MEMBERS {
        let committed = std::fs::read(golden.join(member))
            .unwrap_or_else(|e| panic!("read committed golden member {member}: {e}"));
        let regenerated = std::fs::read(tmp.path().join(member))
            .unwrap_or_else(|e| panic!("read regenerated member {member}: {e}"));
        assert_eq!(
            committed, regenerated,
            "committed golden {member} must be byte-identical to a fresh regeneration \
             (D-03 — regenerate the golden if this is an intended change)"
        );
    }
}

/// WBSV-08 / T-92-05: the committed golden passes its OWN fail-closed boot
/// integrity gate (the loader recomputes the evidence + combined hashes and the
/// stamp binding, and accepts the golden).
#[test]
fn golden_passes_boot_integrity() {
    let source = LocalDirSource::new(golden_dir());
    let bundle = load_bundle(&source).expect("committed golden passes boot integrity");
    assert_eq!(bundle.stamp.bundle_id, BUNDLE_ID);
    assert_eq!(bundle.stamp.version, VERSION);
    assert_eq!(bundle.manifest.workflow, BUNDLE_ID);
    assert_eq!(bundle.changelog.to_version, VERSION);
    // Full output surface (no privileged headline — WBSV-01): four named outputs.
    assert_eq!(
        bundle.cell_map.outputs.len(),
        4,
        "golden carries four named outputs"
    );
    assert_eq!(
        bundle.cell_map.inputs.len(),
        3,
        "golden carries three inputs (numeric + enum + numeric)"
    );
    // D-18: at least one declared annotation (bracket boundaries).
    assert!(
        !bundle.manifest.annotations.is_empty(),
        "golden manifest declares bracket-boundary annotations"
    );
}
