//! Vendor `application/vnd.pmcp.*` media-type constants (RESEARCH Pattern 3)
//! for every OCI layer this crate packs, plus the standard OCI empty-config
//! blob constants (OCI 1.1's non-null-config-descriptor requirement).
//!
//! # Layer inventory
//!
//! - `ServerPackage` decomposes into content-addressed layers. Exactly ONE
//!   binary layer is always present, and it is one of two mutually exclusive
//!   shapes: the embedded bootstrap binary ([`MT_SERVER_BOOTSTRAP`] — supplied
//!   as raw bytes, never a struct field) OR a binary REFERENCE
//!   ([`MT_SERVER_BINARY_REF`] — a small JSON layer naming the digest and
//!   media type of a binary the target environment resolves for itself; a
//!   pure-config Shape A server carries this one and no bytes at all).
//!   Alongside it are the four typed sections — the deploy descriptor
//!   ([`MT_SERVER_DEPLOY_DESCRIPTOR`]), the cedar policy set
//!   ([`MT_SERVER_CEDAR_POLICY_SET`]), tool metadata
//!   ([`MT_SERVER_TOOL_METADATA`]), and the declared config slots
//!   ([`MT_SERVER_CONFIG_SLOTS`]) — plus the `name`/`version`/top-level
//!   `digest` "envelope" layer ([`MT_SERVER_ENVELOPE`]) so every remaining
//!   field round-trips losslessly by plain serialize/deserialize.
//! - Two OPTIONAL vendor-content layers may follow: the author's verbatim
//!   server config file ([`MT_SERVER_CONFIG`]) and, for an OpenAPI-backed
//!   server, its spec file ([`MT_SERVER_OPENAPI_SPEC`]). Both carry raw
//!   author bytes (never re-derived from a parsed struct) and record the
//!   original file name in their descriptor's
//!   `org.opencontainers.image.title` annotation. Either may be absent, and
//!   absence is exactly the layer NOT being in the manifest — there is no
//!   absence marker (D-14). An absent [`MT_SERVER_OPENAPI_SPEC`] layer is the
//!   author's declaration of a curated-only server, mirroring
//!   `pmcp-openapi-server`'s `--spec: Option<PathBuf>`, never a silent drop of
//!   a spec that was supplied.
//! - Layers are located at unpack time by MEDIA TYPE, never by position — the
//!   optional layers make any positional contract false.
//! - `AgentPackage`/`TeamPackage`/`WorkflowManifest` each pack as a SINGLE
//!   JSON layer ([`MT_AGENT_CONFIG`]/[`MT_TEAM_CONFIG`]/
//!   [`MT_WORKFLOW_MANIFEST`]) — the whole struct serialized once; no
//!   decomposition needed since (unlike `ServerPackage`) they don't carry a
//!   large binary blob that must live in its own layer.
//!
//! Every layer's `Descriptor` uses [`MediaType::Other`] via [`vendor_media_type`]
//! (RESEARCH Pattern 3) — never a hand-rolled parallel media-type enum.

use oci_spec::image::{Descriptor, MediaType};
use std::str::FromStr;

// ---------------------------------------------------------------------
// ServerPackage layers
// ---------------------------------------------------------------------

/// The compiled bootstrap Lambda binary — supplied to `pack_server` as raw
/// `&[u8]`, never inlined on `ServerPackage` (cross-AI review: binary
/// payloads are OCI layers, not typed-struct fields).
pub const MT_SERVER_BOOTSTRAP: &str = "application/vnd.pmcp.mcp-server.bootstrap.v1+binary";
/// The `name`/`version`/top-level `digest`/`binary_ref` fields not covered by
/// any of the other typed `ServerPackage` sections.
pub const MT_SERVER_ENVELOPE: &str = "application/vnd.pmcp.mcp-server.envelope.v1+json";
/// The typed equivalent of a `.pmcp/deploy.toml` (`ServerPackage.deploy`).
pub const MT_SERVER_DEPLOY_DESCRIPTOR: &str =
    "application/vnd.pmcp.mcp-server.deploy-descriptor.v1+json";
/// The server's cedar-policy set (`ServerPackage.policies`).
pub const MT_SERVER_CEDAR_POLICY_SET: &str =
    "application/vnd.pmcp.mcp-server.cedar-policy-set.v1+json";
/// The server's tool/connector metadata (`ServerPackage.tools`).
pub const MT_SERVER_TOOL_METADATA: &str = "application/vnd.pmcp.mcp-server.tool-metadata.v1+json";
/// The server's declared config slots (`ServerPackage.config_slots`).
pub const MT_SERVER_CONFIG_SLOTS: &str = "application/vnd.pmcp.mcp-server.config-slots.v1+json";
/// The author's server config file (`config.toml`), carried VERBATIM as raw
/// bytes. Generic across all three Shape A pure-config siblings (SQL,
/// workbook, OpenAPI) — the config's *dialect* is the server binary's
/// concern, not the package format's, so there is one media type rather than
/// one per kind. The original file name travels in the layer descriptor's
/// `org.opencontainers.image.title` annotation.
pub const MT_SERVER_CONFIG: &str = "application/vnd.pmcp.mcp-server.config.v1+toml";
/// An OpenAPI-backed server's spec file, carried VERBATIM as raw bytes.
/// Unlike [`MT_SERVER_CONFIG`] this one IS per-kind: only an OpenAPI server
/// has a spec, and its bytes are the spec document exactly as the author
/// wrote it (JSON or YAML — the extension travels in the descriptor's
/// `org.opencontainers.image.title` annotation).
pub const MT_SERVER_OPENAPI_SPEC: &str = "application/vnd.pmcp.mcp-server.openapi-spec.v1";
/// A REFERENCE to a server binary the package does not embed: the digest and
/// media type of a blob the *target environment* resolves for itself. The
/// mutually-exclusive counterpart of [`MT_SERVER_BOOTSTRAP`] — a package
/// carries exactly one of the two, never both and never neither. The payload
/// is a canonical-JSON `BinaryRef`.
pub const MT_SERVER_BINARY_REF: &str = "application/vnd.pmcp.mcp-server.binary-ref.v1+json";

