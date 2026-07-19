//! Phase 110 Plan 05 — `cargo pmcp package capture <path>` unconfigured-error
//! test (CLI-04).
//!
//! The configured-success path (Bearer header, `CAPTURE_PATH`, package bytes,
//! 2xx→Ok / non-2xx→Err) is proven by the `mockito` unit test in the lib-safe
//! `capture_upload` seam (`cargo test -p cargo-pmcp --lib capture_upload`). This
//! integration test covers the OTHER contract: with no configured platform
//! target/credentials, `capture` must fail with actionable guidance naming
//! `configure`/`auth` — never a panic or a silent stub.
//!
//! Isolation (Codex MEDIUM): `HOME` is overridden on the CHILD PROCESS only
//! (`.env("HOME", <tempdir>)`), so the empty config/cache is scoped to the
//! subprocess — no in-process env mutation.

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

/// With an empty `HOME` (no `~/.pmcp/config.toml`, no token cache), `capture`
/// exits non-zero and names `configure` and/or `auth` in its error.
#[test]
fn capture_unconfigured_errors_naming_configure_or_auth() {
    let home = tempfile::tempdir().unwrap();
    let pkg = tempfile::tempdir().unwrap();

    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["package", "capture", pkg.path().to_str().unwrap()])
        // Child-process-only HOME override — isolates config/cache lookup.
        .env("HOME", home.path())
        // Ensure no ambient target selection leaks in from the runner env.
        .env_remove("PMCP_TARGET")
        .env_remove("PMCP_API_URL")
        .assert()
        .failure()
        .stderr(contains("configure").or(contains("auth")));
}
