//! Approval token generation and verification.
//!
//! MVP uses HMAC-SHA256 for token signing. Full implementation will use AWS KMS.

use crate::types::{ExecutionError, RiskLevel};
use hmac::{Hmac, KeyInit, Mac};
use secrecy::{ExposeSecret, SecretBox};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

/// Zeroizing wrapper for HMAC token secrets.
///
/// ## Security Properties
/// - Memory is zeroed on drop via `zeroize` (through `secrecy::SecretBox`)
/// - **Explicitly does NOT implement:** `Debug`, `Display`, `Clone`, `PartialEq`,
///   `Serialize`, `Deserialize` -- preventing accidental logging, serialization, or copying
/// - Secret bytes accessed only via `expose_secret()` which returns `&[u8]`
///
/// ## Threat Model
/// Protects against: accidental logging, memory dumps after drop,
/// clone-and-forget patterns, comparison side channels, JSON serialization leakage.
/// Does NOT protect against: active memory forensics while the secret
/// is in use, side-channel attacks on the HMAC computation itself.
///
/// ## Usage in Structs
/// When embedding `TokenSecret` in a struct that derives `Serialize`:
/// ```rust,ignore
/// #[derive(serde::Serialize)]
/// struct MyServer {
///     #[serde(skip)]  // REQUIRED -- TokenSecret does not implement Serialize
///     token_secret: TokenSecret,
///     // ... other fields
/// }
/// ```
pub struct TokenSecret(SecretBox<[u8]>);

// SAFETY NOTE: TokenSecret intentionally does NOT derive or implement:
// - Debug (prevents logging secret bytes)
// - Display (prevents printing secret bytes)
// - Clone (prevents accidental copies that bypass zeroize)
// - Serialize / Deserialize (prevents JSON/wire leakage)
// - PartialEq / Eq (prevents timing side-channel comparisons)
// These denials are verified by negative trait tests in Plan 05.

impl TokenSecret {
    /// Create from raw bytes. The input Vec is consumed and its contents
    /// copied into a SecretBox. The original Vec is NOT zeroed -- callers
    /// should use `from_env()` for maximum security.
    pub fn new(secret: impl Into<Vec<u8>>) -> Self {
        let bytes: Vec<u8> = secret.into();
        Self(SecretBox::new(Box::from(bytes.as_slice())))
    }

    /// Read from an environment variable. The string value is converted
    /// to bytes and wrapped immediately.
    pub fn from_env(var: &str) -> Result<Self, std::env::VarError> {
        let val = std::env::var(var)?;
        Ok(Self::new(val.into_bytes()))
    }

    /// Expose the secret bytes for cryptographic operations.
    /// Callers MUST NOT log or persist the returned slice.
    pub fn expose_secret(&self) -> &[u8] {
        self.0.expose_secret()
    }
}

/// Approval token that authorizes code execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalToken {
    /// Unique request ID (prevents replay attacks)
    pub request_id: String,

    /// SHA-256 hash of the canonicalized code
    pub code_hash: String,

    /// User ID from the access token
    pub user_id: String,

    /// MCP session ID (prevents cross-session usage)
    pub session_id: String,

    /// Server that validated the code
    pub server_id: String,

    /// Hash of schema + permissions (detects context changes)
    pub context_hash: String,

    /// Assessed risk level
    pub risk_level: RiskLevel,

    /// Unix timestamp when token was created
    pub created_at: i64,

    /// Unix timestamp when token expires
    pub expires_at: i64,

    /// HMAC signature over all fields above
    pub signature: String,
}

impl ApprovalToken {
    /// Encode the token to a string for transport.
    pub fn encode(&self) -> Result<String, serde_json::Error> {
        let json = serde_json::to_string(self)?;
        Ok(base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            json.as_bytes(),
        ))
    }

    /// Decode a token from a string.
    pub fn decode(encoded: &str) -> Result<Self, TokenDecodeError> {
        let bytes =
            base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, encoded)
                .map_err(|_| TokenDecodeError::InvalidBase64)?;

        let json = String::from_utf8(bytes).map_err(|_| TokenDecodeError::InvalidUtf8)?;

        serde_json::from_str(&json).map_err(|_| TokenDecodeError::InvalidJson)
    }

    /// Get the payload bytes for signing/verification.
    /// Build the canonical payload bytes for HMAC signing/verification.
    ///
    /// BREAKING CHANGE (v0.1.0 pre-release): This now uses `Display` formatting
    /// for `risk_level` (stable "LOW"/"MEDIUM"/"HIGH"/"CRITICAL") instead of
    /// `Debug` formatting. Tokens signed with the prior `Debug` format ("Low",
    /// "Medium", etc.) will fail verification after this change.
    fn payload_bytes(&self) -> Vec<u8> {
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.request_id,
            self.code_hash,
            self.user_id,
            self.session_id,
            self.server_id,
            self.context_hash,
            self.risk_level,
            self.created_at,
            self.expires_at,
        )
        .into_bytes()
    }
}

