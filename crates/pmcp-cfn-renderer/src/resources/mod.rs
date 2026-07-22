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
//! expansion (fail-closed validated first). Task 5 added `http_api` (the
//! `aws-lambda` target's own HTTP API Gateway stack shape) plus
//! `aws-lambda`-flavored siblings of `lambda::render_function`,
//! `iam::render_execution_role`, and `logs::render_log_group` — the
//! `aws-lambda` target's Lambda/Role/Policy/LogGroup differ from the
//! `pmcp-run` kernel's (own tags, no composition IAM sugar, different fixed
//! memory size/log retention — see each `*_aws_lambda` function's doc
//! comment) even though both targets render the same 4 resource FAMILIES.
//! `cognito` and `dynamodb` land in a later task.

pub mod http_api;
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

/// Standard cost-allocation tags for the `aws-lambda` target's own stack
/// shape (array-of-`{Key,Value}` form — `AWS::Lambda::Function`,
/// `AWS::IAM::Role`, `AWS::Logs::LogGroup`). Unlike [`standard_tags`]'s
/// `pmcp-run` shape (`project = "hosting"`, the shared pmcp.run edge stack),
/// an `aws-lambda`-target stack is its own independently deployed unit —
/// `project` is the server's own name, and `target` is `"aws-lambda"` —
/// mirroring `cdk.Tags.of(this).add(...)` in the `aws-lambda` branch of
/// `cargo-pmcp/src/commands/deploy/init.rs::render_stack_ts`.
#[must_use]
pub(crate) fn aws_lambda_tags(service: &str) -> serde_json::Value {
    let mut tags = BTreeMap::new();
    tags.insert("managed-by", "pmcp");
    tags.insert("project", service);
    tags.insert("service", service);
    tags.insert("target", "aws-lambda");
    serde_json::Value::Array(
        tags.into_iter()
            .map(|(key, value)| serde_json::json!({ "Key": key, "Value": value }))
            .collect(),
    )
}

/// Same tag set as [`aws_lambda_tags`], but in the flat `Map` shape
/// (`{"key": "value"}`) that `AWS::ApiGatewayV2::Api`/`::Stage`'s `Tags`
/// property requires. CFN's stack-wide `cdk.Tags.of(this).add(...)`
/// propagation still applies the same 4 tags, but each resource TYPE
/// renders `Tags` in whatever shape its own CFN spec declares — API
/// Gateway v2 resources use the `Map` shape, not the `List of Tag` shape
/// every other taggable resource in this crate uses (see the `http-api`
/// golden's `Api-0`/`Stage-0` `Tags` vs its `Function-0`/`Role-0` `Tags`).
#[must_use]
pub(crate) fn aws_lambda_map_tags(service: &str) -> serde_json::Value {
    serde_json::json!({
        "managed-by": "pmcp",
        "project": service,
        "service": service,
        "target": "aws-lambda",
    })
}

#[cfg(test)]
mod tests {
    use super::{aws_lambda_map_tags, aws_lambda_tags, standard_tags};
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

    #[test]
    fn aws_lambda_tags_use_the_service_name_as_project() {
        assert_eq!(
            aws_lambda_tags("my-server"),
            json!([
                { "Key": "managed-by", "Value": "pmcp" },
                { "Key": "project", "Value": "my-server" },
                { "Key": "service", "Value": "my-server" },
                { "Key": "target", "Value": "aws-lambda" },
            ])
        );
    }

    #[test]
    fn aws_lambda_map_tags_is_the_flat_map_shape_of_the_same_tag_set() {
        assert_eq!(
            aws_lambda_map_tags("my-server"),
            json!({
                "managed-by": "pmcp",
                "project": "my-server",
                "service": "my-server",
                "target": "aws-lambda",
            })
        );
    }
}
