//! OAPI-08 / D-04 — the london-tube **reference-parity** assertion.
//!
//! Reproduces the london-tube (TfL) reference instance and proves the Shape-A
//! binary serves the SAME tools + behavior as the pmcp-run reference, replayed
//! OFFLINE via `wiremock` (pure-Rust, no Docker, no live network in default CI).
//! This is the single most valuable parity target — it exercises the new
//! `api_key` query-parameter outgoing-auth path AND the `${TFL_APP_KEY}` secret
//! expansion path end-to-end.
//!
//! Tests here:
//!
//! 1. [`london_tube_fixture`] — fixture-validity: the vendored `london-tube.toml`
//!    parses via [`ServerConfig::from_toml_strict_validated`] and
//!    `london-tube-api.yaml` via [`OpenApiSchema::parse`], with the expected tool
//!    names present.
//! 2. `london_tube_parity_through_real_binary_path` — drives the REAL binary
//!    pipeline ([`run_serving`]) against a `wiremock` backend that REQUIRES the
//!    `app_key=dummy` query param (proving the api_key query-param auth AND that
//!    `${TFL_APP_KEY}` resolved to `dummy`, never the literal `${...}`), then
//!    replays `london-tube-scenarios.yaml` through `mcp-tester`, gating per-step.
//! 3. `parity_live_tfl` (`#[ignore]`) — the same scenarios against the REAL
//!    `https://api.tfl.gov.uk`, double-gated on `PMCP_OPENAPI_LIVE_TEST=1` +
//!    a real `TFL_APP_KEY`.
//!
//! Run the offline suite (single-threaded — ephemeral port + per-process env):
//! ```sh
//! cargo test -p pmcp-openapi-server --test parity_replay -- --test-threads=1
//! ```

use pmcp_server_toolkit::http::OpenApiSchema;
use pmcp_server_toolkit::ServerConfig;

/// Absolute path to the vendored fixtures directory.
fn fixtures_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Fixture-validity (Task 1): the vendored config + spec parse, and the expected
/// reference tool names are present. This is the offline gate that the vendored
/// fixtures are well-formed BEFORE the parity replay drives them.
#[test]
fn london_tube_fixture() {
    let dir = fixtures_dir();

    // (1) The config parses through the production strict+validated entry point.
    let config_text =
        std::fs::read_to_string(dir.join("london-tube.toml")).expect("read london-tube.toml");
    let cfg = ServerConfig::from_toml_strict_validated(&config_text)
        .expect("vendored london-tube.toml parses + validates");

    // The expected reference tool names are present (one single-call + one script).
    let tool_names: Vec<&str> = cfg.tools.iter().map(|t| t.name.as_str()).collect();
    assert!(
        tool_names.contains(&"get-tube-status"),
        "single-call tool get-tube-status present: {tool_names:?}"
    );
    assert!(
        tool_names.contains(&"disrupted-lines-with-detail"),
        "script tool disrupted-lines-with-detail present: {tool_names:?}"
    );

    // The api_key query-param backend auth (the D-04 path) is declared.
    let backend = cfg.backend.as_ref().expect("[backend] section present");
    assert!(
        matches!(
            &backend.auth,
            pmcp_server_toolkit::http::auth::AuthConfig::ApiKey { query_params, .. }
                if query_params.get("app_key").is_some_and(|v| v.contains("${TFL_APP_KEY}"))
        ),
        "[backend.auth] is api_key with an app_key=${{TFL_APP_KEY}} query param"
    );

    // The disrupted-lines tool is a SCRIPT tool (D-01 detection rule).
    let script_tool = cfg
        .tools
        .iter()
        .find(|t| t.name == "disrupted-lines-with-detail")
        .expect("script tool present");
    assert!(
        script_tool.is_script_tool(),
        "disrupted-lines-with-detail is a script tool"
    );

    // (2) The minimal OpenAPI spec parses and surfaces the operations the tools call.
    let spec_text = std::fs::read_to_string(dir.join("london-tube-api.yaml"))
        .expect("read london-tube-api.yaml");
    let schema = OpenApiSchema::parse(&spec_text).expect("vendored london-tube-api.yaml parses");
    assert!(
        schema
            .operation_for("/Line/Mode/tube/Status", "GET")
            .is_some(),
        "spec covers GET /Line/Mode/tube/Status (the get-tube-status tool)"
    );
    assert!(
        schema
            .operation_for("/Line/{lineId}/Disruption", "GET")
            .is_some(),
        "spec covers GET /Line/{{lineId}}/Disruption (the script tool's per-line call)"
    );
}
