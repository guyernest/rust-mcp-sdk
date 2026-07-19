//! Phase 110 Plan 05 — `cargo pmcp package show <path>` offline-render tests
//! (CLI-04).
//!
//! `show` opens a local OCI image-layout `.pmcp` package and renders its kind +
//! key manifest fields with NO network access. These tests build a real agent
//! fixture with `pmcp_package::oci::pack_agent`, invoke the actual binary, and
//! assert the rendered output — plus the edge cases the plan calls out (Codex
//! MEDIUM): a non-OCI-layout path and a zero-manifest layout must both error
//! with a clear message rather than being indexed blindly.

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use pmcp_package::oci::{pack_agent, OciLayout};
use pmcp_package::{AgentPackage, ConfigSlot, SlotType};

/// Build a minimal, valid `AgentPackage` fixture (mirrors the s50 shape used by
/// the `agent new` scaffold — built from the real struct so it round-trips).
fn sample_agent_package() -> AgentPackage {
    AgentPackage {
        name: "triage-agent".to_string(),
        version: semver::Version::new(1, 0, 0),
        instructions: "You triage incoming support tickets.".to_string(),
        llm: ConfigSlot {
            slot: SlotType::LlmProvider {
                name: "primary-llm".to_string(),
                tested_value: "anthropic".to_string(),
            },
        },
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
/// the layout root path (the `<path>` argument `package show` expects).
fn write_agent_fixture(dir: &std::path::Path) {
    let layout = OciLayout::create(dir).expect("create OCI layout");
    pack_agent(&sample_agent_package(), &layout).expect("pack agent fixture");
}

/// A generated `.pmcp` agent fixture renders its kind + name fully offline.
#[test]
fn show_renders_agent_fixture_offline() {
    let dir = tempfile::tempdir().unwrap();
    write_agent_fixture(dir.path());

    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["package", "show", dir.path().to_str().unwrap()])
        .assert()
        .success()
        // Kind is rendered lowercase (`agent`), and the manifest name field shows.
        .stdout(contains("agent"))
        .stdout(contains("triage-agent"));
}

/// A path that is NOT an OCI image layout (an empty tempdir with no `index.json`)
/// errors before any unpack, with an actionable message (V5).
#[test]
fn show_rejects_non_layout_path() {
    let dir = tempfile::tempdir().unwrap();

    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["package", "show", dir.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains("OCI").or(contains("layout")).or(contains("index.json")));
}

/// A zero-manifest layout (a freshly-created, empty OCI layout) is rejected with
/// a clear error — the handler does NOT try to index an empty manifest list.
#[test]
fn show_rejects_zero_manifest_layout() {
    let dir = tempfile::tempdir().unwrap();
    OciLayout::create(dir.path()).expect("create empty OCI layout");

    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["package", "show", dir.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains("manifest"));
}
