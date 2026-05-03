//! Phase 79 Wave 1: post-deploy verification schema types.
//!
//! Defines the `[post_deploy_tests]` block that operators add to
//! `.pmcp/deploy.toml` plus the typed [`TestOutcome`] enum that Plan 79-03's
//! verifier consumes (data sourced from the Wave-0
//! `mcp_tester::PostDeployReport` JSON contract).
//!
//! ## Why this module exists
//!
//! Cost-coach's Failure Mode C (proven 2026-04-23) was: widget bundle deployed
//! correctly to Lambda, but the JS SDK was misconfigured (missing `onteardown`
//! handler) — runtime broken, deploy reported "successful". Wave 3's
//! orchestrator probes the live endpoint via the existing
//! `cargo pmcp test {check, conformance, apps}` subcommands and consumes their
//! `--format=json` output (Wave-0 `PostDeployReport`) to render a typed
//! verdict.
//!
//! ## Phase 76 IamConfig precedent (mirrored here)
//!
//! `Option::is_none` skip on `DeployConfig::post_deploy_tests` preserves
//! byte-identity for files lacking the `[post_deploy_tests]` section.
//!
//! ## Revision-3 supersessions
//!
//! - **HIGH-G2 (rollback hard-reject):** [`OnFailure`] has variants `{Fail,
//!   Warn}` ONLY. The string `"rollback"` is hard-rejected by both the
//!   custom [`Deserialize`] impl AND the [`std::str::FromStr`] impl with the
//!   verbatim error in [`ROLLBACK_REJECT_MESSAGE`]. Operators who explicitly
//!   configure rollback must change to `"fail"` or `"warn"` — the previously-
//!   planned reserve-but-warn-then-fallback was a UX trap (operators assume
//!   rollback happened and ignore the broken-but-live state). Future phase
//!   that verifies `DeployTarget::rollback()` impls will add the variant +
//!   migration note.
//! - **HIGH-C2 (auth handled by child):** [`InfraErrorKind`] has variants
//!   `{Subprocess, Timeout, AuthOrNetwork}` ONLY — no `AuthMissing` variant.
//!   Subprocesses inherit the parent's env via Tokio Command default and
//!   self-resolve auth via the existing `AuthMethod::None` path that already
//!   covers Phase 74 cache + automatic refresh.

use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize};

/// Config for the post-deploy verification lifecycle (Phase 79).
///
/// Fields default to the values documented in `79-CONTEXT.md` "Post-deploy
/// verification". The whole block is wrapped in
/// `Option<PostDeployTestsConfig>` on `DeployConfig` so files lacking the
/// section round-trip byte-identically.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostDeployTestsConfig {
    /// Whether post-deploy verification runs at all. Default `true`.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Which checks to run, in order. Default
    /// `["connectivity", "conformance", "apps"]`.
    #[serde(default = "default_checks")]
    pub checks: Vec<String>,

    /// Which apps-validation strict mode to use. Default
    /// [`AppsMode::ClaudeDesktop`].
    #[serde(default)]
    pub apps_mode: AppsMode,

    /// What to do when a verification check fails. Default [`OnFailure::Fail`].
    #[serde(default)]
    pub on_failure: OnFailure,

    /// Per-test subprocess timeout (seconds). Default 60.
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,

    /// Pre-test wait after Lambda hot-swap to let pooled connections drain
    /// (milliseconds). Default 2000.
    #[serde(default = "default_warmup_grace_ms")]
    pub warmup_grace_ms: u64,
}

fn default_enabled() -> bool {
    true
}

fn default_checks() -> Vec<String> {
    vec![
        "connectivity".to_string(),
        "conformance".to_string(),
        "apps".to_string(),
    ]
}

fn default_timeout_seconds() -> u64 {
    60
}

fn default_warmup_grace_ms() -> u64 {
    2000
}

impl Default for PostDeployTestsConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            checks: default_checks(),
            apps_mode: AppsMode::default(),
            on_failure: OnFailure::default(),
            timeout_seconds: default_timeout_seconds(),
            warmup_grace_ms: default_warmup_grace_ms(),
        }
    }
}

