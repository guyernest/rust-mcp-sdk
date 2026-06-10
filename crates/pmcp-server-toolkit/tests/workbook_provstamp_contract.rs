//! Codex HIGH #3 contract test (Phase 92 Plan 03 Task 1): the served
//! [`ProvStamp`] `combined_hash` carries the `BUNDLE.lock` COMBINED hash — never
//! the source-workbook hash.
//!
//! This integration test loads the committed `tax-calc@1.1.0` golden via the
//! fail-closed `BundleLoader`, builds the served `ProvStamp` from it, and asserts
//! the chain `envelope.combined_hash == ProvStamp.combined_hash ==
//! WorkbookBundle.stamp.combined == BundleLock.combined` — and that the value is
//! DISTINCT from `BundleLock.workbook_hash` (so the stamp can never carry the
//! source-workbook hash).

#![cfg(feature = "workbook")]

use std::path::{Path, PathBuf};

use pmcp_server_toolkit::workbook::{to_iserror_result, ProvStamp, WorkbookToolError};
use pmcp_workbook_runtime::{load_bundle, LocalDirSource};

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tax-calc@1.1.0")
}

#[test]
fn provstamp_combined_hash_equals_bundlelock_combined_for_golden() {
    let source = LocalDirSource::new(golden_dir());
    let bundle = load_bundle(&source).expect("committed golden passes boot integrity");

    let stamp = ProvStamp::from_bundle(&bundle);

    // ProvStamp.combined_hash == WorkbookBundle.stamp.combined == BundleLock.combined.
    assert_eq!(
        stamp.combined_hash, bundle.stamp.combined,
        "ProvStamp.combined_hash carries the BUNDLE.lock combined hash"
    );
    assert_eq!(stamp.bundle_id, bundle.stamp.bundle_id);
    assert_eq!(stamp.version, bundle.stamp.version);

    // Codex HIGH #3: the stamp's combined_hash is DISTINCT from the source
    // workbook hash — it can never carry the source-workbook hash.
    assert_ne!(
        stamp.combined_hash, bundle.stamp.workbook_hash,
        "the served stamp must NOT carry the source-workbook hash"
    );

    // The rendered envelope surfaces the same combined_hash under `provenance`,
    // with NO `workbook_hash` key anywhere in the stamp.
    let envelope = to_iserror_result(&WorkbookToolError::invalid_input("x"), &stamp);
    assert_eq!(
        envelope["provenance"]["combined_hash"],
        serde_json::json!(bundle.stamp.combined)
    );
    assert!(
        envelope["provenance"].get("workbook_hash").is_none(),
        "the provenance stamp must never carry a workbook_hash key"
    );
}
