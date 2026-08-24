# Phase 121 — Deferred Items

Out-of-scope discoveries logged during execution. Per the executor scope boundary,
these were NOT fixed: they are pre-existing and not caused by this phase's changes.

## D1. Pre-existing clippy lints in dependency crates (found: 121-01 Task 3)

`cargo clippy -p pmcp-openapi-server --all-targets -- -D warnings` (plan 121-01
Task 3's literal verify command) exits **101**, but every failure is in a
DEPENDENCY crate, not in `pmcp-openapi-server`:

| Crate | Lint | Location | Count |
|-------|------|----------|-------|
| `mcp-tester` | `clippy::manual_filter` | `crates/mcp-tester/src/scenario_executor.rs:653,671` | 2 (errors under `-D warnings`) |
| `pmcp-server-toolkit` | `clippy::redundant_guard` | lib | 1 (warning) |

**Proven pre-existing:** `git diff --stat f3f55f3d..HEAD` shows plan 121-01 touches
only `Makefile`, `crates/pmcp-openapi-server/Cargo.toml`, and files under
`crates/pmcp-openapi-server/tests/`. Neither dependency crate is in this plan's
`files_modified`.

**Why it was never noticed:** `make lint` is `cargo clippy --features full --lib
--tests` with **no `-p`** (`Makefile:169`), so it resolves to the root `pmcp`
package only. Neither `mcp-tester` nor `pmcp-server-toolkit` is clippy-gated by
the repo gate or by CI, so a bare `-D warnings` run on them is STRICTER than
anything that gates a merge. These lints block nothing today.

**Status after 121-01:** `pmcp-openapi-server` itself generates **zero** clippy
warnings at `--all-targets` (verified: `cargo clippy -p pmcp-openapi-server
--all-targets` exits 0 with no `pmcp-openapi-server ... generated N warnings`
line). The plan's INTENT — this crate's own code clean at a bar stricter than the
repo gate — is met and proven.

**Follow-up:** a later simplify pass may apply the two `Option::filter` rewrites
in `mcp-tester` and the `redundant_guard` fix in `pmcp-server-toolkit`. Doing so
here would have widened a costly-reversibility task into two currently-green
crates outside its declared artifacts, for no PKG-04 benefit.

## D2. `contoso_m365_parity.rs` carries duplicate `fixtures_dir` / `examples_dir` (found: 121-01 Task 3)

`crates/pmcp-openapi-server/tests/contoso_m365_parity.rs` defines its own copies of
`fixtures_dir` (line 60) and `examples_dir` (line 66), which are now ALSO available
as `pub` items in `tests/common/mod.rs`.

Deliberately NOT collapsed: plan 121-01 D-02 names only `parity_replay.rs`'s
helpers, and widening a costly-reversibility extraction to a second currently-green
file buys no PKG-04 benefit. A later simplify pass should switch
`contoso_m365_parity.rs` to `mod common;` and delete the two local copies.

## D3. `SlotClass`'s own doc comment is stale in the SAME way `detect_deviation`'s was (found: 121-03 Task 3)

Plan 121-03 Task 3 corrected `detect_deviation`'s rustdoc, which claimed the
function fires only for the `LlmProvider` / `BudgetOverride` variants. While
confirming the evidence in `classify`, the identical staleness turned up ONE
level up, on the enum the corrected doc now delegates to:

`crates/pmcp-package/src/slot/classification.rs:12-14` — `SlotClass`'s
`BehaviorRelevant` variant doc reads *"`LlmProvider` / `BudgetOverride` — carries
a `tested_value`"*. Phase 120 made `Endpoint` and `AuthMode` behavior-relevant
too, so this enumeration is incomplete for the same reason and by the same
change. (`IdentityBearing`'s list is still accurate.)

**The code is correct; only the doc is wrong** — `classify` derives the family
from the single predicate `slot.tested_value().is_some()`
(`classification.rs:25-29`) and contains no variant list, which is precisely what
the corrected `detect_deviation` doc now says.

NOT fixed here: plan 121-03's `files_modified` is `roundtrip_e2e.rs` and
`deviation.rs` only. `classification.rs` is a third file in a
workspace-EXCLUDED crate, and editing it would widen this plan's artifact list
for zero PKG-04 benefit. A one-paragraph fix for whoever is next in that file;
it changes no behaviour.

---

# Deferred during GAP-CLOSURE planning (plans 121-04 / 121-05, 2026-08-24)

`121-VERIFICATION.md` recorded 2 BLOCKER gaps (CR-01, CR-02) plus 10 WARNING and 4 INFO
findings. Only the two BLOCKERs were planned — they are the only findings that falsify a
must_have. The WARNINGs below are real and correctly diagnosed, but none falsifies a ROADMAP
success criterion or a plan must_have as literally stated, and none is in a file plans 121-04
or 121-05 edit. Expanding a gap-closure plan to reach one would widen its declared artifact
list for zero PKG-04 benefit — the same reasoning as D1/D2/D3 above.

## D4. WR-01 — the "STRONG form" contrast comment overclaims (`roundtrip_e2e.rs`)

`roundtrip_endpoint_drift_is_reported`'s identity-bearing contrast comment claims more than the
assertion proves: there are three independent reasons `detect_deviation` returns `None` for two
different secrets, and only one of them is the short-circuit under test. **Doc accuracy, not a
false test result** — the assertion itself is correct and SC2 is verified. A one-paragraph
comment fix for whoever is next in that file.

## D5. WR-04 — the SC4 structural guard does not handle `/* */` block comments

The delimiter-balanced assertion-span scanner in `roundtrip_e2e_asserts_nothing_about_manifest_shape`
can self-blind if a future block comment contains an unpaired `"`. Confirmed by the verifier that
no block comment in today's file does, so the guard is compliant NOW. A future edit could silently
defeat it. Future-robustness gap in a currently-green guard.

## D6. WR-05 — the SC4 structural guard scans only the six `assert*!` macros

It does not scan `panic!` or `.expect(...)` call sites, so a deny-listed manifest-shape token
introduced through one of those would pass. Confirmed by the verifier that no such site currently
carries a deny-listed token. Same class as D5: real future-robustness gap, not a present defect.

## D7. WR-07 — `handle_a.abort()` is a request, not an awaited guarantee

`roundtrip_e2e.rs` aborts environment A's server task without awaiting the join handle, so A's
teardown is not ordered with respect to B's startup. Has never produced a false result because A
and B bind separate ephemeral ports (D-12) and run sequentially (D-10).

## D8. WR-08 — single-threaded safety is enforced only by the Makefile, not in-binary

The process-global `TFL_BASE_URL` / `TFL_APP_KEY` writes are safe only because
`Makefile:333` passes `--test-threads=1`. Nothing inside the test binary enforces it, so a direct
`cargo test -p pmcp-openapi-server` without the flag could interleave env-var writes. Plan 121-05
explicitly does NOT fix this and lists dropping `--test-threads=1` as a prohibition; the durable
fix is an in-binary serialization guard (a shared mutex, or the existing `tfl_env_lock` extended
to cover the round-trip tests). Flagged as edge-probe row 7 (`concurrency`) in both gap-closure
plans.

## D9. WR-09 — `serve_environment` never restores `TFL_*` after itself

It relies on being the last writer in a serialized binary. `EnvVarGuard` exists in
`tests/common/mod.rs` and is already used correctly by the 121-03 negative tests; routing
`serve_environment`'s writes through it would close this. Same root as D8.

## D10. INFO — `cargo package -p pmcp-openapi-server` fails locally on `pmcp-server-toolkit ^0.1.2`

Measured at gap-closure plan time: a local `cargo package -p pmcp-openapi-server --no-verify
--allow-dirty` fails with `failed to select a version for the requirement pmcp-server-toolkit =
"^0.1.2"` (`candidate versions found which didn't match: 0.1.1, 0.1.0`). This is **NOT a defect**
— `pmcp-server-toolkit` publishes at an earlier `release.yml` step, so 0.1.2 exists by the time
`pmcp-openapi-server` publishes. Recorded here because it masks every later requirement (cargo
reports the first unresolvable one and stops), which is why plan 121-04 Task 1 uses an isolated
de-versioned manifest copy instead of a bare dry-run. A future release engineer should not
mistake this for a regression.

## D11. Cross-reference — `scripts/check-release-coverage.sh` remains blind to workspace-excluded crates

`scripts/check-release-coverage.sh:14-16` records that root `cargo metadata --no-deps` cannot see
`pmcp-package` (its own `[workspace]` table), so no machine check covers its publish step or its
position in the order. CR-01 is a direct consequence: the ordering constraint it violated is
enforced by hand-maintained CLAUDE.md prose only. Plan 121-04 Task 3 writes the specific
constraint into the ledger at both ends but does NOT extend the check — **Phase 124 (PKGR-01)
already owns closing that blind spot** and this is not Phase 121's to take.
