//! BA RATIFICATION recording (WBCO-02/D-04) — the recorded sign-off.
//!
//! On BA ratification the candidate [`Manifest`] becomes canonical: it is STAMPED
//! (`ratified = true`, `ratified_by`, `ratified_at`, and the source
//! `workbook_hash`) and an append-only JSONL audit line is written to the
//! ratification sidecar. Thereafter [`is_conformant`] binds the ratification to the
//! workbook's CONTENT HASH so a real content change reverts it to candidate.
//!
//! # Why the hash is PASSED IN (not re-read here)
//!
//! The lighthouse re-opened the `.xlsx` with `umya` inside ratify to compute a
//! content hash. In the SDK the offline compiler already holds the canonical
//! content hash from the ingest / provenance stage (the umya-isolated boundary), so
//! ratify takes it as a `&str` PARAMETER and never links the reader — keeping this
//! module reader-free and the sign-off a pure recording step (D-04 "stay-in-Excel":
//! ratification is a recorded approver + date, not a re-computation).

use std::path::Path;

use super::model::Manifest;

/// A ratification failure: sidecar I/O or audit-record serialization.
#[derive(Debug, thiserror::Error)]
pub enum RatifyError {
    /// The ratification sidecar could not be opened/appended.
    #[error("failed to append the ratification sidecar {path}: {detail}")]
    Sidecar {
        /// The sidecar path.
        path: String,
        /// The underlying I/O error.
        detail: String,
    },
    /// A JSONL audit record could not be serialized.
    #[error("failed to serialize the ratification record: {0}")]
    Serde(String),
}

/// The JSONL audit record appended to the sidecar (one line per ratification).
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct RatificationRecord {
    workbook_hash: String,
    ratified_by: String,
    ratified_at: String,
}

/// RATIFY the candidate `manifest` against the source `workbook_hash` (the
/// canonical content hash the ingest/provenance stage already computed): stamp
/// `workbook_hash`, `ratified = true`, `ratified_by`, `ratified_at` (RFC3339), and
/// APPEND one JSONL audit line to `sidecar`.
///
/// The append is TRUTHFUL — opens the sidecar in append mode and writes one
/// serialized record + newline (never read-push-rewrite, so concurrent appends
/// never clobber prior records). No path component is ever derived from workbook
/// content.
pub fn ratify(
    manifest: &mut Manifest,
    workbook_hash: &str,
    ratified_by: &str,
    sidecar: &Path,
) -> Result<(), RatifyError> {
    let ratified_at = chrono::Utc::now().to_rfc3339();

    manifest.workbook_hash = Some(workbook_hash.to_string());
    manifest.ratified = true;
    manifest.ratified_by = Some(ratified_by.to_string());
    manifest.ratified_at = Some(ratified_at.clone());

    let record = RatificationRecord {
        workbook_hash: workbook_hash.to_string(),
        ratified_by: ratified_by.to_string(),
        ratified_at,
    };
    let mut line = serde_json::to_string(&record).map_err(|e| RatifyError::Serde(e.to_string()))?;
    line.push('\n');

    if let Some(parent) = sidecar.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| RatifyError::Sidecar {
                path: sidecar.display().to_string(),
                detail: e.to_string(),
            })?;
        }
    }
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(sidecar)
        .map_err(|e| RatifyError::Sidecar {
            path: sidecar.display().to_string(),
            detail: e.to_string(),
        })?;
    file.write_all(line.as_bytes())
        .map_err(|e| RatifyError::Sidecar {
            path: sidecar.display().to_string(),
            detail: e.to_string(),
        })?;

    Ok(())
}

/// Whether `manifest` is CONFORMANT against the CURRENT `current_hash`: the manifest
/// is ratified AND its stamped `workbook_hash` equals `current_hash`. A real content
/// change (a different hash) ⇒ mismatch ⇒ reverts to candidate (an un-ratified
/// manifest is never conformant — D-04).
#[must_use]
pub fn is_conformant(manifest: &Manifest, current_hash: &str) -> bool {
    manifest.ratified && manifest.workbook_hash.as_deref() == Some(current_hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::model::{CellRole, Dtype, Role};
    use std::path::PathBuf;

    fn tmp(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static C: AtomicU64 = AtomicU64::new(0);
        let n = C.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("wbc-ratify-{tag}-{}-{n}.jsonl", std::process::id()))
    }

    fn candidate() -> Manifest {
        Manifest {
            schema_version: 1,
            workflow: "wf".to_string(),
            workbook_hash: None,
            ratified: false,
            ratified_by: None,
            ratified_at: None,
            cells: vec![CellRole {
                cell: "1_Inputs!E6".to_string(),
                role: Role::Input,
                name: None,
                unit: None,
                meaning: None,
                dtype: Dtype::Number,
                colour_evidence: None,
                source: "colour+guide".to_string(),
                notes: None,
                tier: None,
                allowed_values: None,
            }],
            loop_block: None,
            governed_data: Vec::new(),
            changelog: vec![],
            capability_calls: vec![],
            annotations: vec![],
        }
    }

    #[test]
    fn ratify_stamps_the_sign_off_fields() {
        let mut m = candidate();
        let sidecar = tmp("stamp");
        ratify(&mut m, "abc123", "ba@test", &sidecar).expect("ratify");
        assert!(m.ratified, "ratified flips true (D-04 sign-off)");
        assert_eq!(m.ratified_by.as_deref(), Some("ba@test"));
        assert!(m.ratified_at.is_some(), "an ISO-8601 date is stamped");
        assert_eq!(m.workbook_hash.as_deref(), Some("abc123"));
        let _ = std::fs::remove_file(&sidecar);
    }

    #[test]
    fn ratify_appends_one_jsonl_line_per_call() {
        let sidecar = tmp("append");
        let mut m1 = candidate();
        ratify(&mut m1, "h1", "ba-one", &sidecar).expect("ratify 1");
        let first = std::fs::read_to_string(&sidecar).expect("read");
        let mut m2 = candidate();
        ratify(&mut m2, "h2", "ba-two", &sidecar).expect("ratify 2");
        let both = std::fs::read_to_string(&sidecar).expect("read 2");
        let lines: Vec<&str> = both.lines().collect();
        assert_eq!(lines.len(), 2, "two ratifications => two lines");
        assert!(both.starts_with(&first), "truthful append, not rewrite");
        assert!(lines[0].contains("ba-one"));
        assert!(lines[1].contains("ba-two"));
        let _ = std::fs::remove_file(&sidecar);
    }

    #[test]
    fn conformance_binds_to_the_content_hash() {
        let mut m = candidate();
        let sidecar = tmp("conform");
        ratify(&mut m, "hash-v1", "ba@test", &sidecar).expect("ratify");
        assert!(
            is_conformant(&m, "hash-v1"),
            "same content stays conformant"
        );
        assert!(
            !is_conformant(&m, "hash-v2"),
            "a real content change reverts to candidate"
        );
        let _ = std::fs::remove_file(&sidecar);
    }

    #[test]
    fn unratified_manifest_is_not_conformant() {
        let m = candidate(); // ratified = false
        assert!(
            !is_conformant(&m, "anything"),
            "an un-ratified manifest is never conformant"
        );
    }
}
