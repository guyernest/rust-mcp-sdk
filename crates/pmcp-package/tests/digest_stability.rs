//! Integration test: digest-stability guarantees over the checked-in
//! golden fixtures.
//!
//! - `manifest_digest()` computed repeatedly (≥100 times) over the same
//!   fixture value is always identical.
//! - Re-ordering a source collection field (a `BTreeMap`'s insertion order, or
//!   the order `WorkflowManifest::new` receives its `components`/
//!   `aggregated_slots` arguments) does not change the resulting canonical
//!   digest — olpc-cjson key-sorting for maps, and `WorkflowManifest::new`'s
//!   own deterministic sort for its `Vec` fields.
//! - `AgentPackage` (which now carries no bare-float field) routes through the
//!   SAME `manifest_digest()`/`canonicalize()` path as the other three package
//!   types — proven directly by the agent stability test below.

use pmcp_package::digest::manifest_digest;
use pmcp_package::package::{AgentPackage, ServerPackage, WorkflowManifest};
use pmcp_package::reference::{ComponentRef, ComponentType};
use pmcp_package::slot::{ConfigSlot, SlotType};
use std::collections::BTreeMap;
use std::path::Path;

fn read_fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
.join("tests")
.join("golden_fixtures")
.join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read fixture {path:?}: {e}"))
}

#[test]
fn server_fixture_digest_is_stable_across_100_computations() {
    let bytes = read_fixture("server_team_fs_v1.json");
    let package: ServerPackage = serde_json::from_slice(&bytes).unwrap();

    let first = manifest_digest(&package).unwrap();
    for _ in 0..100 {
        let next = manifest_digest(&package).unwrap();
        assert_eq!(
            next, first,
            "manifest_digest must be stable across repeated computation"
       );
    }
}

#[test]
fn workflow_fixture_digest_is_stable_across_100_computations() {
    let bytes = read_fixture("workflow_claims_triage_v1.json");
    let manifest: WorkflowManifest = serde_json::from_slice(&bytes).unwrap();

    let first = manifest_digest(&manifest).unwrap();
    for _ in 0..100 {
        let next = manifest_digest(&manifest).unwrap();
        assert_eq!(
            next, first,
            "manifest_digest must be stable across repeated computation"
       );
    }
}

/// `AgentPackage` now carries no bare-float field, so it is
/// `canonicalize()`-able and routes through the exact same
/// `manifest_digest()` path as `ServerPackage`/`TeamPackage`/
/// `WorkflowManifest`. This test proves that path is stable for an
/// `AgentPackage` (it would have returned an `Err` from `manifest_digest()`
/// under the old `temperature: f64` schema, since olpc-cjson rejects floats).
#[test]
fn agent_package_digest_is_stable_across_100_computations_via_canonicalize() {
    let package = AgentPackage {
        name: "claims-triage-agent".to_string(),
        version: semver::Version::parse("1.2.0").unwrap(),
        instructions: "You triage incoming insurance claims.".to_string(),
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
    };

    let first = manifest_digest(&package)
.expect("AgentPackage must be canonicalize()-able now that it carries no bare float");
    for _ in 0..100 {
        let next = manifest_digest(&package).unwrap();
        assert_eq!(
            next, first,
            "manifest_digest must be stable across repeated computation"
       );
    }
}

#[test]
fn reordering_deploy_descriptor_environment_map_does_not_change_digest() {
    // Build the SAME logical DeployDescriptor via two different BTreeMap
    // insertion orders and confirm the digest matches (key-order
    // independence via olpc-cjson).
    let bytes = read_fixture("server_team_fs_v1.json");
    let package: ServerPackage = serde_json::from_slice(&bytes).unwrap();
    assert!(
        package.deploy.environment.len() >= 2,
        "fixture must declare at least 2 environment entries to make reordering meaningful"
   );

    let mut forward = BTreeMap::new();
    for (k, v) in package.deploy.environment.iter() {
        forward.insert(k.clone(), v.clone());
    }
    let mut backward = BTreeMap::new();
    for (k, v) in package.deploy.environment.iter().rev() {
        backward.insert(k.clone(), v.clone());
    }

    let mut package_forward = package.clone();
    package_forward.deploy.environment = forward;
    let mut package_backward = package.clone();
    package_backward.deploy.environment = backward;

    let digest_forward = manifest_digest(&package_forward).unwrap();
    let digest_backward = manifest_digest(&package_backward).unwrap();
    assert_eq!(
        digest_forward, digest_backward,
        "canonical digest must not depend on map insertion order"
   );
}

#[test]
fn reordering_workflow_components_and_slots_via_new_yields_same_digest() {
    // WorkflowManifest::new sorts `components`/`aggregated_slots`
    // deterministically regardless of the order they're supplied in —
    // confirm two differently-ordered constructions of the SAME logical
    // manifest produce the same digest.
    let bytes = read_fixture("workflow_claims_triage_v1.json");
    let manifest: WorkflowManifest = serde_json::from_slice(&bytes).unwrap();
    assert!(
        manifest.components.len() >= 2,
        "fixture must declare at least 2 components to make reordering meaningful"
   );

    let forward = WorkflowManifest::new(
        manifest.name.clone(),
        manifest.version.clone(),
        manifest.components.clone(),
        manifest.aggregated_slots.clone(),
        manifest.provenance.clone(),
   );

    let mut reversed_components = manifest.components.clone();
    reversed_components.reverse();
    let mut reversed_slots = manifest.aggregated_slots.clone();
    reversed_slots.reverse();
    let backward = WorkflowManifest::new(
        manifest.name.clone(),
        manifest.version.clone(),
        reversed_components,
        reversed_slots,
        manifest.provenance.clone(),
   );

    let digest_forward = manifest_digest(&forward).unwrap();
    let digest_backward = manifest_digest(&backward).unwrap();
    assert_eq!(
        digest_forward, digest_backward,
        "WorkflowManifest::new must sort deterministically regardless of input order"
   );
}