/// Apps-validation strict mode. The `--apps-mode=` flag on
/// `cargo pmcp test apps` and `cargo pmcp deploy` accepts the same three
/// kebab-case strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AppsMode {
    /// Generic apps validation — no host-specific signals required.
    Standard,
    /// ChatGPT host signals (e.g. structured-content shape).
    Chatgpt,
    /// Claude Desktop host signals — `onteardown` handler, widget meta tags,
    /// etc. Default per `79-CONTEXT.md`. Phase 78 promoted this from
    /// placeholder to a real strict mode.
    #[default]
    ClaudeDesktop,
}

/// Failure-handling policy.
///
/// REVISION 3 (cross-AI review HIGH-G2 supersession): the `Rollback` variant
/// is REMOVED. Both serde deserialization and `FromStr` reject `"rollback"`
/// with the actionable error in [`ROLLBACK_REJECT_MESSAGE`]. The future phase
/// that verifies `DeployTarget::rollback()` impls will add the variant + a
/// migration note to CHANGELOG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OnFailure {
    /// CLI exits nonzero. **The new (broken) Lambda revision STAYS LIVE.**
    ///
    /// CI/CD pipelines that interpret nonzero exit as "auto-rollback me" will
    /// misread this — the deploy DID succeed at the infrastructure level; only
    /// the post-deploy verification failed. To roll back, run
    /// `cargo pmcp deploy rollback --target <target>` manually (note: rollback
    /// implementations across all 4 targets are currently stubs — see Phase 79
    /// deferred items).
    ///
    /// Plan 79-03 augments this with exit code 3 + GitHub Actions
    /// `::error::` annotation for CI/CD-friendly machine signals.
    #[default]
    Fail,

    /// CLI prints a warning and exits zero. The (potentially broken) revision
    /// stays live; pipeline continues.
    Warn,
}

/// Verbatim hard-reject error for `on_failure="rollback"`. Used by both the
/// custom [`Deserialize`] impl AND the [`std::str::FromStr`] impl so config +
/// CLI share one rejection path. REVISION 3 supersession per CONTEXT.md
/// HIGH-G2.
pub const ROLLBACK_REJECT_MESSAGE: &str =
    "on_failure='rollback' is not yet implemented in this version of cargo-pmcp. \
     Change to 'fail' (default) or 'warn'. Auto-rollback support will land in a \
     future phase that verifies the existing DeployTarget::rollback() trait implementations.";

/// Custom `Deserialize` to hard-reject `"rollback"` at config validation time
/// per HIGH-G2. The default `#[derive(Deserialize)]` would accept any of the
/// listed variants — this impl narrows to `{fail, warn}` and emits the
/// actionable error for `"rollback"`.
impl<'de> Deserialize<'de> for OnFailure {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "fail" => Ok(OnFailure::Fail),
            "warn" => Ok(OnFailure::Warn),
            "rollback" => Err(D::Error::custom(ROLLBACK_REJECT_MESSAGE)),
            other => Err(D::Error::custom(format!(
                "invalid on_failure='{other}' — valid values are 'fail' or 'warn'"
            ))),
        }
    }
}

impl std::str::FromStr for OnFailure {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "fail" => Ok(OnFailure::Fail),
            "warn" => Ok(OnFailure::Warn),
            "rollback" => Err(ROLLBACK_REJECT_MESSAGE.to_string()),
            other => Err(format!(
                "invalid on_failure='{other}' — valid values are 'fail' or 'warn'"
            )),
        }
    }
}

