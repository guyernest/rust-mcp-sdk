//! P902-PARITY — the Contoso M365 **oauth_passthrough** reference-parity assertion.
//!
//! The 90.2 analog of `parity_replay.rs` (london-tube), re-skinned to Contoso /
//! Microsoft Graph / per-user delegated `oauth_passthrough`. It proves the Shape-A
//! binary serves the two Contoso tools correctly AND — the headline difference vs
//! london-tube — that the inbound user `Authorization: Bearer` is FORWARDED to the
//! Graph backend, replayed OFFLINE via `wiremock` (pure-Rust, no Docker, no live
//! network in default CI).
//!
//! The single most valuable difference from `parity_replay.rs`: `ServerTester::new`
//! is constructed with `Some("contoso-user-tok")` as its 4th arg (the inbound bearer
//! hook, mcp-tester `tester.rs:96-101`), so the wiremock Graph mock can REQUIRE the
//! forwarded `Authorization: Bearer contoso-user-tok` header AND the test can ASSERT
//! it on every recorded backend request — the passthrough proof (fails if absent).
//!
//! The mock range addresses and response rows are LOADED from the canonical
//! `tests/fixtures/contoso-m365-workbook.json` (Plan 01), not re-typed, so the parity
//! mock provably cannot drift from the config / code-mode / docs copies.
//!
//! Tests here (all `contoso_m365_*`-prefixed so the positional verify filter resolves
//! on the file stem — Plan 01 verify-filter lesson, `oauth_passthrough_e2e.rs:18-20`):
//!
//! 1. [`contoso_m365_fixture`] — fixture-validity: the vendored `contoso-m365.toml`
//!    parses via [`ServerConfig::from_toml_strict_validated`] with both tool names +
//!    `oauth_passthrough` auth + both tools `is_script_tool()`; `contoso-m365-api.yaml`
//!    parses via [`OpenApiSchema::parse`] and `operation_for` resolves the range-read op.
//! 2. [`contoso_m365_pointable_example_parses`] — the shipped `examples/contoso-m365.toml`
//!    parses + validates with the SAME Code Mode surface (drift guard).
//! 3. `contoso_m365_parity_through_real_binary_path` — drives the REAL binary
//!    ([`run_serving`]) against a `wiremock` Graph backend that REQUIRES the forwarded
//!    `Authorization: Bearer contoso-user-tok` header, replays
//!    `contoso-m365-scenarios.yaml` through `mcp-tester`, gates per-step, THEN asserts
//!    the forwarded bearer on every recorded backend request (passthrough proof).
//! 4. `contoso_m365_live` (`#[ignore]`) — best-effort live scaffold, NOT the gate.
//!
//! Run the offline suite (single-threaded — ephemeral port + per-process state):
//! ```sh
//! cargo test -p pmcp-openapi-server --test contoso_m365_parity -- --test-threads=1
//! ```

use std::time::Duration;

use mcp_tester::{ScenarioExecutor, ServerTester, TestScenario};
use pmcp_openapi_server::{run_serving, Args};
use pmcp_server_toolkit::http::OpenApiSchema;
use pmcp_server_toolkit::ServerConfig;
use serde_json::{json, Value};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The inbound user bearer the parity test forwards. `ServerTester::new`'s 4th arg
/// is `Some(USER_TOKEN)`, so the harness injects `Authorization: Bearer <USER_TOKEN>`
/// on every request (mcp-tester `tester.rs:96-101`); the wiremock Graph mock REQUIRES
/// it and the recorded-request assertion re-checks it — the passthrough proof. (The
/// harness also injects an `X-API-Key` header, but that is incidental: the delegated
/// credential is the `Authorization` bearer ONLY — we assert on that header.)
const USER_TOKEN: &str = "contoso-user-tok";

/// Absolute path to the vendored fixtures directory.
fn fixtures_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Absolute path to the published `examples/` directory (ships with the crate;
/// `tests/` is excluded from the tarball but `examples/` is NOT — Cargo.toml:14).
fn examples_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples")
}

