//! Integration test: one negative case per required failure mode (CONTEXT /
//! 168-VALIDATION.md dimension 6), each asserting the SPECIFIC
//! `PackageError` variant (or `Option<Deviation>` value, for) via
//! `matches!`, and that the Display message is actionable (contains the
//! relevant identifier).
//!
//! All imports below come from the crate-root re-export block added to
//! `src/lib.rs` in this same plan (dual-consumer ergonomics) — this
//! file doubles as a live usage proof of that re-export surface.

use pmcp_package::reference::ComponentType;
use pmcp_package::{
    aggregate, detect_deviation, pack_workflow, unpack_workflow, validate_deploy_descriptor,
    ComponentRef, ConfigSlot, DeployDescriptor, ManifestDigest, OciLayout, PackageError,
    ServerPackage, SlotType, WorkflowManifest,
};
use std::path::Path;

fn read_fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden_fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read fixture {path:?}: {e}"))
}

// --- 1. Malformed manifest JSON -> serde parse error -----------------------

#[test]
fn malformed_manifest_json_fails_to_parse() {
    let result: std::result::Result<ServerPackage, serde_json::Error> =
        serde_json::from_str("{not valid json");
    let json_err = result.expect_err("malformed JSON must fail to parse");
    let package_err: PackageError = json_err.into();
    assert!(
        matches!(package_err, PackageError::Serialize(_)),
        "expected Serialize, got {package_err:?}"
    );
}

// --- 2. Unknown DeployDescriptor field -> parse rejection ------------

