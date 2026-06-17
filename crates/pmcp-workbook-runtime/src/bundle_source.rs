//! The dumb-byte [`BundleSource`] trait + its local-dir and embedded impls
//! (Phase 92, Plan 01 — WBSV-09, WBSV-08 boundary).
//!
//! A [`BundleSource`] exposes ONLY raw-byte access to a bundle's members:
//! [`BundleSource::read_artifact`] (one member's exact bytes) and
//! [`BundleSource::list_artifacts`] (the sorted member set). It deliberately
//! CANNOT return a parsed bundle — no source impl can pre-parse and thereby
//! skip the integrity gate. The single shared
//! [`crate::bundle_loader::load`] is the ONLY parse+verify path (WBSV-08, the
//! type-level bypass impossibility, threat T-92-03).
//!
//! Two impls ship:
//!
//! - [`LocalDirSource`] — reads a bundle from a directory tree on disk. One
//!   source = one bundle@version (D-08); the constructor takes the bundle root.
//! - [`EmbeddedSource`] (behind the `embedded` feature) — reads the SAME bundle
//!   baked into the binary via an [`include_dir::Dir`] (WBSV-09). The two return
//!   identical bytes for the same member, so the loader verifies them identically.
//!
//! The trait is SYNC (no `async_trait`, D-07): a byte accessor has no I/O
//! concurrency need on the boot path, and a sync trait stays object-safe +
//! `Send + Sync` without an executor.

use std::path::{Path, PathBuf};

use thiserror::Error;

/// Raw-byte access to a single bundle's members.
///
/// This trait is the WBSV-08 boundary (threat T-92-03): it exposes ONLY bytes,
/// never a parsed bundle, so no impl can bypass the shared
/// [`crate::bundle_loader::load`] integrity gate. It is `Send + Sync` so the
/// served binary can hold an `Arc<dyn BundleSource>` across handler tasks, and
/// it is object-safe (both methods take `&self`, no generics) so `Box<dyn
/// BundleSource>` works.
///
/// # Example
///
/// A minimal in-memory source — the doctest defines a LOCAL dummy impl, never a
/// downstream crate, so there is no circular doctest dependency.
///
/// ```
/// use pmcp_workbook_runtime::{BundleSource, BundleSourceError};
///
/// struct OneMember;
///
/// impl BundleSource for OneMember {
///     fn read_artifact(&self, name: &str) -> Result<Vec<u8>, BundleSourceError> {
///         if name == "manifest.json" {
///             Ok(b"{}".to_vec())
///         } else {
///             Err(BundleSourceError::NotFound { member: name.to_string() })
///         }
///     }
///     fn list_artifacts(&self) -> Result<Vec<String>, BundleSourceError> {
///         Ok(vec!["manifest.json".to_string()])
///     }
/// }
///
/// let src = OneMember;
/// assert_eq!(src.read_artifact("manifest.json").unwrap(), b"{}");
/// assert!(src.read_artifact("missing.json").is_err());
/// ```
pub trait BundleSource: Send + Sync {
    /// Return the EXACT bytes of the member named `name` (a bundle-relative
    /// path such as `"manifest.json"` or `"evidence/changelog.json"`).
    ///
    /// # Errors
    ///
    /// Returns [`BundleSourceError::NotFound`] when no such member exists, or
    /// [`BundleSourceError::Io`] when the underlying read fails.
    fn read_artifact(&self, name: &str) -> Result<Vec<u8>, BundleSourceError>;

    /// Return the SORTED list of every member's bundle-relative path
    /// (including nested members like `"evidence/changelog.json"`).
    ///
    /// The loader uses this to enforce its fail-closed membership policy, so
    /// the list MUST be complete and sorted for a stable diagnostic.
    ///
    /// # Errors
    ///
    /// Returns [`BundleSourceError::Io`] when the member set cannot be
    /// enumerated.
    fn list_artifacts(&self) -> Result<Vec<String>, BundleSourceError>;
}

/// Errors a [`BundleSource`] may surface.
///
/// `#[non_exhaustive]` so future source kinds (S3, registry — the documented
/// extension seam) can add failure modes additively without a semver break.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BundleSourceError {
    /// The underlying byte read or directory walk failed.
    #[error("bundle source I/O error: {0}")]
    Io(String),

    /// The requested member does not exist in this bundle.
    #[error("bundle member not found: {member}")]
    NotFound {
        /// The bundle-relative member path that was requested.
        member: String,
    },
}