// ============================================================================
// Canonical dataset loader — Plan 01's contoso-m365-workbook.json is THE single
// source of truth. The wiremock mock addresses + response rows are DERIVED from
// it (no hand-typed rows), so the parity mock cannot drift from the config /
// code-mode / docs copies.
// ============================================================================

/// The canonical Contoso workbook dataset, baked into the test binary at compile
/// time so the mock rows/addresses are LOADED (never re-typed). Single source of
/// truth: `tests/fixtures/contoso-m365-workbook.json` (Plan 01).
const CANONICAL_WORKBOOK_JSON: &str = include_str!("fixtures/contoso-m365-workbook.json");

/// Parse the canonical workbook json once.
fn canonical_workbook() -> Value {
    serde_json::from_str(CANONICAL_WORKBOOK_JSON)
        .expect("canonical contoso-m365-workbook.json parses")
}

/// The canonical-json `customers[]` entry for a `customer_id`. Both the range
/// address and the row are derived from this single node — so a given id is
/// looked up ONCE per use, and the lookup predicate lives in one place.
fn find_customer<'a>(workbook: &'a Value, customer_id: &str) -> &'a Value {
    workbook["customers"]
        .as_array()
        .expect("customers array")
        .iter()
        .find(|c| c["customer_id"] == json!(customer_id))
        .unwrap_or_else(|| panic!("customer {customer_id} present in canonical json"))
}

/// The Customers-sheet range address for a `customer_id`, read from the canonical
/// json's `customers[].address` (e.g. `C001 -> A2:D2`). The mock path is built from
/// this — NOT a hand-typed literal.
fn customer_address(workbook: &Value, customer_id: &str) -> String {
    find_customer(workbook, customer_id)["address"]
        .as_str()
        .expect("customer address is a string")
        .to_string()
}

/// All Customers data rows (header excluded) as `[customer_id, name, segment, region]`
/// rows — the `values` the Graph mock returns for the whole-sheet `A2:D7` block the
/// `get_customer` script reads before filtering by the customer_id column (column 0).
fn all_customer_rows(workbook: &Value) -> Vec<Value> {
    workbook["customers"]
        .as_array()
        .expect("customers array")
        .iter()
        .map(|c| {
            json!([
                c["customer_id"].clone(),
                c["name"].clone(),
                c["segment"].clone(),
                c["region"].clone(),
            ])
        })
        .collect()
}

/// The Orders-block range address for a `customer_id`, read from the canonical
/// json's `orders_blocks` (e.g. `C001 -> A2:D3`). The mock path is built from this.
fn orders_block_address(workbook: &Value, customer_id: &str) -> String {
    workbook["orders_blocks"][customer_id]
        .as_str()
        .unwrap_or_else(|| panic!("orders_blocks has a block for {customer_id}"))
        .to_string()
}

/// All Orders data rows as `[order_id, customer_id, order_date, amount]` rows — the
/// `values` the Graph mock returns for the whole-sheet `A2:D7` block the
/// `get_customer_orders` script reads before filtering by the customer_id column (1).
fn all_order_rows(workbook: &Value) -> Vec<Value> {
    workbook["orders"]
        .as_array()
        .expect("orders array")
        .iter()
        .map(|o| {
            json!([
                o["order_id"].clone(),
                o["customer_id"].clone(),
                o["order_date"].clone(),
                o["amount"].clone(),
            ])
        })
        .collect()
}

// ============================================================================
// Shared Code Mode showcase-surface assertion (drift guard between the vendored
// fixture and the shipped pointable example — the EXACT resource URIs + prompt
// name recorded in 90.2-01-SUMMARY).
// ============================================================================

/// The Code Mode showcase surface BOTH the vendored fixture and the pointable
/// example must ship (P902-FIXTURE / P902-EXAMPLE): the three context resource URIs
/// plus the `start_code_mode` prompt. Asserting it in one place keeps the two
/// configs from drifting on the showcase surface. `label` names the config under
/// test so a failure points at the right file. URIs/name are VERBATIM from
/// 90.2-01-SUMMARY.
fn assert_contoso_code_mode_surface(cfg: &ServerConfig, label: &str) {
    let resource_uris: Vec<&str> = cfg.resources.iter().map(|r| r.uri.as_str()).collect();
    for uri in [
        "docs://contoso-m365/schema",
        "docs://contoso-m365/examples",
        "code-mode://learnings",
    ] {
        assert!(
            resource_uris.contains(&uri),
            "{label} ships the {uri} resource: {resource_uris:?}"
        );
    }
    assert!(
        cfg.prompts.iter().any(|p| p.name == "start_code_mode"),
        "{label} ships the start_code_mode prompt: {:?}",
        cfg.prompts.iter().map(|p| &p.name).collect::<Vec<_>>()
    );
}

