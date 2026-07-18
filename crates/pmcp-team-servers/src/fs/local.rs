//! The local-directory [`TeamFsBackend`] implementation (path-jailed to a root).
//!
//! [`LocalDirBackend`] serves a real directory tree a human can open in an
//! editor: a `workspace/` directory (the working area) plus a SIBLING
//! `review/` directory (the sync target, D-09). It is the dev-grade reference
//! backend — scaled/hardened storage stays platform-side behind the
//! [`TeamFsBackend`] seam.
//!
//! # Path safety (T-109-02-01)
//!
//! Containment is proven **lexically, in memory, BEFORE any filesystem side
//! effect** by [`normalize`]: it resolves `.`/`..` by string manipulation,
//! rejects absolute paths and `..` underflow, and rejects embedded NUL — with
//! NO `canonicalize`-then-IO window (the review-flagged TOCTOU pattern is
//! explicitly avoided). [`LocalDirBackend::resolve`] then joins the normalized
//! relative path under a trusted absolute root and, as defense in depth,
//! rejects any EXISTING component that is a symlink.
//!
//! ## Symlink / TOCTOU stance
//!
//! The local reference backend REJECTS symlink components outright. It does NOT
//! implement race-resistant `openat`-style traversal — that is deliberately out
//! of scope for a dev backend (documented non-goal). A hardened platform
//! backend behind the same trait can add it.

use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use async_trait::async_trait;

use crate::fs::backend::{Entry, FsError, Stat, TeamFsBackend};

/// Normalizes a caller-supplied RELATIVE path into a contained relative path,
/// resolving `.`/`..` **purely lexically** (no filesystem access whatsoever).
///
/// Rules:
/// - an embedded NUL byte is rejected ([`FsError::InvalidPath`]);
/// - an absolute path or a Windows-style prefix/root is rejected
///   ([`FsError::PathEscape`]);
/// - `.` components are collapsed;
/// - `..` pops the last accumulated component; if it would pop above the root
///   (underflow) the path is rejected ([`FsError::PathEscape`]);
/// - the empty path and `.` both normalize to the root (empty relative path).
///
/// On success the returned [`PathBuf`] is guaranteed to contain NO `..` or root
/// components, so joining it under any base cannot escape that base. This is
/// the sole containment proof — it happens BEFORE any I/O.
///
/// # Errors
///
/// [`FsError::InvalidPath`] for an embedded NUL; [`FsError::PathEscape`] for an
/// absolute path, a Windows prefix, or `..` underflow.
///
/// # Examples
///
/// ```
/// use pmcp_team_servers::fs::local::normalize;
///
/// assert_eq!(normalize("a/b/../c").unwrap(), std::path::Path::new("a/c"));
/// assert_eq!(normalize("./a/./b").unwrap(), std::path::Path::new("a/b"));
/// assert!(normalize("../x").is_err());
/// assert!(normalize("/abs").is_err());
/// assert!(normalize("a/../../x").is_err());
/// ```
pub fn normalize(rel: &str) -> Result<PathBuf, FsError> {
    if rel.contains('\0') {
        return Err(FsError::InvalidPath(format!("embedded NUL: {rel:?}")));
    }

    let mut out: Vec<std::ffi::OsString> = Vec::new();
    for comp in Path::new(rel).components() {
        match comp {
            // Absolute path (`/…`) or a Windows drive/UNC prefix — never relative.
            Component::RootDir | Component::Prefix(_) => {
                return Err(FsError::PathEscape(format!("not a relative path: {rel:?}")));
            },
            // `.` — no-op.
            Component::CurDir => {},
            // `..` — pop; underflow escapes the root.
            Component::ParentDir => {
                if out.pop().is_none() {
                    return Err(FsError::PathEscape(format!("`..` escapes root: {rel:?}")));
                }
            },
            Component::Normal(seg) => out.push(seg.to_os_string()),
        }
    }

    let mut normalized = PathBuf::new();
    for seg in out {
        normalized.push(seg);
    }
    Ok(normalized)
}

