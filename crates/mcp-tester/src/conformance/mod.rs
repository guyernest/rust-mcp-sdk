//! MCP Protocol Conformance Test Suite
//!
//! Validates any MCP server against the MCP protocol spec (2025-11-25).
//! Scenarios are grouped by domain: Core, Tools, Resources, Prompts, Tasks.
//! Each domain reports independently -- a server with no resources passes
//! if it correctly reports empty capabilities.

pub(crate) mod core_domain;
pub(crate) mod prompts;
pub(crate) mod resources;
pub(crate) mod tasks;
pub(crate) mod tools;

use crate::report::TestReport;
use crate::tester::ServerTester;
use std::time::Instant;

/// MCP protocol domain for conformance filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConformanceDomain {
    Core,
    Tools,
    Resources,
    Prompts,
    Tasks,
}

impl ConformanceDomain {
    /// Parse domain name from string (case-insensitive).
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "core" => Some(Self::Core),
            "tools" => Some(Self::Tools),
            "resources" => Some(Self::Resources),
            "prompts" => Some(Self::Prompts),
            "tasks" => Some(Self::Tasks),
            _ => None,
        }
    }

    /// All domains in execution order.
    pub fn all() -> Vec<Self> {
        vec![
            Self::Core,
            Self::Tools,
            Self::Resources,
            Self::Prompts,
            Self::Tasks,
        ]
    }
}

/// Orchestrates conformance test execution across domains.
pub struct ConformanceRunner {
    strict: bool,
    domains: Option<Vec<ConformanceDomain>>,
}

impl ConformanceRunner {
    /// Create a new runner. If `domains` is None, all domains run.
    pub fn new(strict: bool, domains: Option<Vec<ConformanceDomain>>) -> Self {
        Self { strict, domains }
    }

    /// Run conformance tests against the server.
    /// Core domain always runs first (handles initialization).
    /// Other domains are skipped if core fails or if domain is filtered out.
    pub async fn run(&self, tester: &mut ServerTester) -> TestReport {
        let mut report = TestReport::new();
        let start = Instant::now();

        // Core always runs first -- it initializes the server connection
        if self.should_run(ConformanceDomain::Core) {
            for result in core_domain::run_core_conformance(tester).await {
                report.add_test(result);
            }
        }

        // Only proceed with other domains if core didn't fail
        if !report.has_failures() {
            if self.should_run(ConformanceDomain::Tools) {
                for result in tools::run_tools_conformance(tester).await {
                    report.add_test(result);
                }
            }

            if self.should_run(ConformanceDomain::Resources) {
                for result in resources::run_resources_conformance(tester).await {
                    report.add_test(result);
                }
            }

            if self.should_run(ConformanceDomain::Prompts) {
                for result in prompts::run_prompts_conformance(tester).await {
                    report.add_test(result);
                }
            }

            if self.should_run(ConformanceDomain::Tasks) {
                for result in tasks::run_tasks_conformance(tester).await {
                    report.add_test(result);
                }
            }
        }

        if self.strict {
            report.apply_strict_mode();
        }

        report.duration = start.elapsed();
        report
    }

    fn should_run(&self, domain: ConformanceDomain) -> bool {
        self.domains
            .as_ref()
            .map_or(true, |d| d.contains(&domain))
    }
}
