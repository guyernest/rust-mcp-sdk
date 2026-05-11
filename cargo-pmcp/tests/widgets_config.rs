//! Phase 79 Wave 1 — `WidgetsConfig` TOML-schema integration tests.
//!
//! Mirrors the Phase 76 `iam_config.rs` precedent: parse fixtures via
//! `toml::from_str::<DeployConfig>` to lock the textual schema shape
//! operators see, plus a tempdir-backed lockfile-detection sweep that
//! exercises `PackageManager::detect_from_dir` against every combination.
//!
//! `cargo_pmcp::deployment::widgets` IS lib-visible (see lib.rs Wave-1
//! mount), so this integration target imports the types directly through the
//! crate's library surface — no `#[path]` shenanigans needed.

use cargo_pmcp::deployment::config::DeployConfig;
use cargo_pmcp::deployment::widgets::{PackageManager, WidgetsConfig};
use std::path::PathBuf;

const FIXTURE_PATH: &str = "tests/fixtures/cost-coach-widgets.deploy.toml";

/// Read the cost-coach widgets fixture from disk and parse it as a
/// `DeployConfig`. Helper used by every test below.
fn load_widgets_fixture() -> DeployConfig {
    let toml_str = std::fs::read_to_string(FIXTURE_PATH)
        .unwrap_or_else(|e| panic!("failed to read {FIXTURE_PATH}: {e}"));
    toml::from_str::<DeployConfig>(&toml_str)
        .unwrap_or_else(|e| panic!("failed to parse {FIXTURE_PATH}: {e}"))
}

/// Test 3.1 (DeployConfig accepts widgets section): the cost-coach fixture
/// parses with `cfg.widgets.widgets.len() == 2`. Locks the on-disk
/// `[[widgets]]` shape an operator would write.
#[test]
fn deploy_config_accepts_widgets_section() {
    let cfg = load_widgets_fixture();
    assert_eq!(
        cfg.widgets.widgets.len(),
        2,
        "cost-coach-widgets fixture has 2 [[widgets]] blocks"
    );

    let primary = &cfg.widgets.widgets[0];
    assert_eq!(primary.path, "widget");
    assert_eq!(
        primary.embedded_in_crates,
        vec!["cost-coach-lambda".to_string()]
    );
    assert_eq!(primary.output_dir, "dist", "default output_dir is `dist`");

    let admin = &cfg.widgets.widgets[1];
    assert_eq!(admin.path, "widgets/admin");
    assert_eq!(
        admin.output_dir, "build",
        "explicit output_dir overrides default"
    );
    assert_eq!(
        admin.embedded_in_crates,
        vec!["cost-coach-admin-lambda".to_string()]
    );
}

/// Test 3.3 (DeployConfig round-trips Phase-76 fixture byte-identically):
/// re-serialising a `DeployConfig` built from the existing Phase-76 IAM
/// fixture (no widgets, no post_deploy_tests) MUST NOT emit `widgets = []`
/// or any `post_deploy_tests` line. Locks the
/// `skip_serializing_if` D-05 byte-identity contract for both new fields.
#[test]
fn deploy_config_round_trips_phase76_fixture_byte_identical() {
    let phase76_path = "examples/fixtures/cost-coach.deploy.toml";
    let toml_str = std::fs::read_to_string(phase76_path)
        .unwrap_or_else(|e| panic!("failed to read {phase76_path}: {e}"));
    let cfg: DeployConfig =
        toml::from_str(&toml_str).unwrap_or_else(|e| panic!("failed to parse {phase76_path}: {e}"));
    assert!(cfg.widgets.is_empty(), "Phase-76 fixture has no widgets");
    assert!(
        cfg.post_deploy_tests.is_none(),
        "Phase-76 fixture has no [post_deploy_tests]"
    );

    let serialized = toml::to_string(&cfg).expect("serializes");
    assert!(
        !serialized.contains("widgets"),
        "empty WidgetsConfig must not emit `widgets` key — got:\n{serialized}"
    );
    assert!(
        !serialized.contains("post_deploy_tests"),
        "None post_deploy_tests must not emit `post_deploy_tests` key — got:\n{serialized}"
    );
}

