//! Phase 79 Wave 1 — `PostDeployTestsConfig` TOML-schema integration tests.
//!
//! Mirrors the Phase 76 `iam_config.rs` precedent: parse fixtures via
//! `toml::from_str::<DeployConfig>` to lock the textual schema shape AND
//! exercise the REVISION 3 HIGH-G2 hard-reject path against the
//! `post-deploy-rollback-rejected.deploy.toml` fixture.

use cargo_pmcp::deployment::config::DeployConfig;
use cargo_pmcp::deployment::post_deploy_tests::{OnFailure, ROLLBACK_REJECT_MESSAGE};
use std::str::FromStr;

const FAIL_FIXTURE: &str = "tests/fixtures/post-deploy-fail.deploy.toml";
const ROLLBACK_FIXTURE: &str = "tests/fixtures/post-deploy-rollback-rejected.deploy.toml";

fn load_fixture(path: &str) -> Result<DeployConfig, toml::de::Error> {
    let toml_str =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
    toml::from_str::<DeployConfig>(&toml_str)
}

/// Test 3.2 (DeployConfig accepts post_deploy_tests section): the
/// `post-deploy-fail.deploy.toml` fixture parses with
/// `cfg.post_deploy_tests.is_some()` and `on_failure == OnFailure::Fail`.
#[test]
fn deploy_config_accepts_post_deploy_tests_section() {
    let cfg = load_fixture(FAIL_FIXTURE).expect("post-deploy-fail fixture parses");
    let pdt = cfg
        .post_deploy_tests
        .as_ref()
        .expect("post_deploy_tests should be Some");
    assert!(pdt.enabled);
    assert_eq!(
        pdt.checks,
        vec![
            "connectivity".to_string(),
            "conformance".to_string(),
            "apps".to_string(),
        ]
    );
    assert_eq!(pdt.on_failure, OnFailure::Fail);
    assert_eq!(pdt.timeout_seconds, 60);
    assert_eq!(pdt.warmup_grace_ms, 2000);
}

/// Test 3.4 (rollback_fixture_hard_rejects — REVISION 3 SUPERSESSION):
/// `DeployConfig::load`-equivalent (`toml::from_str::<DeployConfig>`)
/// against `post-deploy-rollback-rejected.deploy.toml` returns `Err` whose
/// Display contains the verbatim ROLLBACK_REJECT_MESSAGE.
///
/// Replaces the pre-revision-3 `rollback_reserved_fixture_parses` test
/// which expected `Ok` parse. Per CONTEXT.md "Review-Driven Supersessions"
/// #2 (HIGH-G2).
#[test]
fn rollback_fixture_hard_rejects() {
    let result = load_fixture(ROLLBACK_FIXTURE);
    let err = result.expect_err("rollback fixture must hard-reject at parse time");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("not yet implemented"),
        "expected ROLLBACK_REJECT_MESSAGE in error, got: {msg}"
    );
    assert!(
        msg.contains("'fail'") && msg.contains("'warn'"),
        "expected actionable migration hints, got: {msg}"
    );
    // Cross-check: the constant exists and the error contains a meaningful
    // prefix from it.
    assert!(
        ROLLBACK_REJECT_MESSAGE.contains("not yet implemented"),
        "ROLLBACK_REJECT_MESSAGE constant content drifted"
    );
}

/// Property-style: every `OnFailure` variant round-trips serialize → deserialize
/// via TOML. `"rollback"` ALWAYS errors via both serde AND `FromStr`.
#[test]
fn on_failure_variants_round_trip_and_rollback_always_errors() {
    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
    struct W {
        on_failure: OnFailure,
    }

    for v in [OnFailure::Fail, OnFailure::Warn] {
        let original = W { on_failure: v };
        let s = toml::to_string(&original).expect("serializes");
        let reparsed: W = toml::from_str(&s).expect("re-parses");
        assert_eq!(reparsed, original, "round-trip failed for {v:?}");
    }

    // Serde rejects "rollback".
    let err =
        toml::from_str::<W>(r#"on_failure = "rollback""#).expect_err("serde must reject rollback");
    assert!(
        format!("{err:#}").contains("not yet implemented"),
        "serde rejection must use the verbatim message"
    );

    // FromStr rejects "rollback" with the same actionable message.
    let err = OnFailure::from_str("rollback").expect_err("FromStr must reject rollback");
    assert!(err.contains("not yet implemented"));
    assert!(err.contains("'fail'") && err.contains("'warn'"));
}
