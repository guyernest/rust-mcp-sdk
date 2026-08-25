//! Integration test: golden-fixture-based + programmatically-built
//! round-trip tests for all four AI-Package types (losslessness), plus
//! the canonical-byte-identity assertion against the two checked-in
//! golden fixtures (`tests/golden_fixtures/*.json`).
//!
//! `server_team_fs_v1.json` and `workflow_claims_triage_v1.json` are stored
//! in CANONICAL byte form (the exact bytes `canonicalize()` emits — compact,
//! olpc-cjson key-sorted JSON), authored by a one-off generator that wrote
//! `canonicalize(&value)`'s output directly to disk (see the plan's
//! recommended authoring procedure). The byte-identical assertion below
//! therefore compares the fixture bytes against `canonicalize(&parsed)` and
//! cannot fight the canonicalizer.
//!
//! `AgentPackage`/`TeamPackage` are NOT fixture-backed (no checked-in golden
//! file for either) — they are built programmatically here and only
//! round-tripped through pack/unpack, per this plan's scope (only two golden
//! fixtures are required: one server, one workflow).

use pmcp_package::digest::{canonicalize, ManifestDigest};
use pmcp_package::oci::media_types::{
    ANNOTATION_ATTESTATION_ISSUER, ANNOTATION_ATTESTATION_PAYLOAD_TYPE,
    ANNOTATION_ATTESTATION_SUBJECT, ARTIFACT_TYPE_SERVER, ARTIFACT_TYPE_TEAM, MT_ATTESTATION,
};
use pmcp_package::oci::{
    pack_agent, pack_server, pack_team, pack_workflow, unpack_agent, unpack_server, unpack_team,
    unpack_workflow, AttestationFile, BinaryMode, OciLayout, UnpackedAttestation, UnpackedBinary,
};
use pmcp_package::package::{
    AgentPackage, HumanRole, ServerPackage, TeamLimits, TeamMember, TeamPackage, TeamRole,
    WorkflowManifest,
};
use pmcp_package::reference::{ComponentRef, ComponentType};
use pmcp_package::slot::{ConfigSlot, SlotType};
mod common;

/// Read a checked-in golden fixture's raw bytes (delegates to the shared
/// `common::fixture_bytes` so the path/panic logic lives once per crate).
fn read_fixture(name: &str) -> Vec<u8> {
    common::fixture_bytes(name)
}

// ---------------------------------------------------------------------
// Fixture-backed types: ServerPackage, WorkflowManifest
// ---------------------------------------------------------------------

#[test]
fn server_package_fixture_round_trips_and_matches_canonical_bytes() {
    let fixture_bytes = read_fixture("server_team_fs_v1.json");
    let parsed: ServerPackage =
        serde_json::from_slice(&fixture_bytes).expect("fixture must parse as ServerPackage");

    // canonicality: the checked-in fixture bytes ARE canonicalize(&parsed)'s
    // output, byte-for-byte — no re-pretty-printing, no key reordering.
    let recanonicalized = canonicalize(&parsed).expect("ServerPackage must canonicalize");
    assert_eq!(
        recanonicalized, fixture_bytes,
        "server_team_fs_v1.json must be stored in canonical byte form"
    );

    let bootstrap = b"fake-arm64-bootstrap-binary-bytes-for-testing".to_vec();
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();
    pack_server(
        &parsed,
        BinaryMode::Embedded(&bootstrap),
        None,
        None,
        None,
        &layout,
    )
    .unwrap();
    let unpacked = unpack_server(&layout).unwrap();

    assert_eq!(
        unpacked.package, parsed,
        "ServerPackage must round-trip pack/unpack losslessly"
    );
    assert_eq!(
        unpacked.binary,
        UnpackedBinary::Embedded(bootstrap),
        "bootstrap bytes must round-trip pack/unpack losslessly"
    );
}

#[test]
fn workflow_manifest_fixture_round_trips_and_matches_canonical_bytes() {
    let fixture_bytes = read_fixture("workflow_claims_triage_v1.json");
    let parsed: WorkflowManifest =
        serde_json::from_slice(&fixture_bytes).expect("fixture must parse as WorkflowManifest");

    let recanonicalized = canonicalize(&parsed).expect("WorkflowManifest must canonicalize");
    assert_eq!(
        recanonicalized, fixture_bytes,
        "workflow_claims_triage_v1.json must be stored in canonical byte form"
    );

    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();
    pack_workflow(&parsed, &layout).unwrap();
    let unpacked = unpack_workflow(&layout).unwrap();

    assert_eq!(
        unpacked, parsed,
        "WorkflowManifest must round-trip pack/unpack losslessly"
    );
    assert!(
        unpacked.validate_all_pinned().is_ok(),
        "the fixture manifest must be fully pinned"
    );
}

