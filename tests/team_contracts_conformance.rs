//! Conformance gate over `contracts/team-servers-v1.yaml` and its versioned fixtures.
//!
//! This is a dependency-free (`serde_json` + `std` only) schema-aware test suite that:
//! 1. asserts the contract enumerates the four team-server tool surfaces and all 19
//!    static tool names plus the two dynamic tool-family prefixes;
//! 2. validates every fixture against the versioned fixture schema
//!    (`schema_version`, `case_id`, `server`, `request.name`, `expect{outcome,match,response}`);
//! 3. cross-references every fixture's `request.name` (or its `team_mcp__` /
//!    `team_approval__` prefix) against the contract text;
//! 4. asserts per-server coverage and the presence of high-value negative fixtures.
//!
//! Both the contract and the fixtures directory are resolved via `CARGO_MANIFEST_DIR`
//! so the test is location-independent. Exhaustive executable behavior (running the
//! fixtures against a live server) is deferred to Phase 109 (TEAM-06).

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// The 19 static tool names the contract must enumerate:
/// 11 `fs__*` + 6 `mem__*` + the two unnamespaced legacy approval names.
const STATIC_TOOL_NAMES: &[&str] = &[
    "fs__list",
    "fs__read",
    "fs__write",
    "fs__append_file",
    "fs__head",
    "fs__stat",
    "fs__create_directory",
    "fs__get_download_url",
    "fs__sync_to_review",
    "fs__sync_from_review",
    "fs__complete_task",
    "mem__add",
    "mem__get",
    "mem__search",
    "mem__list_recent",
    "mem__delete",
    "mem__complete_task",
    "resolve_approval",
    "get_approval",
];

/// The two dynamic tool families are enumerated by prefix, not by full name.
const DYNAMIC_TOOL_PREFIXES: &[&str] = &["team_approval__ask_", "team_mcp__"];

/// One equation per server surface.
const EQUATION_KEYS: &[&str] = &[
    "fs_tool_surface",
    "mem_tool_surface",
    "approval_tool_surface",
    "team_dispatch_surface",
];

/// Expected fixture sub-directories (one per server surface).
const EXPECTED_SERVERS: &[&str] = &["team-fs", "mem-mcp", "approval-mcp", "team-mcp"];

fn contract_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("contracts/team-servers-v1.yaml")
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("contracts/team-servers/fixtures")
}

fn read_contract() -> String {
    fs::read_to_string(contract_path()).expect("read contracts/team-servers-v1.yaml")
}

/// A parsed conformance fixture plus provenance (its server directory + path).
struct Fixture {
    server_dir: String,
    path: PathBuf,
    value: Value,
}

/// Walk `contracts/team-servers/fixtures/<server>/*.json`, parsing each file.
fn load_fixtures() -> Vec<Fixture> {
    let root = fixtures_dir();
    let mut fixtures = Vec::new();
    for server_entry in fs::read_dir(&root).expect("read fixtures dir") {
        let server_path = server_entry.expect("server dir entry").path();
        if !server_path.is_dir() {
            continue;
        }
        let server_dir = server_path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("server dir name")
            .to_owned();
        for file_entry in fs::read_dir(&server_path).expect("read server dir") {
            let path = file_entry.expect("fixture entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let text = fs::read_to_string(&path).expect("read fixture json");
            let value: Value = serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("fixture {} is not valid JSON: {e}", path.display()));
            fixtures.push(Fixture {
                server_dir: server_dir.clone(),
                path,
                value,
            });
        }
    }
    fixtures
}

/// A tool name is captured if a dynamic-family prefix matches (and the prefix is in
/// the contract) or the exact static name appears in the contract.
fn tool_is_captured(contract: &str, name: &str) -> bool {
    for prefix in DYNAMIC_TOOL_PREFIXES {
        if name.starts_with(prefix) {
            return contract.contains(prefix);
        }
    }
    contract.contains(name)
}

#[test]
fn contract_declares_all_equations_and_tool_names() {
    let contract = read_contract();

    for key in EQUATION_KEYS {
        assert!(
            contract.contains(key),
            "contract missing equation key '{key}'"
        );
    }

    assert_eq!(
        STATIC_TOOL_NAMES.len(),
        19,
        "expected exactly 19 static tool names (11 fs__* + 6 mem__* + 2 approval-static)"
    );
    for name in STATIC_TOOL_NAMES {
        assert!(
            contract.contains(name),
            "contract missing static tool name '{name}'"
        );
    }
    for prefix in DYNAMIC_TOOL_PREFIXES {
        assert!(
            contract.contains(prefix),
            "contract missing dynamic tool-family prefix '{prefix}'"
        );
    }
}

#[test]
fn fixtures_conform_to_versioned_schema() {
    let fixtures = load_fixtures();
    assert!(!fixtures.is_empty(), "no fixtures found under fixtures dir");

    for fx in &fixtures {
        let v = &fx.value;
        let loc = fx.path.display();

        assert_eq!(
            v["schema_version"].as_str(),
            Some("1"),
            "{loc}: schema_version must be the string \"1\""
        );

        let case_id = v["case_id"]
            .as_str()
            .unwrap_or_else(|| panic!("{loc}: case_id must be a string"));
        assert!(!case_id.is_empty(), "{loc}: case_id must be non-empty");

        let server = v["server"]
            .as_str()
            .unwrap_or_else(|| panic!("{loc}: server must be a string"));
        assert_eq!(
            server, fx.server_dir,
            "{loc}: server '{server}' must match its directory '{}'",
            fx.server_dir
        );

        let name = v["request"]["name"]
            .as_str()
            .unwrap_or_else(|| panic!("{loc}: request.name must be a string"));
        assert!(!name.is_empty(), "{loc}: request.name must be non-empty");

        let outcome = v["expect"]["outcome"]
            .as_str()
            .unwrap_or_else(|| panic!("{loc}: expect.outcome must be a string"));
        assert!(
            matches!(outcome, "success" | "error"),
            "{loc}: expect.outcome must be 'success' or 'error', got '{outcome}'"
        );

        assert!(
            v["expect"]["match"].is_string(),
            "{loc}: expect.match must be present as a string"
        );
        assert!(
            !v["expect"]["response"].is_null(),
            "{loc}: expect.response must be present"
        );
    }
}

#[test]
fn every_fixture_tool_is_captured_in_contract() {
    let contract = read_contract();
    let fixtures = load_fixtures();

    for fx in &fixtures {
        let name = fx.value["request"]["name"]
            .as_str()
            .expect("request.name checked by schema test");
        assert!(
            tool_is_captured(&contract, name),
            "{}: tool '{name}' is not captured in the contract",
            fx.path.display()
        );
    }
}

#[test]
fn coverage_spans_all_servers_and_negative_cases() {
    let fixtures = load_fixtures();

    for server in EXPECTED_SERVERS {
        let count = fixtures.iter().filter(|f| f.server_dir == *server).count();
        assert!(count >= 1, "server '{server}' has no fixtures");
    }

    let negatives = fixtures
        .iter()
        .filter(|f| f.value["expect"]["outcome"].as_str() == Some("error"))
        .count();
    assert!(
        negatives >= 4,
        "expected at least 4 negative/security fixtures, found {negatives}"
    );
}

#[test]
fn at_least_one_related_task_meta_fixture() {
    let fixtures = load_fixtures();
    let found = fixtures
        .iter()
        .any(|f| !f.value["expect"]["response"]["_meta"]["related_task"].is_null());
    assert!(
        found,
        "expected at least one fixture placing related_task under a top-level _meta \
         (fs__complete_task / team_mcp__<member> ToolOutput::Result surface)"
    );
}
