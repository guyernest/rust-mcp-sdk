//! Phase 110 Plan 05 — `cargo pmcp package inspect <path>` offline-render tests
//! (CLI-04).
//!
//! `inspect` opens a local OCI image-layout `.pmcp` package and renders its kind +
//! key manifest fields with NO network access. These tests build a real agent
//! fixture with `pmcp_package::oci::pack_agent`, invoke the actual binary, and
//! assert the rendered output — plus the edge cases the plan calls out (Codex
//! MEDIUM): a non-OCI-layout path and a zero-manifest layout must both error
//! with a clear message rather than being indexed blindly.

use assert_cmd::Command;
use pmcp_package::oci::{pack_agent, pack_server, AttestationFile, BinaryMode, OciLayout};
use pmcp_package::package::{
    AssetsSection, AuthSection, AwsSection, DeployDescriptor, ObservabilitySection, ServerSection,
    TargetSection,
};
use pmcp_package::{
    AgentPackage, CedarPolicySet, ConfigSlot, ManifestDigest, ServerPackage, SlotType,
};
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

/// Build a minimal, valid `AgentPackage` fixture (mirrors the s50 shape used by
/// the `agent new` scaffold — built from the real struct so it round-trips).
fn sample_agent_package() -> AgentPackage {
    AgentPackage {
        name: "triage-agent".to_string(),
        version: semver::Version::new(1, 0, 0),
        instructions: "You triage incoming support tickets.".to_string(),
        llm: ConfigSlot::new(SlotType::LlmProvider {
            name: "primary-llm".to_string(),
            tested_value: "anthropic".to_string(),
        }),
        max_tokens: 4096,
        max_iterations: 25,
        connectors: vec![],
        tool_selection: None,
        input_schema: None,
        output_schema: None,
        importance: None,
        finalizer_role: None,
        budget_defaults: vec![],
    }
}

/// Pack `sample_agent_package()` into a fresh OCI layout under `dir` and return
/// the layout root path (the `<path>` argument `package inspect` expects).
fn write_agent_fixture(dir: &std::path::Path) {
    let layout = OciLayout::create(dir).expect("create OCI layout");
    pack_agent(&sample_agent_package(), &layout).expect("pack agent fixture");
}

/// A generated `.pmcp` agent fixture renders its kind + name fully offline.
#[test]
fn inspect_renders_agent_fixture_offline() {
    let dir = tempfile::tempdir().unwrap();
    write_agent_fixture(dir.path());

    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["package", "inspect", dir.path().to_str().unwrap()])
        .assert()
        .success()
        // Kind is rendered lowercase (`agent`), and the manifest name field shows.
        .stdout(contains("agent"))
        .stdout(contains("triage-agent"));
}

/// A path that is NOT an OCI image layout (an empty tempdir with no `index.json`)
/// errors before any unpack, with an actionable message (V5).
#[test]
fn inspect_rejects_non_layout_path() {
    let dir = tempfile::tempdir().unwrap();

    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["package", "inspect", dir.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(
            contains("OCI")
                .or(contains("layout"))
                .or(contains("index.json")),
        );
}

// ---------------------------------------------------------------------
// Attestation carriage — the end-to-end render path, offline, from a fixture
// ---------------------------------------------------------------------

/// A minimal, valid `ServerPackage` fixture built from the real struct.
fn sample_server_package() -> ServerPackage {
    ServerPackage {
        name: "london-tube".to_string(),
        version: semver::Version::new(1, 0, 0),
        digest: None,
        deploy: DeployDescriptor {
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
            environment: std::collections::BTreeMap::new(),
            secrets: std::collections::BTreeMap::new(),
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
                exclude: vec![],
            }),
            iam: None,
            gcp: None,
            layout: None,
        },
        policies: CedarPolicySet(vec![]),
        tools: vec![],
        config_slots: vec![],
    }
}

/// The binary a Shape A pure-config server NAMES rather than carries.
fn referenced_binary() -> BinaryMode<'static> {
    BinaryMode::Referenced {
        digest: ManifestDigest::from_bytes(b"pmcp-openapi-server-v1.0.0-aarch64"),
        media_type: "application/x-lambda-bootstrap; arch=arm64".to_string(),
    }
}

const TEST_ISSUER: &str = "https://issuer.test.invalid/pmcp-run";
const TEST_PAYLOAD_TYPE: &str = "application/vnd.test.attestation-payload";

/// Attestation payload bytes that are deliberately NOT valid JSON (and not
/// valid UTF-8 either). If anything on the carriage path parsed the payload,
/// packing or unpacking would fail — so a passing render is evidence that the
/// bytes travel opaquely.
const OPAQUE_PAYLOAD: &[u8] = b"\x00\x01 this is not json \xff\xfe \x00";

/// Pack `sample_server_package()` into a fresh layout at `dir`, carrying an
/// attestation whose subject is the digest of the SAME package packed WITHOUT
/// one. Returns the subject digest that was claimed.
fn write_attested_server_fixture(dir: &std::path::Path) -> String {
    let package = sample_server_package();

    // The subject an attestation names is the UNATTESTED package's digest, so
    // it has to be computed by packing without the attestation layer first.
    let unattested_dir = tempfile::tempdir().expect("create the unattested scratch layout");
    let unattested_layout =
        OciLayout::create(unattested_dir.path()).expect("create the unattested scratch layout");
    let subject = pack_server(
        &package,
        referenced_binary(),
        None,
        None,
        None,
        &unattested_layout,
    )
    .expect("the unattested package must pack")
    .as_str()
    .to_string();

    let layout = OciLayout::create(dir).expect("create the attested OCI layout");
    pack_server(
        &package,
        referenced_binary(),
        None,
        None,
        Some(AttestationFile {
            bytes: OPAQUE_PAYLOAD,
            subject: &subject,
            issuer: TEST_ISSUER,
            payload_type: TEST_PAYLOAD_TYPE,
        }),
        &layout,
    )
    .expect("the attested package must pack");

    subject
}

/// The tracer's own end-to-end check: attestation bytes supplied to
/// `pack_server` survive to the real `cargo pmcp package inspect` binary, which
/// renders the issuer and the claimed subject digest — offline, from a fixture,
/// with a payload that is not parseable JSON.
#[test]
fn inspect_renders_an_attested_server_fixture_offline() {
    let dir = tempfile::tempdir().unwrap();
    let subject = write_attested_server_fixture(dir.path());

    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["package", "inspect", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("server"))
        .stdout(contains(TEST_ISSUER))
        .stdout(contains(subject));
}

/// A zero-manifest layout (a freshly-created, empty OCI layout) is rejected with
/// a clear error — the handler does NOT try to index an empty manifest list.
#[test]
fn inspect_rejects_zero_manifest_layout() {
    let dir = tempfile::tempdir().unwrap();
    OciLayout::create(dir.path()).expect("create empty OCI layout");

    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["package", "inspect", dir.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains("manifest"));
}
