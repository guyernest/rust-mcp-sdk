//! `AWS::IAM::Role` + `AWS::IAM::Policy` rendering for the MCP server's
//! Lambda execution role.
//!
//! Task 3 ships only the BASE execution role every `pmcp-run` server gets —
//! the Lambda-basic-execution trust policy, plus the fixed
//! composition-support inline policy CDK always attaches (X-Ray tracing +
//! DynamoDB foundation-server discovery + cross-Lambda invoke). This
//! mirrors the `pmcp-run` branch of
//! `cargo-pmcp/src/commands/deploy/init.rs::render_stack_ts` verbatim
//! (`mcpFunction.addToRolePolicy(...)` x3, plus the L2 `lambda.Function`
//! construct's own default-role wiring) — NOT the general
//! `[iam]`-declared-statement expansion. That lands in Task 4, which
//! extends [`render_execution_role`]'s returned policy with additional
//! statements/resources rather than duplicating this module.
//!
//! CDK renders a Lambda's attached inline permissions as a SEPARATE
//! `AWS::IAM::Policy` resource (`McpFunction/ServiceRole/DefaultPolicy`),
//! never as a `Policies:` block inline on the role — [`render_execution_role`]
//! matches that shape (two resources, not one) because the semantic-golden
//! harness compares against real `cdk synth` output.

use crate::{
    logical_ids,
    params::RenderParams,
    resources::{standard_tags, MCP_SERVERS_TABLE},
    template::CfnResource,
};
use pmcp_package::package::DeployDescriptor;
use serde_json::json;

/// CDK's construct-path-hash-derived name for the Lambda's default inline
/// policy. CDK computes a resource's logical-ID hash from its construct
/// path RELATIVE TO THE STACK (the stack's own id is excluded — see
/// `Stack.allocateLogicalId` in aws-cdk-lib), so this value is identical
/// for every `pmcp-run` server regardless of server/stack name — verified
/// against the checked-in `plain-lambda` golden captured from a real `cdk
/// synth`. This renderer is otherwise hash-free by design (see
/// `logical_ids`); this one literal is kept so output stays byte-compatible
/// with what already-deployed CDK-synthesized stacks carry.
const DEFAULT_POLICY_NAME: &str = "McpFunctionServiceRoleDefaultPolicy29310C43";

/// Render the MCP server's Lambda execution role and its default inline
/// policy: `[(role_id, role), (policy_id, policy)]`.
#[must_use]
pub fn render_execution_role(d: &DeployDescriptor, p: &RenderParams) -> Vec<(String, CfnResource)> {
    vec![render_role(d), render_policy(p)]
}

/// The base `AWS::IAM::Role`: Lambda service trust policy +
/// `AWSLambdaBasicExecutionRole`.
fn render_role(d: &DeployDescriptor) -> (String, CfnResource) {
    let properties = json!({
        "AssumeRolePolicyDocument": {
            "Version": "2012-10-17",
            "Statement": [{
                "Effect": "Allow",
                "Action": "sts:AssumeRole",
                "Principal": { "Service": "lambda.amazonaws.com" },
            }],
        },
        "ManagedPolicyArns": [{
            "Fn::Join": ["", [
                "arn:",
                { "Ref": "AWS::Partition" },
                ":iam::aws:policy/service-role/AWSLambdaBasicExecutionRole",
            ]],
        }],
        "Tags": standard_tags(&d.server.name),
    });
    (
        logical_ids::for_execution_role().to_string(),
        CfnResource {
            type_: "AWS::IAM::Role".to_string(),
            properties,
            depends_on: vec![],
        },
    )
}

