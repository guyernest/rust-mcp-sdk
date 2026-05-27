//! Plan 86-06 TEST-06 — env-gated, authentic config-only pmcp.run deploy test.
//!
//! This is the SC-4 deliverable (D-11): proof that a `cargo pmcp new --kind
//! sql-server` config-only server deploys to a REAL pmcp.run target via the
//! Plan 05 deploy path and that the Phase 79 post-deploy lifecycle
//! (`connectivity`/`check` + `conformance` + `apps`) runs cleanly.
//!
//! # Double-gated — never runs in normal CI (threat T-86-06-01)
//!
//! The deploy is activated ONLY when BOTH conditions hold:
//!
//!   1. The env var `PMCP_RUN_DEPLOY_TEST` is set (the `npm_skip_gate` idiom,
//!      copied from `widgets_orchestrator.rs:36-45`). When it is absent the test
//!      prints a reason and `return`s — it NEVER fails the suite, so a normal
//!      `cargo test` (no creds) stays green.
//!   2. The test carries `#[ignore]`, so a bare `cargo test` does not even
//!      construct the deploy path (defense in depth alongside the early-return
//!      skip). It must be run with `--ignored`.
//!
//! In addition to the gate, running the deploy for real REQUIRES live
//! pmcp.run / AWS credentials and `cargo lambda` on PATH (Assumption A5). A gate
//! set WITHOUT creds is operator error (not a CI path): the real deploy
//! subprocess fails fast and this test surfaces its nonzero exit + captured
//! output with an install/cred hint.
//!
//! # How to run the REAL deploy (operator, documented for the SUMMARY)
//!
//! ```sh
//! # Requires: pmcp.run login + AWS creds + `cargo lambda` installed.
//! PMCP_RUN_DEPLOY_TEST=1 \
//!   cargo test -p cargo-pmcp --test deploy_config_only -- --ignored --test-threads=1 --nocapture
//! ```
//!
//! On success the test reads the deployed endpoint from
//! `<crate>/.pmcp/deployment.toml`, optionally re-confirms the Phase 79
//! lifecycle via an explicit `run_post_deploy_tests`, then best-effort tears
//! down the disposable server (`cargo pmcp deploy destroy --yes`), tolerating a
//! logged cleanup failure (D-11: a deliberately-left server is acceptable).
//!
//! # M1 — drive deploy via the REAL binary SUBPROCESS, not in-process
//!
//! The deploy is driven by spawning the built `cargo-pmcp` binary
//! (`env!("CARGO_BIN_EXE_cargo-pmcp")`) and asserting its exit code. We do NOT
//! call the deploy fn in-process: the deploy path may `std::process::exit` on a
//! post-deploy failure, which would abort the whole test process. A clean exit
//! (0) means the command's own `run_post_deploy_tests` (check + conformance +
//! apps) ran clean inside the command. The spawned child is wrapped in the
//! shared `ChildGuard` (Plan 04) so a panic cannot leak the deploy subprocess.

use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

// The shared [patch.crates-io] writer + ChildGuard + repo_root, written ONCE in
// Plan 04 (scaffold_sql_server.rs) and REUSED here (M1 / Codex 86-06 HIGH).
#[path = "support/scaffold_patch.rs"]
mod scaffold_patch;

use scaffold_patch::{append_crates_io_patch, ChildGuard};

/// Env-gate (the `widgets_orchestrator.rs:36-45` `npm_skip_gate` idiom). Returns
/// `Some(reason)` when the gate is ABSENT — the test prints it and `return`s
/// WITHOUT failing (never `panic!`/`assert!`). Returns `None` when the gate is
/// present and the real deploy path should run.
fn deploy_gate() -> Option<&'static str> {
    if std::env::var("PMCP_RUN_DEPLOY_TEST").is_ok() {
        None
    } else {
        Some("PMCP_RUN_DEPLOY_TEST not set — skipping real pmcp.run config-only deploy integration test")
    }
}

