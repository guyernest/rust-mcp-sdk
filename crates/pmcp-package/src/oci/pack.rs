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
//!    [`ManifestDigest`] (identity key). One hash, one source of truth —
//!    no separate re-derivation of "the" manifest digest.
//!
//! The `ServerPackage` binary is a SEPARATE [`BinaryMode`] parameter to
//! `pack_server`, never a field read off the struct (cross-AI review: binary
//! payloads are OCI layers, not typed-struct fields) — it becomes either an
//! `MT_SERVER_BOOTSTRAP` layer (embedded bytes) or an `MT_SERVER_BINARY_REF`
//! layer (a digest the target environment resolves), never both.

use crate::digest::{canonicalize, ManifestDigest};
use crate::error::{PackageError, Result};
use crate::oci::layout::OciLayout;
use crate::oci::media_types::{
    vendor_media_type, ARTIFACT_TYPE_SERVER, EMPTY_CONFIG_BLOB, MT_EMPTY_CONFIG,
    MT_SERVER_BINARY_REF, MT_SERVER_BOOTSTRAP, MT_SERVER_CEDAR_POLICY_SET, MT_SERVER_CONFIG,
    MT_SERVER_CONFIG_SLOTS, MT_SERVER_DEPLOY_DESCRIPTOR, MT_SERVER_ENVELOPE,
    MT_SERVER_TOOL_METADATA,
};
use crate::oci::SingleLayerPackage;
use crate::package::{AgentPackage, BinaryRef, ServerPackage, TeamPackage, WorkflowManifest};
use oci_spec::image::{
    Descriptor, ImageManifestBuilder, MediaType, ANNOTATION_TITLE, SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// `ServerPackage` fields NOT covered by the deploy/cedar/tools/config-slots
/// layers — `name`, `version` and the top-level `digest` — packed as their own
/// small JSON layer ([`MT_SERVER_ENVELOPE`]) so every field round-trips
/// losslessly by plain serialize/deserialize. `pub(super)` so `oci::unpack`
/// (a sibling module) can reconstruct a `ServerPackage` from it.
///
/// The binary reference is deliberately NOT a field here (D-08): it lives in
/// its own [`MT_SERVER_BINARY_REF`] layer, so "which binary" is one fact in
/// one place rather than a struct field that could disagree with a layer.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct ServerEnvelope {
    pub(super) name: String,
    pub(super) version: semver::Version,
    pub(super) digest: Option<ManifestDigest>,
}

/// How a packed server names its binary — exactly one of two mutually
/// exclusive shapes (D-05, D-06). A package always carries one binary layer;
/// which of the two it is decides whether the package is self-contained or
/// resolved by the target environment.
///
/// [`BinaryMode::Referenced`]'s `digest` is taken VERBATIM from the caller and
/// is NON-optional: `pmcp-package` never contacts a registry to derive or
/// confirm it (milestone Decision 2 forbids a registry client), and a
/// referenced binary with no digest would be an instruction to run an
/// unpinned binary. The canonical producer in this repo is
/// `cargo-pmcp`'s `deployment::targets::aws_lambda::artifact`, which builds it
/// via [`ManifestDigest::from_bytes`] against the release `.sha256` sidecar.
///
/// Note that a bare-hex sidecar (`<64 hex chars>` with no prefix) is NOT a
/// valid digest string — [`ManifestDigest::parse`] requires the `sha256:`
/// prefix, so prepend it before parsing.
#[derive(Debug, Clone)]
pub enum BinaryMode<'a> {
    /// The binary's bytes are embedded in the package as an
    /// [`MT_SERVER_BOOTSTRAP`] layer. The package is self-contained.
    Embedded(&'a [u8]),
    /// The binary is NOT embedded: the package records only its digest and
    /// media type in an [`MT_SERVER_BINARY_REF`] layer, and the target
    /// environment resolves the bytes for itself.
    Referenced {
        /// Content digest of the binary the target environment must run.
        digest: ManifestDigest,
        /// Descriptive media-type hint for that binary (e.g.
        /// `application/x-lambda-bootstrap; arch=arm64`).
        media_type: String,
    },
}

/// The author's server config file, carried into the package VERBATIM.
///
/// `bytes` are written to the [`MT_SERVER_CONFIG`] layer byte-for-byte and are
/// never re-derived from a parsed struct — packing must not rewrite,
/// templatize, normalize or reformat what the author wrote. `file_name` is
/// recorded in the layer descriptor's `org.opencontainers.image.title`
/// annotation so unpack can restore it under its original name.
///
/// Distinct from [`OpenApiSpecFile`] on purpose: two types rather than one
/// shared "named blob" means a caller cannot transpose the config and the spec.
#[derive(Debug, Clone, Copy)]
pub struct ConfigFile<'a> {
    /// The config file's original name, e.g. `london-tube.toml`.
    pub file_name: &'a str,
    /// The config file's exact bytes.
    pub bytes: &'a [u8],
}

