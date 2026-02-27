use chrono::{DateTime, Utc};
use clap::ValueEnum;
use colored::*;
use prettytable::{row, Table};
use serde::{Deserialize, Serialize};
use std::time::Duration;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestCategory {
    Core,
    Protocol,
    Tools,
    Resources,
    Prompts,
    Performance,
    Compatibility,
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
        match format {
            OutputFormat::Pretty => self.print_pretty(),
            OutputFormat::Json => self.print_json(),
            OutputFormat::Minimal => self.print_minimal(),
            OutputFormat::Verbose => self.print_verbose(),
        }
    }

    fn print_pretty(&self) {
        println!();
        println!("{}", "TEST RESULTS".cyan().bold());
        println!("{}", "═".repeat(60).cyan());
        println!();

        // Group tests by category
        let mut by_category: std::collections::HashMap<String, Vec<&TestResult>> =
            std::collections::HashMap::new();

        for test in &self.tests {
            let category = format!("{:?}", test.category);
            by_category.entry(category).or_default().push(test);
        }

        // Print each category
        for (category, tests) in by_category {
            println!("{}", format!("{}:", category).yellow().bold());
            println!();

            for test in tests {
                self.print_test_result_pretty(test);
            }
            println!();
        }

        // Print summary
        self.print_summary_pretty();

        // Print recommendations if there are failures
        if self.has_failures() {
            self.print_recommendations();
        }
    }

    fn print_test_result_pretty(&self, test: &TestResult) {
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

        print!("  {} {:<40}", status_symbol, name);

        // Print duration if significant
        if test.duration.as_millis() > 100 {
            print!(" {:>6}ms", test.duration.as_millis());
        } else {
            print!("         ");
        }

        // Print details or error
        if let Some(error) = &test.error {
            println!(" {}", error.red());
        } else if let Some(details) = &test.details {
            if test.status == TestStatus::Warning {
                println!(" {}", details.yellow());
            } else {
                println!(" {}", details.dimmed());
            }
        } else {
            println!();
        }
    }

    fn print_summary_pretty(&self) {
        println!("{}", "═".repeat(60).cyan());
        println!("{}", "SUMMARY".cyan().bold());
        println!("{}", "═".repeat(60).cyan());
        println!();

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

        table.printstd();
        println!();

        // Overall status
        let overall = if self.summary.failed > 0 {
            "FAILED".red().bold()
        } else if self.summary.warnings > 0 {
            "PASSED WITH WARNINGS".yellow().bold()
        } else {
            "PASSED".green().bold()
        };

        println!("Overall Status: {}", overall);
    }

    fn print_recommendations(&self) {
        println!();
        println!("{}", "RECOMMENDATIONS".yellow().bold());
        println!("{}", "═".repeat(60).yellow());
        println!();

        let failed_tests: Vec<_> = self
            .tests
            .iter()
            .filter(|t| t.status == TestStatus::Failed)
            .collect();

        if failed_tests.is_empty() {
            return;
        }

        // Group failures by category
        let mut protocol_failures = 0;
        let mut tool_failures = 0;
        let mut core_failures = 0;

        for test in &failed_tests {
            match test.category {
                TestCategory::Protocol => protocol_failures += 1,
                TestCategory::Tools => tool_failures += 1,
                TestCategory::Core => core_failures += 1,
                _ => {},
            }
        }

        if core_failures > 0 {
            println!("  • Fix core connectivity issues first");
            println!("    - Verify server is running and accessible");
            println!("    - Check network configuration and firewall rules");
        }

        if protocol_failures > 0 {
            println!("  • Review MCP protocol implementation");
            println!("    - Ensure JSON-RPC 2.0 compliance");
            println!("    - Verify protocol version compatibility");
            println!("    - Check required method implementations");
        }

        if tool_failures > 0 {
            println!("  • Debug tool implementations");
            println!("    - Verify tool registration and handlers");
            println!("    - Check input validation and error handling");
            println!("    - Review tool response formats");
        }

        println!();
        println!("Run with --verbose for detailed error information");
    }

    fn print_json(&self) {
        let json = serde_json::to_string_pretty(self).unwrap();
        println!("{}", json);
    }

    fn print_minimal(&self) {
        let status = if self.summary.failed > 0 {
            "FAIL"
        } else {
            "PASS"
        };

        println!(
            "{}: {} passed, {} failed, {} warnings in {:.2}s",
            status,
            self.summary.passed,
            self.summary.failed,
            self.summary.warnings,
            self.duration.as_secs_f64()
        );
    }

    fn print_verbose(&self) {
        self.print_pretty();

        println!();
        println!("{}", "DETAILED TEST INFORMATION".cyan().bold());
        println!("{}", "═".repeat(60).cyan());
        println!();

        for test in &self.tests {
            println!("Test: {}", test.name.bold());
            println!("  Category: {:?}", test.category);
            println!("  Status: {:?}", test.status);
            println!("  Duration: {:?}", test.duration);

            if let Some(error) = &test.error {
                println!("  Error: {}", error.red());
            }

            if let Some(details) = &test.details {
                println!("  Details: {}", details);
            }

            println!();
        }
    }
}