// ---------------------------------------------------------------------
// Programmatically-built types: AgentPackage, TeamPackage
// ---------------------------------------------------------------------

fn sample_agent_package() -> AgentPackage {
    AgentPackage {
        name: "claims-triage-agent".to_string(),
        version: semver::Version::parse("1.2.0").unwrap(),
        instructions: "You triage incoming insurance claims and route them to specialists."
            .to_string(),
        llm: ConfigSlot::new(SlotType::LlmProvider {
            name: "primary-llm".to_string(),
            tested_value: "anthropic".to_string(),
        }),
        max_tokens: 8192,
        max_iterations: 15,
        connectors: vec![ComponentRef::Range {
            name: "claims-database".to_string(),
            range: semver::VersionReq::parse("^2").unwrap(),
            component_type: ComponentType::Server,
        }],
        tool_selection: Some(serde_json::json!({ "claims-database": ["lookup_claim"] })),
        input_schema: None,
        output_schema: Some(serde_json::json!({ "type": "object" })),
        importance: Some("HIGH".to_string()),
        finalizer_role: Some("formatter".to_string()),
        budget_defaults: vec![ConfigSlot::new(SlotType::BudgetOverride {
            name: "monthly-cap".to_string(),
            tested_value: "500".to_string(),
        })],
    }
}

#[test]
fn agent_package_round_trips_losslessly() {
    let package = sample_agent_package();
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();

    pack_agent(&package, &layout).unwrap();
    let unpacked = unpack_agent(&layout).unwrap();

    assert_eq!(
        unpacked, package,
        "AgentPackage must round-trip pack/unpack losslessly"
    );
}

fn sample_team_package() -> TeamPackage {
    let entry_point = ComponentRef::Range {
        name: "claims-triage-agent".to_string(),
        range: semver::VersionReq::parse("^1").unwrap(),
        component_type: ComponentType::Agent,
    };
    let human_role = HumanRole {
        role: "approver".to_string(),
        description: "Approves high-value claims".to_string(),
        responsibilities: vec!["review".to_string(), "approve".to_string()],
        channel_hints: vec!["email".to_string()],
    };
    TeamPackage {
        name: "claims-triage-team".to_string(),
        version: semver::Version::parse("1.0.0").unwrap(),
        entry_point: entry_point.clone(),
        members: vec![TeamMember {
            agent: entry_point,
            role: TeamRole::EntryPoint,
        }],
        human_roles: vec![human_role.clone()],
        limits: TeamLimits {
            max_team_depth: 3,
            max_team_total_tokens: 200_000,
            max_team_wall_clock_seconds: 600,
            poll_interval_ms: 2000,
        },
        built_in_servers: vec![ComponentRef::Range {
            name: "team-fs".to_string(),
            range: semver::VersionReq::parse("^1").unwrap(),
            component_type: ComponentType::Server,
        }],
        finalizer_agents: vec![],
        budget_defaults: vec![],
        config_slots: vec![human_role.to_config_slot()],
    }
}

#[test]
fn team_package_round_trips_losslessly() {
    let package = sample_team_package();
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();

    pack_team(&package, &layout).unwrap();
    let unpacked = unpack_team(&layout).unwrap();

    assert_eq!(
        unpacked, package,
        "TeamPackage must round-trip pack/unpack losslessly"
    );
}

// ---------------------------------------------------------------------
// Attestation carriage — the properties the tracer only demonstrated once
// ---------------------------------------------------------------------

const ATTESTATION_ISSUER: &str = "https://issuer.test.invalid/pmcp-run";
const ATTESTATION_PAYLOAD_TYPE: &str = "application/vnd.test.attestation-payload";

/// An attestation payload that is neither valid JSON nor valid UTF-8.
///
/// Both properties are load-bearing: if anything on the carriage path parsed
/// the payload as JSON, or assumed it was a string, packing or unpacking would
/// fail. `\xff` and `\xfe` are never valid UTF-8 lead bytes, and `\x80` is a
/// continuation byte with no lead.
const OPAQUE_ATTESTATION_BYTES: &[u8] = b"\x00\x01 not json \xff\xfe\x80 \x00";