/// Fixture-validity (Task 1): the vendored config + spec parse, both Contoso tools
/// are present SCRIPT tools, the backend auth is `oauth_passthrough`, and the curated
/// Graph spec resolves the range-read op. The offline gate that the vendored fixtures
/// are well-formed BEFORE the parity replay drives them.
#[test]
fn contoso_m365_fixture() {
    let dir = fixtures_dir();

    // (1) The config parses through the production strict+validated entry point.
    let config_text =
        std::fs::read_to_string(dir.join("contoso-m365.toml")).expect("read contoso-m365.toml");
    let cfg = ServerConfig::from_toml_strict_validated(&config_text)
        .expect("vendored contoso-m365.toml parses + validates");

    // Both Contoso tool names are present.
    let tool_names: Vec<&str> = cfg.tools.iter().map(|t| t.name.as_str()).collect();
    assert!(
        tool_names.contains(&"get_customer"),
        "script tool get_customer present: {tool_names:?}"
    );
    assert!(
        tool_names.contains(&"get_customer_orders"),
        "script tool get_customer_orders present: {tool_names:?}"
    );

    // The Code Mode showcase surface (three resources + start_code_mode prompt).
    assert_contoso_code_mode_surface(&cfg, "vendored fixture");

    // The backend auth is oauth_passthrough (the per-user delegated credential
    // path — ADAPTED from london-tube's api_key match). `target_header` defaults
    // "Authorization"; `required` is true.
    let backend = cfg.backend.as_ref().expect("[backend] section present");
    assert!(
        matches!(
            &backend.auth,
            pmcp_server_toolkit::http::auth::AuthConfig::OAuthPassthrough { target_header, required }
                if target_header.eq_ignore_ascii_case("authorization") && *required
        ),
        "[backend.auth] is oauth_passthrough forwarding the Authorization header (required): {:?}",
        backend.auth
    );

    // BOTH tools are SCRIPT tools (D-01 detection rule — they compute range
    // addresses in JS before the api.get).
    for name in ["get_customer", "get_customer_orders"] {
        let tool = cfg
            .tools
            .iter()
            .find(|t| t.name == name)
            .unwrap_or_else(|| panic!("{name} tool present"));
        assert!(tool.is_script_tool(), "{name} is a script tool");
    }

    // (2) The curated Graph spec parses and surfaces the range-read op the tools
    // call. Plan 01 shipped the PRIMARY OData parens path form; re-assert it parses
    // and resolves via the REAL OpenApiSchema::parse + operation_for.
    let spec_text = std::fs::read_to_string(dir.join("contoso-m365-api.yaml"))
        .expect("read contoso-m365-api.yaml");
    let schema = OpenApiSchema::parse(&spec_text).expect("vendored contoso-m365-api.yaml parses");
    const RANGE_OP_PATH: &str =
        "/drives/{drive-id}/items/{item-id}/workbook/worksheets/{worksheet-id}/range(address='{address}')";
    assert!(
        schema.operation_for(RANGE_OP_PATH, "GET").is_some(),
        "spec covers GET {RANGE_OP_PATH} (the getWorksheetRange range-read op backing both tools)"
    );

    // Sanity: the canonical workbook json loads and exposes the data the mocks use.
    let workbook = canonical_workbook();
    assert_eq!(
        customer_address(&workbook, "C001"),
        "A2:D2",
        "canonical json maps C001 -> A2:D2 (Customers row 2)"
    );
    assert_eq!(
        orders_block_address(&workbook, "C001"),
        "A2:D3",
        "canonical json maps C001 -> A2:D3 (contiguous Orders block)"
    );
}

