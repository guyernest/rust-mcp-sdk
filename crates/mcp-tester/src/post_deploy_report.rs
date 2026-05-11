//! Phase 79 Wave 0: machine-readable test-command output contract.
//!
//! `PostDeployReport` is the structured report emitted by
//! `cargo pmcp test {check, conformance, apps} --format=json`. The Phase 79
//! post-deploy verifier (`cargo-pmcp/src/deployment/post_deploy_tests.rs`,
//! shipped in Plan 79-03) consumes this as
//! `serde_json::from_str::<PostDeployReport>(stdout)?`, eliminating the need
//! to regex-parse pretty terminal output.
//!
//! ## Contract stability
//!
//! `schema_version` is the wire-format guard. Phase 79 ships `"1"`. Future
//! breaking changes MUST bump this and downstream consumers MUST check it
//! before deserializing. Additive field changes (new optional fields with
//! `#[serde(default)]`) do NOT bump the version.
//!
//! ## Why a new struct (vs. extending `TestReport`)
//!
//! `mcp_tester::TestReport` (in `report.rs`) is the existing per-test-suite
//! report. `PostDeployReport` wraps it with metadata the verifier needs:
//! - `command` discriminator (which subcommand emitted this)
//! - `url` for traceability
//! - `mode` for the `apps` subcommand variant
//! - `outcome` enum for the trinary verdict (Passed / `TestFailed` / `InfraError`)
//! - `failures: Vec<FailureDetail>` with pre-formatted `reproduce` strings
//! - `schema_version` for forward-compat
//!
//! Re-using `TestReport` directly would mix concerns; this wrapper keeps the
//! per-test-suite reporter (`TestReport`) and the per-subcommand verifier
//! contract (`PostDeployReport`) cleanly separated.

use crate::TestSummary;
use serde::{Deserialize, Serialize};

/// Top-level machine-readable report emitted by `cargo pmcp test {check,
/// conformance, apps} --format=json`. The canonical contract for Phase 79's
/// post-deploy verifier (Plan 79-03 consumes this via
/// `serde_json::from_str::<PostDeployReport>(stdout)`).
///
/// ## Schema version
/// - `"1"` (Phase 79) — initial release.
///
/// ## Outcome semantics
/// - `Passed` — subcommand exit code 0; `summary` populated where applicable.
/// - `TestFailed` — subcommand exit code 1 (verdict on the new code);
///   `failures` populated.
/// - `InfraError` — subcommand exit code 2 (network / spawn failure / timeout);
///   `failures` may be empty; `summary` may be `None`.
///
/// ## URL hygiene (Threat T-79-15)
/// `cargo pmcp` URL parsing strips embedded credentials before display per the
/// existing convention; `PostDeployReport.url` inherits that hygiene — callers
/// MUST NOT inject a URL with embedded credentials.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PostDeployReport {
    /// Which subcommand emitted this report.
    pub command: TestCommand,

    /// URL the subcommand probed.
    pub url: String,

    /// For `apps`: validation mode (`"claude-desktop"`, `"chatgpt"`, `"standard"`).
    /// For `check` and `conformance`: always `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,

    /// Trinary outcome — see struct doc.
    pub outcome: TestOutcome,

    /// Pass/fail counts. `None` when not applicable (e.g., `check` connectivity
    /// is binary so `summary` is `None` for that subcommand).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<TestSummary>,

    /// Per-failure detail with pre-formatted reproduce commands. Empty `Vec`
    /// when no failures (e.g., `Passed` outcome).
    #[serde(default)]
    pub failures: Vec<FailureDetail>,

    /// Wall-clock duration of the subcommand invocation (milliseconds).
    pub duration_ms: u64,

    /// Wire-format version. Currently `"1"`. Future breaking changes bump.
    pub schema_version: String,
}

impl Default for PostDeployReport {
    fn default() -> Self {
        Self {
            command: TestCommand::Check,
            url: String::new(),
            mode: None,
            outcome: TestOutcome::Passed,
            summary: None,
            failures: Vec::new(),
            duration_ms: 0,
            schema_version: "1".to_string(),
        }
    }
}

/// Discriminator for which subcommand emitted the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TestCommand {
    /// `cargo pmcp test check`
    Check,
    /// `cargo pmcp test conformance`
    Conformance,
    /// `cargo pmcp test apps`
    Apps,
}

/// Trinary outcome for the verifier consumer. Maps to subcommand exit codes:
/// `Passed=0`, `TestFailed=1`, `InfraError=2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TestOutcome {
    /// All tests passed; exit code 0.
    Passed,
    /// One or more tests failed; exit code 1.
    TestFailed,
    /// Network / spawn / timeout failure prevented the test from running; exit code 2.
    InfraError,
}

