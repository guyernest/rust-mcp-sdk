//! `BUNDLE.lock` emission (ART-04/D-05) — per-artifact + combined SHA-256
//! hash-of-hashes anchored on the canonical `workbook_hash`.
//!
//! The lock records, for each emitted artifact (executable / manifest /
//! evidence), a content SHA-256 over its bytes, PLUS a COMBINED hash-of-hashes
//! that flips when ANY single artifact changes (tampering / partial-rebuild
//! detection, D-05). It also carries the `workbook_hash` provenance anchor — the
//! CANONICAL CONTENT projection (`source_workbook_hash`), NOT a raw-bytes hash
//! (the reader's writer is not byte-deterministic; D-05).
//!
//! # Re-export, never hand-roll (the keystone)
//!
//! The runtime-safe lock shapes + the hashing helpers live in
//! [`pmcp_workbook_runtime::artifact_model`] so BOTH the offline emitter and the
//! served binary's integrity check share ONE definition (and cannot drift). This
//! module re-exports them so `crate::artifact::bundle_lock::{BundleLock,
//! ArtifactHashes, build_bundle_lock}` resolves for the emit path. The combined
//! hash is NEVER hand-rolled here — the served loader recomputes it with the SAME
//! [`build_bundle_lock`] / [`fold_evidence_hash`], so any divergence would
//! false-positive the boot-time integrity gate (threat T-93-05-HASH).
//!
//! The emitted lock carries `bundle_id` (Phase 92 D-17 rename) — NOT the
//! lighthouse `workflow` field — so the Phase 92 `BundleLoader` accepts it.

pub use pmcp_workbook_runtime::{
    build_bundle_lock, fold_evidence_hash, sha256_hex, ArtifactHashes, BundleLock,
};

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in for the canonical `source_workbook_hash` content projection.
    fn workbook_hash() -> String {
        sha256_hex(b"S!A1|10|\nS!B1|0.37|")
    }

    #[test]
    fn bundle_lock_records_three_plus_combined() {
        let lock = build_bundle_lock(
            "tax-calc",
            "1.0.0",
            workbook_hash(),
            "{IR}",
            "{MANIFEST}",
            &sha256_hex(b"{EVIDENCE-DIR}"),
        );
        for h in [
            &lock.artifacts.executable,
            &lock.artifacts.manifest,
            &lock.artifacts.evidence,
            &lock.combined,
        ] {
            assert_eq!(h.len(), 64, "each hash is a 64-char sha256 hex");
            assert!(!h.is_empty(), "hashes are non-empty");
        }
        // The combined hash-of-hashes is DISTINCT from each per-artifact hash.
        assert_ne!(lock.combined, lock.artifacts.executable);
        assert_ne!(lock.combined, lock.artifacts.manifest);
        assert_ne!(lock.combined, lock.artifacts.evidence);
    }

    #[test]
    fn bundle_lock_emits_bundle_id_not_workflow() {
        // D-17 rename: the lock's identity field is `bundle_id` (the Phase 92
        // BundleLoader cross-checks it against `manifest.workflow`). We serialize
        // the lock and assert the `bundle_id` key is present (NOT `workflow`).
        let lock = build_bundle_lock(
            "tax-calc",
            "1.1.0",
            workbook_hash(),
            "{IR}",
            "{MANIFEST}",
            &sha256_hex(b"{EVID}"),
        );
        let json = serde_json::to_string_pretty(&lock).expect("serialize lock");
        assert!(
            json.contains("\"bundle_id\""),
            "BUNDLE.lock emits the bundle_id key (D-17): {json}"
        );
        assert!(
            !json.contains("\"workflow\""),
            "BUNDLE.lock does NOT emit the lighthouse workflow field: {json}"
        );
        assert_eq!(lock.bundle_id, "tax-calc");
    }

    #[test]
    fn bundle_lock_hashes_stable_across_runs() {
        let a = build_bundle_lock(
            "tax-calc",
            "1.0.0",
            workbook_hash(),
            "{IR}",
            "{MANIFEST}",
            &sha256_hex(b"{EVID}"),
        );
        let b = build_bundle_lock(
            "tax-calc",
            "1.0.0",
            workbook_hash(),
            "{IR}",
            "{MANIFEST}",
            &sha256_hex(b"{EVID}"),
        );
        assert_eq!(a, b, "bundle-lock hashing is stable across runs");
    }

    #[test]
    fn combined_hash_changes_when_any_artifact_changes() {
        // D-05 tamper detection: a one-byte change to the manifest flips combined.
        let base = build_bundle_lock(
            "tax-calc",
            "1.0.0",
            workbook_hash(),
            "{IR}",
            "{MANIFEST}",
            &sha256_hex(b"{EVID}"),
        );
        let tampered = build_bundle_lock(
            "tax-calc",
            "1.0.0",
            workbook_hash(),
            "{IR}",
            "{MANIFEST }", // one extra byte
            &sha256_hex(b"{EVID}"),
        );
        assert_ne!(
            base.artifacts.manifest, tampered.artifacts.manifest,
            "a manifest byte change flips its per-artifact hash"
        );
        assert_ne!(
            base.combined, tampered.combined,
            "a one-byte manifest change flips the combined hash (D-05)"
        );
        // And a change to the executable also flips combined.
        let tampered_exec = build_bundle_lock(
            "tax-calc",
            "1.0.0",
            workbook_hash(),
            "{IR }",
            "{MANIFEST}",
            &sha256_hex(b"{EVID}"),
        );
        assert_ne!(base.combined, tampered_exec.combined);
    }
}
