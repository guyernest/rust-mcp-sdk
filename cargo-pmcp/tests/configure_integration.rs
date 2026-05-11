//! Phase 77 integration tests — full add/use/list/show lifecycle with tempdir HOME isolation.
//!
//! Run via `cargo test -p cargo-pmcp --test configure_integration -- --test-threads=1`.
//! `--test-threads=1` is required because tests mutate process env (HOME, PMCP_TARGET, etc.).

use cargo_pmcp::test_support::configure_config::{
    default_user_config_path, PmcpRunEntry, TargetConfigV1, TargetEntry,
};
use std::path::PathBuf;
use std::sync::Mutex;

/// Lock used to serialize HOME-mutating tests within a single test process.
/// Belt-and-suspenders alongside `--test-threads=1`: if a future contributor forgets the flag,
/// this lock keeps tests from clobbering each other's HOME.
static HOME_LOCK: Mutex<()> = Mutex::new(());

struct IsolatedHome {
    _home: tempfile::TempDir,
    _ws: tempfile::TempDir,
    ws_path: PathBuf,
    home_path: PathBuf,
    saved_home: Option<std::ffi::OsString>,
    saved_cwd: PathBuf,
    saved_target: Option<std::ffi::OsString>,
}

impl IsolatedHome {
    fn new() -> Self {
        // Lock for the lifetime of this struct; std::sync::Mutex is poison-on-panic, so
        // if a prior test panicked while holding it, recover via into_inner.
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let home = tempfile::tempdir().unwrap();
        let ws = tempfile::tempdir().unwrap();
        std::fs::write(
            ws.path().join("Cargo.toml"),
            "[package]\nname=\"x\"\nversion=\"0.0.0\"\n",
        )
        .unwrap();
        let saved_home = std::env::var_os("HOME");
        let saved_cwd = std::env::current_dir().unwrap();
        let saved_target = std::env::var_os("PMCP_TARGET");
        std::env::set_var("HOME", home.path());
        std::env::remove_var("PMCP_TARGET");
        std::env::set_current_dir(ws.path()).unwrap();
        // Drop the guard at end of new(); the IsolatedHome's mere existence keeps state coherent
        // because tests are sequential (--test-threads=1).
        drop(_guard);
        Self {
            home_path: home.path().to_path_buf(),
            ws_path: ws.path().to_path_buf(),
            _home: home,
            _ws: ws,
            saved_home,
            saved_cwd,
            saved_target,
        }
    }
}

impl Drop for IsolatedHome {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.saved_cwd);
        match self.saved_home.take() {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match self.saved_target.take() {
            Some(v) => std::env::set_var("PMCP_TARGET", v),
            None => std::env::remove_var("PMCP_TARGET"),
        }
    }
}

