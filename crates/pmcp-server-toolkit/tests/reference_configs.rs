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

// =============================================================================
// Plan 84-08 (TEST-07, WARNING #8 + REVIEWS M6): fuzz-corpus seed-parse smoke test.
//
// The single fuzz target `pmcp_server_toolkit_config_parser` exercises
// `ServerConfig::from_toml` under libfuzzer mutation; this test pins down the
// well-formed seeds so they cannot silently rot, while tolerating the REVIEWS M6
// adversarial seeds (extremely-long URL, non-ASCII URL, malformed env-ref,
// SQL-injection-shape URL) returning Err. The invariant for all seeds is "no
// panic" — libfuzzer enforces that under mutation at runtime; here we only
// materialize the Result.
// =============================================================================

#[test]
fn fuzz_corpus_seeds_parse_or_explicitly_fail() {
    let corpus = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fuzz/corpus/pmcp_server_toolkit_config_parser");
    let mut well_formed_seed_ok = 0usize;
    let mut total = 0usize;
    for entry in std::fs::read_dir(&corpus).expect("corpus dir") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some("toml") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&path).unwrap();
        let parsed = pmcp_server_toolkit::config::ServerConfig::from_toml(&text);
        total += 1;
        // Well-formed seeds MUST parse. Adversarial seeds (extremely-long, non-ascii,
        // malformed env-ref, injection-shape) MAY be Ok or Err depending on parser
        // strictness; the invariant is that the parser does NOT panic.
        let is_adversarial = name.contains("url-extremely-long")
            || name.contains("url-non-ascii")
            || name.contains("url-malformed-env-ref")
            || name.contains("url-sql-injection-shape");
        // Plan 90-02: seed-backend.toml carries a `[backend]` block that only
        // parses when the `http` feature is enabled (the field is
        // `#[cfg(feature = "http")]`). Under default features it is an unknown
        // section and Err is the correct, non-panicking outcome; under `http`
        // it MUST parse. Treat it as feature-conditional rather than adversarial.
        let is_http_gated = name == "seed-backend.toml";
        if is_http_gated {
            #[cfg(feature = "http")]
            {
                parsed.unwrap_or_else(|e| {
                    panic!("http-gated seed {name} must parse under --features http: {e}")
                });
                well_formed_seed_ok += 1;
            }
            #[cfg(not(feature = "http"))]
            {
                // No panic is the only invariant when the feature is off.
                let _ = parsed;
            }
        } else if name.starts_with("seed-") && !is_adversarial {
            parsed.unwrap_or_else(|e| panic!("well-formed seed {name} failed to parse: {e}"));
            well_formed_seed_ok += 1;
        }
        // Adversarial seeds: simply assert no panic occurred (the Result was materialized).
        // libfuzzer separately enforces the no-panic invariant under mutation.
    }
    assert!(
        well_formed_seed_ok >= 4,
        "expected at least 4 well-formed seed files (3 per-backend + Plan 00's url seed); iterated {total} files"
    );
}
