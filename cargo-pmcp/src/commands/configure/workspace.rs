//! Workspace-root resolution for Phase 77 named-target selection.
//!
//! Walks up from the current working directory looking for `Cargo.toml`.
//! In a monorepo with sibling servers, this returns the **innermost** server
//! directory — exactly the per-server-marker semantic D-01 promises.

use std::path::PathBuf;

use anyhow::{Context, Result};

/// Walks up from `current_dir()` until a directory containing `Cargo.toml` is found.
/// Returns the directory path. Errors when the walk reaches the filesystem root.
pub fn find_workspace_root() -> Result<PathBuf> {
    let current_dir = std::env::current_dir().context("Failed to get current directory")?;
    let mut dir = current_dir.as_path();
    loop {
        if dir.join("Cargo.toml").exists() {
            return Ok(dir.to_path_buf());
        }
        dir = dir
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Could not find Cargo.toml in any parent directory"))?;
    }
}

/// Walks up from `current_dir()` (inclusive of the current directory itself)
/// looking for a directory that contains `.pmcp/deploy.toml`.
///
/// Unlike [`find_workspace_root`], this anchors on the **deploy config marker**
/// rather than any `Cargo.toml`. This stops the walk at the directory that
/// actually owns the deployment, even when a `Cargo.toml` exists further up
/// the tree (e.g. a `multi-crate-isolated` server nested inside a monorepo
/// whose root carries a workspace `Cargo.toml`).
///
/// Returns `Ok(Some(dir))` for the nearest ancestor (including the current
/// directory) that holds `.pmcp/deploy.toml`. Returns `Ok(None)` — **not an
/// error** — when no such directory is found before reaching the filesystem
/// root, so callers can distinguish "no deploy config yet" (a valid fresh-init
/// state) from a hard failure.
pub fn find_deploy_root() -> Result<Option<PathBuf>> {
    let current_dir = std::env::current_dir().context("Failed to get current directory")?;
    let mut dir = current_dir.as_path();
    loop {
        if dir.join(".pmcp/deploy.toml").exists() {
            return Ok(Some(dir.to_path_buf()));
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => return Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_workspace_root_locates_cargo_toml() {
        // Run from cargo-pmcp/ subdir — should find cargo-pmcp/Cargo.toml or repo-root Cargo.toml
        let root = find_workspace_root().expect("must find Cargo.toml");
        assert!(
            root.join("Cargo.toml").exists(),
            "returned dir must contain Cargo.toml"
        );
    }

    #[test]
    fn find_workspace_root_in_tempdir_with_cargo_toml() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname=\"x\"\nversion=\"0.0.0\"\n",
        )
        .unwrap();
        let nested = tmp.path().join("a").join("b");
        std::fs::create_dir_all(&nested).unwrap();
        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(&nested).unwrap();
        let result = find_workspace_root();
        std::env::set_current_dir(&saved).unwrap();
        let result = result.expect("walk must succeed");
        assert_eq!(
            result.canonicalize().unwrap(),
            tmp.path().canonicalize().unwrap()
        );
    }

    /// (a) `find_deploy_root` is cwd-INCLUSIVE: when the current directory itself
    /// holds `.pmcp/deploy.toml`, it returns that directory.
    #[test]
    fn find_deploy_root_in_cwd() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".pmcp")).unwrap();
        std::fs::write(tmp.path().join(".pmcp/deploy.toml"), "[deployment]\n").unwrap();

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let result = find_deploy_root();
        std::env::set_current_dir(&saved).unwrap();

        let root = result
            .expect("walk must succeed")
            .expect("must find deploy.toml");
        assert_eq!(
            root.canonicalize().unwrap(),
            tmp.path().canonicalize().unwrap()
        );
    }

    /// (b) REGRESSION: the walk must STOP at the `.pmcp/deploy.toml` directory and
    /// must NOT climb to an ancestor `Cargo.toml`. Layout: `tmp/Cargo.toml`
    /// (workspace) + `tmp/server/.pmcp/deploy.toml`, cwd nested at
    /// `tmp/server/gcp-cloud-run` (no `Cargo.toml` at `tmp/server`). The resolver
    /// must return `tmp/server`, explicitly NOT `tmp`.
    ///
    /// NOTE: this test mutates process-global cwd; CI runs `--test-threads=1` so
    /// serialization is guaranteed. Always restore cwd before asserting.
    #[test]
    fn find_deploy_root_stops_at_deploy_toml_not_cargo() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "[workspace]\n").unwrap();
        let server = tmp.path().join("server");
        std::fs::create_dir_all(server.join(".pmcp")).unwrap();
        std::fs::write(server.join(".pmcp/deploy.toml"), "[deployment]\n").unwrap();
        let nested = server.join("gcp-cloud-run");
        std::fs::create_dir_all(&nested).unwrap();

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(&nested).unwrap();
        let result = find_deploy_root();
        std::env::set_current_dir(&saved).unwrap();

        let root = result
            .expect("walk must succeed")
            .expect("must find deploy.toml");
        assert_eq!(
            root.canonicalize().unwrap(),
            server.canonicalize().unwrap(),
            "must stop at the deploy.toml dir"
        );
        assert_ne!(
            root.canonicalize().unwrap(),
            tmp.path().canonicalize().unwrap(),
            "must NOT climb to the Cargo.toml ancestor"
        );
    }

    /// (c) returns `Ok(None)` (not an error) when no ancestor — including cwd —
    /// holds `.pmcp/deploy.toml`, even if a `Cargo.toml` is present.
    #[test]
    fn find_deploy_root_returns_none_when_absent() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "[workspace]\n").unwrap();
        let nested = tmp.path().join("a").join("b");
        std::fs::create_dir_all(&nested).unwrap();

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(&nested).unwrap();
        let result = find_deploy_root();
        std::env::set_current_dir(&saved).unwrap();

        assert!(
            result.expect("walk must succeed").is_none(),
            "no deploy.toml anywhere must yield None"
        );
    }
}
