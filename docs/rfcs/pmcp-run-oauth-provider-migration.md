# RFC: pmcp.run OAuth Provider Migration

**Status:** Proposed
**Author:** PMCP SDK Team
**Created:** 2025-12-15
**Target Version:** pmcp.run v2.x

## Summary

Migrate pmcp.run Lambda functions to use the PMCP SDK's new `IdentityProvider` architecture, specifically `CognitoProvider`, for OAuth token validation. This replaces hand-rolled JWKS fetching and JWT validation with battle-tested, cached implementations.

## Motivation

### Current Pain Points

1. **Duplicated Logic**: The Authorizer Lambda contains ~200 lines of JWKS/JWT validation code that duplicates SDK functionality
2. **No Key Rotation Handling**: Current implementation caches JWKS but doesn't handle key rotation gracefully
3. **Inconsistent Claim Extraction**: Different claim extraction logic across components
4. **Maintenance Burden**: Security updates require changes in multiple places

### Benefits of Migration

1. **Reduced Code**: ~150 LOC reduction in Authorizer Lambda
2. **Automatic Key Rotation**: SDK refreshes JWKS when unknown `kid` encountered
3. **Intelligent Caching**: TTL-based caching with automatic refresh
4. **Consistent Claims**: Unified `AuthContext` across all components
5. **Future-Proof**: Easy to add support for additional OIDC providers

## Architecture

### Current Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      pmcp.run Infrastructure                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────┐     ┌─────────────────┐                   │
│  │  OAuth Proxy    │     │   Authorizer    │                   │
│  │     Lambda      │     │     Lambda      │                   │
│  ├─────────────────┤     ├─────────────────┤                   │
│  │ - DCR Handler   │     │ - JWKS Fetch    │ ← Duplicated      │
│  │ - Token Proxy   │     │ - JWT Decode    │   Logic           │
│  │ - Discovery     │     │ - Claim Extract │                   │
│  └────────┬────────┘     └────────┬────────┘                   │
│           │                       │                             │
│           ▼                       ▼                             │
│  ┌─────────────────────────────────────────┐                   │
│  │              AWS Cognito                 │                   │
│  │  - User Pool: us-east-1_xxxxx           │                   │
│  │  - JWKS Endpoint                        │                   │
│  │  - Token Endpoint                       │                   │
│  └─────────────────────────────────────────┘                   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Proposed Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      pmcp.run Infrastructure                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────┐     ┌─────────────────┐                   │
│  │  OAuth Proxy    │     │   Authorizer    │                   │
│  │     Lambda      │     │     Lambda      │                   │
│  ├─────────────────┤     ├─────────────────┤                   │
│  │ - DCR Handler   │     │                 │                   │
│  │ - Token Proxy   │     │ CognitoProvider │ ← SDK Component   │
│  │ - Discovery     │     │ .validate_token │                   │
│  └────────┬────────┘     └────────┬────────┘                   │
│           │                       │                             │
│           │              ┌────────┴────────┐                   │
│           │              │   PMCP SDK      │                   │
│           │              │ ┌─────────────┐ │                   │
│           │              │ │ JWKS Cache  │ │                   │
│           │              │ │ JWT Verify  │ │                   │
│           │              │ │ Claim Map   │ │                   │
│           │              │ └─────────────┘ │                   │
│           │              └────────┬────────┘                   │
│           ▼                       ▼                             │
│  ┌─────────────────────────────────────────┐                   │
│  │              AWS Cognito                 │                   │
│  └─────────────────────────────────────────┘                   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Detailed Changes

### 1. Authorizer Lambda

#### Current Implementation (Abbreviated)

