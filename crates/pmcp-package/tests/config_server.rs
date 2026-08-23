//! Integration test: the config-only (Shape A) server package, end to end.
//!
//! A pure-config server has no binary of its own — its entire identity is its
//! config file plus a REFERENCE to a runtime binary the target environment
//! resolves. These tests prove that whole path exists and closes: pack with
//! [`BinaryMode::Referenced`] and a [`ConfigFile`], unpack, and get the
//! author's exact bytes back under the author's original file name, with no
//! bootstrap layer anywhere in the manifest.
//!
//! Every layout is a `tempfile::tempdir()` — never a hand-rolled temp dir.
//! The `ServerPackage` fixture is built inline here rather than reaching into
//! the crate's `#[cfg(test)]` fixtures, so this file exercises the same public
//! API an external consumer has.

use proptest::prelude::*;

use pmcp_package::digest::ManifestDigest;
use pmcp_package::error::{PackageError, Result};
use pmcp_package::oci::media_types::{
    MT_SERVER_BOOTSTRAP, MT_SERVER_CONFIG, MT_SERVER_OPENAPI_SPEC,
};
use pmcp_package::oci::{
    pack_server, unpack_server, BinaryMode, ConfigFile, OciLayout, OpenApiSpecFile, UnpackedBinary,
};
use pmcp_package::package::{
    AssetsSection, AuthSection, AwsSection, CedarPolicySet, DeployDescriptor, ObservabilitySection,
    ServerPackage, ServerSection, TargetSection, ToolMetadata,
};
use pmcp_package::slot::{ConfigSlot, SlotType};
use std::collections::BTreeMap;

/// The author's `config.toml`, verbatim — the bytes a Shape A server's whole
/// identity rests on. Deliberately holds comments, blank lines and
/// non-alphabetical key order: if pack ever normalized or re-derived the
/// config from a parsed struct, this content would not survive byte-for-byte.
const CONFIG_TOML: &[u8] = br#"# london-tube MCP server
name    = "london-tube"
version = "1.0.0"

[[config_slots]]
key = "backend.api_key"
kind = "secret"
name = "TFL_API_KEY"

[backend]
kind = "openapi"
# a trailing comment, and irregular   spacing
base_url = "https://api.tfl.gov.uk"
# A slot-declared credential key: it holds an environment REFERENCE, never a
# resolved literal, which is exactly what `pack_server` now enforces.
api_key = "${TFL_API_KEY}"
"#;

const CONFIG_FILE_NAME: &str = "london-tube.toml";

fn minimal_deploy_descriptor() -> DeployDescriptor {
    DeployDescriptor {
        target: TargetSection {
            target_type: "pmcp-run".to_string(),
            version: "1.0.0".to_string(),
        },
        metadata: None,
        aws: AwsSection {
            region: "us-east-1".to_string(),
        },
        server: ServerSection {
            name: "london-tube".to_string(),
            memory_mb: Some(1024),
            timeout_seconds: 30,
            memory: None,
            cpu: None,
            ingress: None,
            allow_unauthenticated: None,
            binary: None,
        },
        environment: BTreeMap::from([("RUST_LOG".to_string(), "info".to_string())]),
        secrets: BTreeMap::new(),
        auth: AuthSection {
            enabled: false,
            provider: "none".to_string(),
            callback_urls: vec![],
            cognito: None,
            dcr: None,
            groups: None,
            scopes: None,
        },
        observability: ObservabilitySection {
            log_retention_days: 30,
            enable_xray: true,
            create_dashboard: true,
            alarms: None,
        },
        composition: None,
        assets: Some(AssetsSection {
            include: vec![],
            exclude: vec!["**/*.tmp".to_string()],
        }),
        iam: None,
        gcp: None,
        layout: None,
    }
}

/// A representative pure-config `ServerPackage`. Note it carries no binary
/// information at all — that is a layer, not a field (D-08).
fn config_server_package() -> ServerPackage {
    ServerPackage {
        name: "london-tube".to_string(),
        version: semver::Version::parse("1.0.0").unwrap(),
        digest: None,
        deploy: minimal_deploy_descriptor(),
        policies: CedarPolicySet(vec![]),
        tools: vec![ToolMetadata {
            name: "get_status".to_string(),
            description: "Current status of every tube line".to_string(),
            annotations: Some(serde_json::json!({ "read_only_hint": true })),
        }],
        // Agrees exactly with CONFIG_TOML's single `[[config_slots]]` entry —
        // `pack_server` compares the two and refuses a package that claims a
        // slot its shipped config does not declare (or vice versa).
        config_slots: vec![ConfigSlot::new(SlotType::Secret {
            name: "TFL_API_KEY".to_string(),
        })
        .with_config_key("backend.api_key")],
    }
}

