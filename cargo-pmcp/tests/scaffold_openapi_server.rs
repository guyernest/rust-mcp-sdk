//! Plan 90-08 — `cargo pmcp new --kind openapi-server` scaffold compile test
//! (OAPI-07 / CF-3/CF-5), mirroring `scaffold_sql_server.rs` (TEST-05).
//!
//! Two tiers, matching the Phase 86 TEST-05 posture (STATE.md: "full cold
//! tempdir execution deferred to the orchestrator; the test asserts the scaffold
//! compiles"):
//!
//!   1. ALWAYS-ON (fast, no build):
//!      - `scaffold_emits_runnable_crate_files` — scaffold via the REAL built
//!        binary into a tempdir and assert the runnable-crate files exist with the
//!        CF-3/CF-4/CF-6 markers (single-call + script tool, code_mode enabled +
//!        dev inline-secret + replace-for-production note, deploy `pmcp-run`).
//!      - `emitted_main_is_le_15_statement_lines_golden_drift` — re-derive the
//!        emitted `main.rs` from the scaffold and assert the ≤15-statement-line
//!        Shape C budget (CF-5), the golden-drift guard (mirror Plan 86-03).
//!
//!   2. ENV-GATED COLD COMPILE (`PMCP_SCAFFOLD_COMPILE_TEST=1`):
//!      - `scaffold_compiles_via_cargo_check` — append the `[patch.crates-io]`
//!        block (the shared `scaffold_patch::append_crates_io_patch`, now covering
//!        `pmcp-openapi-server`) so the unpublished workspace crates resolve
//!        against their in-repo paths, then assert the scaffold COMPILES via
//!        `cargo check` (the OAPI-07 "single runnable crate" promise). This builds
//!        the unpublished toolkit cold in a fresh tempdir (15+ min), so it is
//!        gated for the orchestrator just like the SQL TEST-05 cold path.
//!
//! # Running
//!
//! ```sh
//! cargo test -p cargo-pmcp --test scaffold_openapi_server -- --test-threads=1
//! # full cold compile (orchestrator):
//! PMCP_SCAFFOLD_COMPILE_TEST=1 cargo test -p cargo-pmcp --test scaffold_openapi_server -- --test-threads=1
//! ```

use std::process::Command;

#[path = "support/scaffold_patch.rs"]
mod scaffold_patch;

use scaffold_patch::append_crates_io_patch;

/// The scaffolded crate name (a valid `validate_crate_name` identifier).
const SCAFFOLD_NAME: &str = "scaffold_openapi_demo";

/// Scaffold via the REAL built `cargo-pmcp` binary (NOT an in-process
/// `new::execute` call — the actual command surface a user runs). Returns the
/// scaffolded crate dir inside the (leaked-alive) tempdir.
fn scaffold(tmp: &std::path::Path) -> std::path::PathBuf {
    let status = Command::new(env!("CARGO_BIN_EXE_cargo-pmcp"))
        .args(["new", "--kind", "openapi-server", SCAFFOLD_NAME])
        .current_dir(tmp)
        .status()
        .expect("spawn the real cargo-pmcp binary to scaffold");
    assert!(
        status.success(),
        "`cargo pmcp new --kind openapi-server {SCAFFOLD_NAME}` must succeed (exit {status:?})"
    );
    let crate_dir = tmp.join(SCAFFOLD_NAME);
    assert!(
        crate_dir.join("Cargo.toml").is_file(),
        "scaffold must emit Cargo.toml at {}",
        crate_dir.display()
    );
    crate_dir
}