/// P902-EXAMPLE smoke: the user-pointable `examples/contoso-m365.toml` (the config a
/// user runs via `pmcp-openapi-server --config <path>`) parses + validates through the
/// SAME strict entry point the binary uses, and carries the full Code Mode showcase
/// surface. Proves the published example config is well-formed without booting a server.
#[test]
fn contoso_m365_pointable_example_parses() {
    let config_text = std::fs::read_to_string(examples_dir().join("contoso-m365.toml"))
        .expect("read examples/contoso-m365.toml");
    let cfg = ServerConfig::from_toml_strict_validated(&config_text)
        .expect("pointable examples/contoso-m365.toml parses + validates");

    // Same Code Mode showcase surface as the vendored fixture (shared helper — drift guard).
    assert_contoso_code_mode_surface(&cfg, "pointable example");

    // The pointable example carries the SAME oauth_passthrough auth surface.
    let backend = cfg.backend.as_ref().expect("[backend] section present");
    assert!(
        matches!(
            &backend.auth,
            pmcp_server_toolkit::http::auth::AuthConfig::OAuthPassthrough { .. }
        ),
        "pointable example [backend.auth] is oauth_passthrough: {:?}",
        backend.auth
    );
}

/// Mount the Contoso Graph range-read mocks on the wiremock server, REQUIRING the
/// forwarded `Authorization: Bearer contoso-user-tok` header on every matcher (the
/// passthrough proof — the mock serves ONLY when the inbound user bearer was
/// forwarded). Each tool reads its whole sheet block (`A2:D7`) and filters by the
/// customer_id column in-script, so the mock returns the FULL Customers / Orders
/// blocks (rows + addresses LOADED from the canonical workbook json, NOT hand-typed)
/// and the tool does the filtering.
///
/// Two matchers, both from the canonical dataset:
/// - `get_customer(..)` -> Customers whole block `A2:D7` -> all customer rows.
/// - `get_customer_orders(..)` -> Orders whole block `A2:D7` -> all order rows.
async fn mount_contoso(server: &MockServer, workbook: &Value) {
    let bearer = format!("Bearer {USER_TOKEN}");

    // get_customer — Customers whole-block read (A2:D7); the script filters by column 0.
    let customers_addr = workbook["all_customers_address"]
        .as_str()
        .expect("all_customers_address is a string");
    let customers_path = format!(
        "/drives/CONTOSO_DRIVE/items/CUSTOMERS_ITEM/workbook/worksheets/Customers/range(address='{customers_addr}')"
    );
    Mock::given(method("GET"))
        .and(path(customers_path))
        .and(header("authorization", bearer.as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({ "address": customers_addr, "values": all_customer_rows(workbook) }),
        ))
        .mount(server)
        .await;

    // get_customer_orders — Orders whole-block read (A2:D7); the script filters by column 1.
    let orders_addr = workbook["all_orders_address"]
        .as_str()
        .expect("all_orders_address is a string");
    let orders_path = format!(
        "/drives/CONTOSO_DRIVE/items/ORDERS_ITEM/workbook/worksheets/Orders/range(address='{orders_addr}')"
    );
    Mock::given(method("GET"))
        .and(path(orders_path))
        .and(header("authorization", bearer.as_str()))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                json!({ "address": orders_addr, "values": all_order_rows(workbook) }),
            ),
        )
        .mount(server)
        .await;
}

/// Build a temp `contoso-m365.toml` from the vendored fixture with its
/// `[backend] base_url` overridden to point at the wiremock server. String
/// replacement keeps the rest of the fixture (auth, tools, code_mode) byte-identical.
fn temp_config_pointing_at(backend_url: &str) -> String {
    const REFERENCE_BASE_URL: &str = r#"base_url = "https://graph.microsoft.com/v1.0""#;
    let reference = std::fs::read_to_string(fixtures_dir().join("contoso-m365.toml"))
        .expect("read contoso-m365.toml");
    assert!(
        reference.contains(REFERENCE_BASE_URL),
        "contoso-m365.toml must contain the Graph base_url line to override"
    );
    reference.replace(REFERENCE_BASE_URL, &format!("base_url = \"{backend_url}\""))
}

