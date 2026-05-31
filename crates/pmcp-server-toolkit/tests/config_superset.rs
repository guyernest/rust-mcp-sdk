//! REF-01 superset regression gate (Plan 85-01 Task 1).
//!
//! Extends `reference_configs.rs` to cover the **fourth** reference config — the
//! SQLite Chinook reference (`tests/fixtures/reference-config.toml`) — which the
//! toolkit could not parse before Plan 85-01: it uses `[database] file_path`,
//! `[server] is_reference`, and a `[shared_policy_store]` section, all of which
//! were unknown fields under `#[serde(deny_unknown_fields)]`.
//!
//! The fix (per RESEARCH §Pitfall 1 + PATTERNS §8 + config.rs:24) is **always to
//! ADD the missing field, never to loosen `deny_unknown_fields`**. This test
//! locks that invariant in three directions:
//!
//! 1. All four reference configs parse + validate (REF-01 SC-2).
//! 2. A renamed/typo'd field is still REJECTED (deny_unknown_fields stays strict).
//! 3. (REVIEW FIX #6) Athena `${VAR}` in `output_location` parses **verbatim** —
//!    proving SC-2 parse-only is unaffected by the token-secret-only `${VAR}`
//!    expansion added in Task 2 (no config-load-time expansion attempted).

use pmcp_server_toolkit::config::ServerConfig;

#[test]
fn chinook_reference_config_parses_and_validates() {
    // Before Plan 85-01 this FAILED on `unknown field 'file_path'`.
    let toml = include_str!("fixtures/reference-config.toml");
    let cfg = ServerConfig::from_toml_strict_validated(toml)
        .expect("Chinook reference config must parse + validate — REF-01 superset (Plan 85-01)");

    assert_eq!(
        cfg.database.backend_type.as_deref(),
        Some("sqlite"),
        "Chinook reference uses the sqlite backend"
    );
    assert_eq!(
        cfg.database.file_path.as_deref(),
        Some("/var/task/assets/chinook.db"),
        "the additive [database] file_path field must round-trip"
    );
    assert!(
        cfg.server.is_reference,
        "[server] is_reference = true must round-trip"
    );

    let store = cfg
        .shared_policy_store
        .as_ref()
        .expect("[shared_policy_store] section must round-trip");
    assert!(store.creates_shared_store, "creates_shared_store = true");
    assert!(store.export_to_ssm, "export_to_ssm = true");
    assert_eq!(
        store.ssm_path.as_deref(),
        Some("/pmcp/policy-stores/sql-api")
    );
    assert!(
        store.templates.contains(&"PermitAllSelects".to_string()),
        "the templates array must round-trip"
    );
}

#[test]
fn open_images_config_still_parses_and_validates() {
    // Additive fields must not break the 3 pre-existing fixtures.
    let toml = include_str!("fixtures/open-images-config.toml");
    let cfg = ServerConfig::from_toml_strict_validated(toml)
        .expect("open-images config must still parse + validate after additive fields");
    assert_eq!(cfg.database.backend_type.as_deref(), Some("athena"));
}

#[test]
fn imdb_config_still_parses_and_validates() {
    let toml = include_str!("fixtures/imdb-config.toml");
    ServerConfig::from_toml_strict_validated(toml)
        .expect("imdb config must still parse + validate after additive fields");
}

#[test]
fn msr_vtt_config_still_parses_and_validates() {
    let toml = include_str!("fixtures/msr-vtt-config.toml");
    ServerConfig::from_toml_strict_validated(toml)
        .expect("msr-vtt config must still parse + validate after additive fields");
}

#[test]
fn var_in_output_location_parses_verbatim() {
    // REVIEW FIX #6: the Athena open-images config carries
    // `output_location = "s3://...-${AWS_ACCOUNT_ID}-${AWS_REGION}/..."`. The
    // toolkit must parse this VERBATIM — no expansion at config-load time. This
    // proves SC-2 parse-only is unaffected by the token-secret-only ${VAR}
    // expansion (Task 2); non-secret fields keep ${...} as a literal string.
    let toml = include_str!("fixtures/open-images-config.toml");
    let cfg = ServerConfig::from_toml_strict_validated(toml).expect("open-images parses");
    let out = cfg
        .database
        .output_location
        .as_deref()
        .expect("open-images declares output_location");
    assert!(
        out.contains("${AWS_ACCOUNT_ID}"),
        "output_location must retain the verbatim ${{AWS_ACCOUNT_ID}} substring (no expansion at parse): {out}"
    );
    assert!(
        out.contains("${AWS_REGION}"),
        "output_location must retain the verbatim ${{AWS_REGION}} substring (no expansion at parse): {out}"
    );
}

#[test]
fn renames_rejected() {
    // deny_unknown_fields stays strict: a typo'd field name (filepath, no
    // underscore between file and path's owning intent) is REJECTED — the fix
    // for a missing field is to ADD it, never to loosen the parser.
    let toml = r#"
        [server]
        name = "demo"
        version = "0.1.0"

        [database]
        type = "sqlite"
        filepath = "/var/task/assets/chinook.db"
    "#;
    let err = ServerConfig::from_toml(toml)
        .expect_err("a renamed/typo'd field (filepath) must be rejected by deny_unknown_fields");
    assert!(
        matches!(err, pmcp_server_toolkit::ToolkitError::Parse(_)),
        "got: {err:?}"
    );
}
