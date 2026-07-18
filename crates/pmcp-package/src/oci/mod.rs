//! Local OCI Image Layout pack/unpack (Phase 168's titular scope): serialize
//! each of the four package types into an OCI Image Layout on disk with
//! custom `application/vnd.pmcp.*` media types per layer, content-address
//! each blob by sha256, and verify every blob's digest before deserializing
//! (I-1/I-2). Pure local disk I/O — no network, no `oci-client` — so
//! Phase 169's registry push/pull can consume these exact `oci_spec::image`
//! types with zero translation.
//!
//! - [`media_types`] — vendor media-type constants per layer + the standard
//!   OCI empty-config blob constants.
//! - [`layout`] — [`OciLayout`], the local Image Layout directory
//!   reader/writer (`oci-layout` + `index.json` + `blobs/sha256/<hex>`).
//! - [`pack`] — `pack_server`/`pack_agent`/`pack_team`/`pack_workflow`.
//! - [`unpack`] — `unpack_server`/`unpack_agent`/`unpack_team`/`unpack_workflow`.

pub mod layout;
pub mod media_types;
pub mod pack;
pub mod unpack;

pub use layout::OciLayout;
pub use pack::{pack_agent, pack_server, pack_team, pack_workflow};
pub use unpack::{unpack_agent, unpack_server, unpack_team, unpack_workflow};

use crate::oci::media_types::{
    ARTIFACT_TYPE_AGENT, ARTIFACT_TYPE_TEAM, ARTIFACT_TYPE_WORKFLOW, MT_AGENT_CONFIG, MT_TEAM_CONFIG,
    MT_WORKFLOW_MANIFEST,
};
use crate::package::{AgentPackage, TeamPackage, WorkflowManifest};

/// The three single-layer package kinds (agent/team/workflow) share ONE
/// pack/unpack path: serialize to a single canonical-JSON config layer under a
/// vendor media type, wrapped in a manifest carrying the kind's `artifactType`.
/// This trait is the single source of truth binding each kind to its
/// media-type / artifact-type / layer-name constants, so
/// [`pack::pack_single_layer`] and [`unpack::unpack_single_layer`] are fully
/// generic over it (no per-kind copy-paste). `ServerPackage` is deliberately
/// NOT a member — it is multi-layer (bootstrap + envelope + 4 typed sections)
/// and keeps its own bespoke pack/unpack path.
pub(crate) trait SingleLayerPackage: serde::Serialize + serde::de::DeserializeOwned {
    /// Vendor media type for this kind's single config layer.
    const LAYER_MEDIA_TYPE: &'static str;
    /// OCI `artifactType` recorded on the manifest.
    const ARTIFACT_TYPE: &'static str;
    /// Human-readable layer name used in "missing layer" errors.
    const LAYER_NAME: &'static str;
    /// The package's declared name (used for `index.json` annotations).
    fn name(&self) -> &str;
    /// The package's declared version (used for `index.json` annotations).
    fn version(&self) -> &semver::Version;
}

impl SingleLayerPackage for AgentPackage {
    const LAYER_MEDIA_TYPE: &'static str = MT_AGENT_CONFIG;
    const ARTIFACT_TYPE: &'static str = ARTIFACT_TYPE_AGENT;
    const LAYER_NAME: &'static str = "agent-config";
    fn name(&self) -> &str {
        &self.name
    }
    fn version(&self) -> &semver::Version {
        &self.version
    }
}

impl SingleLayerPackage for TeamPackage {
    const LAYER_MEDIA_TYPE: &'static str = MT_TEAM_CONFIG;
    const ARTIFACT_TYPE: &'static str = ARTIFACT_TYPE_TEAM;
    const LAYER_NAME: &'static str = "team-config";
    fn name(&self) -> &str {
        &self.name
    }
    fn version(&self) -> &semver::Version {
        &self.version
    }
}

impl SingleLayerPackage for WorkflowManifest {
    const LAYER_MEDIA_TYPE: &'static str = MT_WORKFLOW_MANIFEST;
    const ARTIFACT_TYPE: &'static str = ARTIFACT_TYPE_WORKFLOW;
    const LAYER_NAME: &'static str = "workflow-manifest";
    fn name(&self) -> &str {
        &self.name
    }
    fn version(&self) -> &semver::Version {
        &self.version
    }
}
