//! Phase 76 Wave 1 — golden-file backward-compat invariants for `stack.ts`.
//!
//! **What this file does:** reads the committed golden files at
//! `cargo-pmcp/tests/golden/pmcp-run-empty.ts` and
//! `cargo-pmcp/tests/golden/aws-lambda-empty.ts` and asserts the
//! invariants Phase 76 Part 1 introduced:
//!
//! - Both goldens contain a `new cdk.CfnOutput(this, 'McpRoleArn', ...)` block
//!   with `value: mcpFunction.role!.roleArn` (D-01).
//! - pmcp-run golden uses `exportName: pmcp-${serverId}-McpRoleArn`;
//!   aws-lambda golden uses `exportName: pmcp-${serverName}-McpRoleArn`.
//! - aws-lambda golden imports `aws-cdk-lib/aws-iam` (D-03).
//! - pmcp-run golden has `McpRoleArn` positioned AFTER `DashboardUrl`.
//!
//! **Why this split exists:** the byte-identical comparison lives in-crate
//! inside `src/commands/deploy/init.rs`'s `wave1_stack_ts_tests` module
//! (the `golden_*` tests, gated on `UPDATE_GOLDEN=1` for regeneration).
//! That is the only place where `InitCommand::render_stack_ts` is reachable,
//! because `InitCommand`'s fields and `render_stack_ts` itself sit behind a
//! bin-only module tree that `cargo-pmcp/src/lib.rs` intentionally does not
//! re-export (see `lib.rs` — only `loadtest`/`pentest`/`test_support` are
//! exposed). This integration-test file complements that by checking the
//! committed goldens from the outside so drift is caught even if someone
//! deletes the in-crate tests.
//!
//! Regenerate goldens:
//!   `UPDATE_GOLDEN=1 cargo test -p cargo-pmcp -- golden`

use std::path::PathBuf;

fn golden_path(filename: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(filename)
}

fn read_golden(filename: &str) -> String {
    let path = golden_path(filename);
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "missing golden {}: {e}\nRegenerate with:\n\
             \tUPDATE_GOLDEN=1 cargo test -p cargo-pmcp -- golden",
            path.display()
        )
    })
}

#[test]
fn pmcp_run_golden_contains_mcp_role_arn_cfn_output() {
    let ts = read_golden("pmcp-run-empty.ts");
    assert!(
        ts.contains("new cdk.CfnOutput(this, 'McpRoleArn', {"),
        "pmcp-run golden missing McpRoleArn CfnOutput"
    );
    assert!(
        ts.contains("value: mcpFunction.role!.roleArn"),
        "pmcp-run golden missing mcpFunction.role!.roleArn (D-01)"
    );
    assert!(
        ts.contains("exportName: `pmcp-${serverId}-McpRoleArn`"),
        "pmcp-run golden exportName must use serverId interpolation"
    );
}

#[test]
fn aws_lambda_golden_contains_mcp_role_arn_cfn_output() {
    let ts = read_golden("aws-lambda-empty.ts");
    assert!(
        ts.contains("new cdk.CfnOutput(this, 'McpRoleArn', {"),
        "aws-lambda golden missing McpRoleArn CfnOutput"
    );
    assert!(
        ts.contains("value: mcpFunction.role!.roleArn"),
        "aws-lambda golden missing mcpFunction.role!.roleArn (D-01)"
    );
    assert!(
        ts.contains("exportName: `pmcp-${serverName}-McpRoleArn`"),
        "aws-lambda golden exportName must use serverName interpolation"
    );
}

#[test]
fn aws_lambda_golden_imports_aws_iam_module() {
    let ts = read_golden("aws-lambda-empty.ts");
    assert!(
        ts.contains("import * as iam from 'aws-cdk-lib/aws-iam';"),
        "aws-lambda golden missing `import * as iam from 'aws-cdk-lib/aws-iam';` (D-03)"
    );
}

#[test]
fn pmcp_run_golden_places_role_output_after_dashboard_url() {
    // Ordering regression guard: if future waves reposition the role output,
    // the golden diff stays minimal (and this catches reorderings first).
    let ts = read_golden("pmcp-run-empty.ts");
    let dashboard_idx = ts
        .find("'DashboardUrl'")
        .expect("pmcp-run golden must contain DashboardUrl");
    let role_idx = ts
        .find("'McpRoleArn'")
        .expect("pmcp-run golden must contain McpRoleArn");
    assert!(
        role_idx > dashboard_idx,
        "McpRoleArn must be emitted AFTER DashboardUrl in pmcp-run golden"
    );
}

#[test]
fn goldens_exist_for_both_targets() {
    // Smoke test: a missing golden file fails with a clear UPDATE_GOLDEN hint.
    for filename in &["pmcp-run-empty.ts", "aws-lambda-empty.ts"] {
        let path = golden_path(filename);
        assert!(
            path.exists(),
            "missing golden {} — regenerate with:\n\
             \tUPDATE_GOLDEN=1 cargo test -p cargo-pmcp -- golden",
            path.display()
        );
    }
}
