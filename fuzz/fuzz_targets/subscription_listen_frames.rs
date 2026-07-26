//! Fuzz target for the `subscriptions/listen` CLIENT frame decoder (HTTP-04).
//!
//! CLAUDE.md ALWAYS / FUZZ Testing: `cargo +nightly fuzz run
//! subscription_listen_frames -- -runs=20000`. The `+nightly` is load-bearing —
//! `cargo fuzz run` passes `-Zsanitizer=address`, which stable rustc rejects.
//! The recorded campaign lives in `113-FUZZ-EVIDENCE.md`.
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
//!   3. **The overflow latch never clears** (T-113-73) — once the bounded parser
//!      has discarded an oversized line, `overflowed()` stays true for the rest
//!      of the stream. `read_next_frame` polls that flag once per body frame and
//!      ends the stream on the first `true`; if it could clear, a peer could
//!      hide a discarded line behind a subsequent well-formed one.

#![no_main]

use libfuzzer_sys::fuzz_target;

/// The subscription id the decoder is told it owns. Deliberately improbable, so
/// "the input contained this literal" is a meaningful precondition.
const SUBSCRIPTION_ID: &str = "fuzz-subscription-4f1c9a2e";

/// The line-buffer bound the campaign runs the parser at, i.e. the argument the
/// seam hands to `SseParser::with_max_buffer_size`.
///
/// DELIBERATELY tiny. Production bounds this path at 256 KiB
/// (`MAX_LISTEN_LINE_BYTES`), and a fuzzer that must synthesise a quarter of a
/// megabyte of newline-free input to reach the discard-and-latch branch would
/// effectively never reach it — the branch that manipulates buffer state on
/// hostile input would go unfuzzed. 64 bytes puts it within reach of the short
/// inputs libFuzzer actually generates, and the branch is bound-agnostic.
const MAX_BUFFER_SIZE: usize = 64;

/// How the input is sliced into successive "body frames".
///
/// A live listen stream is read incrementally, so the SSE line buffer and the
/// undecoded-UTF-8 tail both carry ACROSS chunks: splits land mid-character and
/// mid-line. Slicing at a fixed width keeps a crash artifact deterministic to
/// replay, and 16 bytes against a 64-byte bound means the bound is reached by
/// accumulation (several chunks) as well as by a single oversized chunk.
const CHUNK_LEN: usize = 16;

fuzz_target!(|data: &[u8]| {
    // A zero-length body frame is itself a case worth feeding, and `chunks()`
    // yields nothing for an empty slice.
    let chunks: Vec<&[u8]> = if data.is_empty() {
        vec![data]
    } else {
        data.chunks(CHUNK_LEN).collect()
    };

    // Invariant 1: arbitrary bytes in, no panic out.
    let (outcomes, overflowed) = pmcp::client::subscriptions::decode_listen_chunks_for_fuzz(
        &chunks,
        SUBSCRIPTION_ID,
        MAX_BUFFER_SIZE,
    );

    // Invariant 2: nothing is delivered from bytes that never named this
    // subscription.
    //
    // The precondition is checked against the RAW bytes, which is sound only
    // when no JSON escape could have spelled the id indirectly: a JSON string
    // of \u-escaped code points decodes to the id without the id's bytes ever
    // appearing literally, and asserting on that input would report a SPURIOUS
    // crash (verification finding WR-08). An input containing no backslash at
    // all cannot carry such an escape, so the literal check applies exactly
    // there — and an escape-spelled id is the SAME id anyway, not the cross-tag
    // escape this invariant exists to catch.
    let text = String::from_utf8_lossy(data);
    if outcomes.iter().any(std::result::Result::is_ok) && !text.contains('\\') {
        assert!(
            text.contains(SUBSCRIPTION_ID),
            "a notification was delivered from a chunk that never carried this subscription's id"
        );
    }

    // Invariant 3: `overflowed()` LATCHES. Once a line has been discarded the
    // stream has lost bytes; a later chunk must not be able to present it as
    // healthy again.
    let mut latched = false;
    for (index, seen) in overflowed.into_iter().enumerate() {
        assert!(
            seen || !latched,
            "overflowed() cleared at chunk {index} after latching — a discarded \
             line would be hidden from the stream-ending check"
        );
        latched |= seen;
    }
});
