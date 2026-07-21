//! # pmcp-cfn-renderer
//!
//! A pure, deterministic `DeployDescriptor` -> CloudFormation renderer for
//! PMCP MCP servers.
//!
//! ## Purity discipline
//!
//! This crate has exactly four dependencies: `pmcp-package` (the descriptor
//! types), `serde`, `serde_json`, and `semver`. It contains **no** AWS SDK,
//! no `tokio`, no `reqwest`, no filesystem access, and no network access —
//! [`render`] is a pure function of its two arguments. This is deliberate:
//! the crate is trust-kernel code run by both `cargo-pmcp` (the CLI) and
//! hosting platforms (e.g. `pmcp.run`), and both must derive byte-identical
//! CloudFormation from the same descriptor.
//!
//! ## Identity/environment split
//!
//! [`render`]'s signature enforces the split: everything about *what* server
//! to deploy comes from `pmcp_package::package::DeployDescriptor` (the
//! closed-set, environment-independent identity contract); everything about
//! *where*/*how* it is deployed this time (account, region, artifact
//! location, resolved environment values, synth-time metadata) comes from
//! [`RenderParams`]. The descriptor can never smuggle an environment-specific
//! value into template identity.
//!
//! ## Determinism
//!
//! [`CfnTemplate::to_canonical_json`] is byte-deterministic: identical
//! inputs to [`render`] always produce an identical canonical JSON string,
//! across runs and platforms. See [`template`]'s module documentation for
//! how this is enforced structurally (via `BTreeMap` fields) rather than by
//! an ad hoc sort step.
//!
//! ## Resource surface
//!
//! The v1 resource surface is exactly seven allowlisted families —
//! `lambda`, `iam`, `logs`, `http_api`, `cognito`, `dynamodb`, `outputs` (see
//! [`resources`]). A descriptor requesting anything outside that surface
//! fails loudly via [`RenderError::UnsupportedSection`], never a silent
//! skip. As of Task 4, [`render`] wires `lambda`, `logs`, `outputs`, and the
//! full `iam` module — the BASE execution role/policy PLUS any declared
//! `[[iam.statements]]` expansion, fail-closed validated first (see
//! [`resources::iam`]'s doc comment) — for the `pmcp-run` target. Every
//! other target type and `auth.enabled = true` still fail loudly via
//! [`RenderError::UnsupportedSection`] rather than silently rendering an
//! incomplete stack; a declared `[[iam.statements]]` entry that fails
//! [`resources::iam::validate`]'s fail-closed rules fails loudly via
//! [`RenderError::Invalid`] instead. `http_api`, `cognito`, and `dynamodb`
//! land in later tasks.
//!
//! ## Logical IDs
//!
//! Logical IDs are derived directly from descriptor names via a documented,
//! hash-free transform — see [`logical_ids`].

pub mod error;
pub mod logical_ids;
pub mod params;
pub mod resources;
pub mod template;

// ---------------------------------------------------------------------
// Crate-root re-exports (so callers can write `pmcp_cfn_renderer::RenderParams`
// rather than `pmcp_cfn_renderer::params::RenderParams`). Deep module paths
// still work — this is additive, not a replacement for the module tree.
// ---------------------------------------------------------------------

pub use error::RenderError;
pub use params::{ArtifactRef, RenderMetadata, RenderParams};
pub use template::{CfnExport, CfnOutput, CfnResource, CfnTemplate};

use pmcp_package::package::DeployDescriptor;
use std::collections::BTreeMap;

