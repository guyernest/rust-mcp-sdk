//! Allowlist-scoped CFN resource builders — one module per resource family.
//!
//! This is the landing zone for the renderer's per-family resource-building
//! logic. The v1 resource surface is EXACTLY seven families, matching the
//! design spec's §4 table: `lambda`, `iam`, `logs`, `http_api`, `cognito`,
//! `dynamodb`, `outputs`. A descriptor requesting anything outside this
//! surface must fail loudly via [`crate::RenderError::UnsupportedSection`] —
//! never a silent skip, and this module must never grow toward
//! CDK-completeness.
//!
//! Task 3 wired the plain-Lambda kernel: `lambda`, `logs`, `outputs`, plus
//! the BASE execution role/policy in `iam` (every `pmcp-run` server gets
//! one, regardless of any declared `[iam]` section — see that module's doc
//! comment). Task 4 extended `iam` with the declared-`[[iam.statements]]`
//! expansion (fail-closed validated first). `http_api`, `cognito`, and
//! `dynamodb` land in later tasks.

pub mod iam;
pub mod lambda;
pub mod logs;
pub mod outputs;

use std::collections::BTreeMap;

/// The shared pmcp.run DynamoDB table used for foundation-server discovery
/// (composition permissions + the `PMCP_ORGANIZATION_ID`/`PMCP_SERVER_ID`
/// env-var pair). Fixed today — the TS scaffold's `mcpServersTable`/
/// `organizationId` CDK-context overrides
/// (`cargo-pmcp/src/commands/deploy/init.rs`) have no descriptor-level
/// equivalent yet, so both are baked-in literal defaults shared by `lambda`
/// and `iam`.
pub(crate) const MCP_SERVERS_TABLE: &str = "McpServer";

/// Fallback organization id baked into every Lambda's environment when no
/// CDK-context override is supplied — mirrors the TS scaffold's
/// `process.env.PMCP_ORGANIZATION_ID || 'default-org'` fallback.
pub(crate) const DEFAULT_ORGANIZATION_ID: &str = "default-org";

/// Standard cost-allocation tags applied to every taggable resource in a
/// `pmcp-run` stack — mirrors `cdk.Tags.of(this).add(...)` in the pmcp-run
/// branch of `cargo-pmcp/src/commands/deploy/init.rs::render_stack_ts`.
/// `AWS::IAM::Policy` does not support the CloudFormation `Tags` property,
/// so callers rendering a `Policy` resource never attach this.
///
/// A `BTreeMap` sorts by key before the final `Vec` build, which is what
/// gives the emitted array its alphabetical `managed-by`/`project`/
/// `service`/`target` order (matching real `cdk synth`'s own alphabetical
/// tag emission) without a separate sort step.
#[must_use]
pub(crate) fn standard_tags(service: &str) -> serde_json::Value {
    let mut tags = BTreeMap::new();
    tags.insert("managed-by", "pmcp");
    tags.insert("project", "hosting");
    tags.insert("service", service);
    tags.insert("target", "pmcp-run");
    serde_json::Value::Array(
        tags.into_iter()
            .map(|(key, value)| serde_json::json!({ "Key": key, "Value": value }))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::standard_tags;
    use serde_json::json;

    #[test]
    fn standard_tags_are_sorted_alphabetically_by_key() {
        assert_eq!(
            standard_tags("my-server"),
            json!([
                { "Key": "managed-by", "Value": "pmcp" },
                { "Key": "project", "Value": "hosting" },
                { "Key": "service", "Value": "my-server" },
                { "Key": "target", "Value": "pmcp-run" },
            ])
        );
    }
}
