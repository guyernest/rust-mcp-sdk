//! Render-time parameters — the identity/environment split boundary.
//!
//! Everything the renderer needs that is environmental (account, region,
//! artifact location, resolved env-var values, synth-time metadata) arrives
//! via [`RenderParams`], kept strictly separate from [`pmcp_package::package::DeployDescriptor`]
//! (the closed-set identity contract). The descriptor can never smuggle an
//! environment-specific value into template identity because [`crate::render`]'s
//! signature does not let it.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Where the deployable artifact (a compiled bootstrap-binary zip) lives.
///
/// The artifact is always a zip, never source code: most MCP servers are
/// built-in types deployed as a **prebuilt published binary**; only
/// custom-Rust servers carry a locally compiled one. Provenance (how the zip
/// was obtained) is the deploy engine's concern, not the renderer's — this
/// type only carries where it lives, plus an optional expected digest that
/// aligns with `pmcp_package`'s `BinaryRef` (verified by the engine before
/// upload; CloudFormation itself cannot verify it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRef {
    /// S3 bucket the artifact zip is (or will be) uploaded to.
    pub s3_bucket: String,
    /// S3 key of the artifact zip within `s3_bucket`.
    pub s3_key: String,
    /// Expected content digest of the artifact, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

/// Synth-time metadata injected into the rendered template.
///
/// Mirrors `cargo-pmcp`'s `metadata.rs::McpMetadata::to_cdk_context` keys:
/// `version` -> `mcp:version`, `server_type` -> `mcp:serverType`,
/// `server_id` -> `mcp:serverId`, `template_id` -> `mcp:templateId`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderMetadata {
    /// Metadata schema version (mirrors `mcp:version`).
    pub version: String,
    /// Server type, e.g. `graphql-api`/`openapi-api`/`custom` (mirrors
    /// `mcp:serverType`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_type: Option<String>,
    /// Unique server identifier (mirrors `mcp:serverId`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_id: Option<String>,
    /// Template that generated this server, if applicable (mirrors
    /// `mcp:templateId`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,
    /// Whether the server's snapshot is baked into the deployment artifact
    /// (mirrors the conditionally emitted `mcp:snapshotBaked`).
    #[serde(default)]
    pub snapshot_baked: bool,
}

/// Everything environmental [`crate::render`] needs, kept OUT of the
/// descriptor's closed set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderParams {
    /// The target AWS account ID.
    pub account_id: String,
    /// The target AWS region.
    pub region: String,
    /// The CloudFormation stack name.
    pub stack_name: String,
    /// Location of the deployable bootstrap-binary artifact.
    pub artifact: ArtifactRef,
    /// Resolved environment variable values (identity-safe values only —
    /// secret VALUES never appear here; secrets stay platform/deploy-time
    /// references).
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    /// Synth-time metadata for the rendered template.
    pub metadata: RenderMetadata,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_params() -> RenderParams {
        RenderParams {
            account_id: "123456789012".to_string(),
            region: "us-east-1".to_string(),
            stack_name: "det-test-stack".to_string(),
            artifact: ArtifactRef {
                s3_bucket: "pmcp-deploy-123456789012-us-east-1".to_string(),
                s3_key: "det-test/bootstrap.zip".to_string(),
                digest: Some("sha256:abc".to_string()),
            },
            environment: BTreeMap::from([("RUST_LOG".to_string(), "info".to_string())]),
            metadata: RenderMetadata {
                version: "1.0.0".to_string(),
                server_type: Some("custom".to_string()),
                server_id: Some("det-test".to_string()),
                template_id: None,
                snapshot_baked: false,
            },
        }
    }

    /// Task 2's golden harness deserializes `RenderParams` from JSON fixture
    /// files — this is the load-bearing round-trip that guarantees.
    #[test]
    fn render_params_round_trips_through_json() {
        let params = sample_params();
        let json = serde_json::to_string(&params).expect("serialize");
        let back: RenderParams = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, params);
    }

    #[test]
    fn artifact_ref_omits_absent_digest_from_json() {
        let artifact = ArtifactRef {
            s3_bucket: "bucket".to_string(),
            s3_key: "key.zip".to_string(),
            digest: None,
        };
        let json = serde_json::to_string(&artifact).expect("serialize");
        assert!(
            !json.contains("digest"),
            "absent digest must be elided, got: {json}"
        );
    }
}
