//! Example: Declaring IAM needs in `.pmcp/deploy.toml`.
//!
//! Demonstrates the end-to-end Phase 76 workflow:
//!   1. Parse a cost-coach-shaped deploy.toml with an [iam] section.
//!   2. Validate it (catches footguns like Allow `*:*` on `*`).
//!   3. Render the TypeScript addToRolePolicy block that cargo-pmcp emits
//!      into the generated CDK stack.
//!
//! Also shows the validator rejecting an Allow-*-* configuration (T-76-02
//! wildcard escalation footgun).
//!
//! Run with:
//!     cargo run -p cargo-pmcp --example deploy_with_iam

use cargo_pmcp::deployment::config::DeployConfig;
use cargo_pmcp::deployment::iam::{render_iam_block, validate};

const COST_COACH_TOML: &str = include_str!("fixtures/cost-coach.deploy.toml");

const INVALID_TOML: &str = r#"
[target]
type = "pmcp-run"
version = "1.0"

[aws]
region = "us-west-2"

[server]
name = "dangerous-server"
memory_mb = 512
timeout_seconds = 30

[environment]
RUST_LOG = "info"

[auth]
enabled = false
provider = "none"

[observability]
log_retention_days = 30
enable_xray = true
create_dashboard = true

[[iam.statements]]
effect = "Allow"
actions = ["*"]
resources = ["*"]
"#;

fn main() {
    println!("=== Phase 76 — Declare IAM in .pmcp/deploy.toml ===\n");

    println!("--- 1. A valid cost-coach-shaped deploy.toml ---\n");
    print_indented(COST_COACH_TOML);
    println!();

    let cfg: DeployConfig = toml::from_str(COST_COACH_TOML)
        .expect("fixture parses — Wave 2 test would have caught a regression");

    println!("--- 2. Parsed IamConfig ---");
    println!(
        "  tables: {} ({})\n  buckets: {} ({})\n  statements: {} ({})\n",
        cfg.iam.tables.len(),
        cfg.iam
            .tables
            .iter()
            .map(|t| t.name.clone())
            .collect::<Vec<_>>()
            .join(", "),
        cfg.iam.buckets.len(),
        cfg.iam
            .buckets
            .iter()
            .map(|b| b.name.clone())
            .collect::<Vec<_>>()
            .join(", "),
        cfg.iam.statements.len(),
        cfg.iam
            .statements
            .iter()
            .map(|s| s.effect.clone())
            .collect::<Vec<_>>()
            .join(", "),
    );

    println!("--- 3. Validating ---");
    match validate(&cfg.iam) {
        Ok(warnings) if warnings.is_empty() => {
            println!("  Valid (no warnings).\n");
        }
        Ok(warnings) => {
            println!("  Valid with {} warning(s):", warnings.len());
            for w in &warnings {
                println!("    warning: {}", w.message);
            }
            println!();
        }
        Err(e) => {
            eprintln!("  Validation failed: {e}");
            std::process::exit(1);
        }
    }

    println!("--- 4. Rendered TypeScript addToRolePolicy block ---");
    let ts = render_iam_block(&cfg.iam);
    print_indented(&ts);
    println!();

    println!("--- 5. Demonstrating validator rejects wildcard Allow ---\n");
    print_indented(INVALID_TOML);
    let bad_cfg: DeployConfig = toml::from_str(INVALID_TOML)
        .expect("wildcard toml parses at serde level — validator catches the semantic error");
    match validate(&bad_cfg.iam) {
        Ok(_) => {
            eprintln!("\nBUG: wildcard Allow accepted by validator — this should never happen");
            std::process::exit(2);
        }
        Err(e) => {
            println!("\n  Validator correctly rejected the invalid config:");
            println!("  {e}");
        }
    }

    println!("\n=== Example complete ===");
}

fn print_indented(s: &str) {
    for line in s.lines() {
        println!("  {line}");
    }
}
