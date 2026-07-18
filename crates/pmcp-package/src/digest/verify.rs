//! Digest tamper-detection primitive (I-1/I-2 core).
//!
//! # Threat Model
//!
//! Bytes claimed to match a digest (a stored blob re-read from disk, or —
//! in a future phase — a registry-pulled blob) must be re-hashed before
//! trust is extended to them. `verify()` is that gate: it recomputes
//! [`ManifestDigest::from_bytes`] over the EXACT bytes given — never a
//! re-derived serialization of a parsed struct (RESEARCH.md anti-pattern:
//! "digest what you actually stored, never re-derive from the parsed
//! struct") — and compares against the caller-supplied `expected` digest.
//! A single-byte difference between the actual and expected bytes is
//! detected and reported as a structured [`PackageError::DigestMismatch`].
//!
//! Plan 05 obtains `expected` from an OCI `Descriptor`'s digest via
//! `ManifestDigest::try_from(&descriptor_digest)` (see
//! `crate::digest::canonical`) before calling this function — the OCI
//! interop boundary is validated there, not here; `verify()` only ever
//! operates on already-typed [`ManifestDigest`] values.

use crate::digest::canonical::ManifestDigest;
use crate::error::{PackageError, Result};

/// Recompute the digest of `bytes` (raw, as given) and compare against
/// `expected`. Returns `Ok(())` on a match, or
/// `Err(PackageError::DigestMismatch { expected, actual })` on any
/// difference — including a single-byte flip.
pub fn verify(expected: &ManifestDigest, bytes: &[u8]) -> Result<()> {
    let actual = ManifestDigest::from_bytes(bytes);
    if &actual == expected {
        Ok(())
    } else {
        Err(PackageError::DigestMismatch {
            expected: expected.as_str().to_string(),
            actual: actual.as_str().to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_succeeds_when_bytes_match_expected_digest() {
        let bytes = b"hello world";
        let digest = ManifestDigest::from_bytes(bytes);
        assert!(verify(&digest, bytes).is_ok());
    }

    #[test]
    fn verify_detects_single_byte_tamper() {
        let original = b"hello world".to_vec();
        let digest = ManifestDigest::from_bytes(&original);
        let mut tampered = original.clone();
        tampered[0] ^= 0x01; // flip a single bit — content differs by one byte
        let err = verify(&digest, &tampered).unwrap_err();
        assert!(matches!(err, PackageError::DigestMismatch { .. }));
    }

    #[test]
    fn verify_mismatch_error_carries_both_digests() {
        let original = b"content-a".to_vec();
        let digest = ManifestDigest::from_bytes(&original);
        let different = b"content-b".to_vec();
        match verify(&digest, &different) {
            Err(PackageError::DigestMismatch { expected, actual }) => {
                assert_eq!(expected, digest.as_str());
                assert_ne!(actual, expected);
            }
            other => panic!("expected DigestMismatch, got {other:?}"),
        }
    }
}