/// Render a `DeployDescriptor` + [`RenderParams`] into a deterministic
/// CloudFormation template.
///
/// # Errors
///
/// Returns [`RenderError::UnsupportedSection`] when the descriptor requests
/// something the renderer doesn't implement yet: a `[target].type` other
/// than `"pmcp-run"` (the only stack shape wired so far — `http_api`/
/// `cognito` land in later tasks) or `auth.enabled = true` (needs the
/// `cognito`/`http_api` modules). Returns [`RenderError::Invalid`] when a
/// declared `[[iam.statements]]` entry fails [`resources::iam::validate`]'s
/// fail-closed rules (bad effect, empty actions/resources, malformed
/// action, or a wildcard-escalation footgun — see that function's doc
/// comment). Never silently skips descriptor content.
///
/// # Current behavior (Task 4)
///
/// For a `pmcp-run`-target descriptor with `auth.enabled = false`, renders
/// the plain-Lambda kernel: the function, its log group, its base execution
/// role + default inline policy (with any validated `[[iam.statements]]`
/// appended), and the fixed five-output set (see [`resources`]).
pub fn render(
    descriptor: &DeployDescriptor,
    params: &RenderParams,
) -> Result<CfnTemplate, RenderError> {
    guard_unsupported(descriptor)?;
    if let Some(iam) = &descriptor.iam {
        // Discard warnings here — `render` is the pure, side-effect-free
        // entry point (no I/O to print them through). Hard errors still
        // fail loudly via `?`. Callers that want the advisory findings
        // (unknown service prefix, cross-account ARN pin) call
        // `resources::iam::validate` directly, same as `render` does —
        // it's a fully public, separately-callable function.
        resources::iam::validate(iam)?;
    }

    let mut resources = BTreeMap::new();
    for (id, resource) in resources::iam::render_execution_role(descriptor, params) {
        resources.insert(id, resource);
    }
    let (function_id, function) = resources::lambda::render_function(descriptor, params);
    resources.insert(function_id, function);
    let (log_group_id, log_group) =
        resources::logs::render_log_group(&descriptor.server.name, PLAIN_LOG_RETENTION_DAYS);
    resources.insert(log_group_id, log_group);

    let outputs = resources::outputs::render_outputs(descriptor, params);

    Ok(CfnTemplate {
        description: format!("MCP Server: {}", descriptor.server.name),
        resources,
        outputs,
        metadata: BTreeMap::new(),
    })
}

/// Log-retention days for the `pmcp-run` plain-Lambda kernel. NOT driven by
/// `[observability].log_retention_days` — the TS scaffold's `pmcp-run`
/// branch hardcodes `RetentionDays.ONE_WEEK` unconditionally (as it does
/// for X-Ray tracing and the console-link `DashboardUrl` output); the
/// `[observability]` section is not wired into this stack shape today.
const PLAIN_LOG_RETENTION_DAYS: u32 = 7;

