//! Phase 93 fuzz target — stress the quarantined `.xlsx` ZIP/XML provenance raw
//! reader against arbitrary byte sequences (T-93-02-DOS).
//!
//! The invariant: ANY input either yields a structured result/finding or a typed
//! [`pmcp_workbook_compiler::provenance::ProvenanceError`] — NEVER a panic, hang,
//! or unbounded allocation. The ZIP/XML hard limits in `raw_parts.rs`
//! (`MAX_ZIP_ENTRY_BYTES` / `MAX_TOTAL_DECOMPRESSED_BYTES` / `MAX_XML_DEPTH`) are
//! the guards; this target proves they hold over attacker-controlled bytes.
//!
//! The harness drives the reader through the compiler crate's
//! `#[cfg(fuzzing)] pub fn fuzz_read_parts` hook, so the raw reader stays
//! `pub(crate)`-quarantined on every non-fuzz build (it is never re-exported).
//!
//! Note: `cargo fuzz` requires a nightly toolchain (libfuzzer-sys uses
//! `-Z sanitizer`) and sets `--cfg fuzzing` for the whole build.
//!
//! Run with:
//! ```sh
//! cargo +nightly fuzz run fuzz_provenance_reader
//! ```
//!
//! Quick sanity smoke:
//! ```sh
//! cargo +nightly fuzz run fuzz_provenance_reader -- -runs=10000
//! ```

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // The hook drives BOTH read_calc_pr and read_app_props over the raw bytes.
    // It exists only under `--cfg fuzzing`; the result is intentionally ignored —
    // the assertion is "no panic / no hang / no unbounded allocation".
    pmcp_workbook_compiler::provenance::raw_parts::fuzz_read_parts(data);
});
