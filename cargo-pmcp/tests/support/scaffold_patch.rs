//! Shared test-support module for the scaffold-to-run integration tests
//! (Plan 86-04 TEST-05 and Plan 86-06 TEST-06).
//!
//! It exposes the two pieces both tests need to drive a REAL `cargo run` against
//! a freshly-scaffolded crate that depends on the as-yet-unpublished
//! `pmcp-server-toolkit 0.1.0`:
//!
//! 1. [`append_crates_io_patch`] — appends a `[patch.crates-io]` block to the
//!    tempdir crate's `Cargo.toml` so the unpublished workspace crates resolve
//!    against their in-repo paths (RESEARCH Pitfall §1, Assumption A1). The
//!    block MUST cover not only the directly-named deps (`pmcp`,
//!    `pmcp-server-toolkit`) but ALSO their transitive unpublished crates
//!    (`pmcp-code-mode`, `pmcp-widget-utils`) — otherwise `cargo` would try to
//!    fetch `pmcp-server-toolkit 0.1.0` from crates.io (where it does not yet
//!    exist) and the build would fail (Gemini transitive-dep note).
//! 2. [`ChildGuard`] — a `Drop`-kill wrapper around a spawned `std::process::Child`
//!    so a panic anywhere in a test body can NOT leak the spawned `cargo run`
//!    server subprocess (M1).
//!
//! [`repo_root`] returns the in-repo workspace root the patch paths derive from.
//!
//! Both `scaffold_sql_server.rs` (this plan) and `deploy_config_only.rs`
//! (Plan 06) pull this in via `#[path = "support/scaffold_patch.rs"] mod
//! scaffold_patch;`, so the patch logic + ChildGuard are written ONCE (M1).
//!
//! Not every helper is exercised by every consumer (each test uses the subset it
//! needs), so the module tolerates dead-code warnings across the two test
//! targets.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// The in-repo workspace root (the directory holding the root `pmcp` crate's
/// `Cargo.toml`). The `[patch.crates-io]` path overrides are derived from this,
/// NOT from any external input (threat T-86-04-01: the paths are trusted because
/// they come from `CARGO_MANIFEST_DIR`, this repo's own build env).
///
/// `CARGO_MANIFEST_DIR` for `cargo-pmcp` is `<repo>/cargo-pmcp`; the workspace
/// root is its parent.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cargo-pmcp manifest dir must have a parent (the workspace root)")
        .to_path_buf()
}

/// Append a `[patch.crates-io]` block to the scaffolded crate's `Cargo.toml`,
/// pointing every unpublished workspace crate the scaffold's dependency graph
/// touches at its in-repo path so `cargo run` in the tempdir resolves without
/// hitting crates.io.
///
/// Coverage (the transitive closure of unpublished crates — Gemini note):
/// - `pmcp`               → `<repo>`                              (root crate)
/// - `pmcp-server-toolkit`→ `<repo>/crates/pmcp-server-toolkit`   (0.1.0, NOT yet on crates.io)
/// - `pmcp-code-mode`     → `<repo>/crates/pmcp-code-mode`        (toolkit's optional dep)
/// - `pmcp-widget-utils`  → `<repo>/crates/pmcp-widget-utils`     (pmcp's dep)
/// - `pmcp-openapi-server`→ `<repo>/crates/pmcp-openapi-server`   (0.1.0, openapi-server scaffold's dispatch/build_server seam)
///
/// The `pmcp-openapi-server` entry is unused by the `sql-server` scaffold's
/// dependency graph (cargo emits a harmless unused-patch warning there) but is
/// REQUIRED by the `openapi-server` scaffold, which depends on it for the
/// `dispatch` + `build_server` orchestrators.
///
/// `crate_dir` is the scaffolded crate root (the dir containing its `Cargo.toml`).
pub fn append_crates_io_patch(crate_dir: &Path) {
    let manifest = crate_dir.join("Cargo.toml");
    let mut content = std::fs::read_to_string(&manifest)
        .unwrap_or_else(|e| panic!("read scaffolded Cargo.toml at {}: {e}", manifest.display()));

    let root = repo_root();
    let path_str = |p: &Path| -> String {
        // TOML string: forward slashes are portable and avoid Windows backslash
        // escaping; the integration test runs on Unix CI but keep it robust.
        p.to_string_lossy().replace('\\', "/")
    };

    let patch = format!(
        "\n\
         # Injected by cargo-pmcp tests/support/scaffold_patch.rs so the\n\
         # unpublished workspace crates resolve against their in-repo paths\n\
         # (Pitfall §1). Covers the full transitive closure of unpublished deps.\n\
         [patch.crates-io]\n\
         pmcp = {{ path = \"{pmcp}\" }}\n\
         pmcp-server-toolkit = {{ path = \"{toolkit}\" }}\n\
         pmcp-code-mode = {{ path = \"{code_mode}\" }}\n\
         pmcp-widget-utils = {{ path = \"{widget}\" }}\n\
         pmcp-openapi-server = {{ path = \"{openapi}\" }}\n",
        pmcp = path_str(&root),
        toolkit = path_str(&root.join("crates/pmcp-server-toolkit")),
        code_mode = path_str(&root.join("crates/pmcp-code-mode")),
        widget = path_str(&root.join("crates/pmcp-widget-utils")),
        openapi = path_str(&root.join("crates/pmcp-openapi-server")),
    );

    content.push_str(&patch);
    std::fs::write(&manifest, content)
        .unwrap_or_else(|e| panic!("write patched Cargo.toml at {}: {e}", manifest.display()));
}

/// Drop-kill guard around a spawned subprocess (M1). On scope exit (including an
/// unwinding panic) `Drop` `kill`s and `wait`s the child, so a failing assertion
/// in a test body can NOT leak the spawned `cargo run` server process.
///
/// The held `Child` is `pub` so consumers can `take()` its stdout/stderr pipes
/// before wrapping (they must do so BEFORE constructing the guard, since `Drop`
/// borrows the child).
pub struct ChildGuard(pub std::process::Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
