//! REF-01 superset integration test.
//!
//! Parses each of the three reference `config.toml` snapshots from
//! `tests/fixtures/` and asserts that the toolkit's [`ServerConfig`] is a
//! **superset** of every field the pmcp-run reference SQL-API servers emit.
//! Failure here = REF-01 broken — the fix is to ADD the missing field to
//! `config.rs`, never to loosen `deny_unknown_fields` (RESEARCH §Pitfall 1,
//! PATTERNS §8 anti-pattern).
//!
//! Per Phase 83 review R8, each fixture must ALSO pass
//! [`ServerConfig::validate`] — real production configs must satisfy the
//! required-field invariants. We invoke
//! [`ServerConfig::from_toml_strict_validated`] which runs parse + validate
//! in one call.

use pmcp_server_toolkit::config::ServerConfig;
use pmcp_server_toolkit::tools::synthesize_from_config;

#[test]
fn open_images_config_parses_and_validates() {
    let toml = include_str!("fixtures/open-images-config.toml");
    let cfg = ServerConfig::from_toml_strict_validated(toml)
        .expect("open-images config must parse + validate — REF-01 superset invariant + review R8");
    assert!(
        !cfg.tools.is_empty(),
        "open-images config must declare at least one [[tools]] entry"
    );
    assert!(
        cfg.code_mode.is_some(),
        "open-images config uses [code_mode] — must round-trip"
    );
}

#[test]
fn imdb_config_parses_and_validates() {
    let toml = include_str!("fixtures/imdb-config.toml");
    let cfg = ServerConfig::from_toml_strict_validated(toml)
        .expect("imdb config must parse + validate — REF-01 superset invariant + review R8");
    assert!(
        !cfg.tools.is_empty(),
        "imdb config must declare at least one [[tools]] entry"
    );
}

#[test]
fn msr_vtt_config_parses_and_validates() {
    let toml = include_str!("fixtures/msr-vtt-config.toml");
    let cfg = ServerConfig::from_toml_strict_validated(toml)
        .expect("msr-vtt config must parse + validate — REF-01 superset invariant + review R8");
    assert!(
        !cfg.tools.is_empty(),
        "msr-vtt config must declare at least one [[tools]] entry"
    );
}

// =============================================================================
// Plan 83-05 (TKIT-07): synthesize_from_config against the same fixtures.
//
// Locks in the headline DX promise that any pmcp-run reference SQL-API
// config.toml synthesizes end-to-end without errors and produces one
// ToolInfo per [[tools]] entry. Anchors Plan 08's `tools_from_config` smoke
// test.
// =============================================================================

/// Shared assertions for "fixture parses + synthesizes" — keeps the per-fixture
/// tests succinct without forcing macro indirection.
fn assert_fixture_synthesizes(toml: &str, label: &str) {
    let cfg = ServerConfig::from_toml_strict_validated(toml).expect("parse + validate");
    let synthesized =
        synthesize_from_config(&cfg).unwrap_or_else(|e| panic!("{label} synthesis failed: {e}"));
    assert_eq!(
        synthesized.len(),
        cfg.tools.len(),
        "{label}: synthesizer must produce one tuple per [[tools]] entry"
    );
    for (name, info, handler) in &synthesized {
        assert!(!name.is_empty(), "{label}: tool name non-empty");
        assert_eq!(
            info.name, *name,
            "{label}: ToolInfo.name matches tuple name"
        );
        assert_eq!(
            info.input_schema["type"].as_str(),
            Some("object"),
            "{label}: every synthesized ToolInfo has an object schema"
        );
        assert!(
            handler.metadata().is_some(),
            "{label}: handler.metadata() must return Some — RESEARCH §Risks #2"
        );
    }
}

#[test]
fn open_images_synthesizes() {
    let toml = include_str!("fixtures/open-images-config.toml");
    assert_fixture_synthesizes(toml, "open-images");
}

#[test]
fn imdb_synthesizes() {
    let toml = include_str!("fixtures/imdb-config.toml");
    assert_fixture_synthesizes(toml, "imdb");
}

#[test]
fn msr_vtt_synthesizes() {
    let toml = include_str!("fixtures/msr-vtt-config.toml");
    assert_fixture_synthesizes(toml, "msr-vtt");
}
