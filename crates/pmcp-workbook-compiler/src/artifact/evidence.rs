//! `evidence/` emission + the evidence-dir content hash.
//!
//! The Phase 92 served loader recognizes exactly two evidence members
//! (`evidence/changelog.json`, `evidence/parser_equivalence.json`) and folds the
//! evidence-dir hash over a FROZEN four-member set (`cell_map.json`,
//! `evidence/changelog.json`, `evidence/parser_equivalence.json`, `layout.json`)
//! in SORTED relative-path order
//! (`pmcp_workbook_runtime::bundle_loader::EVIDENCE_FOLD_MEMBERS`). The fold goes
//! through the runtime's OWN [`fold_evidence_hash`] so the emitter and the
//! loader's `recompute_evidence_hash` byte-reproduce each other by construction
//! (Pitfall 2 / threat T-93-05-HASH).
//!
//! [`ParserEquivalence`] is the D-08 drift-gate record every candidate carries
//! ([`emit_bundle`](super::emit_bundle) always emits it). The
//! [`pmcp_workbook_runtime::VersionChangelog`] is the recorded prev→current
//! transition the served `diff_version` tool reads.

use std::path::Path;

use pmcp_workbook_runtime::fold_evidence_hash;
use serde::{Deserialize, Serialize};

use super::serialize::to_bundle_json;
use super::{sha256_hex, write_file, EmitError};

/// The relative bundle path of the ungated/gated status marker (D-08): the emit
/// status travels WITH the artifact. This is an ADDITIVE, self-contained channel —
/// it is NOT a member of the served loader's FROZEN seven-member set or the
/// `EVIDENCE_FOLD_MEMBERS` fold (adding it there would change the SERVED contract,
/// a Phase-92/93 deliverable, not "expose existing internals").
pub const EVIDENCE_GATE_MARKER: &str = "evidence/gate.json";

/// The relative bundle path of the marker's recorded sha256 digest. A reader
/// recomputes `sha256_hex(read gate.json bytes)` and compares against this file —
/// so a STRIPPED or EDITED marker is DETECTABLE (tamper-evident, T-94-00-MARKER).
pub const EVIDENCE_GATE_DIGEST: &str = "evidence/gate.sha256";

/// The ungated/gated status marker body: `{ "gated": <bool> }`. `gated: false` is
/// the ungated emit's marker; a future gated promote could stamp `gated: true`.
/// Deterministically serialized through the bundle's [`to_bundle_json`] choke point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GateMarker {
    /// Whether this bundle was emitted through the gated promote lane.
    pub gated: bool,
}

/// The D-08 parser-equivalence evidence record: a run-derived attestation that
/// the offline parse and the served evaluator agree on the workbook's cells. It
/// carries ONLY run-derived values (no emit-time timestamps) so the idempotent
/// re-emit stays byte-stable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ParserEquivalence {
    /// How many cells were checked for parse/eval equivalence.
    pub checked_cells: u32,
    /// Whether every checked cell was equivalent.
    pub equivalent: bool,
    /// The derivation method (e.g. the evaluator path that produced the check).
    pub method: String,
}

/// The serialized member bytes the evidence fold covers, in the SAME order the
/// loader's `recompute_evidence_hash` reads them. Each field is the deterministic
/// JSON string already written for the corresponding bundle member.
pub struct EvidenceInputs<'a> {
    /// `cell_map.json` bytes (folded; member lives at the bundle root).
    pub cell_map_json: &'a str,
    /// `evidence/changelog.json` bytes (also written under `evidence/`).
    pub changelog_json: &'a str,
    /// `evidence/parser_equivalence.json` bytes (also written under `evidence/`).
    pub parser_equivalence_json: &'a str,
    /// `layout.json` bytes (folded; member lives at the bundle root).
    pub layout_json: &'a str,
}

