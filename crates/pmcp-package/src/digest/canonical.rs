//! Canonical-bytes-then-digest computation for AI-Package manifests (I-2).
//!
//! # Two distinct hashing paths — do not conflate them
//!
//! - [`ManifestDigest::from_bytes`] hashes the RAW bytes given, exactly as
//!   provided. This is the blob content-addressing constructor: it is used
//!   when the caller already has the exact bytes that were (or will be)
//!   stored on disk / in a registry layer, and wants a digest that
//!   identifies those bytes verbatim.
//! - [`manifest_digest`] canonicalizes a typed struct via
//!   [`olpc_cjson::CanonicalFormatter`] THEN hashes the canonical bytes. This
//!   is the struct-identity constructor: it guarantees the same logical
//!   value always produces the same digest regardless of field-declaration
//!   order or which `Vec`/`BTreeMap`/`HashMap` a collection field happens to
//!   be backed by (olpc-cjson sorts object keys at serialize time).
//!
//! Conflating the two (e.g. hashing a struct's default `serde_json` bytes
//! instead of its canonical bytes) reintroduces the exact
//! HashMap-insertion-order landmine `olpc-cjson` exists to remove — see
//! RESEARCH.md Anti-Patterns.
//!
//! # Construct-only-by-validation newtype (mirrors `ValidatedPath`)
//!
//! [`ManifestDigest`]'s inner `String` field is private. The only ways to
//! build one are [`ManifestDigest::from_bytes`] (infallible — sha256 always
//! produces a well-formed digest), [`ManifestDigest::parse`] (fallible —
//! validates externally-supplied strings), and the validated
//! `TryFrom<&oci_spec::image::Digest>` conversion below. There is no `pub`
//! field literal and no blanket `#[serde(transparent)]` — deserialization
//! from an arbitrary JSON string is routed through `parse()` via
//! `#[serde(try_from = "String")]`, so a malformed digest can never enter a
//! typed struct undetected (T-168-02).

use crate::error::{PackageError, Result};
use olpc_cjson::CanonicalFormatter;
use serde::{Deserialize, Serialize};
use serde_json::Serializer;
use sha2::{Digest as _, Sha256};
use std::fmt;

/// A validated content digest in `sha256:<64-lowercase-hex>` form.
///
/// Constructible only via [`ManifestDigest::from_bytes`], [`ManifestDigest::parse`],
/// or `TryFrom<&oci_spec::image::Digest>` — never via a public field literal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ManifestDigest(String);

impl ManifestDigest {
    /// Hash the given bytes exactly as provided (raw blob content-addressing).
    ///
    /// This does NOT canonicalize — it is the caller's responsibility to
    /// pass already-canonical bytes (via [`canonicalize`]) when the digest
    /// is meant to identify a *struct's* logical content rather than a raw
    /// blob. Use [`manifest_digest`] for the canonicalize-then-hash path.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let hash = Sha256::digest(bytes);
        ManifestDigest(format!("sha256:{}", hex::encode(hash)))
    }

    /// Parse a `sha256:<64-lowercase-hex>` string, validating its format.
    ///
    /// Returns [`PackageError::MalformedDigest`] if the `sha256:` prefix is
    /// missing, or if the remainder is not exactly 64 lowercase ASCII-hex
    /// characters.
    pub fn parse(s: &str) -> Result<Self> {
        let hex_part = s.strip_prefix("sha256:").ok_or_else(|| PackageError::MalformedDigest {
            reason: format!("missing 'sha256:' prefix: {s:?}"),
        })?;
        if !is_lowercase_sha256_hex(hex_part) {
            return Err(PackageError::MalformedDigest {
                reason: format!(
                    "expected 64 lowercase hex chars after 'sha256:', got: {hex_part:?}"
                ),
            });
        }
        Ok(ManifestDigest(s.to_string()))
    }

    /// Borrow the digest as a string slice (`sha256:<hex>` form).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ManifestDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for ManifestDigest {
    type Error = PackageError;

    fn try_from(s: String) -> Result<Self> {
        Self::parse(&s)
    }
}

impl From<ManifestDigest> for String {
    fn from(digest: ManifestDigest) -> String {
        digest.0
    }
}

/// Validated boundary conversion from an `oci_spec::image::Digest` (as
/// carried on a `Descriptor`) into this crate's [`ManifestDigest`].
///
/// Plan 05 uses this to turn an OCI-pulled `Descriptor`'s digest into a
/// `ManifestDigest` before calling [`crate::digest::verify`] — no bare
/// string cast crosses the OCI-interop trust boundary (T-168 threat
/// register: "oci_spec Descriptor digest → ManifestDigest").
impl TryFrom<&oci_spec::image::Digest> for ManifestDigest {
    type Error = PackageError;