/// Errors that can occur when decoding a token.
#[derive(Debug, thiserror::Error)]
pub enum TokenDecodeError {
    #[error(
        "Token is not valid base64 — it may have been truncated or corrupted during transport"
    )]
    InvalidBase64,
    #[error("Token contains invalid UTF-8 bytes after base64 decoding")]
    InvalidUtf8,
    #[error("Token decoded to invalid JSON — the token string may have been truncated, double-encoded, or is not an approval token")]
    InvalidJson,
}

/// Trait for token generators.
pub trait TokenGenerator: Send + Sync {
    /// Generate a signed approval token.
    fn generate(
        &self,
        code: &str,
        user_id: &str,
        session_id: &str,
        server_id: &str,
        context_hash: &str,
        risk_level: RiskLevel,
        ttl_seconds: i64,
    ) -> ApprovalToken;

    /// Verify a token and return Ok if valid.
    fn verify(&self, token: &ApprovalToken) -> Result<(), ExecutionError>;

    /// Verify that submitted code matches the token's code hash.
    fn verify_code(&self, code: &str, token: &ApprovalToken) -> Result<(), ExecutionError>;
}

/// HMAC-based token generator for MVP.
pub struct HmacTokenGenerator {
    secret: TokenSecret,
}

impl HmacTokenGenerator {
    /// Minimum secret length in bytes for HMAC token generation.
    ///
    /// Secrets shorter than this are rejected to prevent trivially forgeable tokens.
    /// 16 bytes (128 bits) is the minimum recommended for HMAC-SHA256.
    pub const MIN_SECRET_LEN: usize = 16;

    /// Create a new HMAC token generator with a `TokenSecret`.
    ///
    /// # Panics
    ///
    /// Panics if the secret is shorter than [`Self::MIN_SECRET_LEN`] (16 bytes).
    /// An empty or very short secret makes tokens trivially forgeable.
    pub fn new(secret: TokenSecret) -> Self {
        assert!(
            secret.expose_secret().len() >= Self::MIN_SECRET_LEN,
            "HMAC token secret must be at least {} bytes, got {}",
            Self::MIN_SECRET_LEN,
            secret.expose_secret().len()
        );
        Self { secret }
    }

    /// Create from raw bytes (backward-compatible migration helper).
    ///
    /// Wraps the bytes in a `TokenSecret` internally. Prefer constructing
    /// a `TokenSecret` directly for new code.
    pub fn new_from_bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Self::new(TokenSecret::new(bytes))
    }

    /// Create from an environment variable.
    pub fn from_env(env_var: &str) -> Result<Self, std::env::VarError> {
        Ok(Self::new(TokenSecret::from_env(env_var)?))
    }

    /// Sign the token payload.
    fn sign(&self, payload: &[u8]) -> String {
        let mut mac = HmacSha256::new_from_slice(self.secret.expose_secret())
            .expect("HMAC can take key of any size");
        mac.update(payload);
        hex::encode(mac.finalize().into_bytes())
    }

    /// Verify the signature.
    fn verify_signature(&self, payload: &[u8], signature: &str) -> bool {
        let mut mac = HmacSha256::new_from_slice(self.secret.expose_secret())
            .expect("HMAC can take key of any size");
        mac.update(payload);

        let expected = hex::decode(signature).unwrap_or_default();
        mac.verify_slice(&expected).is_ok()
    }
}

impl TokenGenerator for HmacTokenGenerator {
    fn generate(
        &self,
        code: &str,
        user_id: &str,
        session_id: &str,
        server_id: &str,
        context_hash: &str,
        risk_level: RiskLevel,
        ttl_seconds: i64,
    ) -> ApprovalToken {
        let now = chrono::Utc::now().timestamp();

        let mut token = ApprovalToken {
            request_id: Uuid::new_v4().to_string(),
            code_hash: hash_code(code),
            user_id: user_id.to_string(),
            session_id: session_id.to_string(),
            server_id: server_id.to_string(),
            context_hash: context_hash.to_string(),
            risk_level,
            created_at: now,
            expires_at: now + ttl_seconds,
            signature: String::new(),
        };

        token.signature = self.sign(&token.payload_bytes());
        token
    }

    fn verify(&self, token: &ApprovalToken) -> Result<(), ExecutionError> {
        let now = chrono::Utc::now().timestamp();
        if now > token.expires_at {
            return Err(ExecutionError::TokenExpired);
        }

        if !self.verify_signature(&token.payload_bytes(), &token.signature) {
            return Err(ExecutionError::TokenInvalid(
                "signature verification failed".into(),
            ));
        }

        Ok(())
    }

