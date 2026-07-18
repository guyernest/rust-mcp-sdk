//! The storage-agnostic team-fs backend trait (list/read/write/stat/…).
//!
//! [`TeamFsBackend`] is an object-safe async trait defining the STORAGE
//! contract for the team-fs reference server. It exposes the ten `fs__*`
//! *storage* operations; task completion (`fs__complete_task`) is a
//! protocol/server concern and lives in [`crate::fs::server`], NOT here — a
//! storage backend has no notion of MCP task lifecycle (109-02 review).
//!
//! The SDK ships one dev-grade implementation, [`crate::fs::local::LocalDirBackend`]
//! (a real directory tree). Scaled/hardened storage stays platform-side behind
//! this same seam.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A single directory entry returned by [`TeamFsBackend::list`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// The entry's file name (final path component), not the full path.
    pub name: String,
    /// The entry's path RELATIVE to the workspace root (forward-slashed).
    pub path: String,
    /// Whether this entry is a directory.
    pub is_dir: bool,
    /// Size in bytes (0 for directories).
    pub size: u64,
}

/// Metadata about a single path, returned by [`TeamFsBackend::stat`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stat {
    /// The path RELATIVE to the workspace root (forward-slashed).
    pub path: String,
    /// Whether the path is a directory.
    pub is_dir: bool,
    /// Size in bytes (0 for directories).
    pub size: u64,
}

/// Errors returned by [`TeamFsBackend`] operations.
///
/// The path-safety arms ([`FsError::PathEscape`], [`FsError::Symlink`],
/// [`FsError::InvalidPath`]) are the security-relevant rejections proven
/// LEXICALLY (for escape/invalid) or via a metadata probe (for symlink)
/// BEFORE any filesystem mutation — see [`crate::fs::local`].
#[derive(Debug, thiserror::Error)]
pub enum FsError {
    /// The requested path does not exist.
    #[error("path not found: {0}")]
    NotFound(String),

    /// The path escapes the jailed root (`..` underflow or an absolute path).
    ///
    /// This is proven by the pure lexical normalizer WITHOUT touching disk,
    /// so a rejected path never produces a filesystem side effect.
    #[error("path escapes root: {0}")]
    PathEscape(String),

    /// An existing component of the path is a symlink.
    ///
    /// The local reference backend REJECTS symlinks outright (documented TOCTOU
    /// stance: race-resistant open-at traversal is out of scope for the dev
    /// backend; hardened storage stays platform-side).
    #[error("path traverses a symlink: {0}")]
    Symlink(String),

    /// The path is syntactically invalid (embedded NUL, non-relative prefix, …).
    #[error("invalid path: {0}")]
    InvalidPath(String),

    /// An underlying I/O error occurred.
    #[error("io error: {0}")]
    Io(String),

    /// The tool arguments were invalid (missing/ill-typed fields).
    #[error("invalid arguments: {0}")]
    InvalidArgs(String),
}

/// The storage-agnostic team-fs backend contract.
///
/// Implementations perform the actual filesystem/object-store I/O behind the
/// `fs__*` tools. Every method takes a path RELATIVE to the backend's root and
/// MUST reject any path that escapes it. Implementations are `Send + Sync` for
/// concurrent request handling and object-safe (usable as `Arc<dyn TeamFsBackend>`).
///
/// # Not a storage concern
///
/// `fs__complete_task` is intentionally absent: task completion is MCP
/// protocol behavior owned by the server layer, not storage.
#[async_trait]
pub trait TeamFsBackend: Send + Sync {
    /// Lists the immediate entries of the directory at `path`.
    ///
    /// # Errors
    ///
    /// - [`FsError::PathEscape`] / [`FsError::InvalidPath`] if `path` is unsafe.
    /// - [`FsError::Symlink`] if any existing component is a symlink.
    /// - [`FsError::NotFound`] if the directory does not exist.
    /// - [`FsError::Io`] on other I/O failure.
    async fn list(&self, path: &str) -> Result<Vec<Entry>, FsError>;

    /// Reads the full contents of the file at `path`.
    ///
    /// # Errors
    ///
    /// - [`FsError::PathEscape`] / [`FsError::InvalidPath`] / [`FsError::Symlink`]
    ///   if `path` is unsafe.
    /// - [`FsError::NotFound`] if the file does not exist.
    /// - [`FsError::Io`] on other I/O failure.
    async fn read(&self, path: &str) -> Result<Vec<u8>, FsError>;