/// A dev-grade [`TeamFsBackend`] backed by a real local directory tree.
///
/// Holds two absolute, canonical roots: `workspace` (the working area) and a
/// sibling `review` directory (the sync target). Both are created on
/// construction. All caller paths are resolved under `workspace` except the
/// review side of the sync operations.
#[derive(Debug, Clone)]
pub struct LocalDirBackend {
    workspace: PathBuf,
    review: PathBuf,
}

impl LocalDirBackend {
    /// Creates a backend rooted at `root`, establishing `root/workspace` and the
    /// sibling `root/review` directories (both created if absent).
    ///
    /// The roots are canonicalized so the resolved paths — and the `file://`
    /// URLs derived from them — are absolute and real.
    ///
    /// # Errors
    ///
    /// [`FsError::Io`] if either directory cannot be created or canonicalized.
    pub fn new(root: impl AsRef<Path>) -> Result<Self, FsError> {
        let root = root.as_ref();
        let workspace = root.join("workspace");
        let review = root.join("review");
        fs::create_dir_all(&workspace).map_err(|e| FsError::Io(e.to_string()))?;
        fs::create_dir_all(&review).map_err(|e| FsError::Io(e.to_string()))?;
        let workspace = fs::canonicalize(&workspace).map_err(|e| FsError::Io(e.to_string()))?;
        let review = fs::canonicalize(&review).map_err(|e| FsError::Io(e.to_string()))?;
        Ok(Self { workspace, review })
    }

    /// The absolute, canonical workspace root.
    #[must_use]
    pub fn workspace_root(&self) -> &Path {
        &self.workspace
    }

    /// The absolute, canonical review root.
    #[must_use]
    pub fn review_root(&self) -> &Path {
        &self.review
    }

    /// Resolves a caller path under the workspace root, proving containment
    /// lexically and rejecting symlink components. See [`LocalDirBackend::resolve`].
    ///
    /// # Errors
    ///
    /// See [`LocalDirBackend::resolve`].
    pub fn resolve_workspace(&self, rel: &str) -> Result<PathBuf, FsError> {
        self.resolve(&self.workspace, rel)
    }

    /// Joins the lexically-[`normalize`]d `rel` under the trusted absolute
    /// `base`, then walks the joined path's EXISTING ancestor components and
    /// rejects any that is a symlink.
    ///
    /// Containment is already guaranteed by [`normalize`] (the returned path has
    /// no `..`/root components); the returned path is additionally asserted to
    /// start with `base` as a belt-and-suspenders invariant. Nonexistent trailing
    /// components are fine — `write`/`create_directory` create them safely under
    /// the contained path.
    ///
    /// # Errors
    ///
    /// - [`FsError::PathEscape`] / [`FsError::InvalidPath`] from [`normalize`].
    /// - [`FsError::Symlink`] if an existing component is a symlink.
    pub fn resolve(&self, base: &Path, rel: &str) -> Result<PathBuf, FsError> {
        let normalized = normalize(rel)?;
        let joined = base.join(&normalized);

        // Defense in depth: the lexical proof already guarantees this, but assert
        // it explicitly so the invariant is airtight even if `normalize` regresses.
        if !joined.starts_with(base) {
            return Err(FsError::PathEscape(format!("escapes root: {rel:?}")));
        }

        // Reject symlink components (explicit TOCTOU stance for the dev backend).
        let mut current = base.to_path_buf();
        for comp in normalized.components() {
            current.push(comp);
            match fs::symlink_metadata(&current) {
                Ok(md) if md.file_type().is_symlink() => {
                    return Err(FsError::Symlink(rel.to_string()));
                },
                // Existing non-symlink component, or a nonexistent component
                // (nothing to check yet) — both are fine.
                Ok(_) | Err(_) => {},
            }
        }

        Ok(joined)
    }
}

