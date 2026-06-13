//! The GOVERNED-ARTIFACT store — ATOMIC on-disk persistence for the promote gate's
//! audit trail (T-93-06-PARTIAL).
//!
//! Two crash-safe write primitives back the gate's governance state:
//!
//! - [`write_approval`] / [`read_approvals`] store each fingerprint-bound
//!   [`ApprovalRecord`] under `<out_root>/<bundle_id>/approvals/<fingerprint>.json`
//!   (ONE file per approval, named by its content fingerprint — so an approval can
//!   never silently overwrite an unrelated one).
//! - [`atomic_write`] writes a single file via a same-dir temp file + `rename`
//!   (POSIX-atomic), so a crash mid-write never leaves a partial/corrupt file.
//! - [`atomic_promote_dir`] renames a fully-written temp directory into its final
//!   `<bundle_id>@<version>/` location, so a baseline is never left partially
//!   written (the CR-02 promote uses this — Task 2).
//!
//! All writes go temp→rename: no reader ever observes a half-written governance
//! artifact (T-93-06-PARTIAL).

use std::path::{Path, PathBuf};

use super::corpus::{candidate_fingerprint, ApprovalRecord, CorpusError};

/// The approvals sub-directory under a bundle's governance root.
pub const APPROVALS_DIR: &str = "approvals";

/// The approvals directory for `bundle_id` under `out_root`:
/// `<out_root>/<bundle_id>/approvals/`.
#[must_use]
pub fn approvals_dir(out_root: &Path, bundle_id: &str) -> PathBuf {
    out_root.join(bundle_id).join(APPROVALS_DIR)
}

/// ATOMICALLY write `contents` to `path` via a same-directory temp file + rename
/// (POSIX-atomic). A crash mid-write leaves the temp file (cleaned on the next
/// write) but NEVER a partial `path` (T-93-06-PARTIAL).
///
/// The temp file is created in the SAME directory as `path` so the final `rename`
/// stays within one filesystem (a cross-device rename is not atomic).
///
/// # Errors
/// Returns [`CorpusError::Io`] on any create / write / rename failure.
pub fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), CorpusError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|e| CorpusError::Io {
        path: parent.display().to_string(),
        detail: e.to_string(),
    })?;

    // A unique same-dir temp name (pid + a monotonic counter keep concurrent emits
    // from colliding on the temp path).
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "artifact".to_string());
    let tmp = parent.join(format!(".{file_name}.tmp-{}-{seq}", std::process::id()));

    std::fs::write(&tmp, contents).map_err(|e| CorpusError::Io {
        path: tmp.display().to_string(),
        detail: e.to_string(),
    })?;
    std::fs::rename(&tmp, path).map_err(|e| {
        // Best-effort cleanup of the temp file on a failed rename.
        let _ = std::fs::remove_file(&tmp);
        CorpusError::Io {
            path: path.display().to_string(),
            detail: e.to_string(),
        }
    })
}

/// ATOMICALLY promote a fully-written `staging` directory to `final_dir` via
/// `rename` (CR-02 / T-93-06-PARTIAL). The caller writes the COMPLETE bundle into
/// `staging` first; this single rename publishes it, so a baseline is never left
/// partially written. Refuses to overwrite an existing `final_dir`
/// ([`CorpusError::Io`]) so a promote can NEVER clobber a prior baseline (CR-02).
///
/// # Errors
/// Returns [`CorpusError::Io`] if `final_dir` already exists or the rename fails.
pub fn atomic_promote_dir(staging: &Path, final_dir: &Path) -> Result<(), CorpusError> {
    if final_dir.exists() {
        return Err(CorpusError::Io {
            path: final_dir.display().to_string(),
            detail: "refusing to overwrite an existing baseline directory (CR-02)".to_string(),
        });
    }
    if let Some(parent) = final_dir.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CorpusError::Io {
            path: parent.display().to_string(),
            detail: e.to_string(),
        })?;
    }
    std::fs::rename(staging, final_dir).map_err(|e| CorpusError::Io {
        path: final_dir.display().to_string(),
        detail: e.to_string(),
    })
}

/// ATOMICALLY persist `record` to
/// `<out_root>/<bundle_id>/approvals/<fingerprint>.json`, where `<fingerprint>` is
/// the record's [`candidate_fingerprint`]. One file per approval — named by its
/// content fingerprint — so an approval can NEVER overwrite an unrelated one
/// (WR-04). The write is atomic temp→rename (T-93-06-PARTIAL).
///
/// # Errors
/// Returns [`CorpusError::Serde`] / [`CorpusError::Io`] on a serialize / write
/// failure.
pub fn write_approval(
    out_root: &Path,
    bundle_id: &str,
    record: &ApprovalRecord,
) -> Result<PathBuf, CorpusError> {
    let fingerprint = candidate_fingerprint(
        &record.prev_bundle_hash,
        &record.candidate_workbook_hash,
        &record.region_deltas,
    );
    let dir = approvals_dir(out_root, bundle_id);
    let path = dir.join(format!("{fingerprint}.json"));
    let body = serde_json::to_string_pretty(record).map_err(|e| CorpusError::Serde {
        what: format!("approval {fingerprint}"),
        detail: e.to_string(),
    })?;
    atomic_write(&path, body.as_bytes())?;
    Ok(path)
}

