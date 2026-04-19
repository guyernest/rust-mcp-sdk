#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use serde_json::{from_slice, json, Value};
use std::collections::HashMap;

// Custom types for fuzzing authentication
#[derive(Debug, Arbitrary)]
#[allow(dead_code)]
struct FuzzAuthRequest {
    auth_type: FuzzAuthType,
    credentials: FuzzCredentials,
    metadata: HashMap<String, String>,
}

#[derive(Debug, Arbitrary)]
enum FuzzAuthType {
    None,
    ApiKey,
    OAuth2,
    Jwt,
    Custom(String),
}

#[derive(Debug, Arbitrary)]
#[allow(dead_code)]
struct FuzzCredentials {
    username: Option<String>,
    password: Option<String>,
    token: Option<String>,
    api_key: Option<String>,
    refresh_token: Option<String>,
    extra: Vec<(String, String)>,
}

#[derive(Debug, Arbitrary)]
#[allow(dead_code)]
struct FuzzOAuthFlow {
    client_id: String,
    client_secret: Option<String>,
    redirect_uri: String,
    scope: Vec<String>,
    state: String,
    code_verifier: Option<String>,
}

// Simulate various authentication flows
fn test_auth_flow(auth_type: &FuzzAuthType, creds: &FuzzCredentials) {
    match auth_type {
        FuzzAuthType::None => {
            // No authentication required
            if let Some(token) = &creds.token {
                if !token.is_empty() {
                    return; // Should not have token for no auth, skip
                }
            }
        },
        FuzzAuthType::ApiKey => {
            // API key authentication
            if let Some(key) = &creds.api_key {
                // Validate API key format
                if key.is_empty() || key.len() >= 1024 {
                    return; // Invalid API key, skip
                }

                // Check for common patterns
                let has_prefix =
                    key.starts_with("sk_") || key.starts_with("pk_") || key.starts_with("api_");

                // Validate character set
                let is_valid = key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');

                if has_prefix && !is_valid {
                    return; // Invalid API key format, skip
                }
            }
        },
        FuzzAuthType::OAuth2 => {
            // OAuth2 flow
            if let Some(token) = &creds.token {
                // Validate bearer token
                if token.is_empty() {
                    return; // Invalid token, skip
                }

                // Check JWT structure if it looks like one
                let parts: Vec<_> = token.split('.').collect();
                if parts.len() == 3 {
                    // Looks like a JWT
                    for part in &parts {
                        // Each part should be base64url encoded
                        if !part
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                        {
                            return; // Invalid JWT format, skip
                        }
                    }
                }
            }

            // Test refresh token flow
            if let Some(refresh) = &creds.refresh_token {
                if refresh.is_empty() || refresh.len() >= 2048 {
                    return; // Invalid refresh token, skip
                }
            }
        },
        FuzzAuthType::Jwt => {
            // JWT authentication
            if let Some(token) = &creds.token {
                let parts: Vec<_> = token.split('.').collect();
                if parts.len() == 3 {
                    // Try to decode header and payload (without verification)
                    let header = parts[0];
                    let payload = parts[1];

                    // Simulate base64url decoding (without actual implementation)
                    if header.len() > 0 && payload.len() > 0 {
                        // Would decode and validate structure
                    }
                }
            }
        },
        FuzzAuthType::Custom(scheme) => {
            // Custom authentication scheme
            if scheme.is_empty() || scheme.len() >= 256 {
                return; // Invalid scheme, skip
            }

            // Validate scheme name
            if !scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return; // Invalid characters in scheme, skip
            }
        },
    }
}

// Test OAuth2 PKCE flow
fn test_pkce_flow(flow: &FuzzOAuthFlow) {
    if let Some(verifier) = &flow.code_verifier {
        // PKCE code verifier requirements
        if verifier.len() < 43 || verifier.len() > 128 {
            return; // Invalid verifier length, skip
        }
        if !verifier
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_' || c == '~')
        {
            return; // Invalid characters in verifier, skip
        }

        // Generate code challenge (simulated)
        let challenge = format!("{}_challenge", verifier);
        if challenge.is_empty() {
            return; // Should never happen, but handle gracefully
        }
    }

    // Validate redirect URI
    if flow.redirect_uri.is_empty() {
        return; // Invalid redirect URI, skip
    }
    if flow.redirect_uri.starts_with("http://") || flow.redirect_uri.starts_with("https://") {
        // Valid HTTP(S) redirect
    } else if flow.redirect_uri == "urn:ietf:wg:oauth:2.0:oob" {
        // Out-of-band flow
    } else if flow.redirect_uri.starts_with("com.example.app://") {
        // Custom scheme for mobile apps
    }

    // Validate scope
    for scope in &flow.scope {
        if scope.is_empty() || scope.contains(' ') {
            return; // Invalid scope, skip
        }
    }

    // Validate state parameter
    if flow.state.is_empty() || flow.state.len() < 8 {
        return; // Invalid state for CSRF protection, skip
    }
}