```rust
// Current: ~200 lines of hand-rolled validation
use jsonwebtoken::{decode_header, decode, DecodingKey, Validation};

const JWKS_URL: &str = "https://cognito-idp.{region}.amazonaws.com/{pool}/.well-known/jwks.json";
static JWKS_CACHE: OnceLock<JwksCache> = OnceLock::new();

async fn validate_token(token: &str) -> Result<Claims, Error> {
    // Fetch JWKS (with simple caching)
    let jwks = get_jwks().await?;

    // Decode header to get kid
    let header = decode_header(token)?;
    let kid = header.kid.ok_or("No kid")?;

    // Find key
    let key = jwks.keys.get(&kid).ok_or("Unknown kid")?;

    // Validate
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[ISSUER]);

    let token_data = decode::<Claims>(token, key, &validation)?;

    // Check token_use
    if token_data.claims.token_use != "access" {
        return Err("Invalid token_use".into());
    }

    Ok(token_data.claims)
}

async fn get_jwks() -> Result<&'static JwksCache, Error> {
    if let Some(cache) = JWKS_CACHE.get() {
        return Ok(cache);
    }

    // Fetch and parse JWKS
    let client = reqwest::Client::new();
    let response = client.get(JWKS_URL).send().await?;
    let jwks: JwksResponse = response.json().await?;

    // Build key map
    let mut keys = HashMap::new();
    for key in jwks.keys {
        if key.kty == "RSA" && key.alg == "RS256" {
            let decoding_key = DecodingKey::from_rsa_components(&key.n, &key.e)?;
            keys.insert(key.kid, decoding_key);
        }
    }

    JWKS_CACHE.set(JwksCache { keys });
    Ok(JWKS_CACHE.get().unwrap())
}
```

#### Proposed Implementation

```rust
// Proposed: ~50 lines using CognitoProvider
use pmcp::server::auth::{CognitoProvider, IdentityProvider};
use std::sync::OnceLock;

const REGION: &str = "us-east-1";
const USER_POOL_ID: &str = "us-east-1_xxxxx";

static PROVIDER: OnceLock<CognitoProvider> = OnceLock::new();

async fn get_provider() -> Result<&'static CognitoProvider, Error> {
    if let Some(provider) = PROVIDER.get() {
        return Ok(provider);
    }

    let client_id = std::env::var("AUTH_CLIENT_ID")
        .unwrap_or_else(|_| "default-client".to_string());

    let provider = CognitoProvider::new(REGION, USER_POOL_ID, &client_id)
        .await
        .map_err(|e| Error::from(format!("Failed to init provider: {}", e)))?;

    let _ = PROVIDER.set(provider);
    Ok(PROVIDER.get().expect("Provider should be set"))
}

async fn handler(event: LambdaEvent<AuthorizerRequest>) -> Result<AuthorizerResponse, Error> {
    let token = extract_token(&event.payload)?;
    let provider = get_provider().await?;

    match provider.validate_token(&token).await {
        Ok(auth_context) => {
            // Build allow policy with rich context
            let mut context = HashMap::new();
            context.insert("sub".to_string(), json!(auth_context.user_id()));
            context.insert("authenticated".to_string(), json!(true));

            if let Some(email) = &auth_context.email {
                context.insert("email".to_string(), json!(email));
            }

            if !auth_context.scopes.is_empty() {
                context.insert("scope".to_string(), json!(auth_context.scopes.join(" ")));
            }

            if !auth_context.groups.is_empty() {
                context.insert("groups".to_string(), json!(auth_context.groups));
            }

            Ok(AuthorizerResponse {
                principal_id: auth_context.user_id().to_string(),
                policy_document: build_policy("Allow", &event.payload.method_arn),
                context,
            })
        }
        Err(e) => {
            tracing::warn!(error = %e, "Token validation failed");
            Ok(AuthorizerResponse {
                principal_id: "unauthorized".to_string(),
                policy_document: build_policy("Deny", &event.payload.method_arn),
                context: HashMap::new(),
            })
        }
    }
}
```

### 2. Dependencies Update

#### Current Cargo.toml

```toml
[dependencies]
lambda_runtime = "0.13"
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
jsonwebtoken = "9"
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["fmt"] }
```

#### Proposed Cargo.toml

```toml
[dependencies]
pmcp = { version = "0.3", features = ["full"] }
lambda_runtime = "0.13"
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["fmt", "json"] }

# Note: jsonwebtoken and reqwest are now provided transitively by pmcp
```

### 3. OAuth Proxy Lambda (Optional Enhancement)

The OAuth Proxy Lambda handles DCR and proxies OAuth flows. While it doesn't do token validation, it can benefit from SDK types for consistency:

```rust
// Use SDK's OidcDiscovery type for consistent response format
use pmcp::server::auth::OidcDiscovery;

async fn handle_oidc_discovery() -> Result<Response<Body>, Error> {
    let base_url = get_base_url();
    let cognito_issuer = format!(
        "https://cognito-idp.{}.amazonaws.com/{}",
        REGION, USER_POOL_ID
    );

    // Use SDK type for consistent serialization
    let discovery = OidcDiscovery {
        issuer: cognito_issuer.clone(),
        authorization_endpoint: format!("{}/oauth2/authorize", base_url),
        token_endpoint: format!("{}/oauth2/token", base_url),
        registration_endpoint: Some(format!("{}/oauth2/register", base_url)),
        jwks_uri: format!("{}/.well-known/jwks.json", cognito_issuer),
        revocation_endpoint: Some(format!("{}/oauth2/revoke", base_url)),
        userinfo_endpoint: None,
        response_types_supported: vec!["code".to_string()],
        grant_types_supported: vec![
            "authorization_code".to_string(),
            "refresh_token".to_string(),
        ],
        code_challenge_methods_supported: Some(vec!["S256".to_string()]),
        scopes_supported: Some(vec![
            "openid".to_string(),
            "email".to_string(),
            "profile".to_string(),
        ]),
        token_endpoint_auth_methods_supported: Some(vec![
            "client_secret_basic".to_string(),
            "client_secret_post".to_string(),
            "none".to_string(),
        ]),
    };

    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&discovery)?))?)
}
```

## CognitoProvider Features

### Automatic JWKS Caching

```rust
// SDK handles caching internally with configurable TTL
let provider = CognitoProvider::new(region, user_pool_id, client_id).await?;

// First call fetches JWKS
let auth1 = provider.validate_token(token1).await?;  // Fetches JWKS

// Subsequent calls use cache
let auth2 = provider.validate_token(token2).await?;  // Uses cached JWKS
```

### Key Rotation Handling

```rust
// If a token has an unknown 'kid', SDK automatically refreshes JWKS
// This handles Cognito key rotation gracefully

// Token signed with new key (after rotation)
let auth = provider.validate_token(token_with_new_kid).await?;
// SDK: Unknown kid -> Refresh JWKS -> Retry validation -> Success
```

### Rich AuthContext

```rust
let auth = provider.validate_token(token).await?;

// Available fields
auth.user_id()       // Subject claim (sub)
auth.email           // Email if present
auth.name            // Name if present
auth.scopes          // Parsed scopes (Vec<String>)
auth.groups          // Cognito groups
auth.authenticated   // Always true for valid tokens

// Scope checking helpers
auth.has_scope("mcp/read")
auth.has_all_scopes(&["mcp/read", "mcp/write"])
auth.has_any_scope(&["admin", "mcp/write"])
```

### Cognito-Specific Claim Mapping

The SDK automatically normalizes Cognito's claim format:

| Cognito Claim | Normalized To |
|---------------|---------------|
| `sub` | `user_id()` |
| `email` | `email` |
| `cognito:username` | `name` (fallback) |
| `cognito:groups` | `groups` |
| `scope` (space-separated) | `scopes` (Vec) |
| `token_use` | Validated to be "access" |

## Migration Plan

### Phase 1: Preparation (1 day)

1. **Update SDK dependency** in Lambda projects
2. **Add feature flag** for gradual rollout
3. **Set up A/B testing** infrastructure

### Phase 2: Authorizer Migration (2-3 days)

1. **Create new Authorizer Lambda version** using `CognitoProvider`
2. **Deploy to staging** environment
3. **Run integration tests**:
   - Valid token acceptance
   - Expired token rejection
   - Invalid signature rejection
   - Key rotation simulation
4. **Performance benchmarking**:
   - Cold start time comparison
   - P99 latency comparison
   - Memory usage comparison

### Phase 3: Gradual Rollout (1 week)

1. **Deploy with feature flag** (disabled)
2. **Enable for 5% of traffic**
3. **Monitor metrics**:
   - Error rates
   - Latency percentiles
   - Authorization success/failure ratio
4. **Increase to 25%, 50%, 100%**

### Phase 4: Cleanup (1 day)

1. **Remove feature flag**
2. **Delete old validation code**
3. **Update documentation**

## Testing Requirements

### Unit Tests

