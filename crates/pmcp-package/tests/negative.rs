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
mod common;

/// Read a checked-in golden fixture's raw bytes (delegates to the shared
/// `common::fixture_bytes` so the path/panic logic lives once per crate).
fn read_fixture(name: &str) -> Vec<u8> {
    common::fixture_bytes(name)
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
    let a = ConfigSlot::new(SlotType::LlmProvider {
        name: "primary-llm".to_string(),
        tested_value: "anthropic".to_string(),
    });
    let b = ConfigSlot::new(SlotType::LlmProvider {
        name: "primary-llm".to_string(),
        tested_value: "openai".to_string(),
    });
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
    use super::common::{referenced_binary, REFERENCED_MEDIA_TYPE};
    use super::*;
    use oci_spec::image::{
        Descriptor, ImageIndexBuilder, ImageManifest, MediaType, SCHEMA_VERSION,
    };
    use pmcp_package::oci::media_types::{
        MT_SERVER_BINARY_REF, MT_SERVER_BOOTSTRAP, MT_SERVER_CONFIG, MT_SERVER_ENVELOPE,
    };
    use pmcp_package::{
        pack_server, unpack_server, BinaryMode, CedarPolicySet, ConfigFile, ConfigSlot, OciLayout,
        ServerPackage,
    };

    /// The config bytes packed alongside the golden server fixture. It declares
    /// the ONE slot that fixture carries, because `pack_server` now requires the
    /// shipped config's `[[config_slots]]` block and the package's slot list to
    /// agree (D-01) and requires a slot-declared credential key to hold an
    /// environment reference rather than a literal (D-04).
    const CONFIG_TOML: &[u8] = b"name = \"london-tube\"\n\n\
        [[config_slots]]\n\
        key = \"backend.api_key\"\n\
        kind = \"secret\"\n\
        name = \"LICHESS_API_KEY\"\n\n\
        [backend]\n\
        api_key = \"${LICHESS_API_KEY}\"\n";

    /// A minimal, well-formed `ServerPackage`, built from the tracked golden
    /// fixture so the deploy/policy/tool sections stay realistic.
    fn server_package() -> ServerPackage {
        let bytes = read_fixture("server_team_fs_v1.json");
        let mut package: ServerPackage = serde_json::from_slice(&bytes).unwrap();
        package.policies = CedarPolicySet(package.policies.0);
        package
    }

    // `referenced_binary()` and `REFERENCED_MEDIA_TYPE` are imported from
    // `common` above — the same values every other test in this crate packs.

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
        // The golden fixture's lone `Secret` slot carries no `config_key` (it is
        // an embedded-package slot). Packing it WITH a config file makes it a
        // config-server slot, so it must name the config path it fills — that is
        // the D-04 rule this helper's package now satisfies. `server_package()`
        // itself is left untouched for the `config: None` call sites.
        let mut package = server_package();
        package.config_slots = package
            .config_slots
            .into_iter()
            .map(|slot| ConfigSlot::new(slot.slot).with_config_key("backend.api_key"))
            .collect();
        pack_server(
            &package,
            referenced_binary(),
            Some(config_file()),
            None,
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

// =====================================================================
// 15-18. Gate B — the pack-time attestation-subject refusal (D-02)
//
// `pack_server` computes the would-be UNATTESTED manifest digest and refuses
// to pack when the supplied subject names anything else, so "an attestation
// attached to the wrong package" is unrepresentable in a produced layout.
//
// The refusal is only worth as much as its ORDERING: a refusal that fired
// after the layers were already on disk would leave a half-written layout
// behind. `a_refused_pack_leaves_the_destination_layout_byte_for_byte_unchanged`
// is the assertion that pins the ordering, and it is what fails if Gate B is
// ever moved after the writing loop.
// =====================================================================

mod attestation_subject {
    use super::common::referenced_binary;
    use super::*;
    use pmcp_package::{pack_server, AttestationFile, OciLayout, ServerPackage};
    use std::collections::BTreeMap;
    use std::path::Path;

    const ISSUER: &str = "https://issuer.test.invalid/pmcp-run";
    const PAYLOAD_TYPE: &str = "application/vnd.test.attestation-payload";

    /// A payload that is neither valid JSON nor valid UTF-8, so a passing test
    /// is evidence that Gate B compares DIGESTS and never reads the payload.
    const OPAQUE_PAYLOAD: &[u8] = b"\x00\x01 not json \xff\xfe";

    fn server_package() -> ServerPackage {
        serde_json::from_slice(&read_fixture("server_team_fs_v1.json"))
            .expect("fixture must parse as ServerPackage")
    }

    /// Pack the fixture WITHOUT an attestation and return the manifest digest —
    /// the one value Gate B accepts as a subject.
    fn unattested_digest() -> ManifestDigest {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        pack_server(
            &server_package(),
            referenced_binary(),
            None,
            None,
            None,
            &layout,
        )
        .expect("the unattested package must pack")
    }

    fn attestation_claiming(subject: &str) -> AttestationFile<'_> {
        AttestationFile {
            bytes: OPAQUE_PAYLOAD,
            subject,
            issuer: ISSUER,
            payload_type: PAYLOAD_TYPE,
        }
    }

    /// Every file under `root`, keyed by its path relative to `root`, valued by
    /// `(byte length, content digest)`.
    ///
    /// The content digest is deliberately stronger than the byte LENGTH the
    /// acceptance criterion asks for: `index.json` can be rewritten to the same
    /// length with different contents, and "the layout is byte-identical to its
    /// pre-call state" is the property actually under test.
    fn snapshot(root: &Path) -> BTreeMap<String, (u64, String)> {
        let mut files = BTreeMap::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let bytes = std::fs::read(&path).unwrap();
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                let digest = ManifestDigest::from_bytes(&bytes).as_str().to_string();
                files.insert(relative, (bytes.len() as u64, digest));
            }
        }
        files
    }

    // --- 15. The matching subject packs, and yields the OTHER digest ------

    /// The accepting half of Gate B, plus 122-02's two-digest property
    /// re-asserted at the gate: a subject that names this package is accepted,
    /// and the digest the attested pack returns is NOT that subject.
    #[test]
    fn an_attestation_whose_subject_names_this_package_packs_and_yields_a_distinct_digest() {
        let unattested = unattested_digest();

        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let attested = pack_server(
            &server_package(),
            referenced_binary(),
            None,
            None,
            Some(attestation_claiming(unattested.as_str())),
            &layout,
        )
        .expect("a subject naming this package must be accepted");

        assert_ne!(
            attested.as_str(),
            unattested.as_str(),
            "the attestation layer is inside the bytes the digest covers, so an attested pack \
             must return a DIFFERENT digest from the subject it names (D-01)"
        );
    }

    // --- 16. A subject naming another package is refused, naming both ----

    #[test]
    fn an_attestation_whose_subject_names_another_package_is_refused_naming_both_digests() {
        let unattested = unattested_digest();
        let other = ManifestDigest::from_bytes(b"an entirely different package");
        assert_ne!(other.as_str(), unattested.as_str());

        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let err = pack_server(
            &server_package(),
            referenced_binary(),
            None,
            None,
            Some(attestation_claiming(other.as_str())),
            &layout,
        )
        .expect_err("an attestation naming a different package must be refused");

        assert!(
            matches!(err, PackageError::AttestationSubjectMismatch { .. }),
            "expected AttestationSubjectMismatch, got {err:?}"
        );
        let message = err.to_string();
        assert!(
            message.contains(other.as_str()),
            "the refusal must name the SUPPLIED subject; message was: {message}"
        );
        assert!(
            message.contains(unattested.as_str()),
            "the refusal must name the COMPUTED subject, or an operator cannot tell which \
             package the attestation should have named; message was: {message}"
        );
    }

    // --- 17. The refusal happens BEFORE the first write ------------------

    /// The ordering assertion. Gate B running after the layer-writing loop
    /// would still return the same `Err`, so the refusal alone proves nothing
    /// about the filesystem — this snapshot is what pins "a rejected pack adds
    /// neither a blob nor an index entry" for the FULL layout: no envelope,
    /// deploy-descriptor, policy, tool-metadata, config-slots, binary,
    /// attestation or empty-config blob may appear.
    #[test]
    fn a_refused_pack_leaves_the_destination_layout_byte_for_byte_unchanged() {
        let other = ManifestDigest::from_bytes(b"an entirely different package");

        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let before = snapshot(dir.path());
        assert!(
            !before.is_empty(),
            "a freshly created layout carries oci-layout and index.json, or this test compares \
             nothing against nothing"
        );

        pack_server(
            &server_package(),
            referenced_binary(),
            None,
            None,
            Some(attestation_claiming(other.as_str())),
            &layout,
        )
        .expect_err("an attestation naming a different package must be refused");

        let after = snapshot(dir.path());
        assert_eq!(
            before, after,
            "a refused pack must add neither a blob nor an index entry — the gate runs BEFORE \
             the first write"
        );
    }

    // --- 18. A malformed subject is an error, never a panic ---------------

    #[test]
    fn a_malformed_subject_is_refused_rather_than_panicking() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let before = snapshot(dir.path());

        let err = pack_server(
            &server_package(),
            referenced_binary(),
            None,
            None,
            Some(attestation_claiming("sha256:unused")),
            &layout,
        )
        .unwrap_err();

        assert!(
            matches!(err, PackageError::MalformedDigest { .. }),
            "a subject that is not sha256:<64 hex> is malformed, not a mismatch; got {err:?}"
        );
        assert!(
            err.to_string().contains("unused"),
            "the refusal must name the malformed value's shape problem; message was: {err}"
        );
        assert_eq!(
            before,
            snapshot(dir.path()),
            "a malformed subject is refused by the same pre-write gate"
        );
    }

    // --- 19. An annotation value canonical JSON cannot represent -----------

    /// A C0 control character in an attestation annotation is refused BEFORE
    /// the first write.
    ///
    /// Found by `tests/attestation_opacity.rs`'s adversarial-annotation
    /// property, which generated `issuer = "\0"` and observed a package that
    /// packed cleanly and then failed to UNPACK: canonical JSON (OLPC/TUF)
    /// escapes only `"` and `\`, so a control character is written literally
    /// and the resulting manifest is not RFC 8259 JSON. Pinned here
    /// deterministically as well, because a property that merely SAMPLES the
    /// refusal could stop reaching it after a strategy edit and nothing would
    /// notice.
    #[test]
    fn an_attestation_annotation_carrying_a_control_character_is_refused_before_any_write() {
        for (label, issuer, payload_type) in [
            ("issuer", "https://issuer.test.invalid/\u{0}", PAYLOAD_TYPE),
            ("payload_type", ISSUER, "application/\u{1b}json"),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let layout = OciLayout::create(dir.path()).unwrap();
            let before = snapshot(dir.path());
            let subject = unattested_digest();

            let err = pack_server(
                &server_package(),
                referenced_binary(),
                None,
                None,
                Some(AttestationFile {
                    bytes: OPAQUE_PAYLOAD,
                    subject: subject.as_str(),
                    issuer,
                    payload_type,
                }),
                &layout,
            )
            .unwrap_err();

            assert!(
                matches!(err, PackageError::AttestationAnnotationInvalid { .. }),
                "a control character in the {label} annotation must be refused as \
                 unrepresentable; got {err:?}"
            );
            assert_eq!(
                before,
                snapshot(dir.path()),
                "the annotation gate runs BEFORE the first write, like every other pack \
                 precondition"
            );
        }
    }

    /// The gate must not widen: a NON-ASCII issuer is perfectly representable
    /// in canonical JSON and must still pack.
    ///
    /// Without this, "refuse anything unusual" would satisfy the test above
    /// while breaking every legitimate internationalized issuer name.
    #[test]
    fn a_non_ascii_attestation_annotation_is_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let subject = unattested_digest();

        pack_server(
            &server_package(),
            referenced_binary(),
            None,
            None,
            Some(AttestationFile {
                bytes: OPAQUE_PAYLOAD,
                subject: subject.as_str(),
                issuer: "https://issuer.test.invalid/café-北京-\u{1f600}",
                payload_type: PAYLOAD_TYPE,
            }),
            &layout,
        )
        .expect("a non-ASCII annotation is representable and must pack");
    }
}