// ---------------------------------------------------------------------
// AgentPackage / TeamPackage / WorkflowManifest — one layer each
// ---------------------------------------------------------------------

/// The whole `AgentPackage` struct, serialized as a single JSON layer.
pub const MT_AGENT_CONFIG: &str = "application/vnd.pmcp.agent.config.v1+json";
/// The whole `TeamPackage` struct, serialized as a single JSON layer.
pub const MT_TEAM_CONFIG: &str = "application/vnd.pmcp.team.config.v1+json";
/// The whole `WorkflowManifest` struct, serialized as a single JSON layer.
pub const MT_WORKFLOW_MANIFEST: &str = "application/vnd.pmcp.workflow.manifest.v1+json";

// ---------------------------------------------------------------------
// artifactType per package kind (OCI 1.1 top-level manifest field)
// ---------------------------------------------------------------------

/// `artifactType` for an `mcp-server` package's `ImageManifest`.
pub const ARTIFACT_TYPE_SERVER: &str = "application/vnd.pmcp.mcp-server.v1";
/// `artifactType` for an `agent` package's `ImageManifest`.
pub const ARTIFACT_TYPE_AGENT: &str = "application/vnd.pmcp.agent.v1";
/// `artifactType` for a `team` package's `ImageManifest`.
pub const ARTIFACT_TYPE_TEAM: &str = "application/vnd.pmcp.team.v1";
/// `artifactType` for a `workflow` package's `ImageManifest`.
pub const ARTIFACT_TYPE_WORKFLOW: &str = "application/vnd.pmcp.workflow.v1";

// ---------------------------------------------------------------------
// Standard OCI empty-config blob (OCI 1.1 non-null config descriptor)
// ---------------------------------------------------------------------

/// Standard OCI 1.1 empty-config media type. Registries/validators (ECR)
/// require a non-null config descriptor even for artifact manifests that
/// have no meaningful "config" of their own.
pub const MT_EMPTY_CONFIG: &str = "application/vnd.oci.empty.v1+json";
/// The empty-config blob's exact content — the most minimal valid JSON
/// object.
pub const EMPTY_CONFIG_BLOB: &[u8] = b"{}";
/// `sha256:<hex>` of [`EMPTY_CONFIG_BLOB`]. Guarded against drift by the
/// `empty_config_digest_matches_hash_of_empty_json_blob` test below — this
/// constant can never silently diverge from the actual hash of `{}`.
pub const EMPTY_CONFIG_DIGEST: &str =
    "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a";
/// Byte length of [`EMPTY_CONFIG_BLOB`].
pub const EMPTY_CONFIG_SIZE: u64 = 2;

/// Map a vendor media-type string constant into an `oci_spec::image::MediaType`.
/// `MediaType::from(&str)` already routes unrecognized strings to
/// `MediaType::Other` (RESEARCH Pattern 3) — this helper just documents the
/// call site's intent.
pub fn vendor_media_type(media_type: &str) -> MediaType {
    MediaType::from(media_type)
}

/// Build the standard, shared empty-config `Descriptor` — non-null config,
/// required by ECR/OCI 1.1-conformant registries and validators. Callers
/// that actually WRITE the config blob to a layout should prefer
/// `OciLayout::write_blob(MediaType::from(MT_EMPTY_CONFIG), EMPTY_CONFIG_BLOB)`
/// (which persists the bytes AND returns an identical `Descriptor`, since
/// both hash the same fixed bytes); this function exists for callers that
/// only need the descriptor shape (e.g. tests, or a caller building a
/// manifest that references an empty-config blob written elsewhere).
pub fn empty_config_descriptor() -> Descriptor {
    let digest = oci_spec::image::Digest::from_str(EMPTY_CONFIG_DIGEST)
        .expect("EMPTY_CONFIG_DIGEST is a well-formed sha256 digest string");
    Descriptor::new(
        vendor_media_type(MT_EMPTY_CONFIG),
        EMPTY_CONFIG_SIZE,
        digest,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digest::ManifestDigest;

    /// Drift guard (Task 1 `<action>`): the checked-in `EMPTY_CONFIG_DIGEST`
    /// constant must always equal the actual sha256 of `EMPTY_CONFIG_BLOB` —
    /// this test fails loudly if the two are ever edited out of sync.
    #[test]
    fn empty_config_digest_matches_hash_of_empty_json_blob() {
        let computed = ManifestDigest::from_bytes(EMPTY_CONFIG_BLOB);
        assert_eq!(computed.as_str(), EMPTY_CONFIG_DIGEST);
    }

    #[test]
    fn vendor_media_type_wraps_custom_string_as_other() {
        let mt = vendor_media_type(MT_WORKFLOW_MANIFEST);
        assert_eq!(mt.to_string(), MT_WORKFLOW_MANIFEST);
        assert!(matches!(mt, MediaType::Other(_)));
    }

    #[test]
    fn empty_config_descriptor_uses_standard_media_type_size_and_digest() {
        let descriptor = empty_config_descriptor();
        assert_eq!(descriptor.media_type().to_string(), MT_EMPTY_CONFIG);
        assert_eq!(descriptor.size(), EMPTY_CONFIG_SIZE);
        assert_eq!(descriptor.digest().to_string(), EMPTY_CONFIG_DIGEST);
    }
}