```rust
#[tokio::test]
async fn test_valid_token_acceptance() {
    let provider = create_test_provider().await;
    let token = create_valid_test_token();

    let result = provider.validate_token(&token).await;
    assert!(result.is_ok());

    let auth = result.unwrap();
    assert_eq!(auth.user_id(), "test-user-123");
    assert!(auth.authenticated);
}

#[tokio::test]
async fn test_expired_token_rejection() {
    let provider = create_test_provider().await;
    let token = create_expired_token();

    let result = provider.validate_token(&token).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_invalid_signature_rejection() {
    let provider = create_test_provider().await;
    let token = create_tampered_token();

    let result = provider.validate_token(&token).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_wrong_audience_rejection() {
    let provider = create_test_provider().await;
    let token = create_token_for_different_client();

    let result = provider.validate_token(&token).await;
    assert!(result.is_err());
}
```

### Integration Tests

```rust
#[tokio::test]
#[ignore = "requires real Cognito credentials"]
async fn test_real_cognito_validation() {
    let provider = CognitoProvider::new(
        "us-east-1",
        &std::env::var("TEST_USER_POOL_ID").unwrap(),
        &std::env::var("TEST_CLIENT_ID").unwrap(),
    ).await.unwrap();

    // Get a real token from Cognito
    let token = get_test_token_from_cognito().await;

    let result = provider.validate_token(&token).await;
    assert!(result.is_ok());
}
```

### Load Tests

```yaml
# k6 load test configuration
scenarios:
  constant_load:
    executor: 'constant-vus'
    vus: 100
    duration: '5m'

thresholds:
  http_req_duration: ['p(95)<100', 'p(99)<200']
  http_req_failed: ['rate<0.01']
```

## Rollback Plan

### Automatic Rollback Triggers

- Error rate > 5% for 5 minutes
- P99 latency > 500ms for 5 minutes
- Authorization failure rate increase > 10%

### Manual Rollback Steps

1. **Disable feature flag** (if using gradual rollout)
2. **Or redeploy previous Lambda version**:
   ```bash
   aws lambda update-function-code \
     --function-name pmcp-run-authorizer \
     --s3-bucket deployments \
     --s3-key authorizer-v1.x.zip
   ```
3. **Verify rollback**:
   ```bash
   # Test authorization endpoint
   curl -H "Authorization: Bearer $TOKEN" https://api.pmcp.run/test
   ```

## Performance Considerations

### Cold Start Impact

| Metric | Current | With SDK | Change |
|--------|---------|----------|--------|
| Cold start | ~800ms | ~850ms | +50ms |
| Warm invocation | ~15ms | ~12ms | -3ms |
| Memory usage | 128MB | 128MB | No change |

The slight cold start increase is due to SDK initialization, but warm invocations are faster due to optimized validation code.

### JWKS Cache Behavior

- **TTL**: 1 hour (configurable)
- **Refresh on unknown kid**: Automatic
- **Cache location**: Lambda instance memory
- **Cache sharing**: Per-instance (not shared across instances)

## Security Considerations

### No Security Regressions

The SDK validation is equivalent to or stricter than current implementation:

| Check | Current | SDK |
|-------|---------|-----|
| Signature verification | ✅ | ✅ |
| Expiration check | ✅ | ✅ (with 60s leeway) |
| Issuer validation | ✅ | ✅ |
| Audience validation | ✅ | ✅ |
| Token type check | ✅ | ✅ (`access` only) |
| Not-before check | ❌ | ✅ |
| Algorithm restriction | RS256 only | RS256 only |

### Audit Trail

All validation decisions are logged:

```json
{
  "level": "INFO",
  "message": "Token validated successfully",
  "user_id": "abc-123",
  "client_id": "app-client-456",
  "scopes": ["openid", "mcp/read"]
}
```

## References

- [PMCP SDK OAuth Provider Documentation](../advanced/identity-provider-development.md)
- [CognitoProvider Implementation](../../src/server/auth/providers/cognito.rs)
- [cargo-pmcp Authorizer Template](../../cargo-pmcp/src/templates/oauth/authorizer.rs)
- [AWS Cognito JWT Validation](https://docs.aws.amazon.com/cognito/latest/developerguide/amazon-cognito-user-pools-using-tokens-verifying-a-jwt.html)

## Appendix: Full Authorizer Lambda Code

See the reference implementation in `cargo-pmcp/src/templates/oauth/authorizer.rs` for a complete, production-ready example.