/// An OpenAPI-backed server's spec file, carried into the package VERBATIM.
///
/// The spec sibling of [`ConfigFile`], with the same verbatim-bytes and
/// original-file-name contract, written to the [`MT_SERVER_OPENAPI_SPEC`]
/// layer.
///
/// [`MT_SERVER_OPENAPI_SPEC`]: crate::oci::media_types::MT_SERVER_OPENAPI_SPEC
#[derive(Debug, Clone, Copy)]
pub struct OpenApiSpecFile<'a> {
    /// The spec file's original name, e.g. `openapi.yaml`.
    pub file_name: &'a str,
    /// The spec file's exact bytes.
    pub bytes: &'a [u8],
}

/// Build the layer descriptor for a named vendor-content file, recording the
/// author's original file name in the descriptor's standard
/// `org.opencontainers.image.title` annotation.
///
/// Unlike the index-descriptor annotations set in [`finalize_pack`] — which are
/// applied AFTER `write_manifest` has already computed the manifest digest and
/// therefore do NOT feed it — a LAYER descriptor's annotations live inside the
/// manifest that `canonicalize` then hashes, so this annotation DOES feed the
/// manifest digest. Renaming the config file changes the package's identity.
fn write_named_file_layer(
    layout: &OciLayout,
    media_type: &str,
    file_name: &str,
    bytes: &[u8],
) -> Result<Descriptor> {
    // Raw author bytes, never `canonicalize` — the crate rule is to digest
    // what was stored, never re-derive it from a parsed struct.
    let mut descriptor = layout.write_blob(vendor_media_type(media_type), bytes)?;
    descriptor.set_annotations(Some(HashMap::from([(
        ANNOTATION_TITLE.to_string(),
        file_name.to_string(),
    )])));
    Ok(descriptor)
}

/// Write the one binary layer this package carries, per [`BinaryMode`].
///
/// Embedded bytes go through `write_blob` unchanged. A reference is a STRUCT
/// layer: the existing [`BinaryRef`] wire type serialized via `canonicalize`,
/// keeping the wire payload's `Option<ManifestDigest>` tolerance while the API
/// type [`BinaryMode::Referenced`] stays non-optional.
fn write_binary_layer(layout: &OciLayout, binary: &BinaryMode<'_>) -> Result<Descriptor> {
    match binary {
        BinaryMode::Embedded(bootstrap) => {
            layout.write_blob(vendor_media_type(MT_SERVER_BOOTSTRAP), bootstrap)
        },
        BinaryMode::Referenced { digest, media_type } => {
            let binary_ref = BinaryRef {
                digest: Some(digest.clone()),
                media_type: media_type.clone(),
            };
            layout.write_blob(
                vendor_media_type(MT_SERVER_BINARY_REF),
                &canonicalize(&binary_ref)?,
            )
        },
    }
}