    fn try_from(digest: &oci_spec::image::Digest) -> Result<Self> {
        if digest.algorithm() != &oci_spec::image::DigestAlgorithm::Sha256 {
            return Err(PackageError::MalformedDigest {
                reason: format!(
                    "expected sha256 algorithm, got: {}",
                    digest.algorithm()
                ),
            });
        }
        Self::parse(&format!("sha256:{}", digest.digest()))
    }
}

fn is_lowercase_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Serialize `value` via `olpc-cjson`'s canonical JSON formatter, returning
/// the exact bytes that should be digested or written to disk.
///
/// The same logical value always produces byte-identical output regardless
/// of struct field-declaration order or source-collection insertion order
/// (object keys are sorted at serialize time) — this is the whole point of
/// routing every digest computation through one function (RESEARCH.md
/// Pattern 2).
pub fn canonicalize<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut ser = Serializer::with_formatter(&mut buf, CanonicalFormatter::new());
    value.serialize(&mut ser).map_err(PackageError::Serialize)?;
    Ok(buf)
}

/// Canonicalize `value` then hash the canonical bytes — the struct-identity
/// digest path (as opposed to [`ManifestDigest::from_bytes`]'s raw-blob
/// path). `manifest_digest(&value) == manifest_digest(&value)` for the same
/// logical value (I-2 stability).
pub fn manifest_digest<T: Serialize>(value: &T) -> Result<ManifestDigest> {
    let bytes = canonicalize(value)?;
    Ok(ManifestDigest::from_bytes(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::BTreeMap;
    use std::str::FromStr;

    /// Field-declaration order deliberately does NOT match alphabetical
    /// order, to prove canonicalize() sorts independent of struct layout.
    #[derive(Serialize)]
    struct Sample {
        b: i32,
        a: String,
    }

    #[derive(Serialize)]
    struct MapSample {
        name: String,
        values: BTreeMap<String, i64>,
    }

    // --- canonicalize() byte-stability (I-2) ---

    #[test]
    fn canonicalize_is_byte_stable_across_repeated_calls() {
        let s = Sample { b: 1, a: "x".to_string() };
        let first = canonicalize(&s).unwrap();
        let second = canonicalize(&s).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn canonicalize_sorts_object_keys_independent_of_declaration_order() {
        // `Sample` declares `b` before `a`; canonical JSON must still emit
        // keys in sorted order.
        let s = Sample { b: 1, a: "x".to_string() };
        let bytes = canonicalize(&s).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert_eq!(text, r#"{"a":"x","b":1}"#);
    }

    proptest! {
        /// Regardless of the ORDER pairs are inserted into a BTreeMap, the
        /// canonical bytes for a struct containing that map are identical —
        /// the key-order-independence property I-2 requires.
        #[test]
        fn canonicalize_is_stable_regardless_of_map_insertion_order(
            mut pairs in prop::collection::vec(("[a-z]{1,8}", any::<i64>()), 0..8)
        ) {
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            pairs.dedup_by(|a, b| a.0 == b.0);

            let mut forward = BTreeMap::new();
            for (k, v) in pairs.iter() {
                forward.insert(k.clone(), *v);
            }
            let mut backward = BTreeMap::new();
            for (k, v) in pairs.iter().rev() {
                backward.insert(k.clone(), *v);
            }

            let sample_a = MapSample { name: "x".to_string(), values: forward };
            let sample_b = MapSample { name: "x".to_string(), values: backward };

            let bytes_a = canonicalize(&sample_a).unwrap();
            let bytes_b = canonicalize(&sample_b).unwrap();
            prop_assert_eq!(bytes_a, bytes_b);
        }
    }

    // --- ManifestDigest::from_bytes (raw blob path) ---

    #[test]
    fn from_bytes_yields_sha256_lowercase_hex_form() {
        let digest = ManifestDigest::from_bytes(b"hello world");
        let s = digest.as_str();
        assert!(s.starts_with("sha256:"));
        let hex_part = s.strip_prefix("sha256:").unwrap();
        assert_eq!(hex_part.len(), 64);
        assert!(hex_part.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
    }

    // --- ManifestDigest::parse (format validation) ---

    #[test]
    fn parse_accepts_well_formed_digest() {
        let hex = "a".repeat(64);
        let s = format!("sha256:{hex}");
        let digest = ManifestDigest::parse(&s).unwrap();
        assert_eq!(digest.as_str(), s);
    }

    #[test]
    fn parse_rejects_missing_prefix() {
        let err = ManifestDigest::parse(&"a".repeat(64)).unwrap_err();
        assert!(matches!(err, PackageError::MalformedDigest { .. }));
    }

    #[test]
    fn parse_rejects_wrong_length() {
        // "sha256:zz" — short, non-hex remainder.
        let err = ManifestDigest::parse("sha256:zz").unwrap_err();
        assert!(matches!(err, PackageError::MalformedDigest { .. }));
    }

    #[test]
    fn parse_rejects_non_hex() {
        let bad = format!("sha256:{}", "g".repeat(64));
        let err = ManifestDigest::parse(&bad).unwrap_err();
        assert!(matches!(err, PackageError::MalformedDigest { .. }));
    }

    #[test]
    fn parse_rejects_uppercase_hex() {
        let bad = format!("sha256:{}", "A".repeat(64));
        let err = ManifestDigest::parse(&bad).unwrap_err();
        assert!(matches!(err, PackageError::MalformedDigest { .. }));
    }

    // --- manifest_digest (canonicalize-then-hash path) ---

    #[test]
    fn manifest_digest_is_stable_across_repeated_calls() {
        let s = Sample { b: 1, a: "x".to_string() };
        let first = manifest_digest(&s).unwrap();
        let second = manifest_digest(&s).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn manifest_digest_canonicalizes_then_hashes() {
        let s = Sample { b: 1, a: "x".to_string() };
        let canonical_bytes = canonicalize(&s).unwrap();
        let expected = ManifestDigest::from_bytes(&canonical_bytes);
        let actual = manifest_digest(&s).unwrap();
        assert_eq!(actual, expected);
    }

    // --- serde: deserialize routes through parse(); serialize emits plain string ---

    #[test]
    fn deserialize_routes_through_parse() {
        let hex = "a".repeat(64);
        let json = format!("\"sha256:{hex}\"");
        let digest: ManifestDigest = serde_json::from_str(&json).unwrap();
        assert_eq!(digest.as_str(), format!("sha256:{hex}"));
    }

    #[test]
    fn deserialize_rejects_malformed_string_via_parse() {
        let result = serde_json::from_str::<ManifestDigest>("\"not-a-digest\"");
        assert!(result.is_err(), "malformed digest string must fail to deserialize");
    }

    #[test]
    fn serialize_emits_plain_sha256_hex_string() {
        let digest = ManifestDigest::from_bytes(b"hello");
        let json = serde_json::to_string(&digest).unwrap();
        assert!(json.starts_with("\"sha256:"));
        assert!(json.ends_with('"'));
        // Not a nested object — a bare JSON string.
        assert_eq!(json.matches('"').count(), 2);
    }

    // --- construct-only-by-validation invariant (mirrors path_validator.rs) ---

    #[test]
    fn manifest_digest_construction_only_via_parse_or_from_bytes() {
        // Compile-time gate: these are the ONLY public constructors — there
        // is no `ManifestDigest(String)` public tuple constructor and no
        // `#[serde(transparent)]` bypass.
        let digest_str = format!("sha256:{}", "0".repeat(64));
        let _from_parse: ManifestDigest = ManifestDigest::parse(&digest_str).unwrap();
        let _from_bytes: ManifestDigest = ManifestDigest::from_bytes(b"anything");
    }

    // --- OCI digest interop (validated boundary, T-168-oci) ---

    #[test]
    fn try_from_oci_digest_succeeds_for_sha256() {
        let hex = "b".repeat(64);
        let oci_digest = oci_spec::image::Digest::from_str(&format!("sha256:{hex}")).unwrap();
        let md = ManifestDigest::try_from(&oci_digest).unwrap();
        assert_eq!(md.as_str(), format!("sha256:{hex}"));
    }

    #[test]
    fn try_from_oci_digest_rejects_non_sha256_algorithm() {
        // sha512 requires a 128-hex-char value per the OCI digest spec.
        let hex = "c".repeat(128);
        let oci_digest = oci_spec::image::Digest::from_str(&format!("sha512:{hex}")).unwrap();
        let err = ManifestDigest::try_from(&oci_digest).unwrap_err();
        assert!(matches!(err, PackageError::MalformedDigest { .. }));
    }
}
