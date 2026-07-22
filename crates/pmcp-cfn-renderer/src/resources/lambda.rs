//! `AWS::Lambda::Function` rendering for the MCP server's Lambda function.
//!
//! Models the `pmcp-run` deployment target's Lambda-only stack shape — no
//! API Gateway; the shared pmcp.run edge routes
//! `https://api.pmcp.run/{deploymentId}/mcp` to this function directly.
//! Mirrors the `pmcp-run` branch of
//! `cargo-pmcp/src/commands/deploy/init.rs::render_stack_ts` verbatim.

use crate::{
    logical_ids,
    params::RenderParams,
    resources::{aws_lambda_tags, standard_tags, DEFAULT_ORGANIZATION_ID, MCP_SERVERS_TABLE},
    template::CfnResource,
};
use pmcp_package::package::DeployDescriptor;
use serde_json::json;
use std::collections::BTreeMap;

/// Fixed memory size for `pmcp-run`-target Lambdas. NOT driven by
/// `[server].memory_mb` — the TS scaffold hardcodes `memorySize: 256` for
/// this target (`memory_mb` is meaningful only for the google-cloud-run
/// target's separate `[server].memory` string field, a non-CFN target this
/// renderer never touches).
const MEMORY_SIZE_MB: i64 = 256;

/// Fixed memory size for `aws-lambda`-target Lambdas — the TS scaffold
/// hardcodes `memorySize: 512` for this branch (vs. `pmcp-run`'s
/// [`MEMORY_SIZE_MB`] = 256). Also NOT driven by `[server].memory_mb`.
const AWS_LAMBDA_MEMORY_SIZE_MB: i64 = 512;

/// Render the MCP server's `AWS::Lambda::Function`: `(logical_id, resource)`.
#[must_use]
pub fn render_function(d: &DeployDescriptor, p: &RenderParams) -> (String, CfnResource) {
    let properties = json!({
        "FunctionName": d.server.name,
        "Runtime": "provided.al2023",
        "Handler": "bootstrap",
        "Architectures": ["arm64"],
        "MemorySize": MEMORY_SIZE_MB,
        "Timeout": d.server.timeout_seconds,
        "Code": { "S3Bucket": p.artifact.s3_bucket, "S3Key": p.artifact.s3_key },
        "Role": { "Fn::GetAtt": [logical_ids::for_execution_role(), "Arn"] },
        "Environment": { "Variables": environment_variables(d, p) },
        "TracingConfig": { "Mode": "Active" },
        "LoggingConfig": { "LogFormat": "JSON" },
        "Tags": standard_tags(&d.server.name),
    });
    (
        logical_ids::for_function().to_string(),
        CfnResource {
            type_: "AWS::Lambda::Function".to_string(),
            properties,
            // CDK adds an explicit dependency from the function onto its
            // execution role's default policy (IAM eventual-consistency
            // safety) as well as the role itself.
            depends_on: vec![
                logical_ids::for_execution_policy().to_string(),
                logical_ids::for_execution_role().to_string(),
            ],
        },
    )
}

/// Render the MCP server's `AWS::Lambda::Function` for the `aws-lambda`
/// target's own (non-pmcp.run) stack shape: `(logical_id, resource)`.
/// Mirrors the `aws-lambda` branch of
/// `cargo-pmcp/src/commands/deploy/init.rs::render_stack_ts` — same
/// `Runtime`/`Handler`/`Architectures`/`Role` wiring as [`render_function`],
/// but a fixed 512 MB memory size, `p.environment` passed through UNCHANGED
/// (no `pmcp-run`-only composition vars — this stack shape has no
/// `MCP_SERVERS_TABLE` discovery table to read, and never calls other
/// foundation-server Lambdas), and `aws-lambda`-flavored tags (own
/// `project`, `target = "aws-lambda"`) via
/// [`crate::resources::aws_lambda_tags`].
#[must_use]
pub fn render_function_aws_lambda(d: &DeployDescriptor, p: &RenderParams) -> (String, CfnResource) {
    let properties = json!({
        "FunctionName": d.server.name,
        "Runtime": "provided.al2023",
        "Handler": "bootstrap",
        "Architectures": ["arm64"],
        "MemorySize": AWS_LAMBDA_MEMORY_SIZE_MB,
        "Timeout": d.server.timeout_seconds,
        "Code": { "S3Bucket": p.artifact.s3_bucket, "S3Key": p.artifact.s3_key },
        "Role": { "Fn::GetAtt": [logical_ids::for_execution_role(), "Arn"] },
        "Environment": { "Variables": p.environment.clone() },
        "TracingConfig": { "Mode": "Active" },
        "LoggingConfig": { "LogFormat": "JSON" },
        "Tags": aws_lambda_tags(&d.server.name),
    });
    (
        logical_ids::for_function().to_string(),
        CfnResource {
            type_: "AWS::Lambda::Function".to_string(),
            properties,
            // Same eventual-consistency dependency ordering as
            // `render_function` — see its own doc comment.
            depends_on: vec![
                logical_ids::for_execution_policy().to_string(),
                logical_ids::for_execution_role().to_string(),
            ],
        },
    )
}

