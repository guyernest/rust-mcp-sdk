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
        matches!(err, PackageError::DigestMismatch {.. }),
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
    deploy.composition.as_mut().expect("fixture must declare [composition]").tier =
        "enterprise".to_string();

    let err = validate_deploy_descriptor(&deploy).unwrap_err();
    assert!(
        matches!(err, PackageError::AllowlistViolation {.. }),
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
        matches!(err, PackageError::InvalidReference {.. }),
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
        matches!(err, PackageError::SlotConflict {.. }),
        "expected SlotConflict, got {err:?}"
   );
    assert!(err.to_string().contains("primary-llm"), "message was: {err}");
}

// --- 8. Malformed digest string -> MalformedDigest -------------------------

#[test]
fn malformed_digest_string_fails_parse_and_deserialize() {
    let err = ManifestDigest::parse("not-a-digest").unwrap_err();
    assert!(
        matches!(err, PackageError::MalformedDigest {.. }),
        "expected MalformedDigest, got {err:?}"
   );
    assert!(err.to_string().contains("not-a-digest"), "message was: {err}");

    let deser_result: std::result::Result<ManifestDigest, _> =
        serde_json::from_str("\"not-a-digest\"");
    assert!(
        deser_result.is_err(),
        "a malformed digest string must fail to deserialize"
   );
}