/// The digest of the runtime binary the target environment must resolve.
/// Supplied verbatim by the caller — `pmcp-package` never derives or confirms
/// it (no registry client by design).
fn referenced_binary_digest() -> ManifestDigest {
    ManifestDigest::from_bytes(b"pmcp-openapi-server-v1.0.0-aarch64")
}

const REFERENCED_MEDIA_TYPE: &str = "application/x-lambda-bootstrap; arch=arm64";

fn referenced_binary() -> BinaryMode<'static> {
    BinaryMode::Referenced {
        digest: referenced_binary_digest(),
        media_type: REFERENCED_MEDIA_TYPE.to_string(),
    }
}

fn config_file() -> ConfigFile<'static> {
    ConfigFile {
        file_name: CONFIG_FILE_NAME,
        bytes: CONFIG_TOML,
    }
}

// ---------------------------------------------------------------------
// PKG-01: a config-only package round-trips its config verbatim
// ---------------------------------------------------------------------

#[test]
fn config_only_package_restores_config_bytes_verbatim_under_its_original_name() {
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();

    pack_server(
        &config_server_package(),
        referenced_binary(),
        Some(config_file()),
        None,
        &layout,
    )
    .unwrap();

    let unpacked = unpack_server(&layout).unwrap();
    let config = unpacked
        .config
        .expect("a package packed WITH a config must unpack WITH a config");

    assert_eq!(
        config.bytes, CONFIG_TOML,
        "config bytes must be byte-identical to what the author supplied — pack must never \
         rewrite, templatize, normalize or reformat them"
    );
    assert_eq!(
        config.file_name, CONFIG_FILE_NAME,
        "the author's original file name must survive the round trip"
    );
    assert_eq!(
        unpacked.spec, None,
        "a package packed without a spec must unpack without one"
    );
}