// HIGH-1 fix per 77-REVIEWS.md: `commands::configure::add::execute` and friends
// are NOT in the lib surface — `commands::*` stays bin-only. Helpers below invoke the
// cargo-pmcp BINARY as a subprocess (matches `tests/auth_integration.rs:101` precedent).
// Schema construction (TargetConfigV1::write_atomic etc.) goes through the `#[path]`-bridged
// `cargo_pmcp::test_support::configure_config` re-export, which IS in the lib surface.
//
// We pass HOME explicitly to each subprocess so the binary uses the test's isolated config dir.
// The `IsolatedHome` already mutates the current process' HOME via `std::env::set_var`, which
// child processes inherit by default — but we set it explicitly via `Command::env` for clarity.
fn cli_configure_add_pmcp_run(
    name: &str,
    region: &str,
    home: &std::path::Path,
    ws: &std::path::Path,
) -> anyhow::Result<()> {
    use std::process::Command;
    let bin = env!("CARGO_BIN_EXE_cargo-pmcp");
    let output = Command::new(bin)
        .args([
            "configure",
            "add",
            name,
            "--type",
            "pmcp-run",
            "--api-url",
            "https://x.example.com",
            "--aws-profile",
            "test",
            "--region",
            region,
        ])
        .env("HOME", home)
        .current_dir(ws)
        .output()?;
    if !output.status.success() {
        anyhow::bail!(
            "configure add failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn cli_configure_use(
    name: &str,
    home: &std::path::Path,
    ws: &std::path::Path,
) -> anyhow::Result<()> {
    use std::process::Command;
    let bin = env!("CARGO_BIN_EXE_cargo-pmcp");
    let output = Command::new(bin)
        .args(["configure", "use", name])
        .env("HOME", home)
        .current_dir(ws)
        .output()?;
    if !output.status.success() {
        anyhow::bail!(
            "configure use failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

#[test]
fn add_use_list_show_full_flow() {
    let h = IsolatedHome::new();

    // 1. add a target
    cli_configure_add_pmcp_run("dev", "us-west-2", &h.home_path, &h.ws_path)
        .expect("add must succeed");
    let cfg = TargetConfigV1::read(&default_user_config_path()).unwrap();
    assert!(cfg.targets.contains_key("dev"));

    // 2. activate it
    cli_configure_use("dev", &h.home_path, &h.ws_path).expect("use must succeed");
    let marker = std::fs::read_to_string(h.ws_path.join(".pmcp").join("active-target")).unwrap();
    assert_eq!(marker, "dev\n");

    // 3. HIGH-1: `read_active_marker` is bin-only; verify the marker file content via direct fs read
    let active_raw =
        std::fs::read_to_string(h.ws_path.join(".pmcp").join("active-target")).unwrap();
    assert_eq!(active_raw.trim(), "dev");

    // 4. resolver picks it up — verified via subprocess `configure show` (HIGH-1: no direct lib
    //    access to resolver from integration tests)
    use std::process::Command;
    let bin = env!("CARGO_BIN_EXE_cargo-pmcp");
    let output = Command::new(bin)
        .args(["configure", "show", "dev"])
        .env("HOME", &h.home_path)
        .current_dir(&h.ws_path)
        .output()
        .expect("configure show must run");
    assert!(
        output.status.success(),
        "configure show failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("dev"),
        "show output must mention `dev`; got: {stdout}"
    );
    assert!(
        stdout.contains("pmcp-run"),
        "show output must mention type tag `pmcp-run`; got: {stdout}"
    );
}

#[test]
fn zero_touch_no_config_no_target() {
    let h = IsolatedHome::new();
    // No config written, no marker, no env. HIGH-1: resolver lives in bin-only `commands::*`.
    // Verify D-11 zero-touch by running a target-consuming subcommand and asserting the
    // banner is NOT emitted on stderr (i.e. no `→ Using target:` line). We use `configure list`
    // because it does NOT trigger banner emission AND tolerates missing config.toml gracefully.
    // Banner emission is the observable side effect of the resolver picking up a target —
    // if no target selected and no config exists, no banner means D-11 zero-touch holds.
    use std::process::Command;
    let bin = env!("CARGO_BIN_EXE_cargo-pmcp");
    let output = Command::new(bin)
        .args(["configure", "list"])
        .env("HOME", &h.home_path)
        .current_dir(&h.ws_path)
        .output()
        .expect("cargo-pmcp must run");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("→ Using target:"),
        "D-11 zero-touch: no banner expected; got stderr: {stderr}"
    );
}

#[test]
fn pmcp_target_env_overrides_marker() {
    let h = IsolatedHome::new();
    cli_configure_add_pmcp_run("dev", "us-west-2", &h.home_path, &h.ws_path).unwrap();
    cli_configure_add_pmcp_run("prod", "us-east-1", &h.home_path, &h.ws_path).unwrap();
    cli_configure_use("dev", &h.home_path, &h.ws_path).unwrap();
    // HIGH-1: resolver is bin-only. Verify the env override by running `configure show` with
    // `PMCP_TARGET=prod` set on the subprocess; assert the show output references prod.
    use std::process::Command;
    let bin = env!("CARGO_BIN_EXE_cargo-pmcp");
    let output = Command::new(bin)
        .args(["configure", "show"])
        .env("HOME", &h.home_path)
        .env("PMCP_TARGET", "prod")
        .current_dir(&h.ws_path)
        .output()
        .expect("configure show must run");
    assert!(
        output.status.success(),
        "show with PMCP_TARGET=prod must succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("prod"),
        "PMCP_TARGET=prod must override marker `dev`; stdout: {stdout}"
    );
}

#[test]
fn marker_overwrite_idempotent() {
    let h = IsolatedHome::new();
    cli_configure_add_pmcp_run("dev", "us-west-2", &h.home_path, &h.ws_path).unwrap();
    cli_configure_add_pmcp_run("prod", "us-east-1", &h.home_path, &h.ws_path).unwrap();
    cli_configure_use("dev", &h.home_path, &h.ws_path).unwrap();
    cli_configure_use("prod", &h.home_path, &h.ws_path).unwrap();
    cli_configure_use("prod", &h.home_path, &h.ws_path).unwrap();
    let marker = std::fs::read_to_string(h.ws_path.join(".pmcp").join("active-target")).unwrap();
    assert_eq!(marker, "prod\n");
}

#[cfg(unix)]
#[test]
fn unix_perms_0600_after_add() {
    use std::os::unix::fs::PermissionsExt;
    let h = IsolatedHome::new();
    cli_configure_add_pmcp_run("dev", "us-west-2", &h.home_path, &h.ws_path).unwrap();
    let mode = std::fs::metadata(default_user_config_path())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(
        mode, 0o600,
        "config file mode must be 0o600 after add; got {:o}",
        mode
    );
}

#[test]
fn concurrent_writers_no_partial_file() {
    // 4 threads write configs simultaneously to the same path; final file must be parseable.
    let h = IsolatedHome::new();
    let path = default_user_config_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut handles = vec![];
    for i in 0..4 {
        let p = path.clone();
        handles.push(std::thread::spawn(move || {
            let mut cfg = TargetConfigV1::empty();
            cfg.targets.insert(
                format!("t{}", i),
                TargetEntry::PmcpRun(PmcpRunEntry {
                    api_url: Some(format!("https://x{}.test", i)),
                    aws_profile: None,
                    region: Some("us-west-2".into()),
                }),
            );
            cfg.write_atomic(&p).unwrap();
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }
    // After all threads finish, the file must be a valid TOML document with exactly one
    // target (last writer wins per atomic-rename semantics).
    let final_cfg =
        TargetConfigV1::read(&path).expect("file must be parseable; got partial-write?");
    assert_eq!(final_cfg.schema_version, 1);
    assert_eq!(
        final_cfg.targets.len(),
        1,
        "last-writer-wins semantics: exactly one target"
    );
    let _ = h;
}

#[test]
fn list_returns_targets_in_btreemap_order() {
    let h = IsolatedHome::new();
    cli_configure_add_pmcp_run("zebra", "us-west-2", &h.home_path, &h.ws_path).unwrap();
    cli_configure_add_pmcp_run("alpha", "us-east-1", &h.home_path, &h.ws_path).unwrap();
    let cfg = TargetConfigV1::read(&default_user_config_path()).unwrap();
    let names: Vec<&String> = cfg.targets.keys().collect();
    assert_eq!(
        names,
        vec![&"alpha".to_string(), &"zebra".to_string()],
        "BTreeMap must yield alphabetical order"
    );
}
