//! `cargo pmcp workbook lint <wb.xlsx>` — run the dialect linter standalone (WBCL-02).
//!
//! Ingests the workbook, runs `pmcp_workbook_compiler::dialect::linter::lint`
//! against `DialectRules::default()`, and renders the resulting `LintReport` in
//! either rich human text (default, the BA surface) or `--format json` (the
//! library's already-`Serialize` `LintReport`, no parallel DTO — D-09).
//!
//! Exit-code contract (D-10): only `Severity::Error` findings block — the handler
//! exits non-zero ([`super::EXIT_ERROR`]) iff `report.has_errors()`; a
//! warnings/info-only report still PRINTS the findings but exits
//! [`super::EXIT_OK`]. Rendering is a PURE [`format_lint_report`] String function
//! so JSON output is testable without stdout capture; [`print_lint_report`] is the
//! thin stdout wrapper. [`lint_exit_code`] is the pure exit-code mapping reused by
//! `compile.rs` (Plan 94-03) for the lint phase inside compile.
//!
//! Per Phase 74 D-11: data (the findings render) → stdout; advisory status → stderr.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;

use pmcp_workbook_compiler::dialect::linter::lint as dialect_lint;
use pmcp_workbook_compiler::{DialectRules, LintReport, Severity, WorkbookCellSource};

use super::GlobalFlags;

/// Arguments for `cargo pmcp workbook lint`.
#[derive(Debug, Args)]
pub struct LintArgs {
    /// Path to the `.xlsx` workbook to lint.
    pub workbook_path: PathBuf,

    /// Output format: `text` (default) or `json`.
    #[arg(long, default_value = "text")]
    pub format: String,
}

/// Execute `cargo pmcp workbook lint`.
///
/// # Errors
/// Returns an error if the workbook cannot be ingested, if `--format` is
/// unknown, or — per the D-10 exit-code contract — if the report carries any
/// `Severity::Error` finding (a plain `bail!` maps to anyhow's default exit code
/// `1`, which equals [`super::EXIT_ERROR`]).
pub fn execute(args: LintArgs, gf: &GlobalFlags) -> Result<()> {
    let report = lint_workbook(&args.workbook_path)?;

    let not_quiet = gf.should_output() && std::env::var("PMCP_QUIET").is_err();
    print_lint_report(&report, &args.format, not_quiet)?;

    if report.has_errors() {
        anyhow::bail!(
            "lint failed: {} error finding(s) in {}",
            error_count(&report),
            args.workbook_path.display()
        );
    }
    Ok(())
}

/// Ingest the workbook at `path` and run the dialect linter over it.
fn lint_workbook(path: &std::path::Path) -> Result<LintReport> {
    let (map, _ingest_findings) = pmcp_workbook_compiler::ingest::ingest(path)
        .with_context(|| format!("failed to ingest workbook {}", path.display()))?;
    let src = WorkbookCellSource::new(&map);
    Ok(dialect_lint(&src, &DialectRules::default()))
}

/// The number of `Severity::Error` findings in `report`.
fn error_count(report: &LintReport) -> usize {
    report
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count()
}

/// The pure exit-code mapping (D-10): `EXIT_ERROR` iff the report carries any
/// `Severity::Error` finding, else `EXIT_OK`. `Warning`/`Info` are advisory and
/// never block. Reuses the shared `super::EXIT_*` constants. Reused by
/// `compile.rs` (Plan 94-03) for the lint phase inside compile.
pub fn lint_exit_code(report: &LintReport) -> i32 {
    if report.has_errors() {
        super::EXIT_ERROR
    } else {
        super::EXIT_OK
    }
}

/// Render `report` as a String in the requested `format` (PURE — no stdout).
///
/// `"json"` serializes the library's `LintReport` directly (no parallel DTO —
/// D-09); `"text"` renders one located line per finding. Reused by `compile.rs`
/// (Plan 94-03) so the lint phase inside compile shares this renderer.
///
/// # Errors
/// Returns an error for an unknown `format` (naming the valid `text`/`json`
/// values), or if JSON serialization fails.
pub fn format_lint_report(report: &LintReport, format: &str) -> Result<String> {
    match format {
        "json" => {
            serde_json::to_string_pretty(report).context("failed to serialize lint report to JSON")
        },
        "text" => Ok(render_text(report)),
        other => {
            anyhow::bail!("unknown --format `{other}` (expected `text` or `json`)")
        },
    }
}