    fn verify_code(&self, code: &str, token: &ApprovalToken) -> Result<(), ExecutionError> {
        let current_hash = hash_code(code);
        if current_hash != token.code_hash {
            let expected_prefix = if token.code_hash.len() >= 12 {
                &token.code_hash[..12]
            } else {
                &token.code_hash
            };
            let actual_prefix = if current_hash.len() >= 12 {
                &current_hash[..12]
            } else {
                &current_hash
            };
            return Err(ExecutionError::CodeMismatch {
                expected_hash: expected_prefix.to_string(),
                actual_hash: actual_prefix.to_string(),
            });
        }
        Ok(())
    }
}

/// Compute the SHA-256 hash of canonicalized code.
///
/// This is the same hash used in approval tokens. Clients can call this
/// to verify their code will match the token before executing.
pub fn hash_code(code: &str) -> String {
    use sha2::Digest;
    let mut hasher = Sha256::new();
    hasher.update(canonicalize_code(code).as_bytes());
    hex::encode(hasher.finalize())
}

/// Canonicalize code for consistent hashing.
///
/// This normalizes whitespace to ensure semantically identical code
/// produces the same hash, regardless of:
/// - Leading/trailing whitespace or newlines on the whole string
/// - Trailing whitespace on individual lines
/// - Windows vs Unix line endings (\r\n vs \n)
/// - Blank lines between statements
pub fn canonicalize_code(code: &str) -> String {
    let mut result = String::new();
    for line in code.trim().lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(trimmed);
        }
    }
    result
}

/// Compute a context hash from schema and permissions.
pub fn compute_context_hash(schema_hash: &str, permissions_hash: &str) -> String {
    use sha2::Digest;
    let mut hasher = Sha256::new();
    hasher.update(schema_hash.as_bytes());
    hasher.update(b"|");
    hasher.update(permissions_hash.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_generation_and_verification() {
        let generator = HmacTokenGenerator::new(TokenSecret::new(b"test-secret-key!".to_vec()));

        let token = generator.generate(
            "query { users { id } }",
            "user-123",
            "session-456",
            "server-789",
            "context-hash",
            RiskLevel::Low,
            300,
        );

        // Token should verify successfully
        assert!(generator.verify(&token).is_ok());

        // Code should match
        assert!(generator
            .verify_code("query { users { id } }", &token)
            .is_ok());
    }

    #[test]
    fn test_code_mismatch() {
        let generator = HmacTokenGenerator::new(TokenSecret::new(b"test-secret-key!".to_vec()));

        let token = generator.generate(
            "query { users { id } }",
            "user-123",
            "session-456",
            "server-789",
            "context-hash",
            RiskLevel::Low,
            300,
        );

        // Different code should fail
        let result = generator.verify_code("query { orders { id } }", &token);
        assert!(matches!(result, Err(ExecutionError::CodeMismatch { .. })));
    }

    #[test]
    fn test_token_encode_decode() {
        let generator = HmacTokenGenerator::new(TokenSecret::new(b"test-secret-key!".to_vec()));

        let token = generator.generate(
            "query { users { id } }",
            "user-123",
            "session-456",
            "server-789",
            "context-hash",
            RiskLevel::Low,
            300,
        );

        let encoded = token.encode().unwrap();
        let decoded = ApprovalToken::decode(&encoded).unwrap();

        assert_eq!(token.request_id, decoded.request_id);
        assert_eq!(token.code_hash, decoded.code_hash);
        assert_eq!(token.signature, decoded.signature);
    }

    #[test]
    fn test_canonicalize_code() {
        let code1 = "query { users { id } }";
        let code2 = "  query { users { id } }  ";
        let code3 = "query {\n  users {\n    id\n  }\n}";

        // Trimmed versions should be equivalent
        assert_eq!(canonicalize_code(code1), canonicalize_code(code2));

        // Multi-line should normalize differently
        let canonical = canonicalize_code(code3);
        assert!(canonical.contains("query {"));
        assert!(canonical.contains("users {"));
    }

    #[test]
    #[should_panic(expected = "HMAC token secret must be at least 16 bytes")]
    fn test_empty_secret_rejected() {
        let _generator = HmacTokenGenerator::new(TokenSecret::new(b"".to_vec()));
    }

    #[test]
    #[should_panic(expected = "HMAC token secret must be at least 16 bytes")]
    fn test_short_secret_rejected() {
        let _generator = HmacTokenGenerator::new(TokenSecret::new(b"short".to_vec()));
    }
}
