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
//! skip. This task (the crate skeleton) wires none of the seven yet —
//! [`render`] returns an empty-resource template.
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
pub use template::{CfnOutput, CfnResource, CfnTemplate};

use pmcp_package::package::DeployDescriptor;
use std::collections::BTreeMap;

/// Render a `DeployDescriptor` + [`RenderParams`] into a deterministic
/// CloudFormation template.
///
/// # Errors
///
/// Returns [`RenderError`] if the descriptor requests something outside the
/// renderer's resource-family allowlist, is missing a field the renderer
/// needs, or supplies an invalid value for a field. This task's
/// implementation wires no resource modules yet, so it currently never
/// returns `Err` — later tasks add the fallible resource-building calls
/// this signature exists for.
///
/// # Current behavior (crate-skeleton task)
///
/// Always returns an EMPTY-resource template (`Description` only, no
/// `Resources`/`Outputs`/`Metadata` content) — the seven resource-module
/// wirings (`lambda`/`iam`/`logs`/`http_api`/`cognito`/`dynamodb`/`outputs`)
/// land in later tasks.
pub fn render(
    descriptor: &DeployDescriptor,
    _params: &RenderParams,
) -> Result<CfnTemplate, RenderError> {
    Ok(CfnTemplate {
        description: format!("PMCP MCP server: {}", descriptor.server.name),
        resources: BTreeMap::new(),
        outputs: BTreeMap::new(),
        metadata: BTreeMap::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(name: &str) -> DeployDescriptor {
        toml::from_str(&format!(
            r#"
            [target]
            type = "aws-lambda"
            version = "1"
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
            enable_xray = false
            create_dashboard = false
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
        assert_eq!(template.description, "PMCP MCP server: my-server");
    }

    #[test]
    fn render_produces_no_resources_yet() {
        let template = render(&descriptor("my-server"), &params()).unwrap();
        assert!(template.resources.is_empty());
        assert!(template.outputs.is_empty());
        assert!(template.metadata.is_empty());
    }
}