/// Fail loudly on descriptor content this task doesn't render yet, rather
/// than silently producing an incomplete or wrong stack.
fn guard_unsupported(d: &DeployDescriptor) -> Result<(), RenderError> {
    if d.target.target_type != "pmcp-run" {
        return Err(RenderError::UnsupportedSection {
            section: "target".to_string(),
            detail: format!(
                "target type '{}' is not yet rendered (only 'pmcp-run' plain-Lambda \
                 rendering is implemented; 'aws-lambda' needs the http_api module, \
                 a later task)",
                d.target.target_type
            ),
        });
    }
    if d.auth.enabled {
        return Err(RenderError::UnsupportedSection {
            section: "auth".to_string(),
            detail: "auth.enabled = true requires the cognito/http_api resource modules, \
                     not yet implemented"
                .to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal, valid `pmcp-run` descriptor: `auth.enabled = false`, no
    /// `[iam]` section — the shape [`render`] fully supports today.
    fn descriptor(name: &str) -> DeployDescriptor {
        descriptor_with(name, "pmcp-run", false, "")
    }

    fn descriptor_with(
        name: &str,
        target_type: &str,
        auth_enabled: bool,
        iam_block: &str,
    ) -> DeployDescriptor {
        toml::from_str(&format!(
            r#"
            [target]
            type = "{target_type}"
            version = "1.0.0"
            [aws]
            region = "us-east-1"
            [server]
            name = "{name}"
            timeout_seconds = 30
            [auth]
            enabled = {auth_enabled}
            provider = "none"
            [observability]
            log_retention_days = 30
            enable_xray = false
            create_dashboard = false
            {iam_block}
            "#
        ))
        .expect("fixture descriptor parses")
    }

    fn params() -> RenderParams {
        RenderParams {
            account_id: "123456789012".to_string(),
            region: "us-east-1".to_string(),
            stack_name: "unit-test-stack".to_string(),
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
    fn render_description_names_the_server() {
        let template = render(&descriptor("my-server"), &params()).unwrap();
        assert_eq!(template.description, "MCP Server: my-server");
    }

    #[test]
    fn render_produces_the_plain_lambda_resource_kernel() {
        let template = render(&descriptor("my-server"), &params()).unwrap();
        let mut resource_ids: Vec<&String> = template.resources.keys().collect();
        resource_ids.sort();
        assert_eq!(
            resource_ids,
            vec![
                "ExecutionRole",
                "ExecutionRoleDefaultPolicy",
                "LogGroup",
                "McpFunction"
            ]
        );
        assert_eq!(template.outputs.len(), 5);
        assert!(template.metadata.is_empty());
    }

    #[test]
    fn render_rejects_a_target_type_other_than_pmcp_run() {
        let err = render(
            &descriptor_with("my-server", "aws-lambda", false, ""),
            &params(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            RenderError::UnsupportedSection {
                section: "target".to_string(),
                detail: "target type 'aws-lambda' is not yet rendered (only 'pmcp-run' \
                          plain-Lambda rendering is implemented; 'aws-lambda' needs the \
                          http_api module, a later task)"
                    .to_string(),
            }
        );
    }

    #[test]
    fn render_rejects_auth_enabled() {
        let err = render(
            &descriptor_with("my-server", "pmcp-run", true, ""),
            &params(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            RenderError::UnsupportedSection {
                section: "auth".to_string(),
                detail: "auth.enabled = true requires the cognito/http_api resource \
                          modules, not yet implemented"
                    .to_string(),
            }
        );
    }

    #[test]
    fn render_accepts_declared_iam_statements_and_appends_them_to_the_policy() {
        let iam_block = r#"
            [[iam.statements]]
            effect = "Allow"
            actions = ["s3:GetObject"]
            resources = ["arn:aws:s3:::example-bucket/*"]
            "#;
        let template = render(
            &descriptor_with("my-server", "pmcp-run", false, iam_block),
            &params(),
        )
        .expect("declared iam.statements now render, Task 4");
        // Still exactly the same 4-resource plain-Lambda kernel — the
        // declared statement lands inside ExecutionRoleDefaultPolicy's
        // existing Statement array, not a new resource.
        let mut resource_ids: Vec<&String> = template.resources.keys().collect();
        resource_ids.sort();
        assert_eq!(
            resource_ids,
            vec![
                "ExecutionRole",
                "ExecutionRoleDefaultPolicy",
                "LogGroup",
                "McpFunction"
            ]
        );
        let statements = template.resources["ExecutionRoleDefaultPolicy"].properties
            ["PolicyDocument"]["Statement"]
            .as_array()
            .unwrap();
        assert_eq!(statements.len(), 4, "3 base + 1 declared statement");
        assert_eq!(statements[3]["Action"], "s3:GetObject");
    }

    #[test]
    fn render_rejects_an_invalid_declared_iam_statement() {
        // Fail-closed: a wildcard-escalation statement must reject the
        // whole render, not silently drop the offending statement.
        let iam_block = r#"
            [[iam.statements]]
            effect = "Allow"
            actions = ["*"]
            resources = ["*"]
            "#;
        let err = render(
            &descriptor_with("my-server", "pmcp-run", false, iam_block),
            &params(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            RenderError::Invalid {
                section: "iam".to_string(),
                field: "statements[0].actions".to_string(),
                message: "Allow + actions=[\"*\"] + resources=[\"*\"] is a wildcard escalation \
                          footgun — refuse to deploy. Tighten actions and resources."
                    .to_string(),
            }
        );
    }

    #[test]
    fn render_accepts_an_empty_declared_iam_section() {
        // `[iam]` present but with no `[[iam.statements]]` entries renders
        // identically to no `[iam]` section at all.
        let template = render(
            &descriptor_with("my-server", "pmcp-run", false, "[iam]"),
            &params(),
        )
        .unwrap();
        assert_eq!(template.resources.len(), 4);
    }
}