/// Result of one subprocess test invocation. Distinguishes verdict-on-new-code
/// ([`Self::TestFailed`]) from infra flakiness ([`Self::InfraError`]) so CI/CD
/// can decide rollback without conflating the two.
///
/// REVISION 3: the [`TestOutcome::Passed::summary`] /
/// [`TestOutcome::TestFailed::summary`] / [`TestOutcome::TestFailed::recipes`]
/// fields are populated from the Wave-0 `mcp_tester::PostDeployReport`
/// (consumed by 79-03's `spawn_test_subprocess`), NOT from regex-parsing
/// pretty terminal output. This eliminates the F-2 / N-1 / N-3 brittleness
/// chain of revisions 1 + 2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestOutcome {
    /// Subprocess exited 0. `summary` carries the pass-count from the JSON
    /// report when applicable; `None` otherwise (e.g., connectivity check is
    /// binary so no N/M counts apply).
    Passed {
        /// Optional pass-count metric.
        summary: Option<TestSummary>,
    },
    /// Subprocess exited 1 (verdict on the new code).
    TestFailed {
        /// Single-line label / detail (e.g. `"apps"`, `"conformance"`).
        label: String,
        /// Pass-count metric extracted from JSON `PostDeployReport.summary`.
        summary: Option<TestSummary>,
        /// Per-failure reproduction commands extracted from JSON
        /// `PostDeployReport.failures[].reproduce`.
        recipes: Vec<FailureRecipe>,
    },
    /// Subprocess failed at the infrastructure level — child failed to spawn,
    /// timed out, or reported a network/auth error via the JSON outcome.
    InfraError(InfraErrorKind, String),
}

/// Pass/total count populated from the Wave-0 `mcp_tester::PostDeployReport.summary`.
///
/// We KEEP a local 2-bucket struct (passed/total) for the banner formatter —
/// the upstream `mcp_tester::TestSummary` carries 5 buckets but the banner
/// only renders passed/total. Wave 3's JSON consumer maps
/// `mcp_tester::TestSummary { passed, total, .. }` → this 2-bucket form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TestSummary {
    /// Number of passed checks.
    pub passed: u32,
    /// Total number of checks.
    pub total: u32,
}

/// Reproduction command for a single failing tool. Populated from
/// `mcp_tester::FailureDetail.reproduce` (Wave 0 contract). Documentation-only
/// — never `eval`'d, never `Command::spawn`'d (T-79-14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureRecipe {
    /// Verbatim copy-paste-able `cargo pmcp test ...` command line.
    pub command: String,
}

/// Infrastructure-error classification.
///
/// REVISION 3 (HIGH-C2 supersession): `AuthMissing` variant REMOVED.
/// Subprocesses inherit the parent's env via Tokio Command default and
/// self-resolve auth via the existing `AuthMethod::None` path at
/// `cargo-pmcp/src/commands/auth.rs:106-109`, which already supports Phase 74
/// cache + automatic refresh. The orchestrator no longer pre-checks auth
/// presence; a child 401 surfaces as the child's own non-zero exit (mapped
/// to [`Self::AuthOrNetwork`] if the JSON output exists, or [`Self::Subprocess`]
/// if not).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfraErrorKind {
    /// Child process failed to spawn or exited via signal.
    Subprocess,
    /// Subprocess exceeded `timeout_seconds`.
    Timeout,
    /// Child reported network/auth failure (exit code 2-ish OR JSON
    /// `outcome=infra-error`).
    AuthOrNetwork,
}

#[cfg(test)]
mod tests {
    //! Unit tests covering `<behavior>` Tests 2.1..2.10 of Plan 79-01.
    use super::*;
    use std::str::FromStr;

    /// Test 2.1 (round_trip_no_post_deploy_tests_byte_identical): default
    /// `PostDeployTestsConfig` is the documented defaults. Byte-identity for
    /// the `Option::is_none` skip is exercised in `tests/post_deploy_tests_config.rs`.
    #[test]
    fn defaults_round_trip_through_toml() {
        let cfg = PostDeployTestsConfig::default();
        let serialized = toml::to_string(&cfg).expect("serializes");
        let reparsed: PostDeployTestsConfig = toml::from_str(&serialized).expect("re-parses");
        assert_eq!(reparsed.enabled, cfg.enabled);
        assert_eq!(reparsed.checks, cfg.checks);
        assert_eq!(reparsed.apps_mode, cfg.apps_mode);
        assert_eq!(reparsed.on_failure, cfg.on_failure);
        assert_eq!(reparsed.timeout_seconds, cfg.timeout_seconds);
        assert_eq!(reparsed.warmup_grace_ms, cfg.warmup_grace_ms);
    }