/// A [`BundleSource`] that reads a bundle from a directory tree on disk.
///
/// One `LocalDirSource` wraps ONE bundle root = one bundle@version (D-08); the
/// member name is joined onto the root to read bytes. `list_artifacts` walks
/// the tree recursively and returns sorted bundle-relative paths.
#[derive(Debug, Clone)]
pub struct LocalDirSource {
    root: PathBuf,
}

impl LocalDirSource {
    /// Wrap the bundle directory rooted at `path`.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { root: path.into() }
    }

    /// Recursively collect bundle-relative member paths under `dir`, pushing
    /// each into `out` with `/`-normalized separators relative to the root.
    fn collect_members(&self, dir: &Path, out: &mut Vec<String>) -> Result<(), BundleSourceError> {
        let entries = std::fs::read_dir(dir).map_err(|e| BundleSourceError::Io(e.to_string()))?;
        for entry in entries {
            let entry = entry.map_err(|e| BundleSourceError::Io(e.to_string()))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|e| BundleSourceError::Io(e.to_string()))?;
            if file_type.is_dir() {
                self.collect_members(&path, out)?;
            } else {
                let rel = path.strip_prefix(&self.root).map_err(|_| {
                    BundleSourceError::Io(format!(
                        "member {} is not under bundle root {}",
                        path.display(),
                        self.root.display()
                    ))
                })?;
                // Normalize to forward slashes so the member path matches the
                // loader's allow-set regardless of host path separator.
                let normalized = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                out.push(normalized);
            }
        }
        Ok(())
    }
}

impl BundleSource for LocalDirSource {
    fn read_artifact(&self, name: &str) -> Result<Vec<u8>, BundleSourceError> {
        let path = self.root.join(name);
        match std::fs::read(&path) {
            Ok(bytes) => Ok(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(BundleSourceError::NotFound {
                    member: name.to_string(),
                })
            },
            Err(e) => Err(BundleSourceError::Io(e.to_string())),
        }
    }

    fn list_artifacts(&self) -> Result<Vec<String>, BundleSourceError> {
        let mut out = Vec::new();
        self.collect_members(&self.root, &mut out)?;
        out.sort();
        Ok(out)
    }
}

/// A [`BundleSource`] that reads a bundle baked into the binary via
/// [`include_dir::Dir`] (WBSV-09), gated behind the `embedded` feature.
///
/// Downstream callers construct it from an `include_dir!` macro over a committed
/// bundle directory and wrap that static `Dir`:
///
/// ```ignore
/// use include_dir::{include_dir, Dir};
/// use pmcp_workbook_runtime::EmbeddedSource;
///
/// static BUNDLE: Dir = include_dir!("$CARGO_MANIFEST_DIR/bundle");
/// let source = EmbeddedSource::new(&BUNDLE);
/// ```
///
/// It returns the SAME bytes [`LocalDirSource`] does for the same member, so the
/// shared loader integrity-checks an embedded bundle identically to an on-disk
/// one.
#[cfg(feature = "embedded")]
#[derive(Debug, Clone)]
pub struct EmbeddedSource {
    dir: &'static include_dir::Dir<'static>,
}

#[cfg(feature = "embedded")]
impl EmbeddedSource {
    /// Wrap a `'static` [`include_dir::Dir`] produced by the `include_dir!`
    /// macro over a committed bundle directory.
    pub fn new(dir: &'static include_dir::Dir<'static>) -> Self {
        Self { dir }
    }

    /// Recursively collect member paths from an embedded [`include_dir::Dir`].
    fn collect(dir: &include_dir::Dir<'static>, out: &mut Vec<String>) {
        for file in dir.files() {
            out.push(file.path().to_string_lossy().replace('\\', "/"));
        }
        for sub in dir.dirs() {
            Self::collect(sub, out);
        }
    }
}

