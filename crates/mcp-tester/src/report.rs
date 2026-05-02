use chrono::{DateTime, Utc};
use clap::ValueEnum;
use colored::*;
use prettytable::{row, Table};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::time::Duration;

/// Expand `[guide:SLUG]` tokens in `details` strings to absolute URLs into
/// `src/server/mcp_apps/GUIDE.md`. Unknown slugs are left in place.
///
/// Used by the pretty printer to turn validator-emitted anchor tokens into
/// clickable GitHub URLs at print time. The slug list mirrors the Phase 78
/// anchor-token contract — each entry corresponds to an HTML id anchor in
/// `src/server/mcp_apps/GUIDE.md`.
///
/// Per Phase 78 Plan 04 acceptance criteria, this function is `pub` so
/// integration tests can call it directly.
pub fn expand_guide_anchor(details: &str) -> String {
    const KNOWN_SLUGS: &[&str] = &[
        "handlers-before-connect",
        "do-not-pass-tools",
        "csp-external-resources",
        "vite-singlefile",
        "common-failures-claude",
    ];
    const URL_PREFIX: &str =
        "https://github.com/paiml/rust-mcp-sdk/blob/main/src/server/mcp_apps/GUIDE.md#";
    let mut out = details.to_string();
    for slug in KNOWN_SLUGS {
        let token = format!("[guide:{slug}]");
        let url = format!("{URL_PREFIX}{slug}");
        out = out.replace(&token, &url);
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, ValueEnum, Serialize, Deserialize)]
pub enum OutputFormat {
    Pretty,
    Json,
    Minimal,
    Verbose,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TestStatus {
    Passed,
    Failed,
    Warning,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestCategory {
    Core,
    /// HTTP transport-surface conformance (GET/OPTIONS/DELETE on the MCP endpoint).
    ///
    /// Distinct from the JSON-RPC-over-POST `Core` domain: catches Streamable-HTTP
    /// misconfigurations a POST-only suite cannot see (e.g. `GET /mcp` rewritten
    /// to a JSON health endpoint by a reverse proxy or edge function).
    Transport,
    Protocol,
    Tools,
    Resources,
    Prompts,
    Performance,
    Compatibility,
    Apps,
    Tasks,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub name: String,
    pub category: TestCategory,
    pub status: TestStatus,
    pub duration: Duration,
    pub error: Option<String>,
    pub details: Option<String>,
}

impl TestResult {
    /// Create a passing test result.
    pub fn passed(
        name: impl Into<String>,
        category: TestCategory,
        duration: Duration,
        details: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            category,
            status: TestStatus::Passed,
            duration,
            error: None,
            details: Some(details.into()),
        }
    }

    /// Create a failing test result.
    pub fn failed(
        name: impl Into<String>,
        category: TestCategory,
        duration: Duration,
        error: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            category,
            status: TestStatus::Failed,
            duration,
            error: Some(error.into()),
            details: None,
        }
    }