#[test]
fn config_only_package_manifest_carries_no_bootstrap_layer() {
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();

    pack_server(
        &config_server_package(),
        referenced_binary(),
        Some(config_file()),
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

    assert!(
        !media_types.iter().any(|m| m == MT_SERVER_BOOTSTRAP),
        "a config-only package embeds no binary, so no bootstrap layer may exist: {media_types:?}"
    );
    assert!(
        media_types.iter().any(|m| m == MT_SERVER_CONFIG),
        "the config layer must be present: {media_types:?}"
    );
}

// ---------------------------------------------------------------------
// PKG-02: the referenced-binary arm carries a digest and no bytes
// ---------------------------------------------------------------------

#[test]
fn config_only_package_unpacks_to_referenced_binary_with_the_callers_digest() {
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();

    pack_server(
        &config_server_package(),
        referenced_binary(),
        Some(config_file()),
        None,
        &layout,
    )
    .unwrap();

    let unpacked = unpack_server(&layout).unwrap();

    match unpacked.binary {
        UnpackedBinary::Referenced { digest, media_type } => {
            assert_eq!(
                digest,
                referenced_binary_digest(),
                "the referenced digest must be passed through verbatim"
            );
            assert_eq!(media_type, REFERENCED_MEDIA_TYPE);
        },
        UnpackedBinary::Embedded(_) => {
            panic!(
                "a package packed as Referenced must never unpack as Embedded — unpack is a \
                    local, offline operation and must not resolve a local binary (D-07)"
            )
        },
    }
}

// ---------------------------------------------------------------------
// The embedded path is unchanged in behaviour
// ---------------------------------------------------------------------

#[test]
fn an_embedded_package_still_round_trips_its_bootstrap_bytes() {
    let bootstrap = b"fake-arm64-bootstrap-binary-bytes-for-testing".to_vec();
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();

    pack_server(
        &config_server_package(),
        BinaryMode::Embedded(&bootstrap),
        None,
        None,
        &layout,
    )
    .unwrap();

    let unpacked = unpack_server(&layout).unwrap();

    assert_eq!(unpacked.binary, UnpackedBinary::Embedded(bootstrap));
    assert_eq!(
        unpacked.config, None,
        "a package packed without a config must unpack without one"
    );
}

// ---------------------------------------------------------------------
// Determinism: pack is environment-independent
// ---------------------------------------------------------------------

#[test]
fn packing_identical_config_only_inputs_into_two_layouts_yields_one_digest() {
    let package = config_server_package();

    let dir_a = tempfile::tempdir().unwrap();
    let layout_a = OciLayout::create(dir_a.path()).unwrap();
    let digest_a = pack_server(
        &package,
        referenced_binary(),
        Some(config_file()),
        None,
        &layout_a,
    )
    .unwrap();

    let dir_b = tempfile::tempdir().unwrap();
    let layout_b = OciLayout::create(dir_b.path()).unwrap();
    let digest_b = pack_server(
        &package,
        referenced_binary(),
        Some(config_file()),
        None,
        &layout_b,
    )
    .unwrap();

    assert_eq!(
        digest_a, digest_b,
        "pack must be deterministic and environment-independent: a repeated or concurrent pack \
         of identical inputs cannot produce a different package"
    );
}

// ---------------------------------------------------------------------
// A referenced binary must never be unpinned
// ---------------------------------------------------------------------

#[test]
fn a_binary_ref_layer_with_no_digest_is_rejected_at_unpack() {
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();

    pack_server(
        &config_server_package(),
        referenced_binary(),
        Some(config_file()),
        None,
        &layout,
    )
    .unwrap();

    // Rewrite the binary-ref layer with a wire payload whose `digest` decodes
    // to `None` — the one shape the tolerant wire type admits and the API type
    // cannot express. The target environment must never be handed an
    // instruction to run an unpinned binary.
    let index = layout.read_index().unwrap();
    let mut manifest = layout.read_manifest(&index.manifests()[0]).unwrap();
    let unpinned = serde_json::to_vec(&serde_json::json!({
        "media_type": REFERENCED_MEDIA_TYPE,
    }))
    .unwrap();
    let new_layer = layout
        .write_blob(
            oci_spec::image::MediaType::from(pmcp_package::oci::media_types::MT_SERVER_BINARY_REF),
            &unpinned,
        )
        .unwrap();
    let layers: Vec<_> = manifest
        .layers()
        .iter()
        .map(|l| {
            if l.media_type().to_string() == pmcp_package::oci::media_types::MT_SERVER_BINARY_REF {
                new_layer.clone()
            } else {
                l.clone()
            }
        })
        .collect();
    manifest.set_layers(layers);
    let manifest_bytes = pmcp_package::digest::canonicalize(&manifest).unwrap();
    let manifest_descriptor = layout.write_manifest(&manifest_bytes).unwrap();
    let new_index = oci_spec::image::ImageIndexBuilder::default()
        .schema_version(oci_spec::image::SCHEMA_VERSION)
        .manifests(vec![manifest_descriptor])
        .build()
        .unwrap();
    layout.write_index(&new_index).unwrap();

    let err = unpack_server(&layout).unwrap_err();
    assert!(
        matches!(err, pmcp_package::PackageError::Layout { .. }),
        "an unpinned binary reference must be a Layout error, got {err:?}"
    );
}

// ---------------------------------------------------------------------
// PKG-01: the OPTIONAL OpenAPI spec layer (D-14, D-15, D-16)
// ---------------------------------------------------------------------

/// The author's OpenAPI document, verbatim. Deliberately YAML with comments
/// and irregular spacing: if pack ever parsed and re-emitted the spec, this
/// content would not survive byte-for-byte.
const SPEC_YAML: &[u8] = br#"# london-tube OpenAPI contract
openapi: 3.1.0
info:
  title:   London Tube API
  version: "1.0.0"
paths: {}
"#;

const SPEC_FILE_NAME: &str = "london-tube-api.yaml";

fn spec_file() -> OpenApiSpecFile<'static> {
    OpenApiSpecFile {
        file_name: SPEC_FILE_NAME,
        bytes: SPEC_YAML,
    }
}

fn layer_media_types(layout: &OciLayout) -> Vec<String> {
    let index = layout.read_index().unwrap();
    let manifest = layout.read_manifest(&index.manifests()[0]).unwrap();
    manifest
        .layers()
        .iter()
        .map(|l| l.media_type().to_string())
        .collect()
}

#[test]
fn a_packed_spec_restores_its_bytes_verbatim_under_its_original_name() {
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();

    pack_server(
        &config_server_package(),
        referenced_binary(),
        Some(config_file()),
        Some(spec_file()),
        &layout,
    )
    .unwrap();

    let unpacked = unpack_server(&layout).unwrap();
    let spec = unpacked
        .spec
        .expect("a package packed WITH a spec must unpack WITH a spec");

    assert_eq!(
        spec.bytes, SPEC_YAML,
        "spec bytes must be byte-identical to what the author supplied — pack must never parse, \
         reformat or re-emit them"
    );
    assert_eq!(
        spec.file_name, SPEC_FILE_NAME,
        "the author's original spec file name must survive the round trip"
    );
}

