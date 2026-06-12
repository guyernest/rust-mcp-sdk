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
use super::{write_file, EmitError};

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
}