    /// Create a warning test result.
    pub fn warning(
        name: impl Into<String>,
        category: TestCategory,
        duration: Duration,
        details: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            category,
            status: TestStatus::Warning,
            duration,
            error: None,
            details: Some(details.into()),
        }
    }

    /// Create a skipped test result.
    pub fn skipped(
        name: impl Into<String>,
        category: TestCategory,
        details: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            category,
            status: TestStatus::Skipped,
            duration: Duration::from_secs(0),
            error: None,
            details: Some(details.into()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestReport {
    pub tests: Vec<TestResult>,
    pub duration: Duration,
    pub timestamp: DateTime<Utc>,
    pub summary: TestSummary,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub warnings: usize,
    pub skipped: usize,
}

impl Default for TestReport {
    fn default() -> Self {
        Self {
            tests: Vec::new(),
            duration: Duration::from_secs(0),
            timestamp: Utc::now(),
            summary: TestSummary {
                total: 0,
                passed: 0,
                failed: 0,
                warnings: 0,
                skipped: 0,
            },
        }
    }
}

impl TestReport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_error(error: anyhow::Error) -> Self {
        let mut report = Self::new();
        report.add_test(TestResult {
            name: "Error".to_string(),
            category: TestCategory::Core,
            status: TestStatus::Failed,
            duration: Duration::from_secs(0),
            error: Some(error.to_string()),
            details: None,
        });
        report
    }

    pub fn add_test(&mut self, test: TestResult) {
        match test.status {
            TestStatus::Passed => self.summary.passed += 1,
            TestStatus::Failed => self.summary.failed += 1,
            TestStatus::Warning => self.summary.warnings += 1,
            TestStatus::Skipped => self.summary.skipped += 1,
        }
        self.summary.total += 1;
        self.tests.push(test);
    }

    pub fn has_failures(&self) -> bool {
        self.summary.failed > 0
    }

    pub fn apply_strict_mode(&mut self) {
        // In strict mode, warnings become failures
        for test in &mut self.tests {
            if test.status == TestStatus::Warning {
                test.status = TestStatus::Failed;
                self.summary.warnings -= 1;
                self.summary.failed += 1;
            }
        }
    }

    pub fn print(&self, format: OutputFormat) {
        let mut stdout = std::io::stdout();
        // Best-effort write to stdout: ignore I/O errors here because the CLI
        // entry point cannot do anything meaningful with a broken-pipe error
        // at the report layer. Tests use `print_to_writer` to capture output.
        let _ = self.print_to_writer(format, &mut stdout);
    }

    /// Writer-seam helper: render the report into any `std::io::Write` sink.
    ///
    /// Phase 78 Plan 04 (Codex MEDIUM): the existing `print` path wrote
    /// directly to stdout via `println!`, which made it impossible for tests
    /// to assert the printed bytes. This helper accepts any writer so tests
    /// can capture into `Vec<u8>` and assert on the content.
    pub fn print_to_writer<W: Write>(
        &self,
        format: OutputFormat,
        w: &mut W,
    ) -> std::io::Result<()> {
        match format {
            OutputFormat::Pretty => self.print_pretty(w),
            OutputFormat::Json => self.print_json(w),
            OutputFormat::Minimal => self.print_minimal(w),
            OutputFormat::Verbose => self.print_verbose(w),
        }
    }

    fn print_pretty<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        writeln!(w)?;
        writeln!(w, "{}", "TEST RESULTS".cyan().bold())?;
        writeln!(w, "{}", "═".repeat(60).cyan())?;
        writeln!(w)?;

        // Group tests by category
        let mut by_category: std::collections::HashMap<String, Vec<&TestResult>> =
            std::collections::HashMap::new();

        for test in &self.tests {
            let category = format!("{:?}", test.category);
            by_category.entry(category).or_default().push(test);
        }

        // Print each category
        for (category, tests) in by_category {
            writeln!(w, "{}", format!("{}:", category).yellow().bold())?;
            writeln!(w)?;

            for test in tests {
                self.print_test_result_pretty(w, test)?;
            }
            writeln!(w)?;
        }

        // Print summary
        self.print_summary_pretty(w)?;

        // Print recommendations if there are failures
        if self.has_failures() {
            self.print_recommendations(w)?;
        }
        Ok(())
    }

    fn print_test_result_pretty<W: Write>(
        &self,
        w: &mut W,
        test: &TestResult,
    ) -> std::io::Result<()> {
        let status_symbol = match test.status {
            TestStatus::Passed => "✓".green().bold(),
            TestStatus::Failed => "✗".red().bold(),
            TestStatus::Warning => "⚠".yellow().bold(),
            TestStatus::Skipped => "○".dimmed(),
        };

        let name = if test.name.len() > 40 {
            format!("{}...", &test.name[..37])
        } else {
            test.name.clone()
        };

        write!(w, "  {} {:<40}", status_symbol, name)?;

        // Print duration if significant
        if test.duration.as_millis() > 100 {
            write!(w, " {:>6}ms", test.duration.as_millis())?;
        } else {
            write!(w, "         ")?;
        }

        // Print details or error. Phase 78 Plan 04: expand `[guide:SLUG]`
        // tokens in `details` strings to absolute GUIDE.md URLs at print
        // time so error messages link to actionable documentation.
        if let Some(error) = &test.error {
            writeln!(w, " {}", error.red())?;
        } else if let Some(details) = &test.details {
            let expanded = expand_guide_anchor(details);
            if test.status == TestStatus::Warning {
                writeln!(w, " {}", expanded.yellow())?;
            } else {
                writeln!(w, " {}", expanded.dimmed())?;
            }
        } else {
            writeln!(w)?;
        }
        Ok(())
    }

    fn print_summary_pretty<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        writeln!(w, "{}", "═".repeat(60).cyan())?;
        writeln!(w, "{}", "SUMMARY".cyan().bold())?;
        writeln!(w, "{}", "═".repeat(60).cyan())?;
        writeln!(w)?;

        let mut table = Table::new();
        table.add_row(row!["Total Tests", self.summary.total.to_string().bold()]);
        table.add_row(row![
            "Passed",
            self.summary.passed.to_string().green().bold()
        ]);

        if self.summary.failed > 0 {
            table.add_row(row!["Failed", self.summary.failed.to_string().red().bold()]);
        }

        if self.summary.warnings > 0 {
            table.add_row(row![
                "Warnings",
                self.summary.warnings.to_string().yellow().bold()
            ]);
        }

        if self.summary.skipped > 0 {
            table.add_row(row!["Skipped", self.summary.skipped.to_string().dimmed()]);
        }

        table.add_row(row![
            "Duration",
            format!("{:.2}s", self.duration.as_secs_f64())
        ]);

        table.print(w)?;
        writeln!(w)?;

        // Overall status
        let overall = if self.summary.failed > 0 {
            "FAILED".red().bold()
        } else if self.summary.warnings > 0 {
            "PASSED WITH WARNINGS".yellow().bold()
        } else {
            "PASSED".green().bold()
        };

        writeln!(w, "Overall Status: {}", overall)?;
        Ok(())
    }

    fn print_recommendations<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        writeln!(w)?;
        writeln!(w, "{}", "RECOMMENDATIONS".yellow().bold())?;
        writeln!(w, "{}", "═".repeat(60).yellow())?;
        writeln!(w)?;

        let failed_tests: Vec<_> = self
            .tests
            .iter()
            .filter(|t| t.status == TestStatus::Failed)
            .collect();

        if failed_tests.is_empty() {
            return Ok(());
        }

        // Group failures by category
        let mut protocol_failures = 0;
        let mut tool_failures = 0;
        let mut core_failures = 0;
        let mut task_failures = 0;

        for test in &failed_tests {
            match test.category {
                TestCategory::Protocol => protocol_failures += 1,
                TestCategory::Tools => tool_failures += 1,
                TestCategory::Core => core_failures += 1,
                TestCategory::Tasks => task_failures += 1,
                _ => {},
            }
        }

        if core_failures > 0 {
            writeln!(w, "  • Fix core connectivity issues first")?;
            writeln!(w, "    - Verify server is running and accessible")?;
            writeln!(w, "    - Check network configuration and firewall rules")?;
        }

        if protocol_failures > 0 {
            writeln!(w, "  • Review MCP protocol implementation")?;
            writeln!(w, "    - Ensure JSON-RPC 2.0 compliance")?;
            writeln!(w, "    - Verify protocol version compatibility")?;
            writeln!(w, "    - Check required method implementations")?;
        }

        if tool_failures > 0 {
            writeln!(w, "  • Debug tool implementations")?;
            writeln!(w, "    - Verify tool registration and handlers")?;
            writeln!(w, "    - Check input validation and error handling")?;
            writeln!(w, "    - Review tool response formats")?;
        }

        if task_failures > 0 {
            writeln!(w, "  - Debug task implementations")?;
            writeln!(
                w,
                "    - Verify task capability is advertised in ServerCapabilities"
            )?;
            writeln!(
                w,
                "    - Check task lifecycle state machine (working -> completed/failed)"
            )?;
            writeln!(
                w,
                "    - Ensure tasks/get and tasks/list return valid Task structures"
            )?;
        }

        writeln!(w)?;
        writeln!(w, "Run with --verbose for detailed error information")?;
        Ok(())
    }

    fn print_json<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self).unwrap();
        writeln!(w, "{}", json)
    }

    fn print_minimal<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        let status = if self.summary.failed > 0 {
            "FAIL"
        } else {
            "PASS"
        };

        writeln!(
            w,
            "{}: {} passed, {} failed, {} warnings in {:.2}s",
            status,
            self.summary.passed,
            self.summary.failed,
            self.summary.warnings,
            self.duration.as_secs_f64()
        )
    }

    fn print_verbose<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        self.print_pretty(w)?;

        writeln!(w)?;
        writeln!(w, "{}", "DETAILED TEST INFORMATION".cyan().bold())?;
        writeln!(w, "{}", "═".repeat(60).cyan())?;
        writeln!(w)?;

        for test in &self.tests {
            writeln!(w, "Test: {}", test.name.bold())?;
            writeln!(w, "  Category: {:?}", test.category)?;
            writeln!(w, "  Status: {:?}", test.status)?;
            writeln!(w, "  Duration: {:?}", test.duration)?;

            if let Some(error) = &test.error {
                writeln!(w, "  Error: {}", error.red())?;
            }

            if let Some(details) = &test.details {
                // Phase 78 Plan 04: also expand in verbose mode for consistency.
                let expanded = expand_guide_anchor(details);
                writeln!(w, "  Details: {}", expanded)?;
            }

            writeln!(w)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Transport variant must exist and round-trip through serde JSON unchanged.
    #[test]
    fn transport_category_serde_roundtrip() {
        let original = TestCategory::Transport;
        let json = serde_json::to_string(&original).expect("serialize");
        assert_eq!(json, "\"Transport\"");
        let parsed: TestCategory = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, TestCategory::Transport);
    }

    /// Transport variant participates in equality / clone / debug like its siblings.
    #[test]
    fn transport_category_traits() {
        let a = TestCategory::Transport;
        let b = a.clone();
        assert_eq!(a, b);
        assert_ne!(a, TestCategory::Core);
        // Debug must produce the variant name (used by print_pretty grouping).
        assert_eq!(format!("{:?}", a), "Transport");
    }

    /// A TestResult tagged Transport must aggregate correctly in the summary counters.
    #[test]
    fn transport_results_aggregate_in_summary() {
        let mut report = TestReport::new();
        report.add_test(TestResult::passed(
            "Transport: GET /mcp",
            TestCategory::Transport,
            Duration::from_millis(10),
            "ok",
        ));
        report.add_test(TestResult::failed(
            "Transport: OPTIONS /mcp",
            TestCategory::Transport,
            Duration::from_millis(10),
            "boom",
        ));
        report.add_test(TestResult::warning(
            "Transport: DELETE /mcp",
            TestCategory::Transport,
            Duration::from_millis(10),
            "warn",
        ));
        assert_eq!(report.summary.total, 3);
        assert_eq!(report.summary.passed, 1);
        assert_eq!(report.summary.failed, 1);
        assert_eq!(report.summary.warnings, 1);
        assert!(report.has_failures());
    }

    // -----------------------------------------------------------------------
    // Phase 78 Plan 04 Task 1: expand_guide_anchor unit tests
    // -----------------------------------------------------------------------

    /// Known slug `handlers-before-connect` is replaced with the canonical URL.
    #[test]
    fn expand_guide_anchor_handlers_before_connect() {
        let out = expand_guide_anchor("Missing handler [guide:handlers-before-connect]");
        assert_eq!(
            out,
            "Missing handler https://github.com/paiml/rust-mcp-sdk/blob/main/src/server/mcp_apps/GUIDE.md#handlers-before-connect"
        );
    }

    /// A string with no token round-trips unchanged.
    #[test]
    fn expand_guide_anchor_no_token() {
        let out = expand_guide_anchor("plain text");
        assert_eq!(out, "plain text");
    }

    /// Unknown slugs are left in place (NOT silently dropped).
    #[test]
    fn expand_guide_anchor_unknown_slug() {
        let input = "see [guide:not-a-real-slug] for details";
        let out = expand_guide_anchor(input);
        assert_eq!(out, input, "unknown slugs must be left in place");
    }

    /// Both known tokens in a single string are fully expanded.
    #[test]
    fn expand_guide_anchor_multiple_tokens() {
        let input = "First [guide:handlers-before-connect], then [guide:common-failures-claude].";
        let out = expand_guide_anchor(input);
        assert!(
            out.contains(
                "https://github.com/paiml/rust-mcp-sdk/blob/main/src/server/mcp_apps/GUIDE.md#handlers-before-connect"
            ),
            "first token must expand; got: {}",
            out
        );
        assert!(
            out.contains(
                "https://github.com/paiml/rust-mcp-sdk/blob/main/src/server/mcp_apps/GUIDE.md#common-failures-claude"
            ),
            "second token must expand; got: {}",
            out
        );
        assert!(
            !out.contains("[guide:"),
            "no [guide:...] tokens must remain; got: {}",
            out
        );
    }

    /// `[guide:common-failures-claude]` expands to a URL ending with `#common-failures-claude`.
    #[test]
    fn expand_guide_anchor_common_failures() {
        let out = expand_guide_anchor("[guide:common-failures-claude]");
        assert!(
            out.ends_with("#common-failures-claude"),
            "expected URL ending with #common-failures-claude; got: {}",
            out
        );
        assert!(
            out.starts_with("https://"),
            "expected absolute URL; got: {}",
            out
        );
    }

    /// REVISION (Codex MEDIUM): the printer wiring itself must produce the
    /// expanded URL, not just the helper. Build a TestReport, render via
    /// `print_to_writer` to a Vec<u8>, then assert on the captured bytes.
    #[test]
    fn pretty_output_includes_expanded_url() {
        // colored output adds ANSI escape sequences which may interfere with
        // substring matching. Disable colorization for this test.
        colored::control::set_override(false);

        let mut report = TestReport::new();
        report.add_test(TestResult {
            name: "[example] handler: onteardown".to_string(),
            category: TestCategory::Apps,
            status: TestStatus::Failed,
            duration: Duration::from_secs(0),
            error: None,
            details: Some(
                "Widget does not register onteardown. [guide:handlers-before-connect]".to_string(),
            ),
        });
        let mut buf: Vec<u8> = Vec::new();
        report
            .print_to_writer(OutputFormat::Pretty, &mut buf)
            .expect("write to Vec<u8> should not fail");
        let captured = String::from_utf8_lossy(&buf);
        assert!(
            captured.contains(
                "https://github.com/paiml/rust-mcp-sdk/blob/main/src/server/mcp_apps/GUIDE.md#handlers-before-connect"
            ),
            "pretty output must contain the expanded URL; got:\n{}",
            captured
        );
        assert!(
            !captured.contains("[guide:handlers-before-connect]"),
            "pretty output must not contain the unexpanded token; got:\n{}",
            captured
        );

        // Restore default colorization behavior.
        colored::control::unset_override();
    }
}