    /// Test 2.2 (parses_explicit_post_deploy_tests_block): operator-shaped
    /// TOML hits every field with documented values.
    #[test]
    fn parses_explicit_post_deploy_tests_block() {
        let toml_str = r#"
enabled = true
checks = ["connectivity", "conformance", "apps"]
apps_mode = "claude-desktop"
on_failure = "fail"
timeout_seconds = 60
warmup_grace_ms = 2000
"#;
        let parsed: PostDeployTestsConfig = toml::from_str(toml_str).expect("parses");
        assert!(parsed.enabled);
        assert_eq!(
            parsed.checks,
            vec![
                "connectivity".to_string(),
                "conformance".to_string(),
                "apps".to_string(),
            ]
        );
        assert_eq!(parsed.apps_mode, AppsMode::ClaudeDesktop);
        assert_eq!(parsed.on_failure, OnFailure::Fail);
        assert_eq!(parsed.timeout_seconds, 60);
        assert_eq!(parsed.warmup_grace_ms, 2000);
    }

    /// Test 2.3 (defaults_match_context_md): the default values are exactly
    /// what `79-CONTEXT.md` documents.
    #[test]
    fn defaults_match_context_md() {
        let cfg = PostDeployTestsConfig::default();
        assert!(cfg.enabled);
        assert_eq!(
            cfg.checks,
            vec![
                "connectivity".to_string(),
                "conformance".to_string(),
                "apps".to_string(),
            ]
        );
        assert_eq!(cfg.apps_mode, AppsMode::ClaudeDesktop);
        assert_eq!(cfg.on_failure, OnFailure::Fail);
        assert_eq!(cfg.timeout_seconds, 60);
        assert_eq!(cfg.warmup_grace_ms, 2000);
    }

