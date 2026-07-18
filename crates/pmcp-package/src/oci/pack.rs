//! `pack_server`/`pack_agent`/`pack_team`/`pack_workflow` — serialize each
//! package type into a local [`OciLayout`] (RESEARCH System Architecture
//! Diagram, pack() steps 1-5):
//!
//! 1. Serialize each logical layer to bytes via [`canonicalize`] (deterministic).
//! 2. `layout.write_blob()` each → a content-addressed `Descriptor` with the
//!    correct vendor `MediaType`.
//! 3. Build an `oci_spec::image::ImageManifest` (empty config + layer
//!    descriptors + `artifactType`).
//! 4. Canonicalize the manifest via `olpc-cjson`.
//! 5. `sha256(canonical manifest bytes)` — stored as the manifest blob's own
//!    content-addressed digest AND returned as the function's
//!    [`ManifestDigest`] (I-2 identity key). One hash, one source of truth —
//!    no separate re-derivation of "the" manifest digest.
//!
//! The `ServerPackage` bootstrap binary is a SEPARATE `&[u8]` parameter to
//! `pack_server`, never a field read off the struct (cross-AI review: binary
//! payloads are OCI layers, not typed-struct fields) — it becomes its own
//! `MT_SERVER_BOOTSTRAP` layer.

use crate::digest::{canonicalize, ManifestDigest};
use crate::error::{PackageError, Result};
use crate::oci::layout::OciLayout;
use crate::oci::media_types::{
    vendor_media_type, ARTIFACT_TYPE_SERVER, EMPTY_CONFIG_BLOB, MT_EMPTY_CONFIG, MT_SERVER_BOOTSTRAP,
    MT_SERVER_CEDAR_POLICY_SET, MT_SERVER_CONFIG_SLOTS, MT_SERVER_DEPLOY_DESCRIPTOR,
    MT_SERVER_ENVELOPE, MT_SERVER_TOOL_METADATA,
};
use crate::oci::SingleLayerPackage;
use crate::package::{AgentPackage, BinaryRef, ServerPackage, TeamPackage, WorkflowManifest};
use oci_spec::image::{Descriptor, ImageManifestBuilder, MediaType, SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// `ServerPackage` fields NOT covered by the deploy/cedar/tools/config-slots
/// layers — `name`, `version`, the top-level `digest`, and `binary_ref` —
/// packed as their own small JSON layer ([`MT_SERVER_ENVELOPE`]) so every
/// field round-trips losslessly by plain serialize/deserialize. `pub(super)`
/// so `oci::unpack` (a sibling module) can reconstruct a `ServerPackage`
/// from it.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct ServerEnvelope {
    pub(super) name: String,
    pub(super) version: semver::Version,
    pub(super) digest: Option<ManifestDigest>,
    pub(super) binary_ref: BinaryRef,
}

/// Pack `package` plus its `bootstrap` binary into `layout` as a local OCI
/// artifact. Returns the canonical I-2 manifest digest.
pub fn pack_server(
    package: &ServerPackage,
    bootstrap: &[u8],
    layout: &OciLayout,
) -> Result<ManifestDigest> {
    let envelope = ServerEnvelope {
        name: package.name.clone(),
        version: package.version.clone(),
        digest: package.digest.clone(),
        binary_ref: package.binary_ref.clone(),
    };

    let bootstrap_descriptor =
        layout.write_blob(vendor_media_type(MT_SERVER_BOOTSTRAP), bootstrap)?;
    let envelope_descriptor = layout.write_blob(
        vendor_media_type(MT_SERVER_ENVELOPE),
        &canonicalize(&envelope)?,
    )?;
    let deploy_descriptor = layout.write_blob(
        vendor_media_type(MT_SERVER_DEPLOY_DESCRIPTOR),
        &canonicalize(&package.deploy)?,
    )?;
    let cedar_descriptor = layout.write_blob(
        vendor_media_type(MT_SERVER_CEDAR_POLICY_SET),
        &canonicalize(&package.policies)?,
    )?;
    let tools_descriptor = layout.write_blob(
        vendor_media_type(MT_SERVER_TOOL_METADATA),
        &canonicalize(&package.tools)?,
    )?;
    let config_slots_descriptor = layout.write_blob(
        vendor_media_type(MT_SERVER_CONFIG_SLOTS),
        &canonicalize(&package.config_slots)?,
    )?;

    let layers = vec![
        bootstrap_descriptor,
        envelope_descriptor,
        deploy_descriptor,
        cedar_descriptor,
        tools_descriptor,
        config_slots_descriptor,
    ];

    finalize_pack(
        layout,
        layers,
        ARTIFACT_TYPE_SERVER,
        &package.name,
        &package.version,
    )
}

/// Pack any single-layer package (agent/team/workflow) into `layout`:
/// serialize it to one canonical-JSON config layer under its vendor media
/// type, then wrap it in a manifest with the kind's `artifactType`. The
/// per-kind constants come from the [`SingleLayerPackage`] impl — one path,
/// no per-kind copy-paste. Returns the canonical I-2 manifest digest.
fn pack_single_layer<P: SingleLayerPackage>(
    package: &P,
    layout: &OciLayout,
) -> Result<ManifestDigest> {
    let layer_descriptor =
        layout.write_blob(vendor_media_type(P::LAYER_MEDIA_TYPE), &canonicalize(package)?)?;
    finalize_pack(
        layout,
        vec![layer_descriptor],
        P::ARTIFACT_TYPE,
        package.name(),
        package.version(),
    )
}