/// The function's environment variables: `p.environment`'s resolved values
/// (e.g. `RUST_LOG`, already resolved from the descriptor by the caller —
/// see `RenderParams`'s identity/environment split) plus the three
/// composition-support vars every `pmcp-run` Lambda gets
/// (`MCP_SERVERS_TABLE`, `PMCP_ORGANIZATION_ID`, `PMCP_SERVER_ID`),
/// mirroring the TS scaffold's `environment: {...}` object. The three fixed
/// vars win over any same-named entry in `p.environment`.
fn environment_variables(d: &DeployDescriptor, p: &RenderParams) -> BTreeMap<String, String> {
    let mut vars = p.environment.clone();
    vars.insert(
        "MCP_SERVERS_TABLE".to_string(),
        MCP_SERVERS_TABLE.to_string(),
    );
    vars.insert(
        "PMCP_ORGANIZATION_ID".to_string(),
        DEFAULT_ORGANIZATION_ID.to_string(),
    );
    vars.insert("PMCP_SERVER_ID".to_string(), d.server.name.clone());
    vars
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{ArtifactRef, RenderMetadata};

    fn descriptor() -> DeployDescriptor {
        toml::from_str(
            r#"
            [target]
            type = "pmcp-run"
            version = "1.0.0"
            [aws]
            region = "us-east-1"
            [server]
            name = "lambda-test"
            memory_mb = 512
            timeout_seconds = 45
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

    fn params(environment: BTreeMap<String, String>) -> RenderParams {
        RenderParams {
            account_id: "123456789012".to_string(),
            region: "us-east-1".to_string(),
            stack_name: "lambda-test-stack".to_string(),
            artifact: ArtifactRef {
                s3_bucket: "bucket".to_string(),
                s3_key: "key.zip".to_string(),
                digest: None,
            },
            environment,
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
    fn memory_size_is_fixed_regardless_of_descriptor_memory_mb() {
        let (_, resource) = render_function(&descriptor(), &params(BTreeMap::new()));
        assert_eq!(resource.properties["MemorySize"], 256);
    }

    #[test]
    fn timeout_comes_from_the_descriptor() {
        let (_, resource) = render_function(&descriptor(), &params(BTreeMap::new()));
        assert_eq!(resource.properties["Timeout"], 45);
    }

    #[test]
    fn environment_merges_params_env_with_fixed_composition_vars() {
        let environment = BTreeMap::from([("RUST_LOG".to_string(), "info".to_string())]);
        let (_, resource) = render_function(&descriptor(), &params(environment));
        assert_eq!(
            resource.properties["Environment"]["Variables"],
            json!({
                "RUST_LOG": "info",
                "MCP_SERVERS_TABLE": "McpServer",
                "PMCP_ORGANIZATION_ID": "default-org",
                "PMCP_SERVER_ID": "lambda-test",
            })
        );
    }

    #[test]
    fn function_depends_on_the_execution_role_and_its_policy() {
        let (_, resource) = render_function(&descriptor(), &params(BTreeMap::new()));
        assert_eq!(
            resource.depends_on,
            vec![
                logical_ids::for_execution_policy().to_string(),
                logical_ids::for_execution_role().to_string(),
            ]
        );
    }

    fn aws_lambda_descriptor() -> DeployDescriptor {
        toml::from_str(
            r#"
            [target]
            type = "aws-lambda"
            version = "1.0.0"
            [aws]
            region = "us-east-1"
            [server]
            name = "http-api-test"
            memory_mb = 512
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

    #[test]
    fn aws_lambda_memory_size_is_fixed_at_512_regardless_of_descriptor_memory_mb() {
        let (_, resource) =
            render_function_aws_lambda(&aws_lambda_descriptor(), &params(BTreeMap::new()));
        assert_eq!(resource.properties["MemorySize"], 512);
    }

    #[test]
    fn aws_lambda_environment_is_passed_through_with_no_composition_vars_added() {
        let environment = BTreeMap::from([("RUST_LOG".to_string(), "info".to_string())]);
        let (_, resource) =
            render_function_aws_lambda(&aws_lambda_descriptor(), &params(environment));
        assert_eq!(
            resource.properties["Environment"]["Variables"],
            json!({ "RUST_LOG": "info" })
        );
    }

    #[test]
    fn aws_lambda_tags_carry_the_server_name_as_project_and_aws_lambda_as_target() {
        let (_, resource) =
            render_function_aws_lambda(&aws_lambda_descriptor(), &params(BTreeMap::new()));
        assert_eq!(
            resource.properties["Tags"],
            json!([
                { "Key": "managed-by", "Value": "pmcp" },
                { "Key": "project", "Value": "http-api-test" },
                { "Key": "service", "Value": "http-api-test" },
                { "Key": "target", "Value": "aws-lambda" },
            ])
        );
    }

    #[test]
    fn aws_lambda_function_depends_on_the_execution_role_and_its_policy() {
        let (_, resource) =
            render_function_aws_lambda(&aws_lambda_descriptor(), &params(BTreeMap::new()));
        assert_eq!(
            resource.depends_on,
            vec![
                logical_ids::for_execution_policy().to_string(),
                logical_ids::for_execution_role().to_string(),
            ]
        );
    }
}
