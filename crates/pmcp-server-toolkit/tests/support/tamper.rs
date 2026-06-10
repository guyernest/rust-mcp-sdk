//! Copy-to-tempdir + corrupt tamper helpers (Phase 92 Plan 02, Task 3 — D-05).
//!
//! There are NO committed corrupt fixtures (D-05): the ONE committed golden lives
//! under `tests/fixtures/tax-calc@1.1.0/`, and every negative-path test COPIES it
//! into a fresh tempdir and corrupts the copy. Each helper provokes a DISTINCT
//! fail-closed [`pmcp_workbook_runtime::BundleLoadError`] variant when the
//! corrupted copy is loaded through `LocalDirSource` + `load_bundle`:
//!
//! - [`flip_byte`] → `IntegrityMismatch` (T-92-01)
//! - [`delete_artifact`] → a load error (missing member — `Source`/`Parse`)
//! - [`desync_lock_version`] → `StampMismatch` (T-92-02)
//! - [`add_unexpected_member`] → `UnexpectedMember` (T-92-22, Codex MEDIUM #9)
//!
//! Usage:
//! ```ignore
//! let dir = copy_golden_to_temp();
//! flip_byte(dir.path(), "manifest.json");
//! let src = LocalDirSource::new(dir.path());
//! assert!(matches!(load_bundle(&src), Err(BundleLoadError::IntegrityMismatch { .. })));
//! ```

use std::path::{Path, PathBuf};

use tempfile::TempDir;

/// The committed golden bundle directory (relative to the crate manifest dir).
pub const GOLDEN_REL: &str = "tests/fixtures/tax-calc@1.1.0";

/// The absolute path to the committed golden bundle directory.
pub fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(GOLDEN_REL)
}

/// Recursively copy `src` into `dst` (creating `dst` and any subdirectories).
fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Copy the committed `tax-calc@1.1.0` golden tree into a fresh self-cleaning
/// tempdir and return the handle (the tree lives at `dir.path()`). The `TempDir`
/// removes the copy on drop, so a caller holds it for the test body.
pub fn copy_golden_to_temp() -> TempDir {
    let dir = tempfile::tempdir().expect("create tempdir for golden copy");
    copy_tree(&golden_dir(), dir.path()).expect("copy golden tree into tempdir");
    dir
}

/// Mutate ONE byte of `member` (a bundle-relative path like `"manifest.json"`),
/// provoking [`pmcp_workbook_runtime::BundleLoadError::IntegrityMismatch`]: the
/// member's recomputed hash diverges from the on-disk lock.
///
/// The mutation keeps the bytes VALID UTF-8 (it rewrites one ASCII alphanumeric
/// byte to a DIFFERENT ASCII alphanumeric byte) so the integrity gate fires — a
/// non-UTF-8 flip would instead hit the loader's `str::from_utf8` check and
/// surface as `Parse` before the integrity recompute.
pub fn flip_byte(dir: &Path, member: &str) {
    let path = dir.join(member);
    let mut bytes = std::fs::read(&path).expect("read member to flip");
    assert!(!bytes.is_empty(), "member {member} must be non-empty to flip");
    // Find the first ASCII alphanumeric byte and bump it to a different ASCII
    // alphanumeric value (digit 0-8 -> +1, '9' -> '0', letters rotate). This
    // changes the content hash while keeping the file present AND valid UTF-8.
    let idx = bytes
        .iter()
        .position(|b| b.is_ascii_alphanumeric())
        .expect("member has at least one ASCII alphanumeric byte to mutate");
    bytes[idx] = match bytes[idx] {
        b'9' => b'0',
        b'z' => b'a',
        b'Z' => b'A',
        other => other + 1,
    };
    std::fs::write(&path, &bytes).expect("rewrite mutated member");
}

/// Delete `member` from the bundle copy, provoking a load error (a missing
/// member surfaces as `Source`/`Parse`, never a panic).
pub fn delete_artifact(dir: &Path, member: &str) {
    let path = dir.join(member);
    std::fs::remove_file(&path).expect("delete member");
}

/// Rewrite `BUNDLE.lock` so its `version` disagrees with the changelog's
/// `to_version`, provoking
/// [`pmcp_workbook_runtime::BundleLoadError::StampMismatch`] on the `version`
/// field.
///
/// The lock is re-serialized with the SAME per-artifact + combined hashes (so the
/// integrity recompute stays self-consistent — the recompute feeds the lock's own
/// triple — and the STAMP-binding gate, not the integrity gate, is what fires).
pub fn desync_lock_version(dir: &Path) {
    let lock_path = dir.join("BUNDLE.lock");
    let bytes = std::fs::read(&lock_path).expect("read BUNDLE.lock");
    let mut lock: pmcp_workbook_runtime::BundleLock =
        serde_json::from_slice(&bytes).expect("parse BUNDLE.lock");
    // Bump the version to one the changelog's to_version does NOT match.
    lock.version = "9.9.9".to_string();
    let rewritten = serde_json::to_string_pretty(&lock).expect("re-serialize lock");
    std::fs::write(&lock_path, rewritten.as_bytes()).expect("rewrite desynced lock");
}

/// Write an extra file NOT in the bundle allow-set, provoking
/// [`pmcp_workbook_runtime::BundleLoadError::UnexpectedMember`] (the fail-closed
/// frozen-membership policy from 92-01 — Codex MEDIUM #9).
pub fn add_unexpected_member(dir: &Path) {
    let path = dir.join("evidence").join("sneaky.json");
    std::fs::write(&path, b"{\"sneaky\":true}").expect("write unexpected member");
}
