//! Shared test support for the Phase-113 integration test files.
//!
//! Files under `tests/common/` are NOT compiled as their own test binaries, so
//! this is the correct home for helpers that several `tests/*.rs` files share.
//! Consume it with `mod common;` from a test binary.

pub mod v2;
