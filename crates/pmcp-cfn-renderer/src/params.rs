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
    /// The EXACT top-level CloudFormation template `Metadata` map [`crate::render`]
    /// should emit verbatim — e.g. the `mcp:*` provenance keys
    /// (`mcp:version`, `mcp:serverType`, `mcp:serverId`, `mcp:resources`,
    /// `mcp:capabilities`, ...) production `cdk synth` bakes via
    /// `Stack.templateOptions.metadata`, one entry per key.
    ///
    /// T7 review fix: before this field existed, [`crate::render`] always
    /// emitted an empty `CfnTemplate::metadata`, discarding
    /// `RenderMetadata`'s fields entirely and losing ALL `mcp:*` provenance
    /// from the uploaded template — the platform's sole provenance channel
    /// for a renderer-synthesized stack. This field is additive alongside
    /// [`RenderMetadata`] (kept for any internal/`cdk`-context use, e.g.
    /// `cargo-pmcp`'s `to_cdk_context`) rather than replacing it, so this
    /// struct's public shape stays backward compatible.
    ///
    /// The renderer treats this map as OPAQUE passthrough content: it
    /// copies it byte-for-byte into `CfnTemplate::metadata`
    /// (`BTreeMap`-backed, so key order — and therefore
    /// `to_canonical_json`'s byte output — is always sorted), never
    /// inspects or derives from it. Populating it with real provenance is
    /// the CALLER's job — see `cargo-pmcp`'s
    /// `deployment::metadata::McpMetadata::to_cloudformation_metadata`, the
    /// maintained DSTK-03 shape both the legacy `cdk` path and this
    /// renderer's `pmcp_run` caller share (`deployment::targets::pmcp_run::deploy::build_render_params`).
    ///
    /// `#[serde(default)]` so already-checked-in golden `params` JSON
    /// fixtures (predating this field) keep deserializing unchanged, into
    /// an empty map — which [`crate::template::CfnTemplate`]'s own
    /// "omit `Metadata` when empty" envelope rule already treats as "no
    /// metadata block", the same behavior [`crate::render`] had before
    /// this fix.
    #[serde(default)]
    pub cloudformation_metadata: BTreeMap<String, serde_json::Value>,
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
            cloudformation_metadata: BTreeMap::new(),
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

    /// T7 review fix: `cloudformation_metadata` round-trips like every
    /// other field once populated.
    #[test]
    fn cloudformation_metadata_round_trips_through_json() {
        let mut params = sample_params();
        params.cloudformation_metadata = BTreeMap::from([
            ("mcp:version".to_string(), serde_json::json!("1.0.0")),
            (
                "mcp:resources".to_string(),
                serde_json::json!({"secrets": []}),
            ),
        ]);
        let json = serde_json::to_string(&params).expect("serialize");
        let back: RenderParams = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, params);
    }

    /// Backward compat: `params` JSON that predates this field (every
    /// checked-in golden fixture at the time this field was added) must
    /// keep deserializing, defaulting to an empty map — see
    /// `RenderParams::cloudformation_metadata`'s doc comment.
    #[test]
    fn cloudformation_metadata_defaults_to_empty_when_absent_from_json() {
        let json = serde_json::json!({
            "account_id": "123456789012",
            "region": "us-east-1",
            "stack_name": "compat-test-stack",
            "artifact": {"s3_bucket": "bucket", "s3_key": "key.zip"},
            "metadata": {"version": "1.0.0", "snapshot_baked": false}
        });
        let params: RenderParams =
            serde_json::from_value(json).expect("deserializes without cloudformation_metadata");
        assert!(params.cloudformation_metadata.is_empty());
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