/// Render the located, collect-all findings as human text (the BA surface, D-09):
/// `<severity> <sheet>!<cell> [<rule>]: <message> — fix: <repair>`, one per line.
fn render_text(report: &LintReport) -> String {
    if report.findings.is_empty() {
        return "no dialect findings".to_string();
    }
    let mut out = String::new();
    for f in &report.findings {
        let location = match &f.cell {
            Some(cell) => format!("{}!{}", f.sheet, cell),
            None => f.sheet.clone(),
        };
        out.push_str(&format!(
            "{} {} [{}]: {} — fix: {}\n",
            severity_label(f.severity),
            location,
            f.rule,
            f.message,
            f.repair
        ));
    }
    out
}

/// A short stable label for a severity tier (used in the text render).
fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

/// Thin stdout wrapper over [`format_lint_report`]: prints the rendered report to
/// stdout (the data channel) and gates a decorative header on `not_quiet`.
///
/// # Errors
/// Propagates an unknown-`format` error from [`format_lint_report`].
pub fn print_lint_report(report: &LintReport, format: &str, not_quiet: bool) -> Result<()> {
    let rendered = format_lint_report(report, format)?;
    if not_quiet && format == "text" {
        eprintln!("dialect lint — {} finding(s)", report.findings.len());
    }
    println!("{}", rendered);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_workbook_compiler::LintFinding;

    fn error_report() -> LintReport {
        let mut report = LintReport::new();
        report.push(LintFinding::new(
            Severity::Error,
            "whitelist/unsupported-fn",
            "1_Inputs",
            Some("A2".to_string()),
            "OFFSET is not in the dialect whitelist",
            "Replace OFFSET with an INDEX/MATCH lookup",
        ));
        report
    }

    fn warnings_only_report() -> LintReport {
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
        report
    }

    #[test]
    fn lint_exit_code_is_error_when_report_has_errors() {
        assert_eq!(lint_exit_code(&error_report()), super::super::EXIT_ERROR);
    }

    #[test]
    fn lint_exit_code_is_ok_for_warnings_only() {
        assert_eq!(
            lint_exit_code(&warnings_only_report()),
            super::super::EXIT_OK
        );
    }

    #[test]
    fn lint_exit_code_is_ok_for_empty_report() {
        assert_eq!(lint_exit_code(&LintReport::new()), super::super::EXIT_OK);
    }

    #[test]
    fn format_json_round_trips_back_to_lint_report() {
        let report = error_report();
        let json = format_lint_report(&report, "json").expect("json render");
        let back: LintReport = serde_json::from_str(&json).expect("deserialize back");
        assert_eq!(back.findings.len(), report.findings.len());
        assert!(back.has_errors());
        assert_eq!(back.findings[0].rule, "whitelist/unsupported-fn");
        assert_eq!(back.findings[0].cell.as_deref(), Some("A2"));
    }

    #[test]
    fn format_text_renders_located_findings_with_repair() {
        let report = error_report();
        let text = format_lint_report(&report, "text").expect("text render");
        assert!(text.contains("error"));
        assert!(text.contains("1_Inputs!A2"));
        assert!(text.contains("whitelist/unsupported-fn"));
        assert!(text.contains("fix: Replace OFFSET"));
    }

    #[test]
    fn format_text_renders_sheet_level_finding_without_cell() {
        let mut report = LintReport::new();
        report.push(LintFinding::new(
            Severity::Error,
            "structure/hidden-sheet",
            "9_Hidden",
            None,
            "sheet is hidden",
            "Unhide the sheet",
        ));
        let text = format_lint_report(&report, "text").expect("text render");
        // No `!cell` suffix for a sheet-level finding.
        assert!(text.contains("error 9_Hidden ["));
        assert!(!text.contains("9_Hidden!"));
    }

    #[test]
    fn format_text_empty_report_says_no_findings() {
        let text = format_lint_report(&LintReport::new(), "text").expect("text render");
        assert_eq!(text, "no dialect findings");
    }

    #[test]
    fn format_unknown_errors_naming_valid_formats() {
        let err = format_lint_report(&error_report(), "yaml").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("text"), "got: {msg}");
        assert!(msg.contains("json"), "got: {msg}");
    }
}