/// Pack an `AgentPackage` into `layout` as a single-layer local OCI artifact.
/// Returns the canonical I-2 manifest digest.
pub fn pack_agent(package: &AgentPackage, layout: &OciLayout) -> Result<ManifestDigest> {
    pack_single_layer(package, layout)
}

/// Pack a `TeamPackage` into `layout` as a single-layer local OCI artifact.
/// Returns the canonical I-2 manifest digest.
pub fn pack_team(package: &TeamPackage, layout: &OciLayout) -> Result<ManifestDigest> {
    pack_single_layer(package, layout)
}

/// Pack a `WorkflowManifest` into `layout` as a single-layer local OCI
/// artifact. Returns the canonical I-2 manifest digest.
pub fn pack_workflow(package: &WorkflowManifest, layout: &OciLayout) -> Result<ManifestDigest> {
    pack_single_layer(package, layout)
}

/// Shared steps 3-6: write the standard empty-config blob, build the
/// `ImageManifest` (config + `layers` + `artifact_type`), store it under its
/// OWN canonical-bytes digest (step 4-5 — this IS the returned
/// [`ManifestDigest`], not a separate re-derivation), record it in
/// `index.json` with `name`/`version` annotations, and return that digest.
fn finalize_pack(
    layout: &OciLayout,
    layers: Vec<Descriptor>,
    artifact_type: &str,
    name: &str,
    version: &semver::Version,
) -> Result<ManifestDigest> {
    let config_descriptor =
        layout.write_blob(MediaType::from(MT_EMPTY_CONFIG), EMPTY_CONFIG_BLOB)?;

    let manifest = ImageManifestBuilder::default()
        .schema_version(SCHEMA_VERSION)
        .media_type(MediaType::ImageManifest)
        .artifact_type(MediaType::Other(artifact_type.to_string()))
        .config(config_descriptor)
        .layers(layers)
        .build()
        .map_err(|e| PackageError::Layout {
            reason: format!("failed to build ImageManifest: {e}"),
        })?;

    // Canonicalize (not plain serde_json) so the stored blob's own
    // content-addressed digest doubles as the I-2 identity digest —
    // one hash, one source of truth (RESEARCH steps 4-5).
    let manifest_bytes = canonicalize(&manifest)?;
    let mut manifest_descriptor = layout.write_manifest(&manifest_bytes)?;

    let annotations = HashMap::from([
        ("name".to_string(), name.to_string()),
        ("version".to_string(), version.to_string()),
    ]);
    manifest_descriptor.set_annotations(Some(annotations));

    let manifest_digest = ManifestDigest::try_from(manifest_descriptor.digest())?;

    let mut index = layout.read_index()?;
    let mut manifests = index.manifests().clone();
    manifests.push(manifest_descriptor);
    index.set_manifests(manifests);
    layout.write_index(&index)?;

    Ok(manifest_digest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oci::media_types::{MT_EMPTY_CONFIG, MT_SERVER_BOOTSTRAP};
    use crate::oci::unpack::tests_support::sample_server_package;

    #[test]
    fn pack_server_writes_bootstrap_layer_with_vendor_media_type() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let (package, bootstrap) = sample_server_package();

        pack_server(&package, &bootstrap, &layout).unwrap();

        let index = layout.read_index().unwrap();
        let manifest_descriptor = &index.manifests()[0];
        let manifest = layout.read_manifest(manifest_descriptor).unwrap();
        assert_eq!(
            manifest.layers()[0].media_type().to_string(),
            MT_SERVER_BOOTSTRAP
        );
        assert_eq!(manifest.layers()[0].size(), bootstrap.len() as u64);
    }

    #[test]
    fn pack_uses_standard_non_null_empty_config_descriptor() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let (package, bootstrap) = sample_server_package();

        pack_server(&package, &bootstrap, &layout).unwrap();

        let index = layout.read_index().unwrap();
        let manifest = layout.read_manifest(&index.manifests()[0]).unwrap();
        assert_eq!(manifest.config().media_type().to_string(), MT_EMPTY_CONFIG);
        assert_eq!(manifest.config().size(), 2);
    }

    #[test]
    fn packing_the_same_package_twice_yields_an_identical_manifest_digest() {
        let (package, bootstrap) = sample_server_package();

        let dir_a = tempfile::tempdir().unwrap();
        let layout_a = OciLayout::create(dir_a.path()).unwrap();
        let digest_a = pack_server(&package, &bootstrap, &layout_a).unwrap();

        let dir_b = tempfile::tempdir().unwrap();
        let layout_b = OciLayout::create(dir_b.path()).unwrap();
        let digest_b = pack_server(&package, &bootstrap, &layout_b).unwrap();

        assert_eq!(
            digest_a, digest_b,
            "I-2: packing identical input must yield an identical digest"
        );
    }

    #[test]
    fn index_json_records_manifest_with_name_and_version_annotations() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let (package, bootstrap) = sample_server_package();

        pack_server(&package, &bootstrap, &layout).unwrap();

        let index = layout.read_index().unwrap();
        let annotations = index.manifests()[0].annotations().as_ref().unwrap();
        assert_eq!(annotations.get("name"), Some(&package.name));
        assert_eq!(
            annotations.get("version"),
            Some(&package.version.to_string())
        );
    }
}