/// Percent-encodes an absolute filesystem path into a valid `file://` URL.
///
/// Encodes every byte except the RFC 3986 unreserved set (`A-Z a-z 0-9 - . _ ~`)
/// and the path separator `/`, so spaces, `#`, `%`, and non-ASCII bytes are all
/// escaped. This is the tested percent-encoding helper the plan mandates in place
/// of `format!("file://{}", path)`, which breaks on those characters.
fn to_file_url(path: &Path) -> String {
    // Absolute paths start with `/`; the encoded body therefore begins with `/`,
    // yielding the canonical `file:///…` triple-slash form.
    let bytes = path.as_os_str().to_string_lossy();
    let mut url = String::from("file://");
    for &b in bytes.as_bytes() {
        let unreserved = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/');
        if unreserved {
            url.push(b as char);
        } else {
            url.push('%');
            url.push(
                char::from_digit((u32::from(b)) >> 4, 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
            url.push(
                char::from_digit((u32::from(b)) & 0xf, 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
        }
    }
    url
}

/// Recursively copies `src` to `dst`, overwriting existing files/dirs.
///
/// Best-effort: on the first failure it returns [`FsError::Io`] and leaves a
/// documented partial state (already-copied entries remain). This is NOT a
/// silent success.
fn copy_recursive(src: &Path, dst: &Path) -> Result<(), FsError> {
    let md = fs::symlink_metadata(src).map_err(|e| FsError::Io(e.to_string()))?;
    if md.file_type().is_symlink() {
        return Err(FsError::Symlink(src.display().to_string()));
    }
    if md.is_dir() {
        fs::create_dir_all(dst).map_err(|e| FsError::Io(e.to_string()))?;
        for entry in fs::read_dir(src).map_err(|e| FsError::Io(e.to_string()))? {
            let entry = entry.map_err(|e| FsError::Io(e.to_string()))?;
            let child_dst = dst.join(entry.file_name());
            copy_recursive(&entry.path(), &child_dst)?;
        }
        Ok(())
    } else {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).map_err(|e| FsError::Io(e.to_string()))?;
        }
        // Overwrite semantics: fs::copy truncates/replaces the destination file.
        fs::copy(src, dst).map_err(|e| FsError::Io(e.to_string()))?;
        Ok(())
    }
}

impl LocalDirBackend {
    /// Shared implementation for both sync directions.
    fn sync(&self, from: &Path, to: &Path, rel: &str) -> Result<(), FsError> {
        let src = self.resolve(from, rel)?;
        let dst = self.resolve(to, rel)?;
        if !src.exists() {
            return Err(FsError::NotFound(rel.to_string()));
        }
        copy_recursive(&src, &dst)
    }
}

/// Converts a resolved absolute path back to its workspace-relative, forward
/// slashed form for [`Entry`]/[`Stat`] reporting.
fn rel_display(base: &Path, abs: &Path) -> String {
    abs.strip_prefix(base)
        .unwrap_or(abs)
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[async_trait]
impl TeamFsBackend for LocalDirBackend {
    async fn list(&self, path: &str) -> Result<Vec<Entry>, FsError> {
        let dir = self.resolve_workspace(path)?;
        let read = fs::read_dir(&dir).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => FsError::NotFound(path.to_string()),
            _ => FsError::Io(e.to_string()),
        })?;
        let mut entries = Vec::new();
        for entry in read {
            let entry = entry.map_err(|e| FsError::Io(e.to_string()))?;
            let md = entry.metadata().map_err(|e| FsError::Io(e.to_string()))?;
            let abs = entry.path();
            entries.push(Entry {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: rel_display(&self.workspace, &abs),
                is_dir: md.is_dir(),
                size: if md.is_dir() { 0 } else { md.len() },
            });
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>, FsError> {
        let file = self.resolve_workspace(path)?;
        fs::read(&file).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => FsError::NotFound(path.to_string()),
            _ => FsError::Io(e.to_string()),
        })
    }

    async fn write(&self, path: &str, bytes: &[u8]) -> Result<(), FsError> {
        let file = self.resolve_workspace(path)?;
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).map_err(|e| FsError::Io(e.to_string()))?;
        }
        fs::write(&file, bytes).map_err(|e| FsError::Io(e.to_string()))
    }

    async fn append_file(&self, path: &str, bytes: &[u8]) -> Result<(), FsError> {
        use std::io::Write;
        let file = self.resolve_workspace(path)?;
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).map_err(|e| FsError::Io(e.to_string()))?;
        }
        let mut fh = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file)
            .map_err(|e| FsError::Io(e.to_string()))?;
        fh.write_all(bytes).map_err(|e| FsError::Io(e.to_string()))
    }

    async fn head(&self, path: &str, max_bytes: usize) -> Result<Vec<u8>, FsError> {
        let file = self.resolve_workspace(path)?;
        let fh = fs::File::open(&file).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => FsError::NotFound(path.to_string()),
            _ => FsError::Io(e.to_string()),
        })?;
        let mut buf = Vec::new();
        fh.take(max_bytes as u64)
            .read_to_end(&mut buf)
            .map_err(|e| FsError::Io(e.to_string()))?;
        Ok(buf)
    }

    async fn stat(&self, path: &str) -> Result<Stat, FsError> {
        let target = self.resolve_workspace(path)?;
        let md = fs::metadata(&target).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => FsError::NotFound(path.to_string()),
            _ => FsError::Io(e.to_string()),
        })?;
        Ok(Stat {
            path: rel_display(&self.workspace, &target),
            is_dir: md.is_dir(),
            size: if md.is_dir() { 0 } else { md.len() },
        })
    }

    async fn create_directory(&self, path: &str) -> Result<(), FsError> {
        let dir = self.resolve_workspace(path)?;
        fs::create_dir_all(&dir).map_err(|e| FsError::Io(e.to_string()))
    }

    async fn get_download_url(&self, path: &str) -> Result<String, FsError> {
        let file = self.resolve_workspace(path)?;
        if !file.exists() {
            return Err(FsError::NotFound(path.to_string()));
        }
        Ok(to_file_url(&file))
    }

    async fn sync_to_review(&self, path: &str) -> Result<(), FsError> {
        let (ws, rv) = (self.workspace.clone(), self.review.clone());
        self.sync(&ws, &rv, path)
    }

    async fn sync_from_review(&self, path: &str) -> Result<(), FsError> {
        let (ws, rv) = (self.workspace.clone(), self.review.clone());
        self.sync(&rv, &ws, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn backend() -> (TempDir, LocalDirBackend) {
        let tmp = TempDir::new().unwrap();
        let backend = LocalDirBackend::new(tmp.path()).unwrap();
        (tmp, backend)
    }

    // ---- pure lexical normalizer (no filesystem) --------------------------

    #[test]
    fn normalize_collapses_interior_dotdot() {
        assert_eq!(normalize("a/b/../c").unwrap(), Path::new("a/c"));
        assert_eq!(normalize("a/./b").unwrap(), Path::new("a/b"));
        assert_eq!(normalize("").unwrap(), Path::new(""));
        assert_eq!(normalize(".").unwrap(), Path::new(""));
    }

    #[test]
    fn normalize_rejects_escape_absolute_and_nul() {
        assert!(matches!(normalize("../x"), Err(FsError::PathEscape(_))));
        assert!(matches!(
            normalize("a/../../x"),
            Err(FsError::PathEscape(_))
        ));
        assert!(matches!(normalize("/abs"), Err(FsError::PathEscape(_))));
        assert!(matches!(normalize("a\0b"), Err(FsError::InvalidPath(_))));
    }

    // ---- round-trips incl. a NEW nested path ------------------------------

    #[tokio::test]
    async fn write_read_round_trip_new_nested_path() {
        let (_tmp, be) = backend();
        // Parent dirs `deep/nested/` do not exist yet.
        be.write("deep/nested/file.txt", b"hello world")
            .await
            .unwrap();
        assert_eq!(
            be.read("deep/nested/file.txt").await.unwrap(),
            b"hello world"
        );
        let stat = be.stat("deep/nested/file.txt").await.unwrap();
        assert!(!stat.is_dir);
        assert_eq!(stat.size, 11);
    }

    #[tokio::test]
    async fn append_and_head_bound() {
        let (_tmp, be) = backend();
        be.append_file("log.txt", b"AAAA").await.unwrap();
        be.append_file("log.txt", b"BBBB").await.unwrap();
        assert_eq!(be.read("log.txt").await.unwrap(), b"AAAABBBB");
        assert_eq!(be.head("log.txt", 4).await.unwrap(), b"AAAA");
    }

    #[tokio::test]
    async fn list_sorts_and_reports_relative_paths() {
        let (_tmp, be) = backend();
        be.write("b.txt", b"1").await.unwrap();
        be.write("a.txt", b"22").await.unwrap();
        be.create_directory("sub").await.unwrap();
        let entries = be.list("").await.unwrap();
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["a.txt", "b.txt", "sub"]);
        assert!(entries.iter().find(|e| e.name == "sub").unwrap().is_dir);
    }

    // ---- review sync both directions with overwrite -----------------------

    #[tokio::test]
    async fn sync_to_and_from_review_with_overwrite() {
        let (_tmp, be) = backend();
        be.write("dir/a.txt", b"v1").await.unwrap();
        be.sync_to_review("dir").await.unwrap();
        let mirrored = be.review_root().join("dir").join("a.txt");
        assert_eq!(fs::read(&mirrored).unwrap(), b"v1");

        // Overwrite the review copy, sync back, workspace is replaced.
        fs::write(&mirrored, b"v2-from-review").unwrap();
        be.sync_from_review("dir/a.txt").await.unwrap();
        assert_eq!(be.read("dir/a.txt").await.unwrap(), b"v2-from-review");
    }

    #[tokio::test]
    async fn sync_missing_source_is_not_found() {
        let (_tmp, be) = backend();
        assert!(matches!(
            be.sync_to_review("nope.txt").await,
            Err(FsError::NotFound(_))
        ));
    }

    // ---- percent-encoded file:// URL --------------------------------------

    #[tokio::test]
    async fn download_url_percent_encodes_space() {
        let (_tmp, be) = backend();
        be.write("my report #1.txt", b"x").await.unwrap();
        let url = be.get_download_url("my report #1.txt").await.unwrap();
        assert!(url.starts_with("file:///"), "got {url}");
        assert!(!url.contains(' '), "raw space not encoded: {url}");
        assert!(url.contains("%20"), "space not percent-encoded: {url}");
        assert!(url.contains("%23"), "'#' not percent-encoded: {url}");
    }

    // ---- side-effect-free rejection of `..`/absolute ----------------------

    #[tokio::test]
    async fn dotdot_write_is_rejected_with_no_side_effect() {
        let (tmp, be) = backend();
        let err = be.write("../escaped.txt", b"pwned").await.unwrap_err();
        assert!(matches!(err, FsError::PathEscape(_)));
        // Prove NO side effect: nothing was written outside the workspace.
        assert!(!tmp.path().join("escaped.txt").exists());
        assert!(!be.workspace_root().join("../escaped.txt").exists());
    }

    #[tokio::test]
    async fn absolute_path_is_rejected() {
        let (_tmp, be) = backend();
        assert!(matches!(
            be.read("/etc/passwd").await,
            Err(FsError::PathEscape(_))
        ));
    }

    // ---- symlink escape rejection -----------------------------------------

    #[cfg(unix)]
    #[tokio::test]
    async fn symlink_escaping_root_is_rejected() {
        use std::os::unix::fs::symlink;
        let (tmp, be) = backend();
        // Create a symlink inside the workspace pointing OUTSIDE the root.
        let outside = tmp.path().join("outside_secret");
        fs::write(&outside, b"top secret").unwrap();
        let link = be.workspace_root().join("link");
        symlink(&outside, &link).unwrap();

        // Reading through the symlink component must be rejected.
        assert!(matches!(be.read("link").await, Err(FsError::Symlink(_))));
        // And a path traversing THROUGH the symlink likewise.
        assert!(matches!(
            be.read("link/child").await,
            Err(FsError::Symlink(_))
        ));
    }

    #[tokio::test]
    async fn read_missing_is_not_found() {
        let (_tmp, be) = backend();
        assert!(matches!(
            be.read("ghost.txt").await,
            Err(FsError::NotFound(_))
        ));
    }
}