/// A unique, disposable server name per run so repeated gated runs do not
/// collide / accumulate live resources (Runtime State Inventory teardown note,
/// threat T-86-06-03). A valid `validate_crate_name` identifier: lowercase
/// ASCII + underscores only.
fn disposable_server_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("pmcp_deploy_test_{nanos}")
}

/// Read the deployed endpoint URL from the `.pmcp/deployment.toml` the deploy
/// writes (`save_deployment_info`, deploy/mod.rs:1072 — a `[deployment]` table
/// with `server_id` + `endpoint`). Returns `None` if the file is absent or the
/// `endpoint` key cannot be parsed.
fn read_deployed_endpoint(crate_dir: &Path) -> Option<String> {
    let path = crate_dir.join(".pmcp").join("deployment.toml");
    let content = std::fs::read_to_string(&path).ok()?;
    let value: toml::Value = toml::from_str(&content).ok()?;
    value
        .get("deployment")
        .and_then(|d| d.get("endpoint"))
        .and_then(|e| e.as_str())
        .map(str::to_string)
}

/// TEST-06 — env-gated + `#[ignore]` real pmcp.run config-only deploy.
///
/// CI default (gate ABSENT, not `--ignored`): SKIPS cleanly — prints the reason
/// and returns, never deploying, never failing. The `#[ignore]` additionally
/// keeps it out of a default `cargo test` run (defense in depth).
#[tokio::test]
#[ignore = "real pmcp.run deploy — set PMCP_RUN_DEPLOY_TEST + creds and run with --ignored"]
async fn config_only_deploy_runs_phase79_lifecycle() {
    // Defense-in-depth SKIP: even reached only under `--ignored`, the env gate
    // must also be present. Print + return — NEVER fail the suite (D-11).
    if let Some(reason) = deploy_gate() {
        eprintln!("{reason}");
        return;
    }

    // From here on the gate is ON: this performs a REAL cloud deploy. It
    // additionally requires live pmcp.run/AWS creds + `cargo lambda` (A5). The
    // deploy subprocess fails fast with an install/cred hint if they are absent
    // (operator error, surfaced below as a nonzero exit + captured output).

    // (1) Isolated, auto-cleaned scratch dir for the scaffold + its build.
    let tmp = tempfile::tempdir().expect("create tempdir");
    let server_name = disposable_server_name();

    // (2) Scaffold the config-only project via the REAL built binary (M1 — the
    //     actual command surface, `cargo pmcp new --kind sql-server <name>`).
    let scaffold_status = Command::new(env!("CARGO_BIN_EXE_cargo-pmcp"))
        .args(["new", "--kind", "sql-server", &server_name])
        .current_dir(tmp.path())
        .status()
        .expect("spawn the real cargo-pmcp binary to scaffold");
    assert!(
        scaffold_status.success(),
        "`cargo pmcp new --kind sql-server {server_name}` must succeed (exit {scaffold_status:?})"
    );

    let crate_dir = tmp.path().join(&server_name);
    assert!(
        crate_dir.join("Cargo.toml").is_file(),
        "scaffold must emit Cargo.toml at {}",
        crate_dir.display()
    );
    assert!(
        crate_dir.join(".pmcp").join("deploy.toml").is_file(),
        "scaffold must emit .pmcp/deploy.toml (target_type=pmcp-run) at {}",
        crate_dir.display()
    );

    // (3) Make the unpublished `pmcp-server-toolkit 0.1.0` (+ its transitive
    //     unpublished workspace crates) resolve via a [patch.crates-io] override
    //     so `cargo lambda build` inside the deploy resolves against the in-repo
    //     paths (M1 / Codex 86-06 HIGH — the scaffolded tempdir needs the patch).
    append_crates_io_patch(&crate_dir);

    // (4) Drive the REAL deploy by spawning the built binary as a SUBPROCESS
    //     (M1 — do NOT call deploy in-process; it may `std::process::exit`). The
    //     emitted .pmcp/deploy.toml already selects `target_type=pmcp-run`
    //     (Plan 05/M3), so a bare `deploy` (None action arm) runs the deploy +
    //     the Phase 79 post-deploy lifecycle. A clean exit (0) ⇒ check +
    //     conformance + apps all passed inside the command.
    let deploy_child = Command::new(env!("CARGO_BIN_EXE_cargo-pmcp"))
        .args(["deploy"])
        .current_dir(&crate_dir)
        .spawn()
        .expect("spawn the real cargo-pmcp deploy subprocess");
    // Wrap IMMEDIATELY so a panic below cannot leak the deploy process (M1,
    // threat T-86-06-03). We still need to wait() on it for the exit code; the
    // guard's Drop is a no-op-after-reap safety net (kill+wait both tolerate an
    // already-exited child).
    let mut guard = ChildGuard(deploy_child);
    let deploy_status = guard
        .0
        .wait()
        .expect("wait on the cargo-pmcp deploy subprocess");
    assert!(
        deploy_status.success(),
        "`cargo pmcp deploy` (config-only → pmcp.run) must exit 0 — a clean exit means the \
         Phase 79 post-deploy lifecycle (connectivity + conformance + apps) ran clean. Got \
         {deploy_status:?}. If this is a cred/tooling failure, ensure pmcp.run login + AWS \
         credentials + `cargo lambda` are available (Assumption A5)."
    );

    // (5) Capture the deployed URL from `.pmcp/deployment.toml` (the
    //     `save_deployment_info` write) for the SUMMARY record.
    let endpoint = read_deployed_endpoint(&crate_dir).unwrap_or_else(|| {
        panic!(
            "deploy succeeded but `.pmcp/deployment.toml` had no `[deployment].endpoint` at {}",
            crate_dir.display()
        )
    });
    eprintln!("TEST-06 deployed config-only server '{server_name}' → {endpoint}");

    // (6) Explicit second confirmation of the Phase 79 lifecycle against the
    //     captured URL (the deploy already ran it; this re-asserts Ok against the
    //     live endpoint). widgets_present=false — a config-only SQL server ships
    //     no widgets.
    let lifecycle = cargo_pmcp::deployment::post_deploy_tests::run_post_deploy_tests(
        &endpoint,
        "pmcp-run",
        false, // widgets_present — config-only server has none
        &cargo_pmcp::deployment::post_deploy_tests::PostDeployTestsConfig::default(),
        false, // quiet — surface the banner on failure
    )
    .await;
    assert!(
        lifecycle.is_ok(),
        "explicit Phase 79 post-deploy lifecycle (check + conformance + apps) must pass against \
         the deployed endpoint {endpoint}: {lifecycle:?}"
    );

    // (7) TEARDOWN (best-effort): destroy the disposable server so repeated
    //     gated runs do not accumulate live resources (threat T-86-06-03).
    //     Tolerate + clearly LOG a cleanup failure — do NOT fail the test on a
    //     teardown error (D-11: a deliberately-left server is acceptable).
    match Command::new(env!("CARGO_BIN_EXE_cargo-pmcp"))
        .args(["deploy", "destroy", "--yes"])
        .current_dir(&crate_dir)
        .status()
    {
        Ok(s) if s.success() => {
            eprintln!("TEST-06 teardown: destroyed disposable server '{server_name}'");
        },
        Ok(s) => {
            eprintln!(
                "TEST-06 teardown WARNING: `deploy destroy --yes` exited {s:?} for '{server_name}'; \
                 the deployed server may still be live — destroy it manually (D-11)."
            );
        },
        Err(e) => {
            eprintln!(
                "TEST-06 teardown WARNING: could not spawn `deploy destroy --yes` for '{server_name}': \
                 {e}; the deployed server may still be live — destroy it manually (D-11)."
            );
        },
    }

    // ChildGuard Drop reaps the (already-waited) deploy child; tempdir
    // auto-cleans. Explicit drop documents the reap point.
    drop(guard);
}
