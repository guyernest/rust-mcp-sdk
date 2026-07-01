//! ALWAYS coverage for the wasm-safe PKCE crypto helper (`pmcp::shared::pkce`).
//!
//! Covers the four WEBCH-01 validation rows:
//!   1. `pkce_rfc7636_vector`        — RFC 7636 Appendix B published vector (correctness)
//!   2. `pkce_verifier_charset_len`  — every verifier is 43 chars, base64url-no-pad charset
//!   3. `pkce_challenge_deterministic` — same verifier always yields the same S256 challenge
//!   4. `pkce_base64url_roundtrip`   — base64url encode→decode is identity and never panics
//!
//! Tests reference the helper through its public re-export path
//! (`pmcp::shared::pkce::*` plus the crate-root convenience re-export) so they
//! also exercise the public API surface shipped in this release.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use pmcp::shared::pkce::{code_challenge_s256, generate_code_verifier, generate_state};
use proptest::prelude::*;

/// (1) RFC 7636 Appendix B vector — pins S256 correctness against the published
/// verifier/challenge pair so a degenerate or hand-rolled digest is caught.
#[test]
fn pkce_rfc7636_vector() {
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    let challenge = code_challenge_s256(verifier);
    assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
}

/// The crate-root re-export resolves to the same helper as the module path.
#[test]
fn pkce_crate_root_reexport_resolves() {
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    assert_eq!(
        pmcp::code_challenge_s256(verifier),
        code_challenge_s256(verifier),
    );
}

proptest! {
    /// (2) Charset + length: every generated verifier is exactly 43 characters
    /// and uses only the base64url-no-pad unreserved charset `[A-Za-z0-9_-]`.
    /// A degenerate RNG (or a wrong byte count) is detectable here (T-103-RNG).
    #[test]
    fn pkce_verifier_charset_len(_seed in any::<u64>()) {
        let verifier = generate_code_verifier().expect("CSPRNG available on host");
        prop_assert_eq!(verifier.len(), 43);
        prop_assert!(
            verifier
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
            "verifier must use only base64url-no-pad unreserved chars: {}",
            verifier
        );
        // `generate_state` shares the verifier's entropy source/shape.
        let state = generate_state().expect("CSPRNG available on host");
        prop_assert_eq!(state.len(), 43);
    }

    /// (3) Determinism: `code_challenge_s256(v)` is a pure function of `v`.
    #[test]
    fn pkce_challenge_deterministic(v in ".*") {
        prop_assert_eq!(code_challenge_s256(&v), code_challenge_s256(&v));
        // The challenge itself is always a valid base64url-no-pad string.
        prop_assert!(
            code_challenge_s256(&v)
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        );
    }

    /// (4) base64url roundtrip: encode→decode is identity for arbitrary bytes
    /// and never panics — the encoding contract the helper relies on.
    #[test]
    fn pkce_base64url_roundtrip(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
        let encoded = URL_SAFE_NO_PAD.encode(&bytes);
        let decoded = URL_SAFE_NO_PAD
            .decode(encoded.as_bytes())
            .expect("URL_SAFE_NO_PAD must decode its own output");
        prop_assert_eq!(decoded, bytes);
    }
}
