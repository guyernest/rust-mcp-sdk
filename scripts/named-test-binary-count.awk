# Extract the PASSED-test count for ONE named cargo test binary.
#
# Usage:
#   awk -v want="tests/roundtrip_e2e.rs" -f scripts/named-test-binary-count.awk <captured-cargo-output>
#
# Prints exactly one integer:
#
#   N >= 1  the named binary ran and passed N tests
#   0       the named binary ran and passed ZERO tests
#   -1      no `Running <want>` line was found — the binary never ran at all
#   -2      a `Running <want>` line was found but no `test result:` line followed
#           it — truncated output or an aborted harness
#
# ---------------------------------------------------------------------------
# Why FIELD EQUALITY and not a substring or a regex
# ---------------------------------------------------------------------------
# The guard this replaces (phase 121 review finding CR-02) asked only whether
# the string `tests/<name>.rs` appeared ANYWHERE in the captured output. Cargo
# prints that path in the target line for every binary it executes, and rustc
# repeats it in every diagnostic emitted for that file (`--> tests/x.rs:12:5`);
# `test-openapi-server` sets no `-D warnings`, so a lone warning satisfied the
# old check. Under awk's default field splitting the three shapes separate
# cleanly and without ambiguity:
#
#   `     Running tests/x.rs (target/debug/deps/x-<hash>)`  -> $1 = Running,  $2 = tests/x.rs
#   `  --> tests/x.rs:12:5`                                 -> $1 = -->
#   `     Running unittests src/lib.rs (...)`               -> $2 = unittests
#
# A regex would also have to escape the `.` characters in `tests/<name>.rs`,
# which an unescaped pattern treats as wildcards — a live hazard that a plain
# `$2 == want` string comparison removes entirely. It additionally keeps two
# binaries whose names share a prefix (`roundtrip_e2e` vs a future
# `roundtrip_e2e_extra`) from matching each other, which a substring cannot.
#
# ---------------------------------------------------------------------------
# Why the PASSED count and not the `running N tests` line
# ---------------------------------------------------------------------------
# Measured in this repo against a single `#[test] #[ignore]` binary:
#
#   running 1 test
#   test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
#
# An all-ignored suite prints `running 1 test`, so a nonzero-`running` check
# passes it while nothing executed. Only field 4 of the `test result:` line —
# the passed count — reports 0 there. Field 4 holds the passed count for both
# the `ok.` and the `FAILED.` forms of that line, so one field works for both.
# `scripts/run-era-matrix.sh` gates on `running [1-9]`, which is correct for ITS
# failure mode (a `#![cfg]`-emptied file, which prints `running 0 tests`) but
# would sail past an ignore sweep; this extractor must not inherit that.
#
# ---------------------------------------------------------------------------
# Why `exit` on the FIRST result line after the match
# ---------------------------------------------------------------------------
# Cargo's doctest trailer emits a bare `test result:` block with NO preceding
# `Running tests/...` line. Without stopping at the first result line after the
# target line, a later unrelated block could be attributed to this binary.
#
# ---------------------------------------------------------------------------
# Why -2 exists rather than printing nothing
# ---------------------------------------------------------------------------
# The obvious spelling of this program prints an EMPTY string when the target
# line was seen but no result line followed. An empty string flows into a shell
# numeric comparison as `[: : integer expression expected`, which returns a
# non-zero status that an `if` reads as false — so the guard would pass
# vacuously in exactly the case where the output cannot be trusted. A gate that
# cannot see must say so, never pass.

$1 == "Running" && $2 == want { seen = 1; next }

seen && $1 == "test" && $2 == "result:" {
    print $4 + 0
    printed = 1
    exit
}

END {
    if (!printed) {
        if (seen) {
            print -2
        } else {
            print -1
        }
    }
}