#[test]
fn deploy_descriptor_rejects_unknown_top_level_field() {
    let fixture_bytes = read_fixture("server_team_fs_v1.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fixture_bytes).unwrap();
    value
        .get_mut("deploy")
        .and_then(|d| d.as_object_mut())
        .unwrap()
        .insert("unexpected_field".to_string(), serde_json::json!(true));

    let result: std::result::Result<DeployDescriptor, _> =
        serde_json::from_value(value["deploy"].clone());
    let json_err =
        result.expect_err("DeployDescriptor must reject an unrecognized top-level field");
    let package_err: PackageError = json_err.into();
    assert!(
        matches!(package_err, PackageError::Serialize(_)),
        "expected Serialize, got {package_err:?}"
    );
    assert!(
        package_err.to_string().contains("unexpected_field"),
        "message was: {package_err}"
    );
}

// --- 3. Digest mismatch -> tampered blob fails unpack -----------------

#[test]
fn tampering_a_blob_causes_digest_mismatch_on_unpack() {
    let fixture_bytes = read_fixture("workflow_claims_triage_v1.json");
    let manifest: WorkflowManifest = serde_json::from_slice(&fixture_bytes).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();
    pack_workflow(&manifest, &layout).unwrap();

    // Flip a single byte in the manifest's sole layer blob on disk. The
    // content-addressed file NAME (from the original digest) is unchanged;
    // only the file's contents mutate — exactly the tamper scenario the
    // digest-verification-before-deserialize gate exists to catch.
    let index = layout.read_index().unwrap();
    let manifest_descriptor = &index.manifests()[0];
    let image_manifest = layout.read_manifest(manifest_descriptor).unwrap();
    let layer_descriptor = &image_manifest.layers()[0];
    let hex = layer_descriptor.digest().digest();
    let blob_path = dir.path().join("blobs").join("sha256").join(hex);
    let mut bytes = std::fs::read(&blob_path).unwrap();
    bytes[0] ^= 0x01;
    std::fs::write(&blob_path, bytes).unwrap();

    let err = unpack_workflow(&layout).unwrap_err();
    assert!(
        matches!(err, PackageError::DigestMismatch { .. }),
        "expected DigestMismatch, got {err:?}"
    );
    assert!(err.to_string().contains("sha256:"), "message was: {err}");
}

// --- 4. Allowlist violation -> disallowed composition tier -----------

#[test]
fn allowlist_violation_on_disallowed_composition_tier() {
    let fixture_bytes = read_fixture("server_team_fs_v1.json");
    let package: ServerPackage = serde_json::from_slice(&fixture_bytes).unwrap();
    let mut deploy = package.deploy.clone();
    deploy
        .composition
        .as_mut()
        .expect("fixture must declare [composition]")
        .tier = "enterprise".to_string();

    let err = validate_deploy_descriptor(&deploy).unwrap_err();
    assert!(
        matches!(err, PackageError::AllowlistViolation { .. }),
        "expected AllowlistViolation, got {err:?}"
    );
    assert!(err.to_string().contains("enterprise"), "message was: {err}");
}

// --- 5. Unpinned workflow ref -> InvalidReference --------------------

#[test]
fn unpinned_component_ref_fails_validate_all_pinned() {
    let fixture_bytes = read_fixture("workflow_claims_triage_v1.json");
    let mut manifest: WorkflowManifest = serde_json::from_slice(&fixture_bytes).unwrap();
    manifest.components.push(ComponentRef::Range {
        name: "unpinned-component".to_string(),
        range: semver::VersionReq::parse("^1").unwrap(),
        component_type: ComponentType::Server,
    });

    let err = manifest.validate_all_pinned().unwrap_err();
    assert!(
        matches!(err, PackageError::InvalidReference { .. }),
        "expected InvalidReference, got {err:?}"
    );
    assert!(
        err.to_string().contains("unpinned-component"),
        "message was: {err}"
    );
}

// --- 6. Behavior-relevant deviation vs identity-bearing no-op --------

#[test]
fn behavior_relevant_deviation_detected_but_identity_bearing_slot_is_not() {
    let tested_llm = SlotType::LlmProvider {
        name: "primary-llm".to_string(),
        tested_value: "anthropic".to_string(),
    };
    let proposed_llm = SlotType::LlmProvider {
        name: "primary-llm".to_string(),
        tested_value: "openai".to_string(),
    };
    let deviation = detect_deviation(&tested_llm, &proposed_llm)
        .expect("a differing llm-provider tested_value must be flagged as a deviation");
    assert_eq!(deviation.slot_name, "primary-llm");
    assert_eq!(deviation.tested, "anthropic");
    assert_eq!(deviation.proposed, "openai");

    let tested_secret = SlotType::Secret {
        name: "API_KEY".to_string(),
    };
    let proposed_secret = SlotType::Secret {
        name: "API_KEY".to_string(),
    };
    assert_eq!(
        detect_deviation(&tested_secret, &proposed_secret),
        None,
        "identity-bearing slots must never be flagged as a deviation"
    );
}

// --- 7. Slot conflict -> aggregate() over divergent tested values ----

#[test]
fn aggregate_returns_slot_conflict_for_divergent_llm_provider_tested_values() {
    let a = ConfigSlot {
        slot: SlotType::LlmProvider {
            name: "primary-llm".to_string(),
            tested_value: "anthropic".to_string(),
        },
    };
    let b = ConfigSlot {
        slot: SlotType::LlmProvider {
            name: "primary-llm".to_string(),
            tested_value: "openai".to_string(),
        },
    };
    let err = aggregate([&a, &b]).unwrap_err();
    assert!(
        matches!(err, PackageError::SlotConflict { .. }),
        "expected SlotConflict, got {err:?}"
    );
    assert!(
        err.to_string().contains("primary-llm"),
        "message was: {err}"
    );
}

// --- 8. Malformed digest string -> MalformedDigest -------------------------

#[test]
fn malformed_digest_string_fails_parse_and_deserialize() {
    let err = ManifestDigest::parse("not-a-digest").unwrap_err();
    assert!(
        matches!(err, PackageError::MalformedDigest { .. }),
        "expected MalformedDigest, got {err:?}"
    );
    assert!(
        err.to_string().contains("not-a-digest"),
        "message was: {err}"
    );

    let deser_result: std::result::Result<ManifestDigest, _> =
        serde_json::from_str("\"not-a-digest\"");
    assert!(
        deser_result.is_err(),
        "a malformed digest string must fail to deserialize"
    );
}

// =====================================================================
// 9-14. Server-layout failure modes (plan 120-02 Task 2)
//
// Every case below hand-builds a MALFORMED server layout by rewriting a
// well-formed one, following this file's established tamper idiom. Because
// the layout is content-addressed and `unpack_server` digest-verifies the
// index descriptor BEFORE parsing the manifest, a rewrite must write a NEW
// manifest blob and REPLACE the index's single descriptor — never edit in
// place, which would fail at `verify` before reaching the code under test.
// =====================================================================

mod server_layout {
    use super::*;
    use oci_spec::image::{
        Descriptor, ImageIndexBuilder, ImageManifest, MediaType, SCHEMA_VERSION,
    };
    use pmcp_package::oci::media_types::{
        MT_SERVER_BINARY_REF, MT_SERVER_BOOTSTRAP, MT_SERVER_CONFIG, MT_SERVER_ENVELOPE,
    };
    use pmcp_package::{
        pack_server, unpack_server, BinaryMode, CedarPolicySet, ConfigFile, OciLayout,
        ServerPackage,
    };

    const REFERENCED_MEDIA_TYPE: &str = "application/x-lambda-bootstrap; arch=arm64";
    const CONFIG_TOML: &[u8] = b"name = \"london-tube\"\n";

    /// A minimal, well-formed `ServerPackage`, built from the tracked golden
    /// fixture so the deploy/policy/tool sections stay realistic.
    fn server_package() -> ServerPackage {
        let bytes = read_fixture("server_team_fs_v1.json");
        let mut package: ServerPackage = serde_json::from_slice(&bytes).unwrap();
        package.policies = CedarPolicySet(package.policies.0);
        package
    }

    fn referenced_binary() -> BinaryMode<'static> {
        BinaryMode::Referenced {
            digest: ManifestDigest::from_bytes(b"pmcp-openapi-server-v1.0.0-aarch64"),
            media_type: REFERENCED_MEDIA_TYPE.to_string(),
        }
    }

    fn config_file() -> ConfigFile<'static> {
        ConfigFile {
            file_name: "london-tube.toml",
            bytes: CONFIG_TOML,
        }
    }

    /// Pack a well-formed, config-only (referenced-binary) server package into
    /// a fresh layout.
    fn packed_config_only(dir: &std::path::Path) -> OciLayout {
        let layout = OciLayout::create(dir).unwrap();
        pack_server(
            &server_package(),
            referenced_binary(),
            Some(config_file()),
            None,
            &layout,
        )
        .unwrap();
        layout
    }

    fn read_manifest(layout: &OciLayout) -> ImageManifest {
        let index = layout.read_index().unwrap();
        layout.read_manifest(&index.manifests()[0]).unwrap()
    }

    /// Write `manifest` as a NEW content-addressed blob and REPLACE the
    /// index's single descriptor with one pointing at it — the only rewrite
    /// shape `unpack_server`'s verify-before-parse accepts.
    fn replace_manifest(layout: &OciLayout, manifest: &ImageManifest) {
        let bytes = pmcp_package::canonicalize(manifest).unwrap();
        let descriptor = layout.write_manifest(&bytes).unwrap();
        let index = ImageIndexBuilder::default()
            .schema_version(SCHEMA_VERSION)
            .manifests(vec![descriptor])
            .build()
            .unwrap();
        layout.write_index(&index).unwrap();
    }

    fn write_layer(layout: &OciLayout, media_type: &str, bytes: &[u8]) -> Descriptor {
        layout
            .write_blob(MediaType::from(media_type), bytes)
            .unwrap()
    }

    // --- 9. A 0.1.x-shaped envelope is refused, never mis-read ------------

    #[test]
    fn an_envelope_carrying_the_legacy_binary_ref_shape_is_refused_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let layout = packed_config_only(dir.path());

        // The 0.1.x envelope shape: the binary's identity lived INSIDE the
        // envelope rather than on its own layer. `ServerEnvelope` has no
        // `deny_unknown_fields`, so a plain deserialize would silently drop
        // this key and hand back a structurally valid 0.2.0 struct.
        let legacy = br#"{"name":"x","version":"1.0.0","binary_ref":{"media_type":"application/x-lambda-bootstrap; arch=arm64"}}"#;
        let new_envelope = write_layer(&layout, MT_SERVER_ENVELOPE, legacy);

        let mut manifest = read_manifest(&layout);
        let layers: Vec<Descriptor> = manifest
            .layers()
            .iter()
            .map(|l| {
                if l.media_type().to_string() == MT_SERVER_ENVELOPE {
                    new_envelope.clone()
                } else {
                    l.clone()
                }
            })
            .collect();
        manifest.set_layers(layers);
        replace_manifest(&layout, &manifest);

        let err = unpack_server(&layout)
            .expect_err("a 0.1.x-shaped envelope must never deserialize into a 0.2.0 struct");
        let message = err.to_string();
        assert!(
            message.contains("0.1"),
            "the refusal must name the shape it found; message was: {message}"
        );
        assert!(
            message.contains("0.2.0"),
            "the refusal must name the version the format changed in; message was: {message}"
        );
    }

    // --- 10. Duplicate media type -> Layout naming the type ---------------

    #[test]
    fn two_layers_sharing_one_media_type_are_rejected_naming_that_type() {
        let dir = tempfile::tempdir().unwrap();
        let layout = packed_config_only(dir.path());

        let mut manifest = read_manifest(&layout);
        let mut layers = manifest.layers().clone();
        let config = layers
            .iter()
            .find(|l| l.media_type().to_string() == MT_SERVER_CONFIG)
            .expect("the packed layout must carry a config layer")
            .clone();
        layers.push(config);
        manifest.set_layers(layers);
        replace_manifest(&layout, &manifest);

        let err = unpack_server(&layout)
            .expect_err("a duplicated media type must be rejected, never last-wins");
        assert!(
            matches!(err, PackageError::Layout { .. }),
            "expected Layout, got {err:?}"
        );
        assert!(
            err.to_string().contains(MT_SERVER_CONFIG),
            "the error must name the duplicated media type; message was: {err}"
        );
    }

    // --- 11. BOTH binary arms -> Layout ------------------------------------

    #[test]
    fn a_manifest_carrying_both_binary_arms_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let layout = packed_config_only(dir.path());

        let mut manifest = read_manifest(&layout);
        let mut layers = manifest.layers().clone();
        layers.push(write_layer(&layout, MT_SERVER_BOOTSTRAP, b"\x7fELF-fake"));
        manifest.set_layers(layers);
        replace_manifest(&layout, &manifest);

        let err = unpack_server(&layout)
            .expect_err("exactly one binary arm is required — both is malformed");
        assert!(
            matches!(err, PackageError::Layout { .. }),
            "expected Layout, got {err:?}"
        );
    }

    // --- 12. NEITHER binary arm -> Layout ----------------------------------

    #[test]
    fn a_manifest_carrying_neither_binary_arm_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let layout = packed_config_only(dir.path());

        let mut manifest = read_manifest(&layout);
        let layers: Vec<Descriptor> = manifest
            .layers()
            .iter()
            .filter(|l| {
                let mt = l.media_type().to_string();
                mt != MT_SERVER_BINARY_REF && mt != MT_SERVER_BOOTSTRAP
            })
            .cloned()
            .collect();
        manifest.set_layers(layers);
        replace_manifest(&layout, &manifest);

        let err = unpack_server(&layout)
            .expect_err("exactly one binary arm is required — neither is malformed");
        assert!(
            matches!(err, PackageError::Layout { .. }),
            "expected Layout, got {err:?}"
        );
    }

    // --- 13. binary-ref with a null digest -> Layout naming the digest -----

    #[test]
    fn a_binary_ref_whose_wire_digest_is_null_is_rejected_naming_the_missing_digest() {
        let dir = tempfile::tempdir().unwrap();
        let layout = packed_config_only(dir.path());

        // `BinaryRef::digest` is `Option<ManifestDigest>` for wire tolerance,
        // so an explicit null decodes to `None` — the one shape the API type
        // cannot express, and an instruction to run an UNPINNED binary.
        let unpinned = serde_json::to_vec(&serde_json::json!({
            "digest": serde_json::Value::Null,
            "media_type": REFERENCED_MEDIA_TYPE,
        }))
        .unwrap();
        let new_ref = write_layer(&layout, MT_SERVER_BINARY_REF, &unpinned);

        let mut manifest = read_manifest(&layout);
        let layers: Vec<Descriptor> = manifest
            .layers()
            .iter()
            .map(|l| {
                if l.media_type().to_string() == MT_SERVER_BINARY_REF {
                    new_ref.clone()
                } else {
                    l.clone()
                }
            })
            .collect();
        manifest.set_layers(layers);
        replace_manifest(&layout, &manifest);

        let err =
            unpack_server(&layout).expect_err("an unpinned binary reference must be rejected");
        assert!(
            matches!(err, PackageError::Layout { .. }),
            "expected Layout, got {err:?}"
        );
        assert!(
            err.to_string().contains("digest"),
            "the error must name the missing field; message was: {err}"
        );
    }

    // --- 14. Regression: the refusal is NARROW ----------------------------

    #[test]
    fn well_formed_0_2_0_packages_of_either_binary_mode_still_unpack() {
        let referenced_dir = tempfile::tempdir().unwrap();
        let referenced_layout = packed_config_only(referenced_dir.path());
        let referenced = unpack_server(&referenced_layout)
            .expect("a well-formed 0.2.0 referenced package must still unpack");
        assert_eq!(referenced.config.unwrap().bytes, CONFIG_TOML);

        let bootstrap = b"fake-arm64-bootstrap-binary-bytes-for-testing".to_vec();
        let embedded_dir = tempfile::tempdir().unwrap();
        let embedded_layout = OciLayout::create(embedded_dir.path()).unwrap();
        pack_server(
            &server_package(),
            BinaryMode::Embedded(&bootstrap),
            None,
            None,
            &embedded_layout,
        )
        .unwrap();
        let embedded = unpack_server(&embedded_layout)
            .expect("a well-formed 0.2.0 embedded package must still unpack");
        assert_eq!(
            embedded.binary,
            pmcp_package::UnpackedBinary::Embedded(bootstrap)
        );
    }
}