/// P902-PARITY — the binding passthrough parity assertion, OFFLINE via wiremock.
///
/// Drives the REAL binary pipeline ([`run_serving`]) against a wiremock Graph backend
/// that REQUIRES the forwarded `Authorization: Bearer contoso-user-tok` header, replays
/// `contoso-m365-scenarios.yaml` through `mcp-tester` (constructed with the bearer as
/// the 4th `ServerTester::new` arg — the KEY DIFFERENCE vs london-tube's `None`), gates
/// per-step, THEN asserts the forwarded bearer on EVERY recorded backend request. This
/// FAILS if the bearer is absent/wrong — the two-sided passthrough proof.
#[tokio::test]
async fn contoso_m365_parity_through_real_binary_path() {
    let workbook = canonical_workbook();

    // (1) Stand up the wiremock Graph backend with bearer-gated matchers (rows + addresses
    // loaded from the canonical workbook json — no hand-typed rows).
    let backend = MockServer::start().await;
    mount_contoso(&backend, &workbook).await;

    // (2) Temp config: the vendored contoso-m365.toml with base_url -> wiremock. The
    // oauth_passthrough auth path holds NO standing credential; the only credential is
    // the inbound user bearer forwarded by the harness.
    let tmp = tempfile::tempdir().expect("create tempdir");
    let config_path = tmp.path().join("contoso-m365.toml");
    std::fs::write(&config_path, temp_config_pointing_at(&backend.uri()))
        .expect("write temp contoso-m365.toml");

    // (3) The REAL binary path: programmatic Args -> run_serving (NO --spec; curated
    // path, D-03). Ephemeral loopback port.
    let args = Args {
        config: config_path,
        spec: None,
        http: "127.0.0.1:0".to_string(),
    };
    let (bound, handle) = tokio::time::timeout(Duration::from_secs(10), run_serving(&args))
        .await
        .expect("run_serving must not hang")
        .expect("REAL binary path must assemble + serve the contoso-m365 config");

    // (4) Replay contoso-m365-scenarios.yaml via mcp-tester. The 4th arg
    // `Some("contoso-user-tok")` is the KEY DIFFERENCE vs london-tube's `None`: the
    // harness injects `Authorization: Bearer contoso-user-tok` so the passthrough
    // path forwards it to the Graph mock (which REQUIRES it).
    let url = format!("http://{bound}");
    let mut tester = ServerTester::new(
        &url,
        Duration::from_secs(30),
        false,            // insecure
        Some(USER_TOKEN), // api_key -> injected as `Authorization: Bearer contoso-user-tok`
        Some("http"),     // force_transport
        None,             // http_middleware_chain
    )
    .expect("construct ServerTester for the spawned HTTP server");

    let mut initialized = false;
    for attempt in 0..20u32 {
        if matches!(
            tester.test_initialize().await.status,
            mcp_tester::report::TestStatus::Passed
        ) {
            initialized = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50 * u64::from(attempt + 1))).await;
    }
    assert!(initialized, "MCP initialize must succeed (readiness)");

    let scenario = TestScenario::from_file(fixtures_dir().join("contoso-m365-scenarios.yaml"))
        .expect("load the contoso-m365 parity contract");
    let mut exec = ScenarioExecutor::new(&mut tester, true /* detailed */);
    let result = exec
        .execute(scenario)
        .await
        .expect("scenario execution must complete without a harness error");

    // (5) PER-STEP GATE: every parity step must pass its own assertions (tool list +
    // tool-output parity). Gate on each `step_results[i].success` (per-step truth).
    let failed: Vec<_> = result
        .step_results
        .iter()
        .filter(|s| !s.success)
        .map(|s| (&s.step_name, &s.error))
        .collect();
    assert!(
        failed.is_empty(),
        "every contoso-m365 parity step must pass — tool list + tool outputs must \
         match the reference scenarios. {}/{} completed; failed={failed:#?}",
        result.steps_completed,
        result.steps_total,
    );

    // (6) PASSTHROUGH PROOF (T-90.2-passthrough-02): the backend RECEIVED the forwarded
    // user bearer on EVERY request (the matchers already enforced this — every served
    // response REQUIRED `Authorization: Bearer contoso-user-tok`), and we re-assert it
    // per-recorded-request. This FAILS if the bearer is absent/wrong, proving the server
    // forwards the user's token and never substitutes a server-held credential. We assert
    // on the `Authorization` header ONLY — the harness-injected X-API-Key is incidental.
    let recorded = backend
        .received_requests()
        .await
        .expect("wiremock records requests");
    assert!(
        !recorded.is_empty(),
        "the parity replay must have hit the Graph backend at least once (proving the \
         tool calls reached wiremock — not a no-op)"
    );
    let expected_bearer = format!("Bearer {USER_TOKEN}");
    for req in &recorded {
        let auth = req
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok());
        assert_eq!(
            auth,
            Some(expected_bearer.as_str()),
            "every backend request must carry the FORWARDED inbound user bearer \
             `Authorization: Bearer contoso-user-tok` (oauth_passthrough proof — \
             the server forwards the user's token, never a server-held credential): {:?}",
            req.url.to_string()
        );
    }

    // Bounded shutdown — no leaked spawned server.
    handle.abort();
}

