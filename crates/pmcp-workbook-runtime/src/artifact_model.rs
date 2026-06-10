//! The RUNTIME-safe bundle artifact model + hashing (Phase 11, Plan 05 / Codex
//! HIGH #2 boundary).
//!
//! These shapes describe the EMITTED bundle that the served binary deserializes
//! and integrity-checks at load:
//!
//! - [`CellEntry`]/[`CellMap`] — the manifest-driven I/O map (Codex HIGH #5).
//! - [`ArtifactHashes`]/[`BundleLock`] — the per-artifact + combined SHA-256
//!   hash-of-hashes integrity record (ART-04/D-05).
//!
//! They live HERE (umya/SWC-free) so BOTH sides share ONE definition rather than
//! the served binary re-declaring byte-for-byte serde mirrors:
//!
//! - `workbook-compiler` (the offline EMITTER) re-exports these from
//!   `artifact::{cell_map,bundle_lock}` via a re-export shim (the SAME pattern
//!   `manifest::model` uses), so the emit path keeps compiling unchanged.
//! - `quote-pricing-server` (the served binary) deserializes these types DIRECTLY
//!   and recomputes integrity via the SAME [`build_bundle_lock`] the emitter used.
//!
//! The hashing helpers ([`sha256_hex`], [`build_bundle_lock`], [`update_field`])
//! are the SINGLE source the emitter and the server-side integrity check share —
//! they MUST byte-reproduce each other or the integrity check false-positives.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// One input/output cell entry in a [`CellMap`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CellEntry {
    /// The Plot-3 JSON key the caller uses for this cell (the LLM-facing name).
    pub plot3_json_key: String,
    /// The `CellEnv` seed coordinate — the fully-qualified `sheet!addr` cell key.
    pub seed_coord: String,
    /// The declared unit (`m2`/`GBP`/…), when known.
    pub unit: Option<String>,
}

/// The manifest-driven I/O cell map (Codex HIGH #5): the inputs/outputs the served
/// `calculate` seeds and projects, plus the named supply-total output cell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CellMap {
    /// One entry per `Role::Input` cell (the seedable per-quote inputs).
    pub inputs: Vec<CellEntry>,
    /// One entry per `Role::Output` cell (the projected answers).
    pub outputs: Vec<CellEntry>,
    /// The fully-qualified cell key (`sheet!addr`) of the single supply-total
    /// output cell — the headline answer the served `calculate` returns.
    pub supply_total_cell: String,
}

/// The three per-artifact content hashes recorded in a [`BundleLock`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ArtifactHashes {
    /// SHA-256 over `executable.ir.json` bytes (64-char hex).
    pub executable: String,
    /// SHA-256 over `manifest.json` bytes (64-char hex).
    pub manifest: String,
    /// SHA-256 over the evidence directory's path+length-prefixed content (64-char
    /// hex; computed by the evidence emitter, which also folds `cell_map.json`).
    pub evidence: String,
}

/// The `BUNDLE.lock` record (ART-04/D-05): the workflow identity, the
/// `workbook_hash` provenance anchor, the three per-artifact content hashes, and
/// the COMBINED hash-of-hashes that flips on any single-artifact change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BundleLock {
    /// The workflow name (e.g. `"ufh-quote"`).
    pub workflow: String,
    /// The semver version (e.g. `"1.0.0"`).
    pub version: String,
    /// The canonical source-workbook CONTENT hash (`source_workbook_hash`), the
    /// provenance anchor binding the bundle to the exact source workbook (D-05).
    pub workbook_hash: String,
    /// The per-artifact content hashes.
    pub artifacts: ArtifactHashes,
    /// The combined hash-of-hashes over the three per-artifact hashes — flips
    /// when ANY artifact changes (tampering / partial-rebuild detection, D-05).
    pub combined: String,
}

/// `hex::encode(Sha256::digest(bytes))` — the single per-artifact content hash.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Feed one length-prefixed field to the digest: the tag, then the u64-LE byte
/// length, then the bytes. Because the length is encoded out-of-band, the field
/// bytes can contain ANY byte without creating an ambiguous boundary (T-7-11).
///
/// This is the SINGLE canonicalization the evidence-dir hash uses; the server's
/// integrity recompute and the emitter MUST share it byte-for-byte.
pub fn update_field(hasher: &mut Sha256, tag: &[u8], data: &[u8]) {
    hasher.update(tag);
    hasher.update((data.len() as u64).to_le_bytes());
    hasher.update(data);
}