fn attestation_fixture_package() -> ServerPackage {
    serde_json::from_slice(&read_fixture("server_team_fs_v1.json"))
        .expect("fixture must parse as ServerPackage")
}

fn attestation_fixture_bootstrap() -> Vec<u8> {
    b"fake-arm64-bootstrap-binary-bytes-for-testing".to_vec()
}

/// Pack the fixture server into a fresh layout at `dir`, with or without an
/// attestation. Returns the layout and the manifest digest `pack_server`
/// produced.
fn pack_fixture_server(
    dir: &std::path::Path,
    attestation: Option<AttestationFile<'_>>,
) -> (OciLayout, ManifestDigest) {
    let layout = OciLayout::create(dir).unwrap();
    let digest = pack_server(
        &attestation_fixture_package(),
        BinaryMode::Embedded(&attestation_fixture_bootstrap()),
        None,
        None,
        attestation,
        &layout,
    )
    .unwrap();
    (layout, digest)
}

/// Build the `AttestationFile` used throughout these tests, claiming `subject`.
fn attestation_claiming(subject: &str) -> AttestationFile<'_> {
    AttestationFile {
        bytes: OPAQUE_ATTESTATION_BYTES,
        subject,
        issuer: ATTESTATION_ISSUER,
        payload_type: ATTESTATION_PAYLOAD_TYPE,
    }
}

/// Pack the fixture twice: once unattested, once carrying an attestation whose
/// subject is the unattested digest. Returns `(unattested_digest,
/// attested_digest, attested_layout)`.
///
/// The two `TempDir`s are returned so the caller keeps them alive — dropping a
/// `TempDir` deletes the layout out from under the test.
fn pack_the_same_package_with_and_without_an_attestation(
) -> (ManifestDigest, ManifestDigest, OciLayout, tempfile::TempDir) {
    let unattested_dir = tempfile::tempdir().unwrap();
    let (_unattested_layout, unattested_digest) = pack_fixture_server(unattested_dir.path(), None);

    let attested_dir = tempfile::tempdir().unwrap();
    let (attested_layout, attested_digest) = pack_fixture_server(
        attested_dir.path(),
        Some(attestation_claiming(unattested_digest.as_str())),
    );
    (
        unattested_digest,
        attested_digest,
        attested_layout,
        attested_dir,
    )
}