/// Pack `package` plus its binary (embedded or referenced) and its optional
/// verbatim config/spec files into `layout` as a local OCI artifact. Returns
/// the canonical manifest digest.
///
/// `config` and `spec` are both optional because an embedded, pre-built server
/// package has neither: they exist for the Shape A pure-config servers, whose
/// entire identity is their config (plus, for OpenAPI, their spec).
///
/// # Errors
///
/// Returns [`PackageError::Layout`] if `spec` is `Some` (the spec layer is not
/// wired yet — it is accepted and explicitly refused rather than silently
/// discarded), if the `ImageManifest` fails to build, or if any blob/index
/// write fails. Returns [`PackageError::Serialize`] if a layer fails to
/// canonicalize.
///
/// [`PackageError::Serialize`]: crate::error::PackageError::Serialize
pub fn pack_server(
    package: &ServerPackage,
    binary: BinaryMode<'_>,
    config: Option<ConfigFile<'_>>,
    spec: Option<OpenApiSpecFile<'_>>,
    layout: &OciLayout,
) -> Result<ManifestDigest> {
    if let Some(spec) = spec {
        // Accepted in the signature so callers can be written once, but not
        // yet wired — refuse loudly rather than dropping the author's bytes.
        return Err(PackageError::Layout {
            reason: format!(
                "spec layer not yet supported (refusing to silently discard '{}')",
                spec.file_name
            ),
        });
    }

    let envelope = ServerEnvelope {
        name: package.name.clone(),
        version: package.version.clone(),
        digest: package.digest.clone(),
    };

    // Push order is deterministic so the manifest bytes (and therefore the
    // manifest digest) are reproducible. It is NOT a read-order contract:
    // the optional layers make position meaningless, and `unpack_server`
    // locates every layer by media type.
    let mut layers = vec![
        write_binary_layer(layout, &binary)?,
        layout.write_blob(
            vendor_media_type(MT_SERVER_ENVELOPE),
            &canonicalize(&envelope)?,
        )?,
        layout.write_blob(
            vendor_media_type(MT_SERVER_DEPLOY_DESCRIPTOR),
            &canonicalize(&package.deploy)?,
        )?,
        layout.write_blob(
            vendor_media_type(MT_SERVER_CEDAR_POLICY_SET),
            &canonicalize(&package.policies)?,
        )?,
        layout.write_blob(
            vendor_media_type(MT_SERVER_TOOL_METADATA),
            &canonicalize(&package.tools)?,
        )?,
        layout.write_blob(
            vendor_media_type(MT_SERVER_CONFIG_SLOTS),
            &canonicalize(&package.config_slots)?,
        )?,
    ];

    if let Some(config) = config {
        layers.push(write_named_file_layer(
            layout,
            MT_SERVER_CONFIG,
            config.file_name,
            config.bytes,
        )?);
    }

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
/// no per-kind copy-paste. Returns the canonical manifest digest.
fn pack_single_layer<P: SingleLayerPackage>(
    package: &P,
    layout: &OciLayout,
) -> Result<ManifestDigest> {
    let layer_descriptor = layout.write_blob(
        vendor_media_type(P::LAYER_MEDIA_TYPE),
        &canonicalize(package)?,
    )?;
    finalize_pack(
        layout,
        vec![layer_descriptor],
        P::ARTIFACT_TYPE,
        package.name(),
        package.version(),
    )
}

/// Pack an `AgentPackage` into `layout` as a single-layer local OCI artifact.
/// Returns the canonical manifest digest.
pub fn pack_agent(package: &AgentPackage, layout: &OciLayout) -> Result<ManifestDigest> {
    pack_single_layer(package, layout)
}

/// Pack a `TeamPackage` into `layout` as a single-layer local OCI artifact.
/// Returns the canonical manifest digest.
pub fn pack_team(package: &TeamPackage, layout: &OciLayout) -> Result<ManifestDigest> {
    pack_single_layer(package, layout)
}

/// Pack a `WorkflowManifest` into `layout` as a single-layer local OCI
/// artifact. Returns the canonical manifest digest.
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
    // content-addressed digest doubles as the identity digest —
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
    use crate::oci::media_types::{MT_EMPTY_CONFIG, MT_SERVER_BINARY_REF, MT_SERVER_BOOTSTRAP};
    use crate::oci::unpack::tests_support::sample_server_package;

    #[test]
    fn pack_server_writes_bootstrap_layer_with_vendor_media_type() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let (package, bootstrap) = sample_server_package();

        pack_server(
            &package,
            BinaryMode::Embedded(&bootstrap),
            None,
            None,
            &layout,
        )
        .unwrap();

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
    fn pack_server_referenced_writes_binary_ref_layer_and_no_bootstrap_layer() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let (package, _bootstrap) = sample_server_package();

        pack_server(
            &package,
            BinaryMode::Referenced {
                digest: ManifestDigest::from_bytes(b"referenced-binary"),
                media_type: "application/x-lambda-bootstrap; arch=arm64".to_string(),
            },
            None,
            None,
            &layout,
        )
        .unwrap();

        let index = layout.read_index().unwrap();
        let manifest = layout.read_manifest(&index.manifests()[0]).unwrap();
        let media_types: Vec<String> = manifest
            .layers()
            .iter()
            .map(|l| l.media_type().to_string())
            .collect();
        assert!(media_types.iter().any(|m| m == MT_SERVER_BINARY_REF));
        assert!(!media_types.iter().any(|m| m == MT_SERVER_BOOTSTRAP));
    }

    #[test]
    fn pack_server_refuses_a_spec_rather_than_silently_discarding_it() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let (package, bootstrap) = sample_server_package();

        let err = pack_server(
            &package,
            BinaryMode::Embedded(&bootstrap),
            None,
            Some(OpenApiSpecFile {
                file_name: "openapi.yaml",
                bytes: b"openapi: 3.1.0",
            }),
            &layout,
        )
        .unwrap_err();

        assert!(matches!(err, PackageError::Layout { .. }), "got {err:?}");
    }

    #[test]
    fn pack_uses_standard_non_null_empty_config_descriptor() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let (package, bootstrap) = sample_server_package();

        pack_server(
            &package,
            BinaryMode::Embedded(&bootstrap),
            None,
            None,
            &layout,
        )
        .unwrap();

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
        let digest_a = pack_server(
            &package,
            BinaryMode::Embedded(&bootstrap),
            None,
            None,
            &layout_a,
        )
        .unwrap();

        let dir_b = tempfile::tempdir().unwrap();
        let layout_b = OciLayout::create(dir_b.path()).unwrap();
        let digest_b = pack_server(
            &package,
            BinaryMode::Embedded(&bootstrap),
            None,
            None,
            &layout_b,
        )
        .unwrap();

        assert_eq!(
            digest_a, digest_b,
            ": packing identical input must yield an identical digest"
        );
    }

    #[test]
    fn index_json_records_manifest_with_name_and_version_annotations() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let (package, bootstrap) = sample_server_package();

        pack_server(
            &package,
            BinaryMode::Embedded(&bootstrap),
            None,
            None,
            &layout,
        )
        .unwrap();

        let index = layout.read_index().unwrap();
        let annotations = index.manifests()[0].annotations().as_ref().unwrap();
        assert_eq!(annotations.get("name"), Some(&package.name));
        assert_eq!(
            annotations.get("version"),
            Some(&package.version.to_string())
        );
    }
}
