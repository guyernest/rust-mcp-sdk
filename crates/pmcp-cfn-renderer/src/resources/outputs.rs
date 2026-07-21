//! `Outputs` rendering — endpoint URL, function identity, and the stable
//! cross-stack execution-role export. Names matter: `cargo-pmcp`'s
//! `deployment/outputs.rs::load_cdk_outputs` and the scaffold's own
//! `CfnOutput` calls (`commands/deploy/init.rs`) already read these exact
//! names, and Task 9's deploy engine depends on the byte-for-byte match.

use crate::{
    logical_ids,
    params::RenderParams,
    template::{CfnExport, CfnOutput},
};
use pmcp_package::package::DeployDescriptor;
use serde_json::json;
use std::collections::BTreeMap;

/// Render the plain-Lambda kernel's fixed output set: `ApiUrl`,
/// `DashboardUrl`, `LambdaArn`, `LambdaName`, `McpRoleArn`.
#[must_use]
pub fn render_outputs(d: &DeployDescriptor, p: &RenderParams) -> BTreeMap<String, CfnOutput> {
    let mut outputs = BTreeMap::new();
    outputs.insert(
        "ApiUrl".to_string(),
        CfnOutput {
            description: Some("MCP endpoint (construct from deployment ID)".to_string()),
            value: json!("https://api.pmcp.run/{use-deployment-id}/mcp"),
            export: None,
        },
    );
    outputs.insert(
        "DashboardUrl".to_string(),
        CfnOutput {
            description: Some("CloudWatch Console".to_string()),
            value: json!(format!(
                "https://console.aws.amazon.com/cloudwatch/home?region={}",
                p.region
            )),
            export: None,
        },
    );
    outputs.insert(
        "LambdaArn".to_string(),
        CfnOutput {
            description: Some("MCP Server Lambda ARN".to_string()),
            value: json!({ "Fn::GetAtt": [logical_ids::for_function(), "Arn"] }),
            export: None,
        },
    );
    outputs.insert(
        "LambdaName".to_string(),
        CfnOutput {
            description: Some("MCP Server Lambda Name".to_string()),
            value: json!({ "Ref": logical_ids::for_function() }),
            export: None,
        },
    );
    outputs.insert(
        "McpRoleArn".to_string(),
        CfnOutput {
            description: Some(
                "MCP Server Lambda execution role ARN (stable export for downstream stacks)"
                    .to_string(),
            ),
            value: json!({ "Fn::GetAtt": [logical_ids::for_execution_role(), "Arn"] }),
            export: Some(CfnExport {
                name: format!("pmcp-{}-McpRoleArn", d.server.name),
            }),
        },
    );
    outputs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{ArtifactRef, RenderMetadata};

    fn descriptor(name: &str) -> DeployDescriptor {
        toml::from_str(&format!(
            r#"
            [target]
            type = "pmcp-run"
            version = "1.0.0"
            [aws]
            region = "us-east-1"
            [server]
            name = "{name}"
            timeout_seconds = 30
            [auth]
            enabled = false
            provider = "none"
            [observability]
            log_retention_days = 30
            enable_xray = true
            create_dashboard = true
            "#
        ))
        .expect("fixture descriptor parses")
    }

    fn params() -> RenderParams {
        RenderParams {
            account_id: "123456789012".to_string(),
            region: "us-west-2".to_string(),
            stack_name: "outputs-test-stack".to_string(),
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
    fn renders_exactly_the_five_expected_output_names() {
        let outputs = render_outputs(&descriptor("out-test"), &params());
        let mut names: Vec<&String> = outputs.keys().collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                "ApiUrl",
                "DashboardUrl",
                "LambdaArn",
                "LambdaName",
                "McpRoleArn"
            ]
        );
    }

    #[test]
    fn dashboard_url_uses_the_params_region() {
        let outputs = render_outputs(&descriptor("out-test"), &params());
        assert_eq!(
            outputs["DashboardUrl"].value,
            json!("https://console.aws.amazon.com/cloudwatch/home?region=us-west-2")
        );
    }

    #[test]
    fn mcp_role_arn_carries_a_stable_export_name() {
        let outputs = render_outputs(&descriptor("out-test"), &params());
        let export = outputs["McpRoleArn"]
            .export
            .as_ref()
            .expect("McpRoleArn has an Export");
        assert_eq!(export.name, "pmcp-out-test-McpRoleArn");
    }

    #[test]
    fn only_mcp_role_arn_has_an_export() {
        let outputs = render_outputs(&descriptor("out-test"), &params());
        for (name, output) in &outputs {
            if name == "McpRoleArn" {
                assert!(output.export.is_some());
            } else {
                assert!(output.export.is_none(), "{name} should have no Export");
            }
        }
    }
}
