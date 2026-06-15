//! ALWAYS example (CLAUDE.md, NO EXCEPTIONS): the WBDL-02 dialect-version
//! compatibility decision over the three outcomes that matter.
//!
//! This uses ONLY the crate's PUBLIC API
//! ([`pmcp_workbook_compiler::dialect_version::parse_dialect_version`] +
//! [`pmcp_workbook_compiler::dialect_version::validate_declared`]) — pure
//! computation, no `.xlsx`, no filesystem. It demonstrates:
//!
//!   (a) ABSENT declaration → resolves to the BASELINE version (accepted);
//!   (b) a COMPATIBLE declared version (`1.0`) → accepted;
//!   (c) an INCOMPATIBLE declared version (`2.0` different major, `1.5` newer
//!       minor) → the typed fail-closed `CompileError`.
//!
//! The incompatible case is an EXPECTED, printed `Err` — it is matched and
//! printed, NOT propagated as a non-zero exit. The process exits 0.
//!
//! Run with: `cargo run -p pmcp-workbook-compiler --example dialect_version_demo`

use pmcp_workbook_compiler::dialect_version::{parse_dialect_version, validate_declared};
use pmcp_workbook_dialect::BASELINE_DIALECT_VERSION;

fn main() {
    println!("WBDL-02 dialect-version compatibility demo\n");

    // (a) ABSENT declaration → baseline. We model "absent" the way the pipeline's
    // resolve does: no cell means the baseline version, which is always accepted.
    let baseline = parse_dialect_version(BASELINE_DIALECT_VERSION)
        .expect("the baseline const is a well-formed version");
    println!(
        "(a) absent declaration -> baseline {BASELINE_DIALECT_VERSION} \
         (parsed {}.{}) -> accepted",
        baseline.major(),
        baseline.minor(),
    );

    // (b) COMPATIBLE declared version → accepted.
    match validate_declared("1.0") {
        Ok(v) => println!(
            "(b) declared `1.0` -> accepted (parsed {}.{})",
            v.major(),
            v.minor()
        ),
        Err(e) => println!("(b) declared `1.0` -> UNEXPECTED error: {e}"),
    }

    // (c) INCOMPATIBLE declared versions → the typed fail-closed CompileError,
    // matched and printed (an EXPECTED Err, not a process failure).
    for declared in ["2.0", "1.5"] {
        match validate_declared(declared) {
            Ok(v) => println!(
                "(c) declared `{declared}` -> UNEXPECTEDLY accepted (parsed {}.{})",
                v.major(),
                v.minor()
            ),
            Err(e) => println!("(c) declared `{declared}` -> typed fail-closed error: {e}"),
        }
    }

    println!("\ndemo complete (incompatible cases are expected, printed errors)");
}