/// Read EVERY recorded [`ApprovalRecord`] under
/// `<out_root>/<bundle_id>/approvals/`. A missing approvals dir is an empty set (no
/// approvals recorded yet — the first-version / pre-accept state). Files are read
/// in sorted-name order for deterministic iteration.
///
/// # Errors
/// Returns [`CorpusError::Io`] / [`CorpusError::Serde`] on a read / parse failure
/// of an existing approval file.
pub fn read_approvals(
    out_root: &Path,
    bundle_id: &str,
) -> Result<Vec<ApprovalRecord>, CorpusError> {
    let dir = approvals_dir(out_root, bundle_id);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| CorpusError::Io {
            path: dir.display().to_string(),
            detail: e.to_string(),
        })?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    entries.sort();

    let mut records = Vec::with_capacity(entries.len());
    for path in entries {
        let bytes = std::fs::read(&path).map_err(|e| CorpusError::Io {
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;
        let record: ApprovalRecord =
            serde_json::from_slice(&bytes).map_err(|e| CorpusError::Serde {
                what: path.display().to_string(),
                detail: e.to_string(),
            })?;
        records.push(record);
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::corpus::{approval_matches, candidate_fingerprint, RegionDelta};
    use pmcp_workbook_runtime::ChangeClass;
    use std::collections::BTreeMap;

    fn tmp_dir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static C: AtomicU64 = AtomicU64::new(0);
        let n = C.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("wbc-gov-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("mk dir");
        dir
    }

    fn sample_record() -> ApprovalRecord {
        let mut d = BTreeMap::new();
        d.insert(
            "3_Out!B2".to_string(),
            RegionDelta {
                old: 20.0,
                new: 30.0,
            },
        );
        ApprovalRecord {
            case_id: "default".to_string(),
            prev_bundle_hash: "prev".to_string(),
            candidate_workbook_hash: "cand".to_string(),
            region_deltas: d,
            change_classes: vec![ChangeClass::FormulaLogic],
            approved_by: "ba@test".to_string(),
            approved_at: "2026-06-12T00:00:00Z".to_string(),
            effective_date: "2026-06-12".to_string(),
        }
    }

    #[test]
    fn approval_stored_at_fingerprint_path() {
        let dir = tmp_dir();
        let record = sample_record();
        let path = write_approval(&dir, "tax-calc", &record).expect("write");
        let fp = candidate_fingerprint(
            &record.prev_bundle_hash,
            &record.candidate_workbook_hash,
            &record.region_deltas,
        );
        assert!(
            path.ends_with(format!("approvals/{fp}.json")),
            "approval is stored at <bundle_id>/approvals/<fingerprint>.json: {path:?}"
        );
        assert!(path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn approvals_round_trip_and_match() {
        let dir = tmp_dir();
        let record = sample_record();
        write_approval(&dir, "tax-calc", &record).expect("write");
        let back = read_approvals(&dir, "tax-calc").expect("read");
        assert_eq!(back.len(), 1);
        let fp = candidate_fingerprint(
            &record.prev_bundle_hash,
            &record.candidate_workbook_hash,
            &record.region_deltas,
        );
        assert!(approval_matches(&back, "default", &fp));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_approvals_dir_is_empty_set() {
        let dir = tmp_dir();
        let back = read_approvals(&dir, "never-written").expect("read");
        assert!(back.is_empty(), "a missing approvals dir is the empty set");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn atomic_write_leaves_no_temp_file() {
        let dir = tmp_dir();
        let path = dir.join("nested/sub/artifact.json");
        atomic_write(&path, b"{\"ok\":true}").expect("atomic write");
        assert_eq!(std::fs::read(&path).expect("read"), b"{\"ok\":true}");
        // No leftover temp files in the target dir.
        let temps: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp-"))
            .collect();
        assert!(temps.is_empty(), "atomic write leaves no temp file");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn atomic_promote_refuses_to_overwrite() {
        let dir = tmp_dir();
        let staging = dir.join("staging");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("BUNDLE.lock"), b"x").unwrap();
        let final_dir = dir.join("tax-calc@1.0.0");
        atomic_promote_dir(&staging, &final_dir).expect("first promote");
        assert!(final_dir.join("BUNDLE.lock").exists());

        // A second promote into the SAME final dir is refused (CR-02 non-overwrite).
        let staging2 = dir.join("staging2");
        std::fs::create_dir_all(&staging2).unwrap();
        std::fs::write(staging2.join("BUNDLE.lock"), b"y").unwrap();
        let err = atomic_promote_dir(&staging2, &final_dir).expect_err("must refuse");
        assert!(matches!(err, CorpusError::Io { .. }));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