/// The default `AWS::IAM::Policy`: X-Ray tracing + DynamoDB
/// foundation-server discovery + cross-Lambda invoke. Always present (the
/// `pmcp-run` scaffold enables X-Ray tracing and composition permissions
/// unconditionally, regardless of `[observability]`/`[composition]` field
/// values — those sections are not wired into this stack shape today).
fn render_policy(p: &RenderParams) -> (String, CfnResource) {
    let region = &p.region;
    let account_id = &p.account_id;
    let properties = json!({
        "PolicyName": DEFAULT_POLICY_NAME,
        "PolicyDocument": {
            "Version": "2012-10-17",
            "Statement": [
                {
                    "Effect": "Allow",
                    "Action": ["xray:PutTraceSegments", "xray:PutTelemetryRecords"],
                    "Resource": "*",
                },
                {
                    "Effect": "Allow",
                    "Action": ["dynamodb:GetItem", "dynamodb:Query"],
                    "Resource": [
                        format!("arn:aws:dynamodb:{region}:{account_id}:table/{MCP_SERVERS_TABLE}"),
                        format!("arn:aws:dynamodb:{region}:{account_id}:table/{MCP_SERVERS_TABLE}/*"),
                    ],
                },
                {
                    "Effect": "Allow",
                    "Action": "lambda:InvokeFunction",
                    "Resource": format!("arn:aws:lambda:{region}:{account_id}:function:*"),
                },
            ],
        },
        "Roles": [{ "Ref": logical_ids::for_execution_role() }],
    });
    (
        logical_ids::for_execution_policy().to_string(),
        CfnResource {
            type_: "AWS::IAM::Policy".to_string(),
            properties,
            depends_on: vec![],
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{ArtifactRef, RenderMetadata};
    use std::collections::BTreeMap;

    fn descriptor() -> DeployDescriptor {
        toml::from_str(
            r#"
            [target]
            type = "pmcp-run"
            version = "1.0.0"
            [aws]
            region = "us-east-1"
            [server]
            name = "iam-test"
            timeout_seconds = 30
            [auth]
            enabled = false
            provider = "none"
            [observability]
            log_retention_days = 30
            enable_xray = true
            create_dashboard = true
            "#,
        )
        .expect("fixture descriptor parses")
    }

    fn params() -> RenderParams {
        RenderParams {
            account_id: "123456789012".to_string(),
            region: "us-east-1".to_string(),
            stack_name: "iam-test-stack".to_string(),
            artifact: ArtifactRef {
                s3_bucket: "bucket".to_string(),
                s3_key: "key.zip".to_string(),
                digest: None,
            },
            environment: BTreeMap::new(),
            metadata: RenderMetadata {
                version: "1.0.0".to_string(),
                server_type: None,
                server_id: None,
                template_id: None,
                snapshot_baked: false,
            },
        }
    }

    #[test]
    fn render_execution_role_returns_role_then_policy() {
        let resources = render_execution_role(&descriptor(), &params());
        assert_eq!(resources.len(), 2);
        assert_eq!(resources[0].0, logical_ids::for_execution_role());
        assert_eq!(resources[0].1.type_, "AWS::IAM::Role");
        assert_eq!(resources[1].0, logical_ids::for_execution_policy());
        assert_eq!(resources[1].1.type_, "AWS::IAM::Policy");
    }

    #[test]
    fn policy_references_region_and_account_in_dynamodb_arns() {
        let resources = render_execution_role(&descriptor(), &params());
        let policy_doc = &resources[1].1.properties["PolicyDocument"]["Statement"][1];
        assert_eq!(
            policy_doc["Resource"][0],
            "arn:aws:dynamodb:us-east-1:123456789012:table/McpServer"
        );
        assert_eq!(
            policy_doc["Resource"][1],
            "arn:aws:dynamodb:us-east-1:123456789012:table/McpServer/*"
        );
    }

    #[test]
    fn role_tags_carry_the_server_name() {
        let resources = render_execution_role(&descriptor(), &params());
        assert_eq!(
            resources[0].1.properties["Tags"],
            json!([
                { "Key": "managed-by", "Value": "pmcp" },
                { "Key": "project", "Value": "hosting" },
                { "Key": "service", "Value": "iam-test" },
                { "Key": "target", "Value": "pmcp-run" },
            ])
        );
    }
}