#[test]
fn a_package_packed_without_a_spec_carries_no_spec_layer_at_all() {
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();

    pack_server(
        &config_server_package(),
        referenced_binary(),
        Some(config_file()),
        None,
        &layout,
    )
    .unwrap();

    let media_types = layer_media_types(&layout);
    assert!(
        !media_types.iter().any(|m| m == MT_SERVER_OPENAPI_SPEC),
        "absence of a spec is the absence of the layer — never an absence marker: {media_types:?}"
    );

    let unpacked = unpack_server(&layout).unwrap();
    assert_eq!(
        unpacked.spec, None,
        "a curated-only server (pmcp-openapi-server's `--spec: Option<PathBuf>`) must pack and \
         unpack cleanly with no spec"
    );
}

#[test]
fn a_json_spec_round_trips_under_exactly_the_same_media_type_as_a_yaml_one() {
    let json_spec = OpenApiSpecFile {
        file_name: "api.json",
        bytes: b"{}",
    };

    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();
    pack_server(
        &config_server_package(),
        referenced_binary(),
        Some(config_file()),
        Some(json_spec),
        &layout,
    )
    .unwrap();

    let unpacked = unpack_server(&layout).unwrap();
    let spec = unpacked.spec.expect("the JSON spec layer must be present");
    assert_eq!(spec.bytes, b"{}", "JSON spec bytes must survive verbatim");
    assert_eq!(spec.file_name, "api.json");

    let json_media_types = layer_media_types(&layout);
    assert!(
        json_media_types.iter().any(|m| m == MT_SERVER_OPENAPI_SPEC),
        "a JSON spec uses the SAME media type as a YAML one — the format is evident from the \
         file-name annotation, not from a second media type: {json_media_types:?}"
    );

    let yaml_dir = tempfile::tempdir().unwrap();
    let yaml_layout = OciLayout::create(yaml_dir.path()).unwrap();
    pack_server(
        &config_server_package(),
        referenced_binary(),
        Some(config_file()),
        Some(spec_file()),
        &yaml_layout,
    )
    .unwrap();
    let yaml_media_types = layer_media_types(&yaml_layout);

    assert_eq!(
        json_media_types
            .iter()
            .filter(|m| *m == MT_SERVER_OPENAPI_SPEC)
            .count(),
        yaml_media_types
            .iter()
            .filter(|m| *m == MT_SERVER_OPENAPI_SPEC)
            .count(),
        "one spec media type covers both formats"
    );
}

#[test]
fn renaming_only_the_spec_file_changes_the_manifest_digest() {
    let package = config_server_package();

    let dir_a = tempfile::tempdir().unwrap();
    let layout_a = OciLayout::create(dir_a.path()).unwrap();
    let digest_a = pack_server(
        &package,
        referenced_binary(),
        Some(config_file()),
        Some(OpenApiSpecFile {
            file_name: "london-tube-api.yaml",
            bytes: SPEC_YAML,
        }),
        &layout_a,
    )
    .unwrap();

    let dir_b = tempfile::tempdir().unwrap();
    let layout_b = OciLayout::create(dir_b.path()).unwrap();
    let digest_b = pack_server(
        &package,
        referenced_binary(),
        Some(config_file()),
        Some(OpenApiSpecFile {
            file_name: "renamed-api.yaml",
            bytes: SPEC_YAML,
        }),
        &layout_b,
    )
    .unwrap();

    assert_ne!(
        digest_a, digest_b,
        "the file-name annotation lives on a LAYER descriptor, which is inside the manifest that \
         gets hashed (D-15) — renaming the spec changes the package's identity"
    );
}

// ---------------------------------------------------------------------
// D-11: layer ORDER is not load-bearing — every read is keyed by media type
// ---------------------------------------------------------------------

/// Number of layers a fully-populated config-only package carries:
/// binary-ref, envelope, deploy-descriptor, cedar-policy-set, tool-metadata,
/// config-slots, config, spec.
const FULL_LAYER_COUNT: usize = 8;

/// Pack a fully-populated config-only package (both optional layers present,
/// so the permutation exercises every media type) into a fresh layout.
fn packed_full_package(dir: &std::path::Path) -> (OciLayout, ManifestDigest) {
    let layout = OciLayout::create(dir).unwrap();
    let digest = pack_server(
        &config_server_package(),
        referenced_binary(),
        Some(config_file()),
        Some(spec_file()),
        &layout,
    )
    .unwrap();
    (layout, digest)
}