/// Read the single layer descriptor whose media type is `media_type`, or
/// `None` if the manifest carries no such layer.
fn layer_annotation(layout: &OciLayout, media_type: &str, key: &str) -> Option<String> {
    let index = layout.read_index().unwrap();
    let manifest = layout.read_manifest(&index.manifests()[0]).unwrap();
    manifest
        .layers()
        .iter()
        .find(|l| l.media_type().to_string() == media_type)
        .and_then(|l| l.annotations().as_ref().and_then(|a| a.get(key)).cloned())
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

/// Rewrite `layout`'s single manifest after applying `mutate` to it.
///
/// An in-place edit is not implementable: `write_manifest` derives both the
/// blob path and the descriptor digest from the bytes, while `unpack_server`
/// digest-verifies the index descriptor BEFORE parsing. So the index
/// descriptor's hand-set annotations are carried across by hand
/// (`finalize_pack` applies them AFTER the digest is computed, so they cannot
/// be recomputed), the mutated manifest is serialized with the SAME
/// canonicalizer `finalize_pack` uses, written as a NEW content-addressed
/// blob, and the index's single descriptor is REPLACED — never appended, which
/// would trip the "expected exactly one manifest" guard and look like a code
/// bug rather than a broken test.
///
/// Mirrors `tests/config_server.rs`'s `rewrite_manifest_layers` and
/// `unpack.rs`'s duplicate-layer test, generalized over the mutation.
fn rewrite_manifest(layout: &OciLayout, mutate: impl FnOnce(&mut oci_spec::image::ImageManifest)) {
    let mut index = layout.read_index().unwrap();
    let old_descriptor = index.manifests()[0].clone();
    let annotations = old_descriptor.annotations().clone();

    let mut manifest = layout.read_manifest(&old_descriptor).unwrap();
    mutate(&mut manifest);

    let manifest_bytes = canonicalize(&manifest).unwrap();
    let mut new_descriptor = layout.write_manifest(&manifest_bytes).unwrap();
    new_descriptor.set_annotations(annotations);
    index.set_manifests(vec![new_descriptor]);
    layout.write_index(&index).unwrap();
}

/// D-01's accepted consequence, pinned so a later reader who finds two digests
/// surprising meets the reasoning at the assertion rather than hunting for it.
///
/// An attestation names the digest of the UNATTESTED package as its subject.
/// The attestation layer, and its annotations, live INSIDE the manifest whose
/// canonical bytes are hashed — so attaching one necessarily changes the
/// package's own digest. An attested package's digest can therefore NEVER
/// equal the subject it names.
///
/// Both "fixes" for that surprise are regressions, and neither may be applied
/// later: comparing the subject against the ATTESTED digest would make the
/// check pass on a mis-attached attestation, and excluding the attestation
/// layer from the canonical digest would weaken `digest::verify` into
/// verifying everything except the one layer an attacker would most want to
/// swap.
#[test]
fn packing_with_and_without_an_attestation_yields_two_distinct_digests() {
    let (unattested_digest, attested_digest, _layout, _dir) =
        pack_the_same_package_with_and_without_an_attestation();

    assert_ne!(
        unattested_digest, attested_digest,
        "attaching an attestation must change the manifest digest — if it did not, the \
         attestation layer would be outside the bytes the digest covers, and a swapped \
         attestation would be invisible to `digest::verify`"
    );
}

/// The other half of the two-digest fact: the subject annotation read back off
/// the packed manifest equals the digest the UNATTESTED pack returned, and NOT
/// the digest of the package carrying it.
#[test]
fn the_attestation_subject_annotation_names_the_unattested_digest() {
    let (unattested_digest, attested_digest, layout, _dir) =
        pack_the_same_package_with_and_without_an_attestation();

    let subject = layer_annotation(&layout, MT_ATTESTATION, ANNOTATION_ATTESTATION_SUBJECT)
        .expect("an attested package's attestation layer must carry a subject annotation");

    assert_eq!(
        subject,
        unattested_digest.as_str(),
        "the subject must name the UNATTESTED package this attestation is about"
    );
    assert_ne!(
        subject,
        attested_digest.as_str(),
        "the subject can never equal the carrying package's own digest (D-01)"
    );
}

/// D-14 for the attestation layer: absence is the layer's absence, observed
/// through the public read path.
#[test]
fn an_unattested_package_round_trips_with_no_attestation() {
    let dir = tempfile::tempdir().unwrap();
    let (layout, _) = pack_fixture_server(dir.path(), None);

    let unpacked = unpack_server(&layout).unwrap();
    assert_eq!(
        unpacked.attestation, None,
        "`attestation: None` in must be `attestation: None` out — never a decoding default"
    );
}

/// D-14 again, this time observed on disk: there is no sentinel layer, no
/// empty attestation layer and no absence marker in the manifest.
#[test]
fn an_unattested_manifest_carries_no_attestation_layer_at_all() {
    let dir = tempfile::tempdir().unwrap();
    let (layout, _) = pack_fixture_server(dir.path(), None);

    let media_types = layer_media_types(&layout);
    assert!(
        !media_types.iter().any(|mt| mt == MT_ATTESTATION),
        "absence of an attestation is the absence of the layer — never a marker; layers were: \
         {media_types:?}"
    );
}

/// Opacity: `pmcp-package` never deserializes, parses or sniffs the payload,
/// so bytes that no parser would accept survive the round trip untouched.
#[test]
fn attestation_bytes_that_are_neither_json_nor_utf8_round_trip_byte_identically() {
    // Establish that the payload really is un-parseable, so a passing round
    // trip is evidence about the carriage path rather than about the fixture.
    //
    // The UTF-8 check goes through an owned `Vec` deliberately: called on the
    // constant directly, rustc's `invalid_from_utf8` lint recognises the
    // literal and warns (which `-D warnings` turns into a build failure). The
    // indirection keeps the assertion runtime-evaluated without weakening it.
    let payload: Vec<u8> = OPAQUE_ATTESTATION_BYTES.to_vec();
    assert!(
        std::str::from_utf8(&payload).is_err(),
        "the opacity fixture must contain invalid UTF-8, or it proves nothing about parsing"
    );
    assert!(
        serde_json::from_slice::<serde_json::Value>(OPAQUE_ATTESTATION_BYTES).is_err(),
        "the opacity fixture must not be valid JSON"
    );

    let dir = tempfile::tempdir().unwrap();
    let (layout, _) = pack_fixture_server(dir.path(), Some(attestation_claiming("sha256:unused")));

    let attestation = unpack_server(&layout)
        .unwrap()
        .attestation
        .expect("the package carried an attestation");
    assert_eq!(
        attestation.bytes,
        OPAQUE_ATTESTATION_BYTES.to_vec(),
        "the payload must come back byte-identical, in full"
    );
}

/// Position independence — asserting the property `pack.rs`'s own comment
/// claims, rather than restating it in prose: the deterministic push order
/// that writes the attestation LAST is not a read-order contract. Layers are
/// located by media type, so re-ordering the manifest cannot change what is
/// read.
#[test]
fn re_ordering_the_manifest_layers_does_not_change_the_attestation_read() {
    let dir = tempfile::tempdir().unwrap();
    let (layout, _) = pack_fixture_server(dir.path(), Some(attestation_claiming("sha256:unused")));

    let media_types = layer_media_types(&layout);
    assert_eq!(
        media_types.last().map(String::as_str),
        Some(MT_ATTESTATION),
        "pack writes the attestation layer last — the fact this test then breaks"
    );

    let baseline = unpack_server(&layout).unwrap().attestation;
    rewrite_manifest(&layout, |manifest| {
        let mut layers = manifest.layers().clone();
        layers.reverse();
        manifest.set_layers(layers);
    });

    let media_types = layer_media_types(&layout);
    assert_eq!(
        media_types.first().map(String::as_str),
        Some(MT_ATTESTATION),
        "the rewrite must actually have moved the attestation layer, or the test is a no-op"
    );

    assert_eq!(
        unpack_server(&layout).unwrap().attestation,
        baseline,
        "the attestation is located by media type, so its position carries no meaning"
    );
}

/// Kind independence — the cross-kind guard the kind-neutral media type
/// requires, and the reason plan 122-07 can reuse `MT_ATTESTATION` unchanged
/// for team packages.
///
/// Package kind and attestation location are INDEPENDENT axes: kind is read
/// from the manifest's `artifactType` plus the typed package layer, while the
/// attestation is located purely by its own media type. Altering ONLY the
/// `artifactType` therefore changes nothing about how the attestation is found
/// or read. If this ever fails, the shared constant has become a kind signal
/// by accident and the team carrier cannot reuse it.
#[test]
fn changing_only_the_artifact_type_does_not_change_how_the_attestation_is_located() {
    let dir = tempfile::tempdir().unwrap();
    let (layout, _) = pack_fixture_server(dir.path(), Some(attestation_claiming("sha256:unused")));

    let index = layout.read_index().unwrap();
    let manifest = layout.read_manifest(&index.manifests()[0]).unwrap();
    assert_eq!(
        manifest.artifact_type().as_ref().map(ToString::to_string),
        Some(ARTIFACT_TYPE_SERVER.to_string()),
        "an attested SERVER layout declares the server artifactType ..."
    );
    assert!(
        layer_media_types(&layout)
            .iter()
            .any(|mt| mt == MT_ATTESTATION),
        "... while its attestation layer declares the KIND-NEUTRAL media type"
    );

    let baseline: UnpackedAttestation = unpack_server(&layout)
        .unwrap()
        .attestation
        .expect("the package carried an attestation");

    rewrite_manifest(&layout, |manifest| {
        manifest.set_artifact_type(Some(oci_spec::image::MediaType::Other(
            ARTIFACT_TYPE_TEAM.to_string(),
        )));
    });

    let after = unpack_server(&layout)
        .unwrap()
        .attestation
        .expect("the attestation must still be located after the artifactType changed");
    assert_eq!(
        after, baseline,
        "kind detection and attestation location are independent axes — nothing may infer a \
         package's kind from the attestation layer, and nothing may use the kind to find it"
    );
    assert_eq!(after.issuer, ATTESTATION_ISSUER);
    assert_eq!(after.payload_type, ATTESTATION_PAYLOAD_TYPE);
}

/// Both non-subject annotations survive the round trip, so a later reader is
/// not left to assume the subject is the only one that is actually carried.
#[test]
fn the_issuer_and_payload_type_annotations_round_trip_verbatim() {
    let dir = tempfile::tempdir().unwrap();
    let (layout, _) = pack_fixture_server(dir.path(), Some(attestation_claiming("sha256:unused")));

    assert_eq!(
        layer_annotation(&layout, MT_ATTESTATION, ANNOTATION_ATTESTATION_ISSUER).as_deref(),
        Some(ATTESTATION_ISSUER)
    );
    assert_eq!(
        layer_annotation(&layout, MT_ATTESTATION, ANNOTATION_ATTESTATION_PAYLOAD_TYPE).as_deref(),
        Some(ATTESTATION_PAYLOAD_TYPE)
    );
}