/// Write the two `evidence/` members and return the evidence-dir content hash.
///
/// Writes `evidence/changelog.json` + `evidence/parser_equivalence.json`, then
/// folds the FROZEN four-member set (`cell_map.json`, `evidence/changelog.json`,
/// `evidence/parser_equivalence.json`, `layout.json`) through the runtime's shared
/// [`fold_evidence_hash`] — the SAME fold the served loader recomputes. The
/// member-path keys MUST match the loader's `EVIDENCE_FOLD_MEMBERS` exactly.
///
/// # Errors
/// Returns [`EmitError::Io`] if an evidence member cannot be written.
pub fn emit_evidence(inputs: &EvidenceInputs<'_>, dir: &Path) -> Result<String, EmitError> {
    let evidence_dir = dir.join("evidence");
    std::fs::create_dir_all(&evidence_dir).map_err(|e| EmitError::Io {
        path: evidence_dir.display().to_string(),
        detail: e.to_string(),
    })?;
    write_file(&evidence_dir.join("changelog.json"), inputs.changelog_json)?;
    write_file(
        &evidence_dir.join("parser_equivalence.json"),
        inputs.parser_equivalence_json,
    )?;

    // Fold the FROZEN member set in the loader's path keys. fold_evidence_hash
    // sorts by path internally, so the input order here is not load-bearing — but
    // the PATH KEYS must match the loader's EVIDENCE_FOLD_MEMBERS exactly.
    let members: [(&str, &[u8]); 4] = [
        ("cell_map.json", inputs.cell_map_json.as_bytes()),
        ("evidence/changelog.json", inputs.changelog_json.as_bytes()),
        (
            "evidence/parser_equivalence.json",
            inputs.parser_equivalence_json.as_bytes(),
        ),
        ("layout.json", inputs.layout_json.as_bytes()),
    ];
    Ok(fold_evidence_hash(&members))
}

/// Serialize a [`ParserEquivalence`] record through the deterministic choke point.
///
/// # Errors
/// Returns [`EmitError::Serde`] on a serialization failure.
pub fn parser_equivalence_json(record: &ParserEquivalence) -> Result<String, EmitError> {
    to_bundle_json(record, "evidence/parser_equivalence.json")
}

/// Write the HASH-COVERED ungated/gated status marker into `bundle_dir`, returning
/// the recorded sha256 hex digest.
///
/// Writes `evidence/gate.json` = `{ "gated": <gated> }` (deterministic via
/// [`to_bundle_json`]) AND records `sha256_hex(json bytes)` into
/// `evidence/gate.sha256` so a later reader detects a stripped or edited marker
/// (tamper-evident, T-94-00-MARKER). This is a SELF-CONTAINED in-crate channel: it
/// does NOT touch [`EvidenceInputs`], [`emit_evidence`], or the served loader's
/// FROZEN `EVIDENCE_FOLD_MEMBERS`/`ALLOWED_MEMBERS` — the frozen evidence-dir fold
/// and the served allow-set are UNCHANGED. It is written into a bundle dir the CLI
/// manages, AFTER emit, so it never trips the served allow-set check at compile time.
///
/// # Errors
/// Returns [`EmitError::Io`] if the `evidence/` dir cannot be created or either
/// member cannot be written, or [`EmitError::Serde`] if the marker cannot be
/// serialized.
pub fn write_gate_marker(bundle_dir: &Path, gated: bool) -> Result<String, EmitError> {
    let json = to_bundle_json(&GateMarker { gated }, EVIDENCE_GATE_MARKER)?;
    let digest = sha256_hex(json.as_bytes());

    let evidence_dir = bundle_dir.join("evidence");
    std::fs::create_dir_all(&evidence_dir).map_err(|e| EmitError::Io {
        path: evidence_dir.display().to_string(),
        detail: e.to_string(),
    })?;
    write_file(&bundle_dir.join(EVIDENCE_GATE_MARKER), &json)?;
    write_file(&bundle_dir.join(EVIDENCE_GATE_DIGEST), &digest)?;
    Ok(digest)
}

