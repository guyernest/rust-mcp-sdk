//! Integration coverage for the outbound HTTP auth providers (OAPI-03 / D-05 / H1).
//!
//! Named `http_auth` so the plan's `cargo test ... http_auth` verify filter
//! resolves to a dedicated test binary. The exhaustive per-variant unit tests
//! live in `src/http/auth.rs`; these assert the public crate-root-reachable
//! surface (`http::auth::*`) behaves end-to-end through the published API.

#![cfg(feature = "http")]

use pmcp_server_toolkit::http::auth::{create_auth_provider, create_passthrough_auth_provider};
use pmcp_server_toolkit::http::AuthConfig;
use proptest::prelude::*;
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

/// Render the credential a static provider emits as a single comparable string:
/// the `Authorization` header value (Bearer/Basic) plus all query-param values
/// (ApiKey), concatenated. Used by the leak property to assert both presence of
/// the resolved secret and absence of the `"${"` placeholder fragment.
async fn rendered_credential(
    auth: &dyn pmcp_server_toolkit::http::auth::HttpAuthProvider,
) -> String {
    let mut headers = HeaderMap::new();
    let mut query: HashMap<String, String> = HashMap::new();
    auth.apply(&mut headers, &mut query, None).await.unwrap();
    let mut parts: Vec<String> = Vec::new();
    if let Some(v) = headers.get(AUTHORIZATION) {
        parts.push(v.to_str().unwrap_or_default().to_string());
    }
    let mut qvals: Vec<String> = query.values().cloned().collect();
    qvals.sort();
    parts.extend(qvals);
    parts.join("|")
}

proptest! {
    // Default proptest case count. Each case sets ONE process env var under the
    // serial `--test-threads=1` guard (the file's tokio tests already require it).
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// No credential variant ever leaks the literal `${...}` placeholder: for a
    /// random env var holding a random non-empty secret, building Bearer / Basic /
    /// OAuth2 / ApiKey with the credential set to `"${<name>}"` yields a provider
    /// whose emitted credential CONTAINS the resolved secret and NEVER contains
    /// the substring `"${"`.
    #[test]
    fn http_auth_no_variant_leaks_secret_placeholder(
        // Exclude `$`, `{`, `}`, control/whitespace chars (so the secret can never
        // reintroduce a `${` fragment) AND any char that `application/x-www-form-
        // urlencoded` would percent-encode (so the OAuth2 token-body matcher sees
        // the literal secret). Alphanumerics plus `_ - .` are form-safe and never
        // contain `${`. Non-empty by construction.
        secret in "[A-Za-z0-9_.-]{1,32}",
    ) {
        let var = "PMCP_TEST_PROP_LEAK_SECRET_VAR";
        std::env::set_var(var, &secret);
        let reference = format!("${{{var}}}");

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        // --- Bearer: Authorization: Bearer <secret> ---
        let bearer = create_auth_provider(&AuthConfig::Bearer {
            token: reference.clone(),
            required: true,
        }).unwrap();
        let rendered = rt.block_on(rendered_credential(bearer.as_ref()));
        prop_assert!(rendered.contains(&secret), "bearer dropped the secret: {rendered}");
        prop_assert!(!rendered.contains("${"), "bearer leaked the placeholder: {rendered}");

        // --- Basic: base64(user:<secret>) ---
        let basic = create_auth_provider(&AuthConfig::Basic {
            username: "u".to_string(),
            password: reference.clone(),
            required: true,
        }).unwrap();
        let rendered = rt.block_on(rendered_credential(basic.as_ref()));
        {
            use base64::Engine;
            let expected = base64::engine::general_purpose::STANDARD
                .encode(format!("u:{secret}"));
            prop_assert!(rendered.contains(&expected), "basic dropped the secret: {rendered}");
        }
        prop_assert!(!rendered.contains("${"), "basic leaked the placeholder: {rendered}");

        // --- ApiKey: query param value == <secret> ---
        let apikey = create_auth_provider(&AuthConfig::ApiKey {
            query_params: [("app_key".to_string(), reference.clone())]
                .into_iter()
                .collect(),
            headers: HashMap::new(),
            required: true,
        }).unwrap();
        let rendered = rt.block_on(rendered_credential(apikey.as_ref()));
        prop_assert!(rendered.contains(&secret), "api_key dropped the secret: {rendered}");
        prop_assert!(!rendered.contains("${"), "api_key leaked the placeholder: {rendered}");

        // --- OAuth2 client_secret: asserted via the token-endpoint form body ---
        // The wiremock matcher only issues a token when `client_secret=<secret>`
        // reaches the wire; the unwrap therefore proves the resolved secret (NOT
        // the literal `${...}`) was sent.
        rt.block_on(async {
            use wiremock::matchers::{body_string_contains, method, path};
            use wiremock::{Mock, MockServer, ResponseTemplate};
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/token"))
                .and(body_string_contains(format!("client_secret={secret}")))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({ "access_token": "issued" }),
                ))
                .mount(&server)
                .await;
            let oauth = create_auth_provider(&AuthConfig::OAuth2ClientCredentials {
                token_url: format!("{}/token", server.uri()),
                client_id: "cid".to_string(),
                client_secret: reference.clone(),
                scopes: vec![],
                required: true,
            }).unwrap();
            let mut headers = HeaderMap::new();
            let mut query: HashMap<String, String> = HashMap::new();
            // Errors (404) if the resolved secret did not reach the body.
            oauth.apply(&mut headers, &mut query, None).await.unwrap();
            let rendered = headers.get(AUTHORIZATION).unwrap().to_str().unwrap().to_string();
            assert!(!rendered.contains("${"), "oauth2 leaked the placeholder: {rendered}");
        });

        std::env::remove_var(var);
    }
}
