//! Byte-stability + boot-integrity + tamper negative-path tests for the committed
//! `tax-calc@1.1.0` golden bundle (Phase 92 Plan 02, Tasks 2 & 3).
//!
//! - [`golden_regeneration_is_byte_identical`] is the CI mechanism enforcing D-03:
//!   any code change that would alter a golden artifact is caught because the
//!   in-repo generator regenerates the bundle into a tempdir and every committed
//!   golden member must match its freshly-generated counterpart byte-for-byte.
//! - [`golden_passes_boot_integrity`] proves the committed golden passes the same
//!   fail-closed `load_bundle` gate real bundles do (WBSV-08).
//! - The `tamper_*` tests prove the copy-then-corrupt helpers each provoke a
//!   DISTINCT fail-closed `BundleLoadError` variant (D-05 — no committed corrupt
//!   fixtures; corruption happens in a tempdir copy).
//!
//! The whole binary is gated on the `workbook` feature (the generator + the
//! runtime loader only exist there). Run with:
//! `cargo test -p pmcp-server-toolkit --features workbook --test fixture_byte_stability`.

#![cfg(feature = "workbook")]

mod support;

use pmcp_workbook_runtime::bundle_loader::ALLOWED_MEMBERS;
use pmcp_workbook_runtime::{load_bundle, BundleLoadError, LocalDirSource};

use support::fixture_gen::{generate_tax_calc_bundle, BUNDLE_ID, VERSION};
use support::tamper::{self, golden_dir};

/// The seven members the golden must carry — the loader's OWN frozen allow-set,
/// so the test cannot desync from the canonical member table.
const MEMBERS: &[&str] = ALLOWED_MEMBERS;

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
    // Full output surface (no privileged headline — WBSV-01): the named outputs
    // across every per-Table tool (WBV2-04 multi-tool model). Two tools:
    // Calculate_Tax (6 outputs — 4 numeric + the WBVER-01 D-07 text + bool formula
    // outputs bracket_label/is_taxable) + Estimate_Refund (1 output) = 7.
    let output_count: usize = bundle.cell_map.tools.iter().map(|t| t.outputs.len()).sum();
    assert_eq!(
        output_count, 7,
        "golden carries seven named outputs across two tools \
         (incl. the text + bool formula outputs added for WBVER-01 / D-07)"
    );
    assert_eq!(
        bundle.cell_map.tools.len(),
        2,
        "golden fans out into two named tools (Calculate_Tax + Estimate_Refund)"
    );
    assert_eq!(
        bundle.cell_map.inputs.len(),
        4,
        "golden carries four inputs (income + enum + deductions + withheld)"
    );
    // D-18: at least one declared annotation (bracket boundaries).
    assert!(
        !bundle.manifest.annotations.is_empty(),
        "golden manifest declares bracket-boundary annotations"
    );
}

// === Task 3: tamper negative paths (D-05 — copy-then-corrupt in a tempdir) ===

/// T-92-01: a single byte mutation in a hash-covered member is rejected with
/// `IntegrityMismatch`.
#[test]
fn tamper_flip_byte_provokes_integrity_mismatch() {
    let dir = tamper::copy_golden_to_temp();
    tamper::flip_byte(dir.path(), "manifest.json");
    let source = LocalDirSource::new(dir.path());
    match load_bundle(&source) {
        Err(BundleLoadError::IntegrityMismatch {
            expected,
            recomputed,
            ..
        }) => assert_ne!(expected, recomputed, "diagnostic carries found-vs-expected"),
        other => panic!("expected IntegrityMismatch, got {other:?}"),
    }
}

/// A deleted member surfaces as a load error (missing member), never a panic.
#[test]
fn tamper_delete_artifact_provokes_load_error() {
    let dir = tamper::copy_golden_to_temp();
    tamper::delete_artifact(dir.path(), "manifest.json");
    let source = LocalDirSource::new(dir.path());
    assert!(
        load_bundle(&source).is_err(),
        "a deleted member must fail closed (load error), not load successfully"
    );
}

/// T-92-02: a lock whose version disagrees with the changelog `to_version` is
/// rejected with `StampMismatch` on the `version` field.
#[test]
fn tamper_desync_lock_version_provokes_stamp_mismatch() {
    let dir = tamper::copy_golden_to_temp();
    tamper::desync_lock_version(dir.path());
    let source = LocalDirSource::new(dir.path());
    match load_bundle(&source) {
        Err(BundleLoadError::StampMismatch { field, .. }) => {
            assert_eq!(
                field, "version",
                "version desync fires the version stamp gate"
            )
        },
        other => panic!("expected StampMismatch on version, got {other:?}"),
    }
}

/// T-92-22 / Codex MEDIUM #9: an extra member outside the frozen allow-set is
/// rejected with `UnexpectedMember` BEFORE parsing.
#[test]
fn tamper_add_unexpected_member_provokes_unexpected_member() {
    let dir = tamper::copy_golden_to_temp();
    tamper::add_unexpected_member(dir.path());
    let source = LocalDirSource::new(dir.path());
    match load_bundle(&source) {
        Err(BundleLoadError::UnexpectedMember { member }) => assert!(
            member.ends_with("sneaky.json"),
            "the unexpected member is named in the diagnostic, got {member}"
        ),
        other => panic!("expected UnexpectedMember, got {other:?}"),
    }
}
