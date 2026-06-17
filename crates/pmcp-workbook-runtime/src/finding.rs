//! The located, collect-all lint-finding types (DIA-02 finding shape).
//!
//! RELOCATED into `workbook-runtime` (Phase 11, Plan 05): the runtime executor's
//! `run()` returns a `Box<LintFinding>` on a dependency cycle, so the finding
//! types must live on the umya-free runtime side. `workbook-compiler` re-exports
//! these from `pmcp_workbook_runtime` so its `dialect::{LintFinding, LintReport,
//! Severity}` surface (and every `crate::dialect::*` consumer) is unchanged.
//!
//! A [`LintFinding`] is the linter's atomic output unit: a `severity` tier, a
//! stable slash-namespaced `rule` id, a `sheet` + optional `cell` LOCATION, a
//! human `message`, and BA-actionable `repair` text. A [`LintReport`] is the
//! collect-all aggregate: the linter never stops at the first problem — it
//! accumulates EVERY finding and answers [`LintReport::has_errors`] as the
//! conformance gate (D-05: only `Error` severity blocks; `Warning`/`Info` do
//! not).
//!
//! These three derive `serde::Serialize` + `serde::Deserialize` +
//! `schemars::JsonSchema` because they serialize to (and round-trip back from)
//! the lint-report artifact + snapshot that Phases 8–11 and the BA consume
//! (`Deserialize` added per D-08 so a served `LintReport` JSON parses back into
//! the typed struct).

use serde::{Deserialize, Serialize};

/// The severity tier of a [`LintFinding`]. Only [`Severity::Error`] gates
/// conformance ([`LintReport::has_errors`]); `Warning`/`Info` are advisory (D-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// A dialect violation that BLOCKS conformance (e.g. an out-of-whitelist
    /// function, a macro-bearing workbook, an array formula).
    Error,
    /// An advisory dialect concern that does NOT block (e.g. a hidden row).
    Warning,
    /// Informational signal surfaced for the BA but not a violation.
    Info,
}

/// A single located dialect finding (DIA-02). The `rule` is a stable
/// slash-namespaced id (e.g. `"whitelist/unsupported-fn"`,
/// `"structure/hidden-sheet"`, `"manifest/role-conflict"`); `repair` carries
/// BA-actionable fix text so a non-engineer can act without a round-trip.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LintFinding {
    /// The conformance-gating tier (only `Error` blocks; D-05).
    pub severity: Severity,
    /// Stable slash-namespaced rule id (`<namespace>/<kebab-rule>`).
    pub rule: String,
    /// The sheet the finding is located on (e.g. `"1_Inputs"`).
    pub sheet: String,
    /// The optional cell address within `sheet` (e.g. `"E6"`); `None` for a
    /// sheet- or workbook-level finding.
    pub cell: Option<String>,
    /// Human-readable description of what was found.
    pub message: String,
    /// BA-actionable repair text describing how to fix the finding.
    pub repair: String,
}

impl LintFinding {
    /// Construct a located finding. `cell` is `None` for sheet/workbook-level
    /// findings.
    pub fn new(
        severity: Severity,
        rule: impl Into<String>,
        sheet: impl Into<String>,
        cell: Option<String>,
        message: impl Into<String>,
        repair: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            rule: rule.into(),
            sheet: sheet.into(),
            cell,
            message: message.into(),
            repair: repair.into(),
        }
    }
}

/// The collect-all aggregate of every [`LintFinding`] from one lint pass
/// (mirrors `CatalogError::Load(Vec<_>)`). The linter accumulates into one
/// report; [`LintReport::has_errors`] is the conformance gate (D-05).
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LintReport {
    /// EVERY finding from the pass, in discovery order.
    pub findings: Vec<LintFinding>,
}

impl LintReport {
    /// An empty report ready to accumulate findings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a single finding.
    pub fn push(&mut self, finding: LintFinding) {
        self.findings.push(finding);
    }

    /// Append every finding from an iterator (so independent passes fold into
    /// one report).
    pub fn extend(&mut self, findings: impl IntoIterator<Item = LintFinding>) {
        self.findings.extend(findings);
    }

    /// The conformance gate (D-05): `true` iff any finding is `Error` severity.
    /// `Warning`/`Info` findings do NOT block conformance.
    pub fn has_errors(&self) -> bool {
        self.findings.iter().any(|f| f.severity == Severity::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_errors_gates_on_error_severity_only() {
        let mut report = LintReport::new();
        report.push(LintFinding::new(
            Severity::Error,
            "whitelist/unsupported-fn",
            "1_Inputs",
            Some("A2".to_string()),
            "OFFSET is not in the dialect whitelist",
            "Replace OFFSET with an INDEX/MATCH lookup",
        ));
        report.push(LintFinding::new(
            Severity::Warning,
            "structure/hidden-row",
            "1_Inputs",
            None,
            "row 7 is hidden",
            "Unhide the row or document why it is hidden",
        ));
        assert!(report.has_errors(), "an Error finding must trip has_errors");
        assert_eq!(report.findings.len(), 2);
    }

    #[test]
    fn has_errors_false_when_only_warnings_and_info() {
        let mut report = LintReport::new();
        report.extend([
            LintFinding::new(
                Severity::Warning,
                "structure/hidden-row",
                "1_Inputs",
                None,
                "row 7 is hidden",
                "Unhide the row",
            ),
            LintFinding::new(
                Severity::Info,
                "structure/note",
                "0_Guide",
                None,
                "guide legend present",
                "No action required",
            ),
        ]);
        assert!(
            !report.has_errors(),
            "warnings and info alone must NOT trip has_errors (D-05)"
        );
    }

    #[test]
    fn lint_finding_serializes_with_repair_field() {
        let finding = LintFinding::new(
            Severity::Error,
            "structure/external-link",
            "1_Inputs",
            Some("E6".to_string()),
            "external link reference [1]Sheet1 found",
            "Inline the referenced value; the dialect forbids external links",
        );
        let json = serde_json::to_value(&finding).expect("serialize finding");
        assert_eq!(
            json["repair"],
            "Inline the referenced value; the dialect forbids external links"
        );
        assert_eq!(json["severity"], "error");
        assert_eq!(json["rule"], "structure/external-link");
        assert_eq!(json["cell"], "E6");
    }

    #[test]
    fn lint_report_round_trips_through_json() {
        let mut report = LintReport::new();
        report.push(LintFinding::new(
            Severity::Error,
            "whitelist/unsupported-fn",
            "1_Inputs",
            Some("A2".into()),
            "msg",
            "repair",
        ));
        let back: LintReport =
            serde_json::from_value(serde_json::to_value(&report).unwrap()).unwrap();
        assert_eq!(back.findings.len(), 1);
        assert!(back.has_errors());
    }
}