/// P902-PARITY (live) — best-effort scaffold against the REAL Microsoft Graph,
/// double-gated (`#[ignore]` + `PMCP_OPENAPI_LIVE_TEST=1` + a real `CONTOSO_GRAPH_TOKEN`).
///
/// NOT the gate. Skips cleanly in credential-less / offline CI (the env early-return).
/// Requires a real workbook (drive/item ids) which we do not ship, so this is a wiring
/// scaffold an operator can adapt — it only verifies the binary assembles + initializes
/// against a real Graph base URL with a real forwarded token, never asserting rows.
///
/// Run with:
/// ```sh
/// PMCP_OPENAPI_LIVE_TEST=1 CONTOSO_GRAPH_TOKEN=<real-graph-token> \
///   cargo test -p pmcp-openapi-server --test contoso_m365_parity contoso_m365_live \
///   -- --ignored --test-threads=1
/// ```
#[tokio::test]
#[ignore = "live network — requires PMCP_OPENAPI_LIVE_TEST=1 + a real CONTOSO_GRAPH_TOKEN"]
async fn contoso_m365_live() {
    // Double-gate: even when run with --ignored, bail unless explicitly enabled AND a
    // real token is present (never hit live Graph by accident).
    if std::env::var("PMCP_OPENAPI_LIVE_TEST").ok().as_deref() != Some("1") {
        eprintln!("contoso_m365_live skipped: set PMCP_OPENAPI_LIVE_TEST=1 to enable");
        return;
    }
    let Ok(token) = std::env::var("CONTOSO_GRAPH_TOKEN") else {
        eprintln!("contoso_m365_live skipped: set CONTOSO_GRAPH_TOKEN to a real Graph token");
        return;
    };
    if token.trim().is_empty() {
        eprintln!("contoso_m365_live skipped: CONTOSO_GRAPH_TOKEN is empty");
        return;
    }

    // Use the vendored fixture VERBATIM — base_url stays the real Graph endpoint.
    let tmp = tempfile::tempdir().expect("create tempdir");
    let config_path = tmp.path().join("contoso-m365.toml");
    std::fs::copy(fixtures_dir().join("contoso-m365.toml"), &config_path)
        .expect("copy contoso-m365.toml");

    let args = Args {
        config: config_path,
        spec: None,
        http: "127.0.0.1:0".to_string(),
    };
    let (bound, handle) = run_serving(&args)
        .await
        .expect("REAL binary path must serve against the live Graph backend");

    let url = format!("http://{bound}");
    let mut tester = ServerTester::new(
        &url,
        Duration::from_secs(60),
        false,
        Some(token.as_str()), // forward the operator's real Graph token
        Some("http"),
        None,
    )
    .expect("construct ServerTester for the spawned HTTP server");
    assert!(
        matches!(
            tester.test_initialize().await.status,
            mcp_tester::report::TestStatus::Passed
        ),
        "MCP initialize must succeed against the live-backed server"
    );

    handle.abort();
}