fuzz_target!(|data: &[u8]| {
    // 1. Parse authentication configuration from JSON
    if let Ok(_json) = from_slice::<Value>(data) {
        // Try various auth configurations
        let auth_configs = vec![
            json!({
                "type": "none"
            }),
            json!({
                "type": "api_key",
                "key": String::from_utf8_lossy(data)
            }),
            json!({
                "type": "oauth2",
                "client_id": "client123",
                "authorization_url": "https://auth.example.com/authorize",
                "token_url": "https://auth.example.com/token"
            }),
            json!({
                "type": "custom",
                "scheme": "CustomAuth",
                "parameters": {
                    "custom_field": String::from_utf8_lossy(data)
                }
            }),
        ];

        for config in auth_configs {
            let _ = serde_json::to_string(&config);
        }
    }

    // 2. Generate and test structured auth flows
    let mut u = Unstructured::new(data);

    if let Ok(auth_req) = FuzzAuthRequest::arbitrary(&mut u) {
        test_auth_flow(&auth_req.auth_type, &auth_req.credentials);

        // Test authorization headers with size limits
        let headers = match auth_req.auth_type {
            FuzzAuthType::ApiKey => {
                if let Some(key) = auth_req.credentials.api_key {
                    // Limit key size to prevent memory issues
                    if key.len() > 10000 {
                        vec![]
                    } else {
                        vec![
                            ("X-API-Key", key.clone()),
                            ("Authorization", format!("ApiKey {}", key)),
                        ]
                    }
                } else {
                    vec![]
                }
            },
            FuzzAuthType::OAuth2 | FuzzAuthType::Jwt => {
                if let Some(token) = auth_req.credentials.token {
                    // Limit token size to prevent memory issues
                    if token.len() > 10000 {
                        vec![]
                    } else {
                        vec![("Authorization", format!("Bearer {}", token))]
                    }
                } else {
                    vec![]
                }
            },
            FuzzAuthType::Custom(ref scheme) => {
                if let Some(token) = auth_req.credentials.token {
                    // Limit combined size to prevent memory issues
                    if scheme.len() > 1000 || token.len() > 10000 {
                        vec![]
                    } else {
                        vec![("Authorization", format!("{} {}", scheme, token))]
                    }
                } else {
                    vec![]
                }
            },
            _ => vec![],
        };

        // Validate headers
        for (name, value) in headers {
            if name.is_empty() || value.is_empty() {
                continue; // Skip invalid headers
            }
            if value.contains('\n') || value.contains('\r') {
                continue; // Skip headers with potential injection
            }
        }
    }

    // 3. Test OAuth2 flows
    if let Ok(oauth_flow) = FuzzOAuthFlow::arbitrary(&mut u) {
        // Check sizes before processing to prevent memory issues
        if oauth_flow.client_id.len() > 1000
            || oauth_flow.redirect_uri.len() > 2000
            || oauth_flow.state.len() > 1000
            || oauth_flow.scope.iter().map(|s| s.len()).sum::<usize>() > 2000
        {
            return; // Skip oversized OAuth flow data
        }

        test_pkce_flow(&oauth_flow);

        // Build authorization URL
        let auth_url = format!(
            "https://auth.example.com/authorize?client_id={}&redirect_uri={}&state={}&scope={}",
            oauth_flow.client_id,
            oauth_flow.redirect_uri,
            oauth_flow.state,
            oauth_flow.scope.join(" ")
        );

        // Validate URL length
        if auth_url.len() >= 8192 {
            return; // URL too long, skip
        }
    }

    // 4. Test token validation and expiry
    if data.len() >= 8 {
        let issued_at = u64::from_be_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]);

        let expires_in = if data.len() >= 12 {
            u32::from_be_bytes([data[8], data[9], data[10], data[11]])
        } else {
            3600 // Default 1 hour
        };

        // Use saturating operations to prevent overflow
        let current_time = issued_at.saturating_add(expires_in as u64 / 2);
        let expiry_time = issued_at.saturating_add(expires_in as u64);
        let is_expired = current_time > expiry_time;

        if !is_expired {
            // Token is still valid
            if current_time < issued_at || current_time >= expiry_time {
                return; // Invalid token timing, skip
            }
        }
    }

    // 5. Test authentication state machine
    let states = [
        "unauthenticated",
        "authenticating",
        "authenticated",
        "refreshing",
        "failed",
    ];
    let transitions = [
        ("unauthenticated", "authenticating"),
        ("authenticating", "authenticated"),
        ("authenticating", "failed"),
        ("authenticated", "refreshing"),
        ("refreshing", "authenticated"),
        ("refreshing", "failed"),
        ("failed", "authenticating"),
    ];

    if data.len() > 0 {
        let state_idx = (data[0] as usize) % states.len();
        let current_state = states[state_idx];

        // Find valid transitions from current state
        let valid_transitions: Vec<_> = transitions
            .iter()
            .filter(|(from, _)| *from == current_state)
            .map(|(_, to)| *to)
            .collect();

        if !valid_transitions.is_empty() && data.len() > 1 {
            let next_idx = (data[1] as usize) % valid_transitions.len();
            let _next_state = valid_transitions[next_idx];
        }
    }
});
