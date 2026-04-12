# Security Model

This document describes the security properties, threat model, and known limitations of `pmcp-code-mode`.

## Token Secret Handling

`TokenSecret` is the core secret material used for HMAC signing of approval tokens.

**Implementation:** Backed by `secrecy::SecretBox<[u8]>` with `zeroize` ensuring memory is cleared on drop.

**Explicitly denied traits:**
- `Debug` -- prevents accidental logging of secret bytes
- `Display` -- prevents printing secret bytes
- `Clone` -- prevents accidental copies that bypass zeroize-on-drop
- `PartialEq` / `Eq` -- prevents timing side-channel comparisons
- `Serialize` / `Deserialize` -- prevents JSON/wire leakage

**Access:** Secret bytes are accessible only via `expose_secret() -> &[u8]`. Internal framework code calls this method; external callers should never need to.

### Serde Safety

`TokenSecret` does not implement `Serialize` or `Deserialize`. Structs containing `TokenSecret` that derive `Serialize` **MUST** annotate the field with `#[serde(skip)]` to prevent compilation errors and potential secret leakage:

```rust,ignore
#[derive(serde::Serialize)]
struct MyConfig {
    #[serde(skip)]  // REQUIRED -- TokenSecret does not implement Serialize
    token_secret: TokenSecret,
    // ... other fields
}
```

The `#[derive(CodeMode)]` macro does not generate `Serialize`/`Deserialize` impls, so this is only relevant when users manually derive serde traits on their server structs.

## HMAC Token Binding

Approval tokens use HMAC-SHA256 to cryptographically bind the validated code to its validation result.

**Token contents:**
- `request_id`: UUID v4 (prevents replay)
- `code_hash`: SHA-256 of the canonicalized code
- `user_id`: From the access token / session context
- `session_id`: MCP session identifier (prevents cross-session usage)
- `server_id`: Server that validated the code
- `context_hash`: SHA-256 of `schema_hash || permissions_hash` (detects context changes)
- `risk_level`: Assessed risk level at validation time
- `created_at` / `expires_at`: Unix timestamps

**Signing:** The full token payload is JSON-serialized and HMAC-SHA256-signed with the `TokenSecret`. The signature is base64-encoded and appended to the token.

## Token Replay Protection

- **`request_id` uniqueness**: Each token includes a UUID v4 `request_id` that is unique per validation request
- **TTL expiry**: Tokens expire after `token_ttl_seconds` (default: 300 seconds / 5 minutes)
- **Session binding**: Token includes `session_id`, preventing use across different MCP sessions

## Code Modification Detection

Any change to the code after validation invalidates the token:

1. `validate_code` computes `SHA-256(canonicalize(code))` and embeds the hash in the token
2. `execute_code` recomputes the hash from the submitted code
3. If the hashes differ, execution is rejected with `TokenVerificationError`

Canonicalization normalizes whitespace to prevent trivial bypass via formatting changes.

## Policy Evaluation

The `PolicyEvaluator` trait provides pluggable authorization:

- **Default-deny semantics**: Without a configured evaluator, only basic config checks (allow_mutations, max_depth, etc.) are performed
- **Cedar support**: The `cedar` feature flag enables local Cedar policy evaluation via `CedarPolicyEvaluator`
- **AWS Verified Permissions**: External `pmcp-avp` crate (not part of this SDK) supports cloud-hosted policy evaluation

## Parser Safety

- **GraphQL**: Parsed via [`graphql-parser`](https://crates.io/crates/graphql-parser) (well-maintained, pure Rust)
- **JavaScript**: Parsed via [SWC](https://swc.rs/) (`swc_ecma_parser`) behind the `openapi-code-mode` feature flag (widely used in production tooling)

Both parsers operate on untrusted input. Malformed input results in parse errors, not panics or undefined behavior.

## Known Limitations

1. **`TokenSecret::new()` does not zeroize the source `Vec`**: The input bytes are copied into `SecretBox`, but the original `Vec` is not zeroed. Use `TokenSecret::from_env()` for maximum security in production, which reads directly from an environment variable.

2. **`NoopPolicyEvaluator` bypasses all policy checks**: This evaluator allows ALL operations unconditionally. It exists for testing and local development only. Production servers MUST implement `PolicyEvaluator` with a real authorization backend.

3. **In-memory token secrets**: `TokenSecret` protects against accidental logging and zeroizes on drop, but the secret bytes are accessible to any code in the same process while the struct is alive. This is inherent to in-memory secret management.

4. **No protection against side-channel attacks on HMAC computation**: The HMAC-SHA256 implementation (`hmac` crate) uses standard constant-time comparison for verification, but does not claim resistance to power analysis or electromagnetic side-channel attacks.

5. **Token does not bind to client identity**: The `user_id` in the token comes from the validation context, not from a cryptographically verified client certificate. An attacker who obtains a valid token can use it from any client within the same session.

## Reporting Security Issues

Please report security vulnerabilities via GitHub Security Advisories at <https://github.com/paiml/rust-mcp-sdk/security/advisories>.