/// Read the marker back from `bundle_dir`, returning `(gated, digest_ok)` where
/// `digest_ok` is `sha256_hex(read gate.json bytes) == read gate.sha256` — the
/// tamper-evidence check (a stripped or edited `gate.json` yields
/// `digest_ok == false`, T-94-00-MARKER).
///
/// # Errors
/// Returns [`EmitError::Io`] if either marker member cannot be read, or
/// [`EmitError::Serde`] if `gate.json` cannot be parsed.
pub fn read_gate_marker(bundle_dir: &Path) -> Result<(bool, bool), EmitError> {
    let json_path = bundle_dir.join(EVIDENCE_GATE_MARKER);
    let json_bytes = std::fs::read(&json_path).map_err(|e| EmitError::Io {
        path: json_path.display().to_string(),
        detail: e.to_string(),
    })?;
    let digest_path = bundle_dir.join(EVIDENCE_GATE_DIGEST);
    let recorded_digest = std::fs::read_to_string(&digest_path).map_err(|e| EmitError::Io {
        path: digest_path.display().to_string(),
        detail: e.to_string(),
    })?;

    let marker: GateMarker = serde_json::from_slice(&json_bytes).map_err(|e| EmitError::Serde {
        what: EVIDENCE_GATE_MARKER.to_string(),
        detail: e.to_string(),
    })?;
    let digest_ok = sha256_hex(&json_bytes) == recorded_digest.trim();
    Ok((marker.gated, digest_ok))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_workbook_runtime::bundle_loader::EVIDENCE_FOLD_MEMBERS;

    #[test]
    fn emit_evidence_writes_both_members_and_folds_hash() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let inputs = EvidenceInputs {
            cell_map_json: "{\"inputs\":[],\"outputs\":[]}",
            changelog_json: "{\"from_version\":\"1.0.0\"}",
            parser_equivalence_json: "{\"checked_cells\":1,\"equivalent\":true,\"method\":\"x\"}",
            layout_json: "{\"descriptor_version\":1}",
        };
        let hash = emit_evidence(&inputs, dir.path()).expect("emit evidence");
        assert_eq!(hash.len(), 64, "evidence hash is a 64-char sha256 hex");

        assert!(dir.path().join("evidence/changelog.json").exists());
        assert!(dir.path().join("evidence/parser_equivalence.json").exists());
    }

    #[test]
    fn evidence_fold_member_keys_match_the_loader_set() {
        // Pitfall 2 guard: the emitter folds EXACTLY the loader's member set. We
        // assert our path keys are a permutation of EVIDENCE_FOLD_MEMBERS.
        let mut ours = [
            "cell_map.json",
            "evidence/changelog.json",
            "evidence/parser_equivalence.json",
            "layout.json",
        ];
        let mut theirs: Vec<&str> = EVIDENCE_FOLD_MEMBERS.to_vec();
        ours.sort_unstable();
        theirs.sort_unstable();
        assert_eq!(
            ours.as_slice(),
            theirs.as_slice(),
            "emitter and loader fold the identical evidence member set"
        );
    }

    #[test]
    fn parser_equivalence_round_trips() {
        let rec = ParserEquivalence {
            checked_cells: 11,
            equivalent: true,
            method: "scalar-eval".to_string(),
        };
        let json = parser_equivalence_json(&rec).expect("serialize");
        let back: ParserEquivalence = serde_json::from_str(&json).expect("parse");
        assert_eq!(back, rec);
    }

    #[test]
    fn gate_marker_round_trips_ungated() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let digest = write_gate_marker(dir.path(), false).expect("write marker");
        assert_eq!(
            digest.len(),
            64,
            "the recorded digest is a 64-char sha256 hex"
        );

        // Both members exist under evidence/.
        assert!(dir.path().join("evidence/gate.json").exists());
        assert!(dir.path().join("evidence/gate.sha256").exists());

        // read_gate_marker round-trips the bool and confirms the digest.
        let (gated, digest_ok) = read_gate_marker(dir.path()).expect("read marker");
        assert!(!gated, "the written bool round-trips (ungated)");
        assert!(digest_ok, "the recorded digest matches the written bytes");
    }

    #[test]
    fn gate_marker_round_trips_gated() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        write_gate_marker(dir.path(), true).expect("write marker");
        let (gated, digest_ok) = read_gate_marker(dir.path()).expect("read marker");
        assert!(gated, "the written bool round-trips (gated)");
        assert!(digest_ok, "the recorded digest matches the written bytes");
    }

    #[test]
    fn gate_marker_digest_covers_the_exact_written_bytes() {
        // The recorded digest is sha256_hex of the EXACT gate.json bytes on disk.
        let dir = tempfile::TempDir::new().expect("tempdir");
        write_gate_marker(dir.path(), false).expect("write marker");
        let json_bytes = std::fs::read(dir.path().join("evidence/gate.json")).expect("read json");
        let recorded =
            std::fs::read_to_string(dir.path().join("evidence/gate.sha256")).expect("read digest");
        assert_eq!(
            sha256_hex(&json_bytes),
            recorded.trim(),
            "the recorded digest covers the exact written gate.json bytes (tamper-evident)"
        );
    }

    #[test]
    fn gate_marker_tamper_is_detected() {
        // Corrupting gate.json after writing flips digest_ok to false.
        let dir = tempfile::TempDir::new().expect("tempdir");
        write_gate_marker(dir.path(), false).expect("write marker");

        // Edit the marker body (flip gated false -> true) WITHOUT updating the
        // recorded digest — a stripped/edited marker.
        std::fs::write(
            dir.path().join("evidence/gate.json"),
            "{\n  \"gated\": true\n}",
        )
        .expect("corrupt marker");

        let (_gated, digest_ok) = read_gate_marker(dir.path()).expect("read marker");
        assert!(
            !digest_ok,
            "an edited gate.json is detected (digest_ok == false)"
        );
    }

    #[test]
    fn frozen_evidence_fold_is_unchanged_by_the_marker_channel() {
        // The marker channel is ADDITIVE: emit_evidence is byte-stable and the
        // FROZEN fold member set still has exactly four members. (This guards
        // T-94-00-FROZEN: the served seven-member contract is untouched.)
        assert_eq!(
            EVIDENCE_FOLD_MEMBERS.len(),
            4,
            "the frozen evidence fold still has exactly four members"
        );

        let inputs = EvidenceInputs {
            cell_map_json: "{\"inputs\":[],\"outputs\":[]}",
            changelog_json: "{\"from_version\":\"1.0.0\"}",
            parser_equivalence_json: "{\"checked_cells\":1,\"equivalent\":true,\"method\":\"x\"}",
            layout_json: "{\"descriptor_version\":1}",
        };

        // emit_evidence over fixed inputs is byte-stable (the marker channel did
        // not perturb the fold) — two emits into two dirs return the SAME hash.
        let d1 = tempfile::TempDir::new().expect("tempdir 1");
        let d2 = tempfile::TempDir::new().expect("tempdir 2");
        let h1 = emit_evidence(&inputs, d1.path()).expect("emit 1");
        let h2 = emit_evidence(&inputs, d2.path()).expect("emit 2");
        assert_eq!(
            h1, h2,
            "emit_evidence is byte-stable (frozen fold untouched)"
        );

        // And writing the gate marker into the SAME dir does NOT change the folded
        // evidence hash (the marker is outside the fold).
        let after_marker = emit_evidence(&inputs, d1.path()).expect("re-emit");
        write_gate_marker(d1.path(), false).expect("write marker into the same dir");
        let after_marker_again = emit_evidence(&inputs, d1.path()).expect("re-emit after marker");
        assert_eq!(
            after_marker, after_marker_again,
            "the evidence fold hash is unchanged whether or not the gate marker is present"
        );
    }
}
