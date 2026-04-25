//! Macro expansion snapshot baselines for Wave 1b refactor safety net
//! (Phase 75 Wave 0 Task 1).
//!
//! Each test invokes `cargo expand` on a self-contained sub-project under
//! `tests/fixtures/<name>/` and pins the resulting expanded source via
//! [`insta`]. Wave 1b is required to keep these snapshots byte-identical (or
//! `cargo insta accept` an intentional change with a documented justification
//! per RESEARCH.md Pitfall 4).
//!
//! Why per-fixture sub-projects rather than a `__test_internal` re-export of
//! the four `expand_*` functions: `pmcp-macros/Cargo.toml` declares
//! `[lib] proc-macro = true`, and the Rust compiler PROHIBITS exporting
//! non-proc-macro `pub` items from a proc-macro crate. The `cargo expand` path
//! is therefore the only viable snapshot mechanism for this crate; both
//! cross-AI reviewers (Gemini HIGH, Codex MEDIUM) flagged the original
//! re-export plan as physically impossible.
//!
//! Requires `cargo-expand` to be installed locally and in CI. The Phase 75
//! Wave 0 Task 1 acceptance check budgets the install:
//!
//! ```bash
//! cargo install cargo-expand --locked
//! ```
//!
//! If `cargo expand` is not on `$PATH`, every test in this file fails fast
//! with a clear "is `cargo-expand` installed?" message rather than asserting
//! against a partial snapshot.

use insta::assert_snapshot;
use std::path::PathBuf;
use std::process::Command;

/// Run `cargo expand` against a fixture sub-project and return the expanded
/// source as a `String`. The fixture's `Cargo.toml` lives at
/// `pmcp-macros/tests/fixtures/<basename>/Cargo.toml`.
///
/// Errors are surfaced via `panic!` so insta sees a clear failure message
/// rather than asserting against a partial / empty expansion.
fn expand_fixture(fixture_basename: &str) -> String {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_manifest = manifest_dir
        .join("tests")
        .join("fixtures")
        .join(fixture_basename)
        .join("Cargo.toml");

    assert!(
        fixture_manifest.exists(),
        "fixture manifest not found: {}",
        fixture_manifest.display()
    );

    let output = Command::new("cargo")
        .args([
            "expand",
            "--manifest-path",
            fixture_manifest.to_str().expect("fixture path is UTF-8"),
        ])
        .output()
        .expect(
            "`cargo expand` failed to spawn — is `cargo-expand` installed? \
             Run `cargo install cargo-expand --locked` and retry.",
        );

    assert!(
        output.status.success(),
        "`cargo expand` failed for fixture `{}`:\n--- stderr ---\n{}\n--- stdout ---\n{}",
        fixture_basename,
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );

    String::from_utf8(output.stdout).expect("`cargo expand` output is not valid UTF-8")
}

#[test]
fn snapshot_expand_mcp_tool() {
    let expanded = expand_fixture("example_mcp_tool");
    assert_snapshot!(expanded);
}

#[test]
fn snapshot_expand_mcp_server() {
    let expanded = expand_fixture("example_mcp_server");
    assert_snapshot!(expanded);
}

#[test]
fn snapshot_expand_mcp_resource() {
    let expanded = expand_fixture("example_mcp_resource");
    assert_snapshot!(expanded);
}

#[test]
fn snapshot_expand_mcp_prompt() {
    let expanded = expand_fixture("example_mcp_prompt");
    assert_snapshot!(expanded);
}
