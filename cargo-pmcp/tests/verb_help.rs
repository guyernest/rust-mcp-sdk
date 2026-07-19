//! Phase 110 Plan 01 — automated `--help` surface assertions for the three new
//! command groups (`agent`, `team`, `package`).
//!
//! The plan's must-have "each group's `--help` lists its subcommands" is asserted
//! here (Codex 110-01 LOW — the truth must be automated, not manual). Every group
//! subcommand is currently a stub that `bail!`s, but `--help` is served by clap
//! before any handler runs, so these assertions exercise the wired CLI surface
//! independent of the stubbed bodies.

use assert_cmd::Command;
use predicates::str::contains;

/// `cargo pmcp agent --help` exits 0 and lists `new` and `dev`.
#[test]
fn agent_help_lists_subcommands() {
    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["agent", "--help"])
        .assert()
        .success()
        .stdout(contains("new"))
        .stdout(contains("dev"));
}

/// `cargo pmcp team --help` exits 0 and lists `dev`.
#[test]
fn team_help_lists_subcommands() {
    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["team", "--help"])
        .assert()
        .success()
        .stdout(contains("dev"));
}

/// `cargo pmcp package --help` exits 0 and lists `show` and `capture`.
#[test]
fn package_help_lists_subcommands() {
    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["package", "--help"])
        .assert()
        .success()
        .stdout(contains("show"))
        .stdout(contains("capture"));
}
