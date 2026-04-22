//! Integration tests for Dynamic Client Registration (RFC 7591) in `OAuthHelper`.
//!
//! Uses mockito to simulate a real OAuth discovery server + DCR endpoint
//! without needing network access. Covers:
//! - RFC 7591 §3.1 `response_types: ["code"]` must appear in the wire body
//! - Scheme guard: `http://`-non-localhost `registration_endpoint` rejected
//! - DCR response body capped at 1 MiB

#![cfg(feature = "oauth")]

use mockito::{Matcher, Server};
use pmcp::client::oauth::{OAuthConfig, OAuthHelper};
use serde_json::json;

fn discovery_body(base: &str, with_reg: bool) -> String {
    let mut v = json!({
        "issuer": base,
        "authorization_endpoint": format!("{}/authorize", base),
        "token_endpoint": format!("{}/token", base),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "grant_types_supported": ["authorization_code"],
        "scopes_supported": ["openid"],
        "token_endpoint_auth_methods_supported": ["none"],
        "code_challenge_methods_supported": ["S256"],
    });
    if with_reg {
        v["registration_endpoint"] = json!(format!("{}/register", base));
    }
    v.to_string()
}

#[tokio::test]
async fn dcr_fires_when_eligible() {
    let mut server = Server::new_async().await;
    let base = server.url();

    let _m_disc = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_body(&base, /*with_reg*/ true))
        .create_async()
        .await;

    // Mock only matches when RFC 7591 §3.1 `response_types` is in the body;
    // a regression that drops the field will produce a 501 and fail the test.
    let _m_dcr = server
        .mock("POST", "/register")
        .match_header("content-type", Matcher::Regex("application/json.*".into()))
        .match_body(Matcher::PartialJsonString(
            json!({ "response_types": ["code"] }).to_string(),
        ))
        .with_status(201)
        .with_body(json!({"client_id": "dcr-issued-id"}).to_string())
        .create_async()
        .await;

    let cfg = OAuthConfig {
        mcp_server_url: Some(base.clone()),
        dcr_enabled: true,
        client_id: None,
        client_name: Some("integration-test".into()),
        ..OAuthConfig::default()
    };
    let helper = OAuthHelper::new(cfg).unwrap();

    let resolved = helper
        .test_resolve_client_id_from_discovery()
        .await
        .unwrap();
    assert_eq!(resolved, "dcr-issued-id");
}

#[tokio::test]
async fn dcr_body_matches_rfc7591() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let _d = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_body(&base, true))
        .create_async()
        .await;

    let _r = server
        .mock("POST", "/register")
        .match_body(Matcher::PartialJsonString(
            json!({
                "grant_types": ["authorization_code"],
                "token_endpoint_auth_method": "none",
                "response_types": ["code"],
            })
            .to_string(),
        ))
        .with_status(200)
        .with_body(json!({"client_id": "x"}).to_string())
        .create_async()
        .await;

    let cfg = OAuthConfig {
        mcp_server_url: Some(base.clone()),
        dcr_enabled: true,
        client_name: Some("assert-body".into()),
        ..OAuthConfig::default()
    };
    OAuthHelper::new(cfg)
        .unwrap()
        .test_resolve_client_id_from_discovery()
        .await
        .unwrap();
}

#[tokio::test]
async fn dcr_rejects_http_non_localhost_registration_endpoint_against_live_mock() {
    // Mock server's discovery advertises a non-localhost http registration
    // endpoint and expects ZERO calls to /register — confirms the SDK rejects
    // the URL before issuing any HTTP request.
    let mut server = Server::new_async().await;
    let base = server.url();

    // Discovery advertises a hostile non-localhost http registration endpoint.
    let discovery = json!({
        "issuer": base,
        "authorization_endpoint": format!("{}/authorize", base),
        "token_endpoint": format!("{}/token", base),
        "registration_endpoint": "http://evil.invalid/register",
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "grant_types_supported": ["authorization_code"],
        "scopes_supported": ["openid"],
        "token_endpoint_auth_methods_supported": ["none"],
        "code_challenge_methods_supported": ["S256"],
    });
    let _disc = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery.to_string())
        .create_async()
        .await;

    // Guard: expect ZERO calls to any /register path on our mock server
    // (the SDK must not even attempt the POST).
    let reg_guard = server
        .mock("POST", "/register")
        .expect(0)
        .create_async()
        .await;

    let cfg = OAuthConfig {
        mcp_server_url: Some(base.clone()),
        dcr_enabled: true,
        client_id: None,
        client_name: Some("regression-t74a".into()),
        ..OAuthConfig::default()
    };
    let err = OAuthHelper::new(cfg)
        .unwrap()
        .test_resolve_client_id_from_discovery()
        .await
        .unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("must be https"),
        "expected scheme-guard error, got: {msg}"
    );
    reg_guard.assert_async().await;
}

#[tokio::test]
async fn dcr_not_fired_when_client_id_present() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let _d = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_body(&base, true))
        .create_async()
        .await;
    let reg_mock = server
        .mock("POST", "/register")
        .with_body(json!({"client_id": "SHOULD-NOT-BE-USED"}).to_string())
        .expect(0) // asserts zero calls
        .create_async()
        .await;

    let cfg = OAuthConfig {
        mcp_server_url: Some(base.clone()),
        dcr_enabled: true,
        client_id: Some("preset".into()),
        ..OAuthConfig::default()
    };
    let resolved = OAuthHelper::new(cfg)
        .unwrap()
        .test_resolve_client_id_from_discovery()
        .await
        .unwrap();
    assert_eq!(resolved, "preset");
    reg_mock.assert_async().await;
}

#[tokio::test]
async fn dcr_rejects_response_larger_than_1mib() {
    // Defense-in-depth: the SDK caps DCR response bodies at 1 MiB to mitigate
    // DoS from a hostile registration_endpoint.
    let mut server = Server::new_async().await;
    let base = server.url();

    let _d = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_body(&base, /*with_reg*/ true))
        .create_async()
        .await;

    // Build a valid-JSON but >1 MiB response body.
    let mut huge = String::with_capacity(1_200_000);
    huge.push_str(r#"{"client_id":"x","extra_padding":""#);
    huge.push_str(&"A".repeat(1_200_000));
    huge.push_str(r#""}"#);

    let _r = server
        .mock("POST", "/register")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(huge)
        .create_async()
        .await;

    let cfg = OAuthConfig {
        mcp_server_url: Some(base.clone()),
        dcr_enabled: true,
        client_id: None,
        client_name: Some("oversize-body-test".into()),
        ..OAuthConfig::default()
    };
    let err = OAuthHelper::new(cfg)
        .unwrap()
        .test_resolve_client_id_from_discovery()
        .await
        .unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("exceeds") && msg.contains("byte cap"),
        "expected 1 MiB cap rejection, got: {msg}"
    );
}