    /// Writes `bytes` to `path`, creating any missing parent directories and
    /// truncating an existing file.
    ///
    /// # Errors
    ///
    /// - [`FsError::PathEscape`] / [`FsError::InvalidPath`] / [`FsError::Symlink`]
    ///   if `path` is unsafe.
    /// - [`FsError::Io`] on I/O failure.
    async fn write(&self, path: &str, bytes: &[u8]) -> Result<(), FsError>;

    /// Appends `bytes` to the file at `path`, creating it (and parents) if absent.
    ///
    /// # Errors
    ///
    /// - [`FsError::PathEscape`] / [`FsError::InvalidPath`] / [`FsError::Symlink`]
    ///   if `path` is unsafe.
    /// - [`FsError::Io`] on I/O failure.
    async fn append_file(&self, path: &str, bytes: &[u8]) -> Result<(), FsError>;

    /// Reads at most `max_bytes` from the start of the file at `path`.
    ///
    /// Bounds the read to avoid loading arbitrarily large files (T-109-02-02).
    ///
    /// # Errors
    ///
    /// - [`FsError::PathEscape`] / [`FsError::InvalidPath`] / [`FsError::Symlink`]
    ///   if `path` is unsafe.
    /// - [`FsError::NotFound`] if the file does not exist.
    /// - [`FsError::Io`] on other I/O failure.
    async fn head(&self, path: &str, max_bytes: usize) -> Result<Vec<u8>, FsError>;

    /// Returns metadata for `path`.
    ///
    /// # Errors
    ///
    /// - [`FsError::PathEscape`] / [`FsError::InvalidPath`] / [`FsError::Symlink`]
    ///   if `path` is unsafe.
    /// - [`FsError::NotFound`] if the path does not exist.
    /// - [`FsError::Io`] on other I/O failure.
    async fn stat(&self, path: &str) -> Result<Stat, FsError>;

    /// Creates the directory at `path` (and any missing parents).
    ///
    /// # Errors
    ///
    /// - [`FsError::PathEscape`] / [`FsError::InvalidPath`] / [`FsError::Symlink`]
    ///   if `path` is unsafe.
    /// - [`FsError::Io`] on I/O failure.
    async fn create_directory(&self, path: &str) -> Result<(), FsError>;

    /// Returns a URL from which the file at `path` can be downloaded.
    ///
    /// For [`crate::fs::local::LocalDirBackend`] this is a percent-encoded
    /// `file://` URL pointing at the real on-disk path (D-08).
    ///
    /// # Errors
    ///
    /// - [`FsError::PathEscape`] / [`FsError::InvalidPath`] / [`FsError::Symlink`]
    ///   if `path` is unsafe.
    /// - [`FsError::NotFound`] if the file does not exist.
    /// - [`FsError::Io`] on other I/O failure.
    async fn get_download_url(&self, path: &str) -> Result<String, FsError>;

    /// Copies `workspace/<path>` into the sibling `review/<path>` tree,
    /// recursively for directories (D-09). Overwrites the destination.
    ///
    /// # Errors
    ///
    /// - [`FsError::PathEscape`] / [`FsError::InvalidPath`] / [`FsError::Symlink`]
    ///   if `path` is unsafe on either side.
    /// - [`FsError::NotFound`] if the source does not exist.
    /// - [`FsError::Io`] on a (possibly partial) copy failure.
    async fn sync_to_review(&self, path: &str) -> Result<(), FsError>;

    /// Copies `review/<path>` back into `workspace/<path>` (the reverse of
    /// [`TeamFsBackend::sync_to_review`]). Overwrites the destination.
    ///
    /// # Errors
    ///
    /// - [`FsError::PathEscape`] / [`FsError::InvalidPath`] / [`FsError::Symlink`]
    ///   if `path` is unsafe on either side.
    /// - [`FsError::NotFound`] if the source does not exist.
    /// - [`FsError::Io`] on a (possibly partial) copy failure.
    async fn sync_from_review(&self, path: &str) -> Result<(), FsError>;
}