#[test]
fn scaffold_emits_runnable_crate_files() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let crate_dir = scaffold(tmp.path());

    // CF-3: a single runnable crate — all five inputs present.
    for f in [
        "Cargo.toml",
        "src/main.rs",
        "config.toml",
        "api.yaml",
        "deploy.toml",
    ] {
        assert!(
            crate_dir.join(f).is_file(),
            "scaffold must emit {f} at {}",
            crate_dir.display()
        );
    }
    assert!(
        crate_dir.join(".pmcp/deploy.toml").is_file(),
        "scaffold must emit .pmcp/deploy.toml (the copy cargo pmcp deploy reads)"
    );

    // Cargo.toml: openapi-code-mode umbrella + the pmcp-openapi-server lib seam.
    let cargo = std::fs::read_to_string(crate_dir.join("Cargo.toml")).unwrap();
    assert!(
        cargo.contains("openapi-code-mode"),
        "toolkit dep must use the openapi-code-mode umbrella"
    );
    assert!(
        cargo.contains("pmcp-openapi-server"),
        "must depend on the pmcp-openapi-server dispatch/build_server lib"
    );

    // config.toml (CF-4): code_mode enabled + DEV inline secret + dev flag + note,
    // plus a single-call AND a script tool.
    let config = std::fs::read_to_string(crate_dir.join("config.toml")).unwrap();
    assert!(
        config.contains("enabled = true"),
        "code_mode must be enabled"
    );
    assert!(
        config.contains("allow_inline_token_secret_for_dev = true"),
        "config must set the dev inline-secret flag (CF-4)"
    );
    assert!(
        config.to_lowercase().contains("replace") && config.to_lowercase().contains("production"),
        "config must carry a replace-for-production note (CF-4)"
    );
    assert!(
        config.contains("[backend]"),
        "config must declare a [backend]"
    );
    assert!(
        config.contains("path = \"/widgets\""),
        "config must declare a single-call tool"
    );
    assert!(
        config.contains("script = "),
        "config must declare a script tool"
    );

    // deploy.toml (CF-6): pmcp-run target, both copies.
    for p in ["deploy.toml", ".pmcp/deploy.toml"] {
        let deploy = std::fs::read_to_string(crate_dir.join(p)).unwrap();
        assert!(
            deploy.contains(r#"type = "pmcp-run""#),
            "{p} must declare target type = \"pmcp-run\" (CF-6)"
        );
    }
}

/// CF-5 golden-drift: the emitted `main.rs` must be the ≤15-statement-line Shape C
/// wiring. Mirrors the Plan 86-03 golden-drift test. Counts STATEMENT lines (skips
/// blanks, comments, and rustfmt method-chain / closing-delimiter continuations).
#[test]
fn emitted_main_is_le_15_statement_lines_golden_drift() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let crate_dir = scaffold(tmp.path());
    let src = std::fs::read_to_string(crate_dir.join("src/main.rs")).unwrap();

    let main_start = src
        .find("async fn main(")
        .expect("emitted main.rs must define `async fn main`");
    let body = &src[main_start..];
    let n = body
        .lines()
        .skip(1) // signature line
        .take_while(|l| l.trim() != "}")
        .map(|l| match l.find("//") {
            Some(idx) => l[..idx].trim(),
            None => l.trim(),
        })
        .filter(|t| {
            if t.is_empty() || t.starts_with("//") || t.starts_with('.') {
                return false;
            }
            !matches!(*t, ")" | ");" | ")?;" | "}" | "};" | "]" | "],")
        })
        .count();

    assert!(
        n <= 15,
        "emitted src/main.rs `main` body is {n} statement lines; CF-5 budget is ≤15 (golden drift)"
    );

    // Defensive CF-5 wiring tokens: the ≤15-line shape really wires the pipeline.
    for tok in [
        "ServerConfig::from_toml_strict_validated",
        "dispatch(&cfg)",
        "build_server(",
        "PMCP_OPENAPI_SERVER_ADDR",
    ] {
        assert!(
            src.contains(tok),
            "emitted main.rs missing CF-5 wiring token: {tok}"
        );
    }
}

/// OAPI-07 cold-compile: the scaffold COMPILES (single runnable crate). Gated for
/// the orchestrator (`PMCP_SCAFFOLD_COMPILE_TEST=1`) because it builds the
/// unpublished toolkit cold in a fresh tempdir (15+ min) — exactly the Phase 86
/// TEST-05 posture (STATE.md: full cold execution deferred to the orchestrator).
#[test]
fn scaffold_compiles_via_cargo_check() {
    if std::env::var("PMCP_SCAFFOLD_COMPILE_TEST").is_err() {
        eprintln!(
            "skipping scaffold cold-compile (set PMCP_SCAFFOLD_COMPILE_TEST=1 to run; \
             builds the unpublished toolkit cold, 15+ min)"
        );
        return;
    }

    let tmp = tempfile::tempdir().expect("create tempdir");
    let crate_dir = scaffold(tmp.path());

    // Resolve the unpublished workspace crates (incl. pmcp-openapi-server) against
    // their in-repo paths so `cargo check` does not hit crates.io (Pitfall §1).
    append_crates_io_patch(&crate_dir);

    let status = Command::new(env!("CARGO"))
        .args(["check", "--quiet"])
        .current_dir(&crate_dir)
        .status()
        .expect("spawn `cargo check` in the scaffolded crate dir");
    assert!(
        status.success(),
        "the scaffolded openapi-server crate must compile via `cargo check` (exit {status:?})"
    );
}
