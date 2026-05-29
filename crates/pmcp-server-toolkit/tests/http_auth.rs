//! Integration coverage for the outbound HTTP auth providers (OAPI-03 / D-05 / H1).
//!
//! Named `http_auth` so the plan's `cargo test ... http_auth` verify filter
//! resolves to a dedicated test binary. The exhaustive per-variant unit tests
//! live in `src/http/auth.rs`; these assert the public crate-root-reachable
//! surface (`http::auth::*`) behaves end-to-end through the published API.

#![cfg(feature = "http")]

use pmcp_server_toolkit::http::auth::{create_auth_provider, create_passthrough_auth_provider};
use pmcp_server_toolkit::http::AuthConfig;
use reqwest::header::{HeaderMap, AUTHORIZATION};
use std::collections::HashMap;

#[tokio::test]
async fn http_auth_bearer_static_applies_only_its_credential() {
    let cfg = AuthConfig::Bearer {
        token: "configured".to_string(),
        required: true,
    };
    let auth = create_auth_provider(&cfg).unwrap();
    let mut headers = HeaderMap::new();
    let mut query = HashMap::new();
    // A static provider ignores the inbound token (T-90-01-06).
    auth.apply(&mut headers, &mut query, Some("inbound"))
        .await
        .unwrap();
    let rendered = headers.get(AUTHORIZATION).unwrap().to_str().unwrap();
    assert_eq!(rendered, "Bearer configured");
    assert!(!rendered.contains("inbound"));
}

#[tokio::test]
async fn http_auth_passthrough_forwards_inbound_token() {
    let cfg = AuthConfig::OAuthPassthrough {
        target_header: "Authorization".to_string(),
        required: true,
    };
    let auth = create_passthrough_auth_provider(&cfg, None).unwrap();
    let mut headers = HeaderMap::new();
    let mut query = HashMap::new();
    auth.apply(&mut headers, &mut query, Some("client-tok"))
        .await
        .unwrap();
    assert_eq!(headers.get(AUTHORIZATION).unwrap(), "Bearer client-tok");
}

#[tokio::test]
async fn http_auth_api_key_query_param_round_trips() {
    let cfg = AuthConfig::ApiKey {
        query_params: [("app_key".to_string(), "secret".to_string())]
            .into_iter()
            .collect(),
        headers: HashMap::new(),
        required: true,
    };
    let auth = create_auth_provider(&cfg).unwrap();
    let mut headers = HeaderMap::new();
    let mut query = HashMap::new();
    auth.apply(&mut headers, &mut query, None).await.unwrap();
    assert_eq!(query.get("app_key"), Some(&"secret".to_string()));
    assert!(headers.is_empty());
}
