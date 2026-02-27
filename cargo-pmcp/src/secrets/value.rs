//! Secure secret value handling with automatic zeroization.

use rand::RngExt;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::fmt;
use zeroize::Zeroize;

/// A secret value that is automatically zeroized when dropped.
///
/// This wrapper ensures that:
/// - Secret values are never accidentally logged
/// - Memory is securely cleared when the value is dropped
/// - Debug and Display show `[REDACTED]` instead of the actual value
#[derive(Clone)]
pub struct SecretValue {
    inner: SecretString,
}

impl SecretValue {
    /// Create a new secret value from a string.
    pub fn new(value: String) -> Self {
        Self {
            inner: SecretString::from(value),
        }
    }

    /// Expose the secret value for use.
    ///
    /// # Security
    /// Use this sparingly and only when necessary (e.g., sending to API).
    /// Never log the exposed value.
    pub fn expose(&self) -> &str {
        self.inner.expose_secret()
    }

    /// Get the length of the secret value.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.inner.expose_secret().len()
    }

    /// Check if the secret value is empty.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.inner.expose_secret().is_empty()
    }

    /// Generate a random secret value.
    pub fn generate(length: usize, charset: SecretCharset) -> Self {
        let mut rng = rand::rng();
        let chars: Vec<char> = match charset {
            SecretCharset::Alphanumeric => {
                "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
                    .chars()
                    .collect()
            },
            SecretCharset::Ascii => (33u8..=126u8).map(|c| c as char).collect(),
            SecretCharset::Hex => "0123456789abcdef".chars().collect(),
        };

        let value: String = (0..length)
            .map(|_| chars[rng.random_range(0..chars.len())])
            .collect();

        Self::new(value)
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl fmt::Display for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl From<String> for SecretValue {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for SecretValue {
    fn from(value: &str) -> Self {
        Self::new(value.to_string())
    }
}

/// Charset options for generated secrets.
#[derive(Debug, Clone, Copy, Default)]
pub enum SecretCharset {
    /// Alphanumeric characters (a-z, A-Z, 0-9)
    #[default]
    Alphanumeric,
    /// All printable ASCII characters
    Ascii,
    /// Hexadecimal characters (0-9, a-f)
    Hex,
}

impl std::str::FromStr for SecretCharset {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "alphanumeric" | "alnum" => Ok(SecretCharset::Alphanumeric),
            "ascii" => Ok(SecretCharset::Ascii),
            "hex" => Ok(SecretCharset::Hex),
            _ => Err(format!(
                "Unknown charset: {}. Use 'alphanumeric', 'ascii', or 'hex'",
                s
            )),
        }
    }
}

/// Metadata about a secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretMetadata {
    /// Secret name (without server prefix)
    pub name: String,
    /// Version number (if supported by provider)
    pub version: Option<u32>,
    /// Creation timestamp (ISO 8601)
    pub created_at: Option<String>,
    /// Last modified timestamp (ISO 8601)
    pub modified_at: Option<String>,
    /// Human-readable description
    pub description: Option<String>,
    /// Custom tags
    #[serde(default)]
    pub tags: std::collections::HashMap<String, String>,
}

impl SecretMetadata {
    /// Create new metadata with just a name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: None,
            created_at: None,
            modified_at: None,
            description: None,
            tags: std::collections::HashMap::new(),
        }
    }
}

/// A named secret for display purposes (value hidden).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretEntry {
    /// Full secret name (including server prefix)
    pub name: String,
    /// Metadata about the secret
    pub metadata: SecretMetadata,
}

/// Input value source for setting a secret.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SecretInput {
    /// Interactive prompt (hidden input)
    Prompt,
    /// Read from stdin
    Stdin,
    /// Read from file
    File(std::path::PathBuf),
    /// Read from environment variable
    EnvVar(String),
    /// Direct value (warning: visible in process list)
    Direct(SecretValue),
    /// Generate random value
    Generate {
        length: usize,
        charset: SecretCharset,
    },
}

/// Zeroizing string for temporary secret handling.
#[derive(Clone)]
#[allow(dead_code)]
pub struct ZeroizingString(String);

#[allow(dead_code)] // Utility type for future use
impl ZeroizingString {
    pub fn new(s: String) -> Self {
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Drop for ZeroizingString {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for ZeroizingString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_value_redacted_debug() {
        let secret = SecretValue::new("super-secret-key".to_string());
        let debug_output = format!("{:?}", secret);
        assert_eq!(debug_output, "[REDACTED]");
        assert!(!debug_output.contains("super-secret-key"));
    }

    #[test]
    fn test_secret_value_redacted_display() {
        let secret = SecretValue::new("super-secret-key".to_string());
        let display_output = format!("{}", secret);
        assert_eq!(display_output, "[REDACTED]");
    }

    #[test]
    fn test_secret_value_expose() {
        let secret = SecretValue::new("my-api-key".to_string());
        assert_eq!(secret.expose(), "my-api-key");
    }

    #[test]
    fn test_secret_generate_alphanumeric() {
        let secret = SecretValue::generate(32, SecretCharset::Alphanumeric);
        assert_eq!(secret.len(), 32);
        assert!(secret.expose().chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_secret_generate_hex() {
        let secret = SecretValue::generate(16, SecretCharset::Hex);
        assert_eq!(secret.len(), 16);
        assert!(secret.expose().chars().all(|c| c.is_ascii_hexdigit()));
    }
}