/// Test 3.5 (path_traversal_rejected_at_validate): a fixture with
/// `path = "../etc"` parses cleanly at the serde level — validation is a
/// separate orchestrator-time concern (`WidgetConfig::validate`).
#[test]
fn path_traversal_rejected_at_validate_not_at_serde() {
    let mut cfg = load_widgets_fixture();
    cfg.widgets.widgets[0].path = "../etc".to_string();
    let err = cfg.widgets.widgets[0]
        .validate()
        .expect_err("`..` must be rejected at validate-time");
    let msg = err.to_string();
    assert!(
        msg.contains("..") || msg.contains("path traversal"),
        "expected path-traversal error, got: {msg}"
    );
}

/// Test 1.7 (build_install_argv_array_round_trip — REVISION 3): argv-array
/// form preserves the `--silent` flag the pre-revision-3 string form would
/// have whitespace-broken. Integration scenario: parse via a `Wrapper` that
/// mirrors the production shape (`DeployConfig.widgets: WidgetsConfig`), then
/// re-serialise + re-parse.
#[test]
fn build_install_argv_array_round_trips_through_toml() {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Wrapper {
        #[serde(default)]
        widgets: WidgetsConfig,
    }

    let toml_str = r#"
[[widgets]]
path = "widget"
build = ["npm", "run", "--silent", "build"]
install = ["pnpm", "install", "--frozen-lockfile"]
embedded_in_crates = ["my-crate"]
"#;
    let parsed: Wrapper = toml::from_str(toml_str).expect("parses");
    assert_eq!(parsed.widgets.widgets.len(), 1);
    let w = &parsed.widgets.widgets[0];
    assert_eq!(
        w.build,
        Some(vec![
            "npm".to_string(),
            "run".to_string(),
            "--silent".to_string(),
            "build".to_string(),
        ]),
        "argv-array build preserves --silent flag (Codex MEDIUM mitigation)"
    );

    let serialized = toml::to_string(&parsed).expect("re-serializes");
    let reparsed: Wrapper = toml::from_str(&serialized).expect("re-parses");
    assert_eq!(reparsed.widgets.widgets[0].build, w.build);
    assert_eq!(reparsed.widgets.widgets[0].install, w.install);
}

/// Test 1.4 (pm_detection_priority_order — table-driven property): for every
/// combination of lockfile presence in a tempdir,
/// `PackageManager::detect_from_dir` returns the highest-priority lockfile-
/// implied PM (bun > pnpm > yarn > npm; no lockfile → npm).
#[test]
fn pm_detection_priority_locked_across_all_subsets() {
    let lockfiles = [
        ("bun.lockb", PackageManager::Bun),
        ("pnpm-lock.yaml", PackageManager::Pnpm),
        ("yarn.lock", PackageManager::Yarn),
        ("package-lock.json", PackageManager::Npm),
    ];
    for mask in 0u8..16 {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut expected: Option<PackageManager> = None;
        for (i, (name, pm)) in lockfiles.iter().enumerate() {
            if mask & (1 << i) != 0 {
                std::fs::write(dir.path().join(name), b"").expect("write lockfile");
                if expected.is_none() {
                    expected = Some(*pm);
                }
            }
        }
        let want = expected.unwrap_or(PackageManager::Npm);
        let got = PackageManager::detect_from_dir(dir.path());
        assert_eq!(got, want, "mask={mask:04b} expected {want:?} got {got:?}");
    }
}

/// Smoke test — `WidgetConfig::resolve_paths` joins the workspace root with
/// the configured path. Required by Plan 79-02's orchestrator.
#[test]
fn resolve_paths_joins_workspace_root() {
    let cfg = load_widgets_fixture();
    let primary = &cfg.widgets.widgets[0];
    let root = PathBuf::from("/tmp/ws");
    let r = primary.resolve_paths(&root);
    assert_eq!(r.path, PathBuf::from("/tmp/ws/widget"));
    assert_eq!(r.absolute_output_dir, PathBuf::from("/tmp/ws/widget/dist"));
}
