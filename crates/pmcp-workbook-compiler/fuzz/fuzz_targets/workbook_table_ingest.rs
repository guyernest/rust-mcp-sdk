//! Phase 100 fuzz target — prove the umya MALFORMED-TABLE-XML panic is CONTAINED
//! at the ingest `catch_unwind` seam (T-100-03 DoS), never a process abort.
//!
//! # The invariant this target ASSERTS
//!
//! `umya` parses every `xl/tables/tableN.xml` EAGERLY inside `reader::xlsx::read`
//! and `.unwrap()`s on malformed table XML (workbook-table-authoring-contract §2
//! caveat 1 / RESEARCH Pitfall 2). Task 1 wraps that eager read in
//! `std::panic::catch_unwind` (`ingest::read_workbook_contained`), mapping a caught
//! panic to a clean `IngestError::MalformedTable` (which `lib.rs` maps to
//! `CompileError::Ingest`). This target feeds arbitrary bytes (and, via the seed
//! corpus, a real xlsx whose `xl/tables/*.xml` parts are corrupted) into
//! `pmcp_workbook_compiler::ingest::ingest` and asserts the call ALWAYS returns a
//! `Result` and NEVER panics/aborts.
//!
//! If the containment seam were missing or broken, a malformed-table input would
//! abort inside `umya` and libfuzzer would record a crash — so this target is the
//! PROOF of the seam, not merely a hope that one exists (it is the SC1 fuzz Wave-0
//! gap from VALIDATION.md).
//!
//! The result is intentionally ignored (no `.unwrap()` that could mask a panic):
//! both `Ok` and `Err(CompileError/IngestError)` are acceptable — the ONLY failure
//! mode this target catches is a panic crossing the umya-isolation boundary.
//!
//! Run with:
//! ```sh
//! cargo +nightly fuzz run workbook_table_ingest -- -runs=20000 -max_total_time=60
//! ```
//!
//! A seed corpus entry (`corpus/workbook_table_ingest/malformed-table-xlsx`) is a
//! real xlsx whose three table parts are replaced with truncated garbage so the
//! corrupted-`tableN.xml` path is reached on the very first run, not just the
//! early zip-reject path.

#![no_main]

use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // ingest() reads from a path, so persist the fuzz bytes to a unique temp file
    // (unique per-iteration so concurrent/parallel fuzz workers never collide).
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "wbc-fuzz-table-ingest-{}-{n}.xlsx",
        std::process::id()
    ));

    // If we cannot even create the temp file, skip this input (an environment
    // problem, not a property violation).
    let Ok(mut f) = std::fs::File::create(&path) else {
        return;
    };
    if f.write_all(data).is_err() {
        let _ = std::fs::remove_file(&path);
        return;
    }
    // Ensure the bytes hit disk before umya opens the path.
    let _ = f.flush();
    drop(f);

    // THE ASSERTED INVARIANT: ingest() returns a Result (Ok or a typed Err) for
    // ANY bytes — a malformed table part MUST have been contained at the
    // catch_unwind seam, never a panic/abort. The result is deliberately discarded.
    let _ = pmcp_workbook_compiler::ingest::ingest(&path);

    let _ = std::fs::remove_file(&path);
});
