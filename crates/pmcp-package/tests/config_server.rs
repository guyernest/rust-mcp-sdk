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

use pmcp_package::digest::ManifestDigest;
use pmcp_package::oci::media_types::{MT_SERVER_BOOTSTRAP, MT_SERVER_CONFIG};
use pmcp_package::oci::{
    pack_server, unpack_server, BinaryMode, ConfigFile, OciLayout, UnpackedBinary,
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

[backend]
kind = "openapi"
# a trailing comment, and irregular   spacing
base_url = "https://api.tfl.gov.uk"
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
        config_slots: vec![ConfigSlot {
            slot: SlotType::Secret {
                name: "TFL_API_KEY".to_string(),
            },
        }],
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
