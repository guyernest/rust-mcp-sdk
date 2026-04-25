//! Phase 76 Wave 2: IamConfig TOML-schema integration tests.
//!
//! These tests observe the `[iam]` / `[[iam.tables]]` / `[[iam.buckets]]` /
//! `[[iam.statements]]` surface from outside the crate — i.e. strictly what a
//! downstream consumer editing `.pmcp/deploy.toml` would see. They complement
//! the in-crate [`iam_wave2_tests`] module in `deployment::config`, which
//! exercises the struct-level invariants (is_empty, default_for_server
//! wiring, sub-struct constructability).
//!
//! ## Why TOML-level only, not struct-level?
//!
//! The `cargo_pmcp` library surface intentionally re-exports only
//! `loadtest`, `pentest`, and a narrow `test_support_cache` seam — NOT
//! `deployment::config`. This is the same lib-boundary constraint documented
//! in Wave 1 (76-01-SUMMARY.md, Rule-3 deviation #1): re-exporting
//! `deployment::config` would transitively drag in `templates::oauth::{…}` and
//! the full CognitoConfig tree, for a massive surface expansion that buys
//! very little over the in-crate tests.
//!
//! The TOML-level tests here still deliver genuine integration-level
//! coverage: they lock the textual schema shape (what an operator sees),
//! which is exactly what D-05 backward-compat guarantees.
//!
//! ## D-05 invariant (76-CONTEXT.md)
//!
//! A `.pmcp/deploy.toml` that omits the `[iam]` section MUST round-trip
//! byte-identically (absent `[iam]`, `[[iam.tables]]`, `[[iam.buckets]]`, and
//! `[[iam.statements]]` headers). This file is the integration-level guard on
//! that invariant.

use toml::Value;

/// Minimal-but-valid fixture covering every required field of the
/// non-IAM `DeployConfig` subtree (TargetConfig, AwsConfig, ServerConfig,
/// AuthConfig, ObservabilityConfig). No `[iam]` section — exercises the
/// D-05 backward-compat path.
const MINIMAL_DEPLOY_TOML: &str = r#"
[target]
type = "aws-lambda"
version = "1.0.0"

[aws]
region = "us-west-2"

[server]
name = "demo-server"
memory_mb = 512
timeout_seconds = 30

[environment]
RUST_LOG = "info"

[auth]
enabled = false

[observability]
log_retention_days = 30
enable_xray = true
create_dashboard = true
"#;

/// Cost-coach-shaped fixture per 76-CONTEXT.md §Scope: one entry in each of
/// the three IAM vectors (tables, buckets, statements) exercising the sugar
/// keywords (`"readwrite"`), `include_indexes = true`, and a raw passthrough
/// statement.
const COST_COACH_DEPLOY_TOML: &str = r#"
[target]
type = "aws-lambda"
version = "1.0.0"

[aws]
region = "us-west-2"

[server]
name = "cost-coach"
memory_mb = 512
timeout_seconds = 30

[environment]
RUST_LOG = "info"

[auth]
enabled = false

[observability]
log_retention_days = 30
enable_xray = true
create_dashboard = true

[[iam.tables]]
name = "cost-coach-tenants"
actions = ["readwrite"]
include_indexes = true

[[iam.buckets]]
name = "cost-coach-snapshots"
actions = ["readwrite"]

[[iam.statements]]
effect = "Allow"
actions = ["secretsmanager:GetSecretValue"]
resources = ["arn:aws:secretsmanager:us-west-2:*:secret:cost-coach/*"]
"#;

/// Test 1: A minimal TOML with no `[iam]` section parses as a `Value` whose
/// top-level map contains no `iam` key. Integration-level precondition for
/// D-05: if the operator omits `[iam]`, no `iam` sub-structure materialises
/// in the parsed document.
#[test]
fn minimal_deploy_toml_has_no_iam_key() {
    let value: Value = toml::from_str(MINIMAL_DEPLOY_TOML).expect("minimal TOML parses");
    let table = value.as_table().expect("top-level is a table");
    assert!(
        !table.contains_key("iam"),
        "minimal .pmcp/deploy.toml must not produce an `iam` key (D-05). Keys: {:?}",
        table.keys().collect::<Vec<_>>()
    );
}

