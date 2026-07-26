//! Fuzz target for the `subscriptions/listen` CLIENT frame decoder (HTTP-04).
//!
//! CLAUDE.md ALWAYS / FUZZ Testing: `cargo fuzz run subscription_listen_frames`.
//!
//! A listen stream's bytes come from a REMOTE server (or whatever intermediary
//! sits between), so the SSE tokenizing, incremental UTF-8 decoding, JSON-RPC
//! classification and `subscriptionId` validation in
//! `pmcp::client::subscriptions` all run on untrusted input.
//!
//! Invariants under arbitrary bytes:
//!   1. **Never panics** (T-113-67) — a hostile or merely broken frame must not
//!      take down a client, and the incremental UTF-8 buffer must not wedge on
//!      an invalid sequence.
//!   2. **Never cross-delivers** (T-113-66) — a frame is delivered to the caller
//!      only if it carried THIS subscription's id. The id used here is
//!      improbable enough that a delivery from bytes not containing it would be
//!      a real cross-tag escape, which is exactly the failure the server-side
//!      `ListenKey` pairing prevents and the client must not paper over.

#![no_main]

use libfuzzer_sys::fuzz_target;

/// The subscription id the decoder is told it owns. Deliberately improbable, so
/// "the input contained this literal" is a meaningful precondition.
const SUBSCRIPTION_ID: &str = "fuzz-subscription-4f1c9a2e";

fuzz_target!(|data: &[u8]| {
    // Invariant 1: arbitrary bytes in, no panic out.
    let outcomes = pmcp::client::subscriptions::decode_listen_chunk_for_fuzz(data, SUBSCRIPTION_ID);

    // Invariant 2: nothing is delivered from bytes that never named this
    // subscription.
    if outcomes.iter().any(std::result::Result::is_ok) {
        let text = String::from_utf8_lossy(data);
        assert!(
            text.contains(SUBSCRIPTION_ID),
            "a notification was delivered from a chunk that never carried this subscription's id"
        );
    }
});