/// Per-failure detail with verbatim message and pre-formatted reproduce command.
///
/// Wave 3's banner formatter consumes `reproduce` directly as a copy-paste-able
/// command line — it MUST include `--mode <mode>` and `--tool <name>` for the
/// `apps` subcommand. For `conformance` failures, the `reproduce` line includes
/// `--domain <name>` when the failing domain is identifiable.
///
/// ## Threat note (T-79-14)
/// The `reproduce` field is documentation-only — never `eval`'d, never
/// `Command::spawn`'d by the producer. Phase 79 Wave 3 verifier likewise
/// treats it as a copy-paste-able UX hint, not an executable. Consumers MUST
/// preserve this rule: never auto-execute the string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureDetail {
    /// Tool name for `apps` failures; domain name for `conformance` failures;
    /// `None` for connectivity / framework-level failures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,

    /// Verbatim failure message from the existing `TestResult.error` /
    /// `TestResult.details`. NOT mutated for the verifier — preserves the same
    /// detail the human terminal output shows.
    pub message: String,

    /// Pre-formatted `cargo pmcp test ...` command line that reproduces this
    /// single failure in isolation. Includes `--mode` and `--tool` for `apps`,
    /// `--domain` for `conformance` (when domain identifiable), or the bare
    /// subcommand for connectivity failures.
    pub reproduce: String,
}

#[cfg(test)]
mod tests {
    //! Round-trip and serialization-format tests that lock the wire-format
    //! contract Phase 79 Wave 3 (Plan 79-03) depends on.
    use super::*;
    use crate::TestSummary;

    fn sample_summary() -> TestSummary {
        TestSummary {
            total: 9,
            passed: 7,
            failed: 1,
            warnings: 1,
            skipped: 0,
        }
    }

    #[test]
    fn post_deploy_report_round_trips_via_serde_json() {
        // Test 1.1: lock the wire-format contract Wave 3 depends on.
        let original = PostDeployReport {
            command: TestCommand::Check,
            url: "http://x".to_string(),
            mode: None,
            outcome: TestOutcome::Passed,
            summary: None,
            failures: vec![],
            duration_ms: 200,
            schema_version: "1".to_string(),
        };
        let json = serde_json::to_string_pretty(&original)
            .expect("PostDeployReport must serialize cleanly");
        let round_tripped: PostDeployReport =
            serde_json::from_str(&json).expect("PostDeployReport must deserialize cleanly");
        assert_eq!(original, round_tripped);
    }

    #[test]
    fn test_command_serializes_kebab_case() {
        // Test 1.2: kebab-case lock — Wave 3 deserializer relies on it.
        assert_eq!(
            serde_json::to_string(&TestCommand::Check).unwrap(),
            "\"check\""
        );
        assert_eq!(
            serde_json::to_string(&TestCommand::Conformance).unwrap(),
            "\"conformance\""
        );
        assert_eq!(
            serde_json::to_string(&TestCommand::Apps).unwrap(),
            "\"apps\""
        );
    }

    #[test]
    fn test_outcome_serializes_kebab_case() {
        // Test 1.3: trinary verdict serialization lock.
        assert_eq!(
            serde_json::to_string(&TestOutcome::Passed).unwrap(),
            "\"passed\""
        );
        assert_eq!(
            serde_json::to_string(&TestOutcome::TestFailed).unwrap(),
            "\"test-failed\""
        );
        assert_eq!(
            serde_json::to_string(&TestOutcome::InfraError).unwrap(),
            "\"infra-error\""
        );
    }

    #[test]
    fn failure_detail_construction() {
        // Test 1.4: --mode and --tool MUST appear in the reproduce string for
        // apps failures so Wave 3 can split / display them.
        let detail = FailureDetail {
            tool: Some("get_spend_summary".to_string()),
            message: "widget cost-summary.html missing onteardown handler".to_string(),
            reproduce:
                "cargo pmcp test apps --url http://x --mode claude-desktop --tool get_spend_summary"
                    .to_string(),
        };
        let json = serde_json::to_string(&detail).unwrap();
        let round_tripped: FailureDetail = serde_json::from_str(&json).unwrap();
        assert_eq!(detail, round_tripped);
        assert!(detail.reproduce.contains("--mode"));
        assert!(detail.reproduce.contains("--tool"));
    }

    #[test]
    fn schema_version_field_is_serialized() {
        // Test 1.5: forward-compat — consumers MUST be able to find this field.
        let report = PostDeployReport::default();
        let json = serde_json::to_string(&report).unwrap();
        assert!(
            json.contains("\"schema_version\":\"1\""),
            "schema_version field missing or wrong; got: {json}"
        );
    }

    #[test]
    fn test_summary_reuses_mcp_tester_test_summary() {
        // Test 1.6: the embedded TestSummary preserves all 5 buckets through
        // serde round-trip (passed/total/failed/warnings/skipped).
        let report = PostDeployReport {
            command: TestCommand::Conformance,
            url: "http://x".to_string(),
            mode: None,
            outcome: TestOutcome::TestFailed,
            summary: Some(sample_summary()),
            failures: vec![],
            duration_ms: 50,
            schema_version: "1".to_string(),
        };
        let json = serde_json::to_string(&report).unwrap();
        let round_tripped: PostDeployReport = serde_json::from_str(&json).unwrap();
        let summary = round_tripped.summary.expect("summary preserved");
        assert_eq!(summary.total, 9);
        assert_eq!(summary.passed, 7);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.warnings, 1);
        assert_eq!(summary.skipped, 0);
    }

    #[test]
    fn default_schema_version_is_1() {
        // Test 1.7: cannot accidentally ship a report with an empty version.
        let report = PostDeployReport::default();
        assert_eq!(report.schema_version, "1");
    }
}
