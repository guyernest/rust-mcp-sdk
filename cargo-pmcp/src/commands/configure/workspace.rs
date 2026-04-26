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
        dir = dir.parent().ok_or_else(|| {
            anyhow::anyhow!("Could not find Cargo.toml in any parent directory")
        })?;
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
}
