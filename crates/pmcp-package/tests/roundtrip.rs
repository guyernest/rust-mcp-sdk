//! Integration test: golden-fixture-based + programmatically-built
//! round-trip tests for all four AI-Package types (D-8 losslessness), plus
//! the I-2 canonical-byte-identity assertion against the two checked-in
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

use pmcp_package::digest::canonicalize;
use pmcp_package::oci::{
    pack_agent, pack_server, pack_team, pack_workflow, unpack_agent, unpack_server, unpack_team,
    unpack_workflow, OciLayout,
};
use pmcp_package::package::{
    AgentPackage, HumanRole, ServerPackage, TeamLimits, TeamMember, TeamPackage, TeamRole,
    WorkflowManifest,
};
use pmcp_package::reference::{ComponentRef, ComponentType};
use pmcp_package::slot::{ConfigSlot, SlotType};
use std::path::Path;

/// Read a checked-in golden fixture's raw bytes from `tests/golden_fixtures/`.
fn read_fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden_fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read fixture {path:?}: {e}"))
}

// ---------------------------------------------------------------------
// Fixture-backed types: ServerPackage, WorkflowManifest
// ---------------------------------------------------------------------

#[test]
fn server_package_fixture_round_trips_and_matches_canonical_bytes() {
    let fixture_bytes = read_fixture("server_team_fs_v1.json");
    let parsed: ServerPackage =
        serde_json::from_slice(&fixture_bytes).expect("fixture must parse as ServerPackage");

    // I-2 canonicality: the checked-in fixture bytes ARE canonicalize(&parsed)'s
    // output, byte-for-byte — no re-pretty-printing, no key reordering.
    let recanonicalized = canonicalize(&parsed).expect("ServerPackage must canonicalize");
    assert_eq!(
        recanonicalized, fixture_bytes,
        "server_team_fs_v1.json must be stored in canonical byte form"
    );

    let bootstrap = b"fake-arm64-bootstrap-binary-bytes-for-testing".to_vec();
    let dir = tempfile::tempdir().unwrap();
    let layout = OciLayout::create(dir.path()).unwrap();
    pack_server(&parsed, &bootstrap, &layout).unwrap();
    let (unpacked, unpacked_bootstrap) = unpack_server(&layout).unwrap();

    assert_eq!(
        unpacked, parsed,
        "ServerPackage must round-trip pack/unpack losslessly (D-8)"
    );
    assert_eq!(
        unpacked_bootstrap, bootstrap,
        "bootstrap bytes must round-trip pack/unpack losslessly (D-8)"
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
        "WorkflowManifest must round-trip pack/unpack losslessly (D-8)"
    );
    assert!(
        unpacked.validate_all_pinned().is_ok(),
        "the fixture manifest must be fully pinned (I-1)"
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
        llm: ConfigSlot {
            slot: SlotType::LlmProvider {
                name: "primary-llm".to_string(),
                tested_value: "anthropic".to_string(),
            },
        },
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
        budget_defaults: vec![ConfigSlot {
            slot: SlotType::BudgetOverride {
                name: "monthly-cap".to_string(),
                tested_value: "500".to_string(),
            },
        }],
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
        "AgentPackage must round-trip pack/unpack losslessly (D-8)"
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
        "TeamPackage must round-trip pack/unpack losslessly (D-8)"
    );
}
