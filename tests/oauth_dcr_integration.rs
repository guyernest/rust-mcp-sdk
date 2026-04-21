//! Integration tests for Dynamic Client Registration (RFC 7591) in `OAuthHelper`.
//!
//! Uses mockito (pmcp dev-dep since 1.5.0) to simulate a real OAuth discovery
//! server + DCR endpoint without needing network access.
//!
//! Covers G9 from 74-VALIDATION.md plus regression guards for:
//! - HIGH-1: RFC 7591 §3.1 `response_types: ["code"]` must appear in the wire body
//! - T-74-A (scheme guard): `http://`-non-localhost `registration_endpoint` rejected
//! - LOW-11: DCR response body capped at 1 MiB

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

    // Review HIGH-1 — assert the POST body contains response_types:["code"].
    // Using match_body with a PartialJsonString ensures the mock only matches
    // when RFC 7591 §3.1 response_types is present. If the SDK ever regresses
    // and drops the field, mockito will return 501 and the test fails.
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

    // Assert POST body contains D-05 shape.
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
    // Warning #15 fix — strengthens T-74-A with a mockito-backed negative test.
    // Mock server exposes discovery advertising a non-localhost http registration_endpoint,
    // expects ZERO calls to /register, confirms the SDK rejects the non-https non-localhost URL.
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
        "expected T-74-A scheme-guard error, got: {msg}"
    );
    reg_guard.assert_async().await; // confirms zero hits
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
    // Review LOW-11 (Gemini) — defense-in-depth. The SDK caps DCR response
    // bodies at 1 MiB to mitigate DoS from a hostile registration_endpoint.
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
