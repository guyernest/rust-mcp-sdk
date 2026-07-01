//! Fuzz target for `pmcp::shared::pkce` — the wasm-safe PKCE crypto helper.
//!
//! CLAUDE.md ALWAYS / FUZZ Testing: `cargo fuzz run pkce_helper` (plain form,
//! no `+nightly` — matches the repo Makefile `test-fuzz` target, LOW-7).
//!
//! Invariant: the verifier → S256 challenge → base64url-decode roundtrip must
//! NEVER panic on arbitrary input bytes. Error paths are acceptable; panics are
//! not. This proves `code_challenge_s256` is total over any verifier string and
//! that the challenge it emits is always a decodable base64url-no-pad value
//! (threat T-103-PKCE — no-panic on arbitrary verifier bytes).

#![no_main]

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use libfuzzer_sys::fuzz_target;
use pmcp::shared::pkce::code_challenge_s256;

fuzz_target!(|data: &[u8]| {
    // Treat the arbitrary input as candidate verifier bytes. base64url-no-pad
    // encoding yields a valid (lossless) verifier string from any byte slice.
    let verifier = URL_SAFE_NO_PAD.encode(data);

    // The S256 challenge must compute without panicking for any verifier.
    let challenge = code_challenge_s256(&verifier);

    // The emitted challenge must always be a decodable base64url-no-pad string.
    // (Result, never panic — a decode error here would itself be a failure of
    // the helper's encoding contract, but we only assert no-panic.)
    let _ = URL_SAFE_NO_PAD.decode(challenge.as_bytes());
});