/// Rewrite `layout`'s single manifest so its `layers` array is `order`, and
/// return the new manifest digest.
///
/// "Rewrite the manifest with the permuted order" is NOT implementable as an
/// in-place edit, and getting that wrong produces a test that measures
/// nothing. `OciLayout::write_blob` derives BOTH the blob path and the
/// descriptor digest from the bytes, while `unpack_server`'s
/// `read_the_one_manifest` digest-verifies the index descriptor's declared
/// digest BEFORE it parses. So:
///
/// - overwrite the original blob path in place -> unpack dies at `verify`,
///   never reaching the media-type lookup under test;
/// - write permuted bytes without touching the index -> the index still
///   points at the ORIGINAL manifest, so unpack reads the unpermuted order
///   and the test passes vacuously.
///
/// The only correct shape is the five steps below: carry the index
/// descriptor's annotations across by hand (`finalize_pack` sets them AFTER
/// the digest is computed, so they cannot be recomputed), permute, serialize
/// with the SAME canonicalizer `finalize_pack` uses, write a NEW
/// content-addressed manifest blob, and REPLACE — never append — the index's
/// single descriptor.
fn rewrite_manifest_layers(layout: &OciLayout, order: &[usize]) -> Result<ManifestDigest> {
    // 1. The existing index descriptor, and its hand-set annotations.
    let mut index = layout.read_index()?;
    let old_descriptor =
        index
            .manifests()
            .first()
            .cloned()
            .ok_or_else(|| PackageError::Layout {
                reason: "index.json carries no manifest to rewrite".to_string(),
            })?;
    let annotations = old_descriptor.annotations().clone();

    // 2. Permute the layer vector.
    let mut manifest = layout.read_manifest(&old_descriptor)?;
    let layers = manifest.layers().clone();
    let permuted: Vec<_> = order
        .iter()
        .map(|&i| {
            layers.get(i).cloned().ok_or_else(|| PackageError::Layout {
                reason: format!("permutation index {i} is out of range"),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    manifest.set_layers(permuted);

    // 3. The SAME canonical form finalize_pack uses, so layer order is the
    //    only difference from the original bytes.
    let manifest_bytes = pmcp_package::canonicalize(&manifest)?;

    // 4. A NEW content-addressed blob, re-annotated by hand.
    let mut new_descriptor = layout.write_manifest(&manifest_bytes)?;
    new_descriptor.set_annotations(annotations);
    let digest = ManifestDigest::try_from(new_descriptor.digest())?;

    // 5. REPLACE the single index entry — a push would make
    //    read_the_one_manifest fail with "expected exactly one manifest",
    //    which looks like a code bug rather than a broken test.
    index.set_manifests(vec![new_descriptor]);
    layout.write_index(&index)?;

    Ok(digest)
}

#[test]
fn the_permutation_helper_actually_rewrites_the_content_addressed_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let (layout, baseline_digest) = packed_full_package(dir.path());

    // The identity permutation reproduces the original bytes exactly — proof
    // the helper's canonical form matches finalize_pack's.
    let identity: Vec<usize> = (0..FULL_LAYER_COUNT).collect();
    let identity_digest = rewrite_manifest_layers(&layout, &identity).unwrap();
    assert_eq!(
        identity_digest, baseline_digest,
        "the identity permutation must round-trip to the SAME digest — a different one would mean \
         the helper serializes differently from finalize_pack, so the property test would be \
         measuring the helper rather than unpack_server"
    );

    // A real permutation must produce DIFFERENT manifest bytes; if it did
    // not, the property test below would be a no-op.
    let mut reversed: Vec<usize> = (0..FULL_LAYER_COUNT).collect();
    reversed.reverse();
    let reversed_digest = rewrite_manifest_layers(&layout, &reversed).unwrap();
    assert_ne!(
        reversed_digest, baseline_digest,
        "a non-identity permutation must yield a different manifest blob — otherwise the helper \
         is a no-op and proves nothing"
    );
}

#[test]
fn a_reversed_layer_order_still_unpacks_to_the_same_server() {
    let baseline_dir = tempfile::tempdir().unwrap();
    let (baseline_layout, _) = packed_full_package(baseline_dir.path());
    let baseline = unpack_server(&baseline_layout).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let (layout, _) = packed_full_package(dir.path());
    let mut reversed: Vec<usize> = (0..FULL_LAYER_COUNT).collect();
    reversed.reverse();
    rewrite_manifest_layers(&layout, &reversed).unwrap();

    assert_eq!(
        unpack_server(&layout).unwrap(),
        baseline,
        "unpack_server resolves every layer by media type, so reversing the manifest's layer \
         array cannot change what it yields"
    );
}

proptest! {
    /// PROPERTY (D-11, CLAUDE.md ALWAYS PROPERTY): for an ARBITRARY
    /// permutation of the manifest's `layers` array, `unpack_server` yields an
    /// EQUAL `UnpackedServer`. Layer position carries no meaning — every read
    /// is keyed by media type — so a layout that only shuffles its layers is
    /// the same package.
    ///
    /// The permutation goes through `rewrite_manifest_layers`, which performs
    /// the full content-addressed rewrite; without it the manifest's
    /// digest-verify-before-parse chain would either reject the layout or
    /// leave the original manifest in place and prove nothing.
    #[test]
    fn any_layer_permutation_unpacks_to_an_equal_server(
        seed in proptest::collection::vec(0u32..1000, FULL_LAYER_COUNT)
    ) {
        let baseline_dir = tempfile::tempdir().unwrap();
        let (baseline_layout, _) = packed_full_package(baseline_dir.path());
        let baseline = unpack_server(&baseline_layout).unwrap();

        let mut order: Vec<usize> = (0..FULL_LAYER_COUNT).collect();
        order.sort_by_key(|&i| seed[i]);

        let dir = tempfile::tempdir().unwrap();
        let (layout, _) = packed_full_package(dir.path());
        rewrite_manifest_layers(&layout, &order).unwrap();

        prop_assert_eq!(unpack_server(&layout).unwrap(), baseline);
    }
}

// =====================================================================
// Plan 120-05 Task 1 — the config's `[[config_slots]]` block is the
// SOURCE OF TRUTH that `pack_server` reads and enforces (D-01).
//
// These exercise the agreement gate through the REAL public API path
// (`pack_server`), not through a unit call — that path is what D-01's
// "pack reads them" claims, and before this it was true of no code.
// =====================================================================

/// Pack `package` with `config_bytes` into a fresh layout, returning the result.
fn pack_with_config(
    package: &ServerPackage,
    config_bytes: &'static [u8],
    dir: &std::path::Path,
) -> Result<ManifestDigest> {
    let layout = OciLayout::create(dir).unwrap();
    pack_server(
        package,
        referenced_binary(),
        Some(ConfigFile {
            file_name: CONFIG_FILE_NAME,
            bytes: config_bytes,
        }),
        None,
        &layout,
    )
}

fn config_slot_violation(err: PackageError) -> (String, String) {
    match err {
        PackageError::ConfigSlotViolation { key, reason } => (key, reason),
        other => panic!("expected ConfigSlotViolation, got: {other}"),
    }
}

/// Test 3: agreement holds — the fixture package and its config describe the
/// same slot set, so the pack succeeds through the real API.
#[test]
fn pack_server_accepts_a_package_whose_slots_agree_with_its_shipped_config() {
    let dir = tempfile::tempdir().unwrap();
    pack_with_config(&config_server_package(), CONFIG_TOML, dir.path())
        .expect("agreeing declarations and package slots must pack");
}

/// Test 4: a declaration in the TOML with no matching package slot is refused,
/// naming the key. This is the "the config declares a slot the package forgot"
/// direction — an environment-specific value would otherwise be baked while the
/// package still looked slot-complete.
#[test]
fn pack_server_refuses_a_declaration_the_package_does_not_carry() {
    const EXTRA_DECLARATION: &[u8] = br#"name = "london-tube"

[[config_slots]]
key = "backend.api_key"
kind = "secret"
name = "TFL_API_KEY"

[[config_slots]]
key = "backend.base_url"
kind = "endpoint"
name = "TFL_BASE_URL"
tested_value = "https://api.tfl.gov.uk"

[backend]
api_key = "${TFL_API_KEY}"
base_url = "${TFL_BASE_URL}"
"#;
    let dir = tempfile::tempdir().unwrap();
    let err = pack_with_config(&config_server_package(), EXTRA_DECLARATION, dir.path())
        .expect_err("a declared slot the package omits must be refused");
    let (key, reason) = config_slot_violation(err);
    assert_eq!(key, "backend.base_url");
    assert!(reason.contains("absent from the package"), "was: {reason}");
}

/// Test 5: the opposite direction — a package inventing a slot its shipped
/// config never declares. The cross-AI review called this one out specifically.
#[test]
fn pack_server_refuses_a_package_slot_the_shipped_config_never_declares() {
    let mut package = config_server_package();
    package.config_slots.push(
        ConfigSlot::new(SlotType::Endpoint {
            name: "TFL_BASE_URL".to_string(),
            tested_value: "https://api.tfl.gov.uk".to_string(),
        })
        .with_config_key("backend.base_url"),
    );
    let dir = tempfile::tempdir().unwrap();
    let err = pack_with_config(&package, CONFIG_TOML, dir.path())
        .expect_err("an invented package slot must be refused");
    let (key, reason) = config_slot_violation(err);
    assert_eq!(key, "backend.base_url");
    assert!(reason.contains("absent from the config"), "was: {reason}");
}

/// Test 6: same key on both sides, different KIND.
#[test]
fn pack_server_refuses_a_kind_disagreement_naming_both_kinds() {
    let mut package = config_server_package();
    package.config_slots = vec![ConfigSlot::new(SlotType::Endpoint {
        name: "TFL_API_KEY".to_string(),
        tested_value: "https://api.tfl.gov.uk".to_string(),
    })
    .with_config_key("backend.api_key")];
    let dir = tempfile::tempdir().unwrap();
    let err = pack_with_config(&package, CONFIG_TOML, dir.path())
        .expect_err("a kind disagreement must be refused");
    let (key, reason) = config_slot_violation(err);
    assert_eq!(key, "backend.api_key");
    assert!(reason.contains("secret"), "was: {reason}");
    assert!(reason.contains("endpoint"), "was: {reason}");
}

/// Test 7: same key and kind, different `name`.
#[test]
fn pack_server_refuses_a_name_disagreement_without_echoing_either_name() {
    let mut package = config_server_package();
    package.config_slots = vec![ConfigSlot::new(SlotType::Secret {
        name: "SENTINEL_DISAGREEING_NAME".to_string(),
    })
    .with_config_key("backend.api_key")];
    let dir = tempfile::tempdir().unwrap();
    let err = pack_with_config(&package, CONFIG_TOML, dir.path())
        .expect_err("a name disagreement must be refused");
    let (key, reason) = config_slot_violation(err);
    assert_eq!(key, "backend.api_key");
    assert!(reason.contains("`name`"), "was: {reason}");
    assert!(
        !reason.contains("SENTINEL_DISAGREEING_NAME"),
        "the error names the FIELD, never the values; was: {reason}"
    );
}

/// Test 7b: same key, kind and name, different `tested_value`.
#[test]
fn pack_server_refuses_a_tested_value_disagreement_without_echoing_either_value() {
    const ENDPOINT_ONLY: &[u8] = br#"[[config_slots]]
key = "backend.base_url"
kind = "endpoint"
name = "TFL_BASE_URL"
tested_value = "https://api.tfl.gov.uk"

[backend]
base_url = "${TFL_BASE_URL}"
"#;
    let mut package = config_server_package();
    package.config_slots = vec![ConfigSlot::new(SlotType::Endpoint {
        name: "TFL_BASE_URL".to_string(),
        tested_value: "https://sentinel.invalid/untested".to_string(),
    })
    .with_config_key("backend.base_url")];
    let dir = tempfile::tempdir().unwrap();
    let err = pack_with_config(&package, ENDPOINT_ONLY, dir.path())
        .expect_err("a tested_value disagreement must be refused");
    let (key, reason) = config_slot_violation(err);
    assert_eq!(key, "backend.base_url");
    assert!(reason.contains("`tested_value`"), "was: {reason}");
    assert!(
        !reason.contains("sentinel.invalid"),
        "the error names the FIELD, never the values; was: {reason}"
    );
}

/// Test 8: with `config: None` no parsing and no agreement check happen at all.
/// The pre-existing embedded shape — a package carrying an undeclared `Secret`
/// slot and no config document — still packs exactly as it always did. This is
/// the regression that would otherwise break every earlier server-package test.
#[test]
fn an_embedded_package_with_an_undeclared_slot_still_packs_because_no_config_is_present() {
    let mut package = config_server_package();
    // No config_key at all, and nothing declares it — legal, because there is
    // no config document for a declaration to live in.
    package.config_slots = vec![ConfigSlot::new(SlotType::Secret {
        name: "TFL_API_KEY".to_string(),
    })];
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();
    let bootstrap = b"fake-bootstrap-bytes".to_vec();
    pack_server(
        &package,
        BinaryMode::Embedded(&bootstrap),
        None,
        None,
        &layout,
    )
    .expect("a config-less package must skip config-slot validation entirely");
    assert_eq!(unpack_server(&layout).unwrap().package, package);
}

/// Test 9 (parse-side): an unknown `kind` in the shipped config is refused by
/// `pack_server` — `pmcp-package` re-validates the vocabulary rather than
/// trusting that the bytes came through the toolkit's `ServerConfig`.
#[test]
fn pack_server_refuses_a_config_declaring_an_unknown_slot_kind() {
    const BAD_KIND: &[u8] = br#"[[config_slots]]
key = "backend.api_key"
kind = "endpont"
name = "TFL_API_KEY"
"#;
    let dir = tempfile::tempdir().unwrap();
    let err = pack_with_config(&config_server_package(), BAD_KIND, dir.path())
        .expect_err("an unknown kind must be refused at pack time");
    let (key, reason) = config_slot_violation(err);
    assert_eq!(key, "backend.api_key");
    assert!(reason.contains("auth_mode"), "was: {reason}");
}

// =====================================================================
// Plan 120-05 Task 2 — D-04 placeholder validation, through the real
// `pack_server` path, and the "a rejected pack writes NOTHING" property.
// =====================================================================

/// The distinctive literal a failing fixture bakes in. Its ABSENCE from the
/// error message is what proves the validator never echoes a config value —
/// asserted, not inspected.
const SENTINEL_CREDENTIAL: &str = "sentinel-leaked-credential";

/// A config whose slot-declared credential key holds a RESOLVED literal — the
/// exact shape D-04 exists to refuse.
const CONFIG_WITH_BAKED_CREDENTIAL: &[u8] = br#"name = "london-tube"

[[config_slots]]
key = "backend.api_key"
kind = "secret"
name = "TFL_API_KEY"

[backend]
api_key = "sentinel-leaked-credential"
"#;

/// Every file currently in the layout's `blobs/sha256/` directory, sorted.
fn blob_file_names(root: &std::path::Path) -> Vec<String> {
    let blobs = root.join("blobs").join("sha256");
    let mut names: Vec<String> = std::fs::read_dir(&blobs)
        .unwrap_or_else(|e| panic!("blobs dir {blobs:?} must exist after create: {e}"))
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Test 2/3: a slot-declared value key holding a resolved literal is refused by
/// `pack_server`, and the error names the key WITHOUT echoing the literal.
#[test]
fn pack_server_refuses_a_config_that_bakes_a_slot_declared_credential() {
    let dir = tempfile::tempdir().unwrap();
    let err = pack_with_config(
        &config_server_package(),
        CONFIG_WITH_BAKED_CREDENTIAL,
        dir.path(),
    )
    .expect_err("a resolved credential at a slot-declared key must not pack");
    let (key, reason) = config_slot_violation(err);
    assert_eq!(key, "backend.api_key");
    assert!(reason.contains("resolved literal"), "was: {reason}");
    assert!(
        !reason.contains(SENTINEL_CREDENTIAL),
        "the message must never carry the value it rejected; was: {reason}"
    );
}

/// Test 9 (the precise form): a rejected pack leaves the layout in its
/// post-`create` state and NOTHING more.
///
/// Asserting only "the index holds no manifest" would miss leaked config bytes
/// sitting in `blobs/sha256/` — and leaked config bytes are the exact thing
/// this validation exists to prevent. `OciLayout::create` already writes
/// `oci-layout`, an empty `index.json` and the blobs directory, so "no layout
/// behind" was never literally true; the checkable claim is that the blob file
/// SET is unchanged.
#[test]
fn a_rejected_pack_adds_neither_a_blob_nor_an_index_entry() {
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();
    let before = blob_file_names(dir.path());

    let err = pack_server(
        &config_server_package(),
        referenced_binary(),
        Some(ConfigFile {
            file_name: CONFIG_FILE_NAME,
            bytes: CONFIG_WITH_BAKED_CREDENTIAL,
        }),
        None,
        &layout,
    )
    .expect_err("the baked credential must abort the pack");
    assert!(matches!(err, PackageError::ConfigSlotViolation { .. }));

    assert_eq!(
        blob_file_names(dir.path()),
        before,
        "a rejected pack must not write a single blob — a leaked config layer here would be \
         the very disclosure the validation exists to prevent"
    );
    assert!(
        layout.read_index().unwrap().manifests().is_empty(),
        "a rejected pack must not record a manifest in the index"
    );
}

/// The auth-mode carve-out (D-17) holds through the real pack path: a
/// slot-declared auth-mode key holding a baked literal packs successfully,
/// because `AuthConfig` is internally tagged and no placeholder form of that
/// key can deserialize at all.
#[test]
fn pack_server_accepts_a_slot_declared_auth_mode_key_holding_a_literal() {
    const AUTH_MODE_CONFIG: &[u8] = br#"[[config_slots]]
key = "backend.auth.type"
kind = "auth_mode"
name = "backend-auth-mode"
tested_value = "api_key"

[backend.auth]
type = "api_key"
"#;
    let mut package = config_server_package();
    package.config_slots = vec![ConfigSlot::new(SlotType::AuthMode {
        name: "backend-auth-mode".to_string(),
        tested_value: "api_key".to_string(),
    })
    .with_config_key("backend.auth.type")];
    let dir = tempfile::tempdir().unwrap();
    pack_with_config(&package, AUTH_MODE_CONFIG, dir.path())
        .expect("the structural auth-mode key is exempt from the placeholder rule (D-17)");
}