/// Build the [`BundleLock`] over the emitted artifact bytes.
///
/// Each per-artifact hash is `hex::encode(Sha256::digest(bytes))`; the combined
/// hash is `Sha256` over the concatenation of the three 64-char hex hashes (a
/// fixed-width concatenation is unambiguous). `workbook_hash` is the
/// caller-supplied `source_workbook_hash` content projection — RECORDED, not
/// recomputed from raw bytes (D-05). A one-byte change to any artifact flips its
/// per-artifact hash, which flips the combined hash (D-05 tamper detection).
pub fn build_bundle_lock(
    workflow: &str,
    version: &str,
    workbook_hash: String,
    ir_json: &str,
    manifest_json: &str,
    evidence_hash: &str,
) -> BundleLock {
    let h_exec = sha256_hex(ir_json.as_bytes());
    let h_manifest = sha256_hex(manifest_json.as_bytes());
    // The evidence hash is computed over the evidence DIR (path+length-prefixed,
    // folding cell_map.json) by the emitter; the lock records it verbatim.
    let h_evidence = evidence_hash.to_string();

    let combined = sha256_hex(format!("{h_exec}{h_manifest}{h_evidence}").as_bytes());

    BundleLock {
        workflow: workflow.to_string(),
        version: version.to_string(),
        workbook_hash,
        artifacts: ArtifactHashes {
            executable: h_exec,
            manifest: h_manifest,
            evidence: h_evidence,
        },
        combined,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workbook_hash() -> String {
        sha256_hex(b"S!A1|10|\nS!B1|0.37|")
    }

    #[test]
    fn bundle_lock_records_three_plus_combined() {
        let lock = build_bundle_lock(
            "ufh-quote",
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
        }
        assert_ne!(lock.combined, lock.artifacts.executable);
        assert_ne!(lock.combined, lock.artifacts.manifest);
        assert_ne!(lock.combined, lock.artifacts.evidence);
    }

    #[test]
    fn bundle_lock_hashes_stable_across_runs() {
        let a = build_bundle_lock(
            "ufh-quote",
            "1.0.0",
            workbook_hash(),
            "{IR}",
            "{MANIFEST}",
            &sha256_hex(b"{EVID}"),
        );
        let b = build_bundle_lock(
            "ufh-quote",
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
        let base = build_bundle_lock(
            "ufh-quote",
            "1.0.0",
            workbook_hash(),
            "{IR}",
            "{MANIFEST}",
            &sha256_hex(b"{EVID}"),
        );
        let tampered = build_bundle_lock(
            "ufh-quote",
            "1.0.0",
            workbook_hash(),
            "{IR}",
            "{MANIFEST }", // one extra byte
            &sha256_hex(b"{EVID}"),
        );
        assert_ne!(base.artifacts.manifest, tampered.artifacts.manifest);
        assert_ne!(base.combined, tampered.combined);
        let tampered_exec = build_bundle_lock(
            "ufh-quote",
            "1.0.0",
            workbook_hash(),
            "{IR }",
            "{MANIFEST}",
            &sha256_hex(b"{EVID}"),
        );
        assert_ne!(base.combined, tampered_exec.combined);
    }

    #[test]
    fn workbook_hash_reuses_content_projection() {
        let wh = workbook_hash();
        let lock = build_bundle_lock(
            "ufh-quote",
            "1.0.0",
            wh.clone(),
            "{IR}",
            "{MANIFEST}",
            &sha256_hex(b"{EVID}"),
        );
        assert_eq!(lock.workbook_hash, wh);
        assert_ne!(lock.workbook_hash, lock.artifacts.executable);
        assert_ne!(lock.workbook_hash, lock.combined);
    }

    #[test]
    fn workflow_and_version_are_parameters_not_hardcoded() {
        let lock = build_bundle_lock(
            "other-workflow",
            "2.3.4",
            workbook_hash(),
            "{IR}",
            "{MANIFEST}",
            &sha256_hex(b"{EVID}"),
        );
        assert_eq!(lock.workflow, "other-workflow");
        assert_eq!(lock.version, "2.3.4");
    }

    #[test]
    fn update_field_is_length_prefixed() {
        // Two fields whose concatenation would collide are distinguished by the
        // out-of-band length prefix.
        let mut a = Sha256::new();
        update_field(&mut a, b"t", b"ab");
        update_field(&mut a, b"t", b"c");
        let mut b = Sha256::new();
        update_field(&mut b, b"t", b"a");
        update_field(&mut b, b"t", b"bc");
        assert_ne!(hex::encode(a.finalize()), hex::encode(b.finalize()));
    }
}
