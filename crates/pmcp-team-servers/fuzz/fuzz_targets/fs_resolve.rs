//! Fuzz target for team-fs path resolution / jail-escape rejection (T-109-02-01).
//!
//! Drives the PURE LEXICAL normalizer and the symlink-checking resolver over
//! adversarial fuzzer bytes (`..`, absolute paths, embedded NUL, non-UTF-8) and
//! asserts two invariants:
//!   1. neither `normalize` nor `resolve` ever panics;
//!   2. on `Ok`, the resolved path is contained within the canonical workspace
//!      root (never escapes it), and the normalized path carries no `..`/root
//!      components.
#![no_main]

use std::path::{Component, PathBuf};
use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use pmcp_team_servers::fs::local::{normalize, LocalDirBackend};

/// A single fixed backend rooted in the OS temp dir, built once. libfuzzer runs
/// the target in one long-lived process, so a `OnceLock` is sufficient (and we
/// avoid depending on `tempfile`, which is not a fuzz-package dependency).
fn backend() -> &'static LocalDirBackend {
    static BACKEND: OnceLock<LocalDirBackend> = OnceLock::new();
    BACKEND.get_or_init(|| {
        let mut root = std::env::temp_dir();
        root.push("pmcp_fs_resolve_fuzz_root");
        LocalDirBackend::new(&root).expect("fuzz backend root")
    })
}

fn assert_contained(rel: &str) {
    // Pure lexical normalize: on success, no `..`/root components survive.
    if let Ok(norm) = normalize(rel) {
        let has_escape = norm
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_)));
        assert!(!has_escape, "normalize leaked an escape component: {norm:?}");
    }

    // Resolver: on success, the path is contained within the workspace root.
    let be = backend();
    if let Ok(resolved) = be.resolve_workspace(rel) {
        let root: &std::path::Path = be.workspace_root();
        assert!(
            resolved.starts_with(root),
            "resolved path escaped root: {resolved:?} not under {root:?}"
        );
    }
}

fuzz_target!(|data: &[u8]| {
    // Model the real input: tool `path` arguments arrive as JSON strings (UTF-8).
    // `from_utf8_lossy` still exercises non-UTF-8 byte sequences (replacement
    // chars), embedded NUL, `..`, and `/` separators.
    let candidate = String::from_utf8_lossy(data);
    assert_contained(&candidate);

    // Also exercise each `/`-delimited raw segment joined back through the
    // normalizer, so partial/odd component boundaries get coverage.
    let joined: PathBuf = candidate.split('/').collect();
    assert_contained(&joined.to_string_lossy());
});
