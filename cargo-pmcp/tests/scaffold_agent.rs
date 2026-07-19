//! Phase 110-02 CLI-01 — end-to-end agent-scaffold integration test.
//!
//! This test exercises the REAL `cargo pmcp agent new <name>` command surface
//! (Codex 110-02 HIGH: the scaffold is a *compilable* deliverable, not just an
//! emitted file tree — do NOT weaken to an in-process `new::execute` call):
//!
//!   1. `tempfile::tempdir()` — an isolated, auto-cleaned scratch dir.
//!   2. Scaffold via the REAL built binary
//!      `cargo pmcp agent new <name>` invoked through
//!      `env!("CARGO_BIN_EXE_cargo-pmcp")` (Cargo sets this for the crate's own
//!      bin in integration tests). This is the ACTUAL command a user runs.
//!   3. Assert the four emitted files exist and that `agent.package.json`
//!      round-trips through `pmcp_package::AgentPackage` (the manifest is the
//!      source of truth — Codex 110-02 MEDIUM).
//!   4. Append a `[patch.crates-io]` block (via the shared
//!      `scaffold_patch::append_crates_io_patch` helper) so the as-yet-unpublished
//!      `pmcp-agent`/`pmcp-package 0.1.0` (and their transitive unpublished
//!      workspace deps) resolve against their in-repo paths.
//!   5. Spawn a REAL `cargo check` subprocess in the scaffold dir — the emitted
//!      manifest-driven runner MUST compile (Codex 110-02 HIGH).
//!   6. Spawn `cargo test --test pin` in the scaffold dir — the in-scaffold pin
//!      tripwire MUST pass against the patched deps (D-05).
//!
//! A second test covers the destination-overwrite policy (Codex 110-02 MEDIUM):
//! a second `agent new` into the now-non-empty dir FAILS unless `--force`.
//!
//! Every spawned child is wrapped in a `ChildGuard` (Drop-kill) so a panic
//! anywhere after spawn cannot leak a subprocess.
//!
//! # Running
//!
//! This test MUST run single-threaded — the `cargo check`/`cargo test`
//! subprocesses compile the unpublished `pmcp-agent` tree (slow cold build in a
//! fresh tempdir target/). It belongs to the `--test-threads=1` group so the
//! heavy tempdir build does not contend:
//!
//! ```sh
//! cargo test -p cargo-pmcp --test scaffold_agent -- --test-threads=1
//! ```

use std::process::{Command, Stdio};

// The shared [patch.crates-io] writer + ChildGuard + repo_root (written once in
// support/scaffold_patch.rs and reused by the sql/openapi scaffold tests too).
#[path = "support/scaffold_patch.rs"]
mod scaffold_patch;

use scaffold_patch::{append_crates_io_patch, ChildGuard};

/// The scaffolded crate name (a valid `validate_crate_name` identifier).
const SCAFFOLD_NAME: &str = "scaffold_agent_demo";

#[test]
fn agent_scaffold_compiles_and_pin_test_passes() {
    // (1) Isolated, auto-cleaned scratch dir for the scaffold + its build.
    let tmp = tempfile::tempdir().expect("create tempdir");

    // (2) Scaffold via the REAL built binary (the actual command surface — not
    //     `agent::new::execute` in-process).
    let scaffold_status = Command::new(env!("CARGO_BIN_EXE_cargo-pmcp"))
        .args(["agent", "new", SCAFFOLD_NAME])
        .current_dir(tmp.path())
        .status()
        .expect("spawn the real cargo-pmcp binary to scaffold");
    assert!(
        scaffold_status.success(),
        "`cargo pmcp agent new {SCAFFOLD_NAME}` must succeed (exit {scaffold_status:?})"
    );

    let crate_dir = tmp.path().join(SCAFFOLD_NAME);

    // (3) The four emitted files must exist.
    for rel in [
        "Cargo.toml",
        "src/main.rs",
        "agent.package.json",
        "tests/pin.rs",
    ] {
        assert!(
            crate_dir.join(rel).is_file(),
            "scaffold must emit {rel} at {}",
            crate_dir.display()
        );
    }

    // (3b) The emitted manifest MUST round-trip through `AgentPackage` (the
    //      runner LOADS this file, so it cannot diverge from the schema).
    let manifest = std::fs::read_to_string(crate_dir.join("agent.package.json"))
        .expect("read emitted agent.package.json");
    let pkg: pmcp_package::AgentPackage = serde_json::from_str(&manifest)
        .expect("emitted agent.package.json must deserialize as pmcp_package::AgentPackage");
    assert_eq!(
        pkg.name, SCAFFOLD_NAME,
        "emitted manifest name must match the scaffold name"
    );

    // (4) Make the unpublished `pmcp-agent`/`pmcp-package 0.1.0` (+ their
    //     transitive unpublished workspace crates) resolve via a
    //     [patch.crates-io] override pointing at the in-repo paths.
    append_crates_io_patch(&crate_dir);

    // (5) Spawn a REAL `cargo check` — the manifest-driven runner MUST compile.
    let mut check = Command::new(env!("CARGO"))
        .args(["check", "--quiet"])
        .current_dir(&crate_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn `cargo check` in the scaffolded crate dir");
    let check_status = check.wait().expect("wait for `cargo check`");
    // Reap-safety documentation: `check` has already exited here, but wrap+drop
    // to mirror the pin child below.
    drop(ChildGuard(check));
    assert!(
        check_status.success(),
        "the emitted agent crate must compile (`cargo check` exit {check_status:?})"
    );

    // (6) Spawn `cargo test --test pin` — the in-scaffold pin tripwire MUST pass.
    let mut pin = Command::new(env!("CARGO"))
        .args(["test", "--test", "pin", "--quiet"])
        .current_dir(&crate_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn `cargo test --test pin` in the scaffolded crate dir");
    let pin_status = pin.wait().expect("wait for `cargo test --test pin`");
    drop(ChildGuard(pin));
    assert!(
        pin_status.success(),
        "the emitted tests/pin.rs tripwire must pass (`cargo test --test pin` exit {pin_status:?})"
    );
}

#[test]
fn agent_new_refuses_nonempty_destination_without_force() {
    let tmp = tempfile::tempdir().expect("create tempdir");

    // First scaffold succeeds into a fresh dir.
    let first = Command::new(env!("CARGO_BIN_EXE_cargo-pmcp"))
        .args(["agent", "new", SCAFFOLD_NAME])
        .current_dir(tmp.path())
        .status()
        .expect("spawn cargo-pmcp for the first scaffold");
    assert!(first.success(), "first `agent new` must succeed");

    // Second scaffold into the now-non-empty dir MUST fail without --force.
    let second = Command::new(env!("CARGO_BIN_EXE_cargo-pmcp"))
        .args(["agent", "new", SCAFFOLD_NAME])
        .current_dir(tmp.path())
        .output()
        .expect("spawn cargo-pmcp for the second scaffold");
    assert!(
        !second.status.success(),
        "second `agent new` into a non-empty dir must fail without --force"
    );
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(
        stderr.contains("--force"),
        "the refusal message must name `--force`; got: {stderr}"
    );

    // With --force it succeeds.
    let forced = Command::new(env!("CARGO_BIN_EXE_cargo-pmcp"))
        .args(["agent", "new", SCAFFOLD_NAME, "--force"])
        .current_dir(tmp.path())
        .status()
        .expect("spawn cargo-pmcp for the forced scaffold");
    assert!(
        forced.success(),
        "`agent new --force` into a non-empty dir must succeed"
    );
}