#[cfg(feature = "embedded")]
impl BundleSource for EmbeddedSource {
    fn read_artifact(&self, name: &str) -> Result<Vec<u8>, BundleSourceError> {
        self.dir.get_file(name).map_or_else(
            || {
                Err(BundleSourceError::NotFound {
                    member: name.to_string(),
                })
            },
            |file| Ok(file.contents().to_vec()),
        )
    }

    fn list_artifacts(&self) -> Result<Vec<String>, BundleSourceError> {
        let mut out = Vec::new();
        Self::collect(self.dir, &mut out);
        out.sort();
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Object-safety + auto-trait assertion: `Box<dyn BundleSource>` must be
    /// `Send + Sync` so the served binary can share an `Arc<dyn BundleSource>`.
    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn bundle_source_trait_object_is_send_sync() {
        assert_send_sync::<Box<dyn BundleSource>>();
    }

    /// A self-cleaning unique temp directory (no `tempfile` dependency — the
    /// runtime crate stays lean; Drop removes the tree).
    struct TempBundle {
        path: PathBuf,
    }

    impl TempBundle {
        fn new(tag: &str) -> Self {
            use std::sync::atomic::{AtomicU64, Ordering};
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let pid = std::process::id();
            let path = std::env::temp_dir().join(format!("pwr-bundle-src-{tag}-{pid}-{n}"));
            std::fs::create_dir_all(path.join("evidence")).unwrap();
            std::fs::write(path.join("manifest.json"), b"{\"manifest\":true}").unwrap();
            std::fs::write(path.join("BUNDLE.lock"), b"{\"lock\":1}").unwrap();
            std::fs::write(path.join("evidence/changelog.json"), b"{\"changelog\":[]}").unwrap();
            Self { path }
        }
    }

    impl Drop for TempBundle {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn local_dir_source_reads_exact_bytes() {
        let bundle = TempBundle::new("read");
        let src = LocalDirSource::new(&bundle.path);
        assert_eq!(
            src.read_artifact("manifest.json").unwrap(),
            b"{\"manifest\":true}"
        );
        assert_eq!(
            src.read_artifact("evidence/changelog.json").unwrap(),
            b"{\"changelog\":[]}"
        );
    }

    #[test]
    fn local_dir_source_lists_sorted_relative_paths_including_nested() {
        let bundle = TempBundle::new("list");
        let src = LocalDirSource::new(&bundle.path);
        let members = src.list_artifacts().unwrap();
        assert_eq!(
            members,
            vec![
                "BUNDLE.lock".to_string(),
                "evidence/changelog.json".to_string(),
                "manifest.json".to_string(),
            ],
            "members are sorted and include the nested evidence path"
        );
    }

    #[test]
    fn local_dir_source_missing_member_returns_not_found_not_panic() {
        let bundle = TempBundle::new("missing");
        let src = LocalDirSource::new(&bundle.path);
        match src.read_artifact("does_not_exist.json") {
            Err(BundleSourceError::NotFound { member }) => {
                assert_eq!(member, "does_not_exist.json");
            },
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn not_found_display_names_the_member() {
        let err = BundleSourceError::NotFound {
            member: "layout.json".to_string(),
        };
        assert!(format!("{err}").contains("layout.json"));
    }

    /// EmbeddedSource over a baked-in tree returns the SAME bytes LocalDirSource
    /// does for the same member (WBSV-09 parity). The embedded tree is the
    /// committed fixture under `tests/fixtures/embedded_bundle`.
    #[cfg(feature = "embedded")]
    #[test]
    fn embedded_source_matches_local_dir_bytes() {
        use include_dir::{include_dir, Dir};
        static FIXTURE: Dir = include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/embedded_bundle");
        let embedded = EmbeddedSource::new(&FIXTURE);

        let manifest_root = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/embedded_bundle"
        );
        let local = LocalDirSource::new(manifest_root);

        for member in ["manifest.json", "evidence/changelog.json"] {
            assert_eq!(
                embedded.read_artifact(member).unwrap(),
                local.read_artifact(member).unwrap(),
                "embedded and local-dir bytes must match for {member}"
            );
        }
        // list_artifacts agrees on the member set.
        assert_eq!(
            embedded.list_artifacts().unwrap(),
            local.list_artifacts().unwrap(),
            "embedded and local-dir member sets must match"
        );
    }
}