    /// Test 2.4 (rollback_hard_rejected — REVISION 3 supersession): given
    /// `on_failure = "rollback"`, deserialization returns `Err` whose Display
    /// contains the verbatim ROLLBACK_REJECT_MESSAGE.
    #[test]
    fn rollback_hard_rejected() {
        let toml_str = r#"on_failure = "rollback""#;
        let err = toml::from_str::<PostDeployTestsConfig>(toml_str)
            .expect_err("rollback must be hard-rejected");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("not yet implemented"),
            "expected hard-reject message, got: {msg}"
        );
        assert!(
            msg.contains("'fail'") && msg.contains("'warn'"),
            "expected actionable migration hints in error, got: {msg}"
        );
    }

    /// Test 2.5 (apps_mode_enum_values): all three documented values parse,
    /// and the default is ClaudeDesktop.
    #[test]
    fn apps_mode_enum_values() {
        #[derive(serde::Deserialize)]
        struct W {
            apps_mode: AppsMode,
        }
        let standard: W = toml::from_str(r#"apps_mode = "standard""#).expect("standard");
        assert_eq!(standard.apps_mode, AppsMode::Standard);
        let chatgpt: W = toml::from_str(r#"apps_mode = "chatgpt""#).expect("chatgpt");
        assert_eq!(chatgpt.apps_mode, AppsMode::Chatgpt);
        let cd: W = toml::from_str(r#"apps_mode = "claude-desktop""#).expect("claude-desktop");
        assert_eq!(cd.apps_mode, AppsMode::ClaudeDesktop);
        assert_eq!(AppsMode::default(), AppsMode::ClaudeDesktop);
    }

    /// Test 2.6 (only_two_on_failure_variants_exist): exhaustive match locks
    /// the variant set. If a third variant is added without updating this
    /// test, compilation fails.
    #[test]
    fn only_two_on_failure_variants_exist() {
        let f = OnFailure::Fail;
        let w = OnFailure::Warn;
        // Exhaustive match — adding a third variant forces a compile error here.
        let label = |v: OnFailure| match v {
            OnFailure::Fail => "fail",
            OnFailure::Warn => "warn",
        };
        assert_eq!(label(f), "fail");
        assert_eq!(label(w), "warn");
    }

    /// Test 2.7 (test_outcome_test_failed_struct_payload_constructs):
    /// `TestFailed { label, summary, recipes }` constructs and a match on it
    /// binds all three fields.
    #[test]
    fn test_outcome_test_failed_struct_payload_constructs() {
        let outcome = TestOutcome::TestFailed {
            label: "apps".to_string(),
            summary: Some(TestSummary {
                passed: 7,
                total: 8,
            }),
            recipes: vec![FailureRecipe {
                command: "cargo pmcp test apps --url http://x --mode claude-desktop --tool foo"
                    .to_string(),
            }],
        };
        match &outcome {
            TestOutcome::TestFailed {
                label,
                summary,
                recipes,
            } => {
                assert_eq!(label, "apps");
                assert_eq!(summary.unwrap().passed, 7);
                assert_eq!(summary.unwrap().total, 8);
                assert_eq!(recipes.len(), 1);
                assert!(recipes[0].command.contains("--mode claude-desktop"));
                assert!(recipes[0].command.contains("--tool foo"));
            },
            other => panic!("expected TestFailed, got {other:?}"),
        }
    }

    /// Test 2.8 (infra_error_kind_variants — REVISION 3): `InfraErrorKind`
    /// has EXACTLY `{Subprocess, Timeout, AuthOrNetwork}`. No `AuthMissing`
    /// variant. Exhaustive match locks the set.
    #[test]
    fn infra_error_kind_variants() {
        let s = InfraErrorKind::Subprocess;
        let t = InfraErrorKind::Timeout;
        let a = InfraErrorKind::AuthOrNetwork;
        let label = |v: InfraErrorKind| match v {
            InfraErrorKind::Subprocess => "subprocess",
            InfraErrorKind::Timeout => "timeout",
            InfraErrorKind::AuthOrNetwork => "auth-or-network",
        };
        assert_eq!(label(s), "subprocess");
        assert_eq!(label(t), "timeout");
        assert_eq!(label(a), "auth-or-network");
    }

    /// Test 2.9 (test_outcome_passed_struct_payload_constructs): both the
    /// `Some(summary)` and `None` variants of `Passed` construct and bind.
    #[test]
    fn test_outcome_passed_struct_payload_constructs() {
        let with_summary = TestOutcome::Passed {
            summary: Some(TestSummary {
                passed: 8,
                total: 8,
            }),
        };
        match &with_summary {
            TestOutcome::Passed { summary: Some(s) } => {
                assert_eq!(s.passed, 8);
                assert_eq!(s.total, 8);
            },
            other => panic!("expected Passed{{Some}}, got {other:?}"),
        }

        let connectivity = TestOutcome::Passed { summary: None };
        match &connectivity {
            TestOutcome::Passed { summary: None } => {},
            other => panic!("expected Passed{{None}}, got {other:?}"),
        }
    }

    /// Test 2.10 (clap_str_parse_for_on_failure — REVISION 3): `FromStr`
    /// rejects `"rollback"` with the same actionable error as serde.
    #[test]
    fn clap_str_parse_for_on_failure() {
        assert_eq!(OnFailure::from_str("fail"), Ok(OnFailure::Fail));
        assert_eq!(OnFailure::from_str("warn"), Ok(OnFailure::Warn));
        let err = OnFailure::from_str("rollback").expect_err("rollback rejected");
        assert!(
            err.contains("not yet implemented"),
            "FromStr must use the same hard-reject message as serde: {err}"
        );
        let err = OnFailure::from_str("nonsense").expect_err("nonsense rejected");
        assert!(
            err.contains("'fail'") && err.contains("'warn'"),
            "unknown variants must list valid options: {err}"
        );
    }
}