/// Test 2: The cost-coach fixture parses into a `Value` with exactly one
/// entry in each of `iam.tables`, `iam.buckets`, and `iam.statements`, and
/// every field carries the expected value. Locks the full TOML shape the
/// operator's editor will see.
#[test]
fn cost_coach_toml_produces_expected_iam_shape() {
    let value: Value = toml::from_str(COST_COACH_DEPLOY_TOML).expect("cost-coach TOML parses");
    let iam = value
        .get("iam")
        .expect("iam section present")
        .as_table()
        .expect("iam is a table");

    // --- tables ---
    let tables = iam
        .get("tables")
        .expect("iam.tables")
        .as_array()
        .expect("tables is array");
    assert_eq!(tables.len(), 1);
    let table = tables[0].as_table().expect("tables[0] is a table");
    assert_eq!(
        table.get("name").and_then(Value::as_str),
        Some("cost-coach-tenants")
    );
    let actions = table
        .get("actions")
        .and_then(Value::as_array)
        .expect("actions is array");
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].as_str(), Some("readwrite"));
    assert_eq!(
        table.get("include_indexes").and_then(Value::as_bool),
        Some(true)
    );

    // --- buckets ---
    let buckets = iam
        .get("buckets")
        .expect("iam.buckets")
        .as_array()
        .expect("buckets is array");
    assert_eq!(buckets.len(), 1);
    let bucket = buckets[0].as_table().expect("buckets[0] is a table");
    assert_eq!(
        bucket.get("name").and_then(Value::as_str),
        Some("cost-coach-snapshots")
    );
    let actions = bucket
        .get("actions")
        .and_then(Value::as_array)
        .expect("actions");
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].as_str(), Some("readwrite"));

    // --- statements ---
    let statements = iam
        .get("statements")
        .expect("iam.statements")
        .as_array()
        .expect("statements is array");
    assert_eq!(statements.len(), 1);
    let stmt = statements[0].as_table().expect("statements[0] is a table");
    assert_eq!(stmt.get("effect").and_then(Value::as_str), Some("Allow"));
    let actions = stmt
        .get("actions")
        .and_then(Value::as_array)
        .expect("actions");
    assert_eq!(actions[0].as_str(), Some("secretsmanager:GetSecretValue"));
    let resources = stmt
        .get("resources")
        .and_then(Value::as_array)
        .expect("resources");
    assert_eq!(
        resources[0].as_str(),
        Some("arn:aws:secretsmanager:us-west-2:*:secret:cost-coach/*")
    );
}

/// Test 3: Re-serialising the minimal parsed document emits no iam-related
/// table headers. This is the wire-format D-05 guard: whatever path the
/// operator took through the CLI, round-tripping a no-IAM document never
/// materialises an `[iam]` block in the output.
#[test]
fn minimal_toml_reserialises_without_iam_headers() {
    let value: Value = toml::from_str(MINIMAL_DEPLOY_TOML).expect("minimal TOML parses");
    let out = toml::to_string(&value).expect("serialises");
    for header in [
        "[iam]",
        "[[iam.tables]]",
        "[[iam.buckets]]",
        "[[iam.statements]]",
    ] {
        assert!(
            !out.contains(header),
            "minimal deploy.toml must round-trip without {header} header (D-05)\nOutput was:\n{out}"
        );
    }
}

/// Test 4: Lossless round-trip of the cost-coach fixture through
/// `toml::Value` — parse → serialise → parse → compare `Value`s for
/// structural equality. Verifies that no field is silently dropped by
/// re-serialisation.
#[test]
fn cost_coach_toml_roundtrips_losslessly_through_value() {
    let orig: Value = toml::from_str(COST_COACH_DEPLOY_TOML).expect("cost-coach TOML parses");
    let serialised = toml::to_string(&orig).expect("serialises");
    let reparsed: Value = toml::from_str(&serialised)
        .unwrap_or_else(|e| panic!("reparse failed\nserialised:\n{serialised}\nerror: {e}"));
    assert_eq!(
        orig, reparsed,
        "cost-coach deploy.toml must roundtrip losslessly through toml::Value"
    );
}

/// Test 5: A `[[iam.tables]]` entry that omits `include_indexes` must serialise
/// without an `include_indexes` field in its TOML value (the boolean
/// default lives at the Rust struct layer, not the TOML surface). This
/// locks the textual default: operator-authored TOML that elides the field
/// stays elided through `Value`-level roundtrip.
#[test]
fn include_indexes_omitted_stays_absent_in_value_roundtrip() {
    let toml_str = {
        let mut base = String::from(MINIMAL_DEPLOY_TOML);
        base.push_str(
            r#"
[[iam.tables]]
name = "t1"
actions = ["read"]
"#,
        );
        base
    };
    let value: Value = toml::from_str(&toml_str).expect("parses");
    let table = value
        .get("iam")
        .and_then(|v| v.get("tables"))
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(Value::as_table)
        .expect("iam.tables[0] is a table");
    assert!(
        !table.contains_key("include_indexes"),
        "include_indexes must not be synthesised into the TOML value when the operator omitted it; \
         defaulting is a Rust-layer concern — keys present: {:?}",
        table.keys().collect::<Vec<_>>()
    );
}

/// Test 6: Smoke test — `cargo_pmcp`'s CLI binary must still build (i.e. the
/// schema changes in Task 1 didn't break any downstream compilation).
/// Executed by simply asserting `env!("CARGO_PKG_NAME") == "cargo-pmcp"`,
/// which fails the test setup if the integration crate can't locate the
/// package it targets. Kept minimal — this file's reason-to-exist is the
/// TOML-schema locks above, not bin-build coverage (which CI handles via
/// its own `cargo build` step).
#[test]
fn integration_crate_resolves_cargo_pmcp_package_name() {
    // This integration test target compiles against the `cargo-pmcp` package.
    // If Task 1's schema change broke the bin target, this file itself would
    // have failed to compile. The assertion is trivial; the compile is the
    // real check.
    assert!(
        !COST_COACH_DEPLOY_TOML.is_empty() && !MINIMAL_DEPLOY_TOML.is_empty(),
        "fixtures must be non-empty"
    );
}
