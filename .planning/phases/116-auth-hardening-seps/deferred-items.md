# Phase 116 — Deferred Items

Out-of-scope discoveries logged during execution. Each names the plan that found it and a
proposed owner. Nothing here was fixed by the finding plan — that is the point of the file.

---

## D-116-EX — No plan in Phase 116 owns CLAUDE.md's ALWAYS-**EXAMPLE** requirement

**Found during:** `116-02` (Task 2), while checking CLAUDE.md's "ALWAYS Requirements for New
Features" against the phase's plan set.

**Finding.** CLAUDE.md § "ALWAYS Requirements for New Features (MANDATORY)" lists four
non-negotiables for every new feature: FUZZ, PROPERTY, UNIT and **EXAMPLE**
(`cargo run --example feature_name`, "must include real-world usage scenario"). Three of the four
have a named owner in this phase:

| ALWAYS requirement | Owner | Status |
|---|---|---|
| PROPERTY | `116-02` (`tests/oauth_iss_validation.rs`, four RFC-derived proptest blocks) | done |
| UNIT | `116-02` (27 integration + 8 inline) and every later source plan | in progress |
| FUZZ | `116-08` — `fuzz/fuzz_targets/oauth_authorization_response.rs` names `validate_authorization_response` explicitly | planned |
| **EXAMPLE** | **none** | **unowned** |

Measured: `grep -n 'examples/' .planning/phases/116-auth-hardening-seps/116-*-PLAN.md` returns one
hit, in `116-01`, and it refers to the pre-existing `examples/s51_v2_tasks_agent.rs` build failure
recorded in the baselines — not to authoring an example.
`grep -n 'cargo run --example\|EXAMPLE demonstration\|examples/oauth'` across all sixteen plans
returns **zero** hits. No plan's `files_modified` names anything under `examples/`.

**Why `116-02` did not just add one.** The plan's Task 2 `<files>` list is explicit and closed
(`src/shared/oauth_validation.rs`, `src/shared/mod.rs`, `src/lib.rs`,
`tests/oauth_iss_validation.rs`). An unowned example is neither a bug in this plan's output, nor
missing critical functionality for correctness/security, nor a blocker to completing the task — so
it is out of scope under the executor's scope boundary rather than an auto-fix.

**What partially discharges it today.** The module ships five executable rustdoc `# Examples`
doctests (module-level end-to-end, plus `AuthorizationRequestRecord::new`,
`validate_authorization_response`, `parse_iss_env_value` and `iss_presence_from`), all passing
under `cargo test --features full,oauth --doc oauth_validation`. Those are runnable demonstrations,
but they are not `cargo run --example`, and `make validate-always`'s `test-examples` step does not
reach them.

**Proposed owner:** `116-15` (the phase-closing plan, which already owns the ALWAYS/A9 evidence
roll-up) or `116-13`. The natural artifact is a single `examples/` binary that walks a complete
hardened flow — build the record, validate a good callback, then show the four typed refusals
(`is_iss_mismatch`, `is_state_mismatch`, duplicate parameter, oversize query) — since it needs no
network and no `oauth` feature and would therefore also serve as the phase's user-facing README
snippet.

**Do not book "ALWAYS requirements satisfied" for Phase 116 until this is closed or explicitly
waived in writing.**

### RESOLVED — `116-08` owned it all along, and has now discharged it

The finding's own table names `116-08` as the FUZZ owner but reads its `files_modified` only for
`fuzz/`. That plan's fourth entry is **`examples/c11_oauth_iss_state_validation.rs`**, and its
Task 3 requires the file to be RUNNABLE, not merely to compile. So the EXAMPLE row was owned; the
`grep -n 'cargo run --example'` that returned zero hits missed it because `116-08`'s Task 3 phrases
the command inside the artifact's own module doc rather than in the plan's prose, and the
`grep -n 'examples/'` in this entry was run against `116-*-PLAN.md` *bodies* — the hit is in
`116-08`'s YAML **frontmatter**.

Discharged by `8b41f7b0`, and measured rather than asserted:

| Obligation | Evidence |
|---|---|
| `cargo run --example c11_oauth_iss_state_validation` | **exit 0**, with NO feature flags |
| the same with `--features full,oauth` | **exit 0**, byte-identical stdout |
| "real-world usage scenario" | four labelled scenarios that EXECUTE the shipped logic: accept, `iss` mismatch, `state` mismatch, and the D-04 precedence resolution including the advertised-but-absent fatal row |
| reachable by `make validate-always`'s `test-examples` step | the file is under `examples/`, so the step's `ls examples/*.rs` loop picks it up |

The ALWAYS table for Phase 116 now reads:

| ALWAYS requirement | Owner | Status |
|---|---|---|
| PROPERTY | `116-02`, `116-04`, `116-05` | done |
| UNIT | `116-02` and every later source plan | in progress |
| FUZZ | `116-08` — `oauth_authorization_response` (AUTH-01/AUTH-03) + `oauth_credential_and_dcr` (AUTH-02/AUTH-03), plus the `dcr_response_parser` extension | **done** |
| **EXAMPLE** | **`116-08` — `examples/c11_oauth_iss_state_validation.rs`** | **done** |

**One ALWAYS gap remains, and it is deliberately NOT this one:** the bounded-read cap BOUNDARY
cannot be fuzzed purely, because it needs a `reqwest::Response`. It is covered instead by the
exactly-at-cap / one-under / one-over mockito triple in `116-06` Task 1. Recorded here so the
ALWAYS audit has one place to look.

**Owner:** closed by `116-08`. `116-15` may cite this section rather than re-deriving it.

---

## D-116-DOC — `make doc-check`'s 28-error baseline is fragile against outer-doc'd modules

**Found during:** `116-02` (Task 2). Recorded because the next two plans (`116-04`, `116-05`) create
`src/shared/` modules the same way and will hit the same trap.

**Finding.** A module that carries BOTH an outer `///` rationale on its `pub mod` declaration in
`src/shared/mod.rs` (which the plans require, so nobody "tidies" a `cfg` onto it) AND an inner `//!`
module doc has its merged documentation resolved in the **declaring** module's scope. Every
unqualified intra-doc link in the inner block then fails with "no item named `X` in scope", and
`make doc-check` runs `RUSTDOCFLAGS="-D warnings"`, so each one is a hard error.

`116-02` added four such errors on its first pass (28 → 32): three unqualified `IssPresence*` links
in the module-doc table, plus one link to a genuinely non-existent path (`crate::client::OAuthConfig`
— the type lives at `crate::client::oauth::OAuthConfig` behind a feature gate). Both were fixed in
the same task and the count returned to exactly 28, but only because the plan happened to require
running `doc-check`.

**Guidance for `116-04` and `116-05`:** fully qualify every intra-doc link in an inner `//!` block
of a module whose declaration carries an outer `///`, and do not link items that live behind a
feature gate the ungated module must not assume — use a plain code span instead. Run `make doc-check`
and diff the `^error` count against 28 **before** committing, not after.

**Proposed owner:** informational; no fix required. `116-15` may wish to fold the rule into the
phase's written conventions.

---

## D-116-LINT — the PMAT write-workflow clause (b) clippy command is WEAKER than `make lint`

**Found during:** `116-03` (Task 1). Measured, not reasoned: clause (b) reported **exit 0** on code
that `make lint` — and therefore `make quality-gate` — rejected with a hard error.

**Finding.** `116-BASELINES.md` § "PMAT Quality-Proxy Write Workflow" clause (b) prescribes:

```
cargo clippy --features full,oauth --lib --tests -- -D clippy::all -W clippy::pedantic -W clippy::nursery
```

`make lint` (`Makefile`) prescribes something materially different:

```
RUSTFLAGS="-D warnings" cargo clippy --features "full" --lib --tests -- -D clippy::all \
    -W clippy::pedantic -W clippy::nursery -W clippy::cargo  <28 × -A clippy::…>
```

Two divergences, pulling in **opposite** directions, so neither command dominates the other:

1. **`RUSTFLAGS="-D warnings"`** promotes every `-W` pedantic/nursery lint to a hard error. Clause (b)
   omits it, so those lints only *warn* and clippy still exits 0. This is the direction that bites:
   `116-03`'s first pass wrote a two-arm `match` in a test whose `Err` arm was empty. Clause (b):
   exit 0, zero hits in the file. `make lint`: `error: clippy::single_match_else`, `make[1]: ***
   [lint] Error 101`, gate red.
2. **`make lint`'s 28-entry `-A` allow-list** (`must_use_candidate`, `uninlined_format_args`,
   `option_if_let_else`, `too_many_lines`, `redundant_closure_for_method_calls`, …). Adding
   `RUSTFLAGS="-D warnings"` to clause (b) *without* that allow-list produces **11 errors in
   `crates/pmcp-widget-utils/src/lib.rs` and `crates/pmcp-code-mode-derive/src/lib.rs`** — every one
   of them a lint `make lint` explicitly allows, in workspace crates the real gate does not lint at
   pedantic strength. Measured at `target/116-verify/116-03-clippy-strict.log`
   (`STRICT_CLIPPY_EXIT=101`, **0** hits in `pmcp` and **0** in any file `116-03` touched). This is
   the pre-existing condition already recorded in project memory: a bare `-D warnings` run over the
   non-root workspace crates is stricter than the gate and does **not** block CI.

**Consequence for the remaining plans.** A plan that runs only clause (b) and books "clippy clean"
can still be rejected by the pre-commit hook and by CI. Clause (b) is a fast *inner-loop* check, not
the gate. **`make lint` (or the full `make quality-gate`) is the authoritative clippy evidence and
must be run before any source-touching task is booked done.** `116-03` did run it, which is the only
reason the defect was found before push.

**Proposed owner:** `116-15`, when it reconciles the phase's evidence. Two options: amend the clause
in `116-BASELINES.md` to name `make lint` as the authoritative form with clause (b) demoted to an
inner-loop convenience, or leave clause (b) as written and add a standing "then run `make lint`"
step. Do **not** "fix" this by adding `RUSTFLAGS="-D warnings"` to clause (b) alone — divergence 2
shows that produces 11 false positives in crates this phase does not own.

---

## D-116-DISK — `make quality-gate`'s doctest stage fails 12 tests when the disk is near-full

**Found during:** `116-03` (Task 1), running the plan's `make quality-gate` verification.

**Finding.** `make quality-gate` reported `test-doc` **FAILED: 416 passed; 12 failed** with
`error: linking with 'cc' failed: exit status: 1` in **twelve files `116-03` never touched**
(`src/server/mod.rs`, `observability/types.rs`, `preset.rs`, `resource_watcher.rs`,
`simple_resources.rs`). The visible output is thousands of lines of
`ld: warning: object file … was built for newer 'macOS' version (26.5) than being linked (11.0)`,
which reads exactly like a toolchain/code regression and is not the cause.

The actual error, recoverable only by filtering the warnings out
(`grep -oE 'ld: [a-z].*' … | grep -v '^ld: warning'`):

```
12 × ld: write() failed, errno=28 (No space left on device)
```

`df -h /` at that moment: **1.3 GiB available, 91% capacity**, with `target/` at **84 GB**
(`target/debug/deps` 34 GB, `target/debug/incremental` 33 GB, `target/debug/examples` 14 GB).

**Resolution applied (not a code change):** `rm -rf target/debug/incremental target/semver-checks
target/wasm32-unknown-unknown` → 37 GiB free. Re-run: `test-doc` **428 passed; 0 failed; 79
ignored** — exactly `416 + 12` — and `make quality-gate` **exit 0**.

**Guidance for every later plan in this phase.** `make quality-gate` links ~430 doctest binaries
against a ~180 MB rlib set, so it is the single most disk-hungry step in the repo. **Run
`df -h /` before treating any `linking with 'cc' failed` as a code defect**, and filter
`ld: warning` lines out before reading the diagnostic. `target/debug/incremental` is the cheapest
33 GB to reclaim; do not `cargo clean` (it discards the whole 84 GB and costs a full rebuild).

**Proposed owner:** informational; no fix required. This is the project-memory
"disk exhaustion fakes code regressions" hazard, hit again and now measured inside Phase 116.

---

## D-116-KEYCHAIN — `make test-unit` fails 14 `streamable_http` tests on a macOS keychain error, at HEAD and before it

**Found during:** `116-04` (Task 2), running the plan's `<verification>` requirement
`make quality-gate`. **Attributed by measurement, not by argument** — see below.

**Finding.** `make test-unit` (`Makefile:216-219`, plain `cargo test --lib --features "full"`)
reports **14 failed** out of 1844. Every one is in `shared::streamable_http::tests`, every one
panics at the *same* pre-existing line, and every one carries the *same* cause:

```
thread '…' panicked at src/shared/streamable_http.rs:458:18:
Failed to load native root certificates: Custom { kind: NotFound, error:
  "no native root CA certificates found (errors: [
     Error { context: \"failed to load user trust settings\",   kind: Os(Error { code: -36, message: \"I/O error.\" }) },
     Error { context: \"failed to load admin trust settings\",  kind: Os(Error { code: -36, message: \"I/O error.\" }) },
     Error { context: \"failed to load system trust settings\", kind: Os(Error { code: -36, message: \"I/O error.\" }) }])" }
```

`src/shared/streamable_http.rs:458` is an `.expect()` on `rustls-native-certs`' `load_native_certs`.
macOS `ioErr` (`-36`) is a generic I/O failure reading the keychain trust settings.

**The decisive measurement.** `116-04`'s own source change was reverted in place
(`git checkout 119eeaea~1 -- src/shared/oauth_validation.rs`) and the identical command re-run:

| Tree | Result |
|---|---|
| with `116-04` Task 1 + Task 2 | `1830 passed; 14 failed` |
| **`src/shared/oauth_validation.rs` reverted to its pre-plan (116-02) content** | **`1826 passed; 14 failed`** |

The passing count differs by exactly **4** — this plan's four new inline tests — and the failing
set is **identical**. The 14 failures therefore predate `116-04` and are not attributable to it.
Log: `target/116-verify/116-04-preplan-testunit.log`. Source restored byte-for-byte afterwards
(`shasum -a 256 -c` → `OK`).

**It is also flaky, not deterministic.** The same 14 tests were observed passing twice in the same
session with the same source: `cargo test --lib --features full shared::streamable_http` →
**33 passed, 0 failed**, and one full `make test-unit` run → **1844 passed; 0 failed** (= 1830 + 14).
Two consecutive unfiltered runs then reproduced the failure
(`target/116-verify/116-04-keychain-repro.log`). Disk was **not** the trigger this time
(29 GiB free at 29% capacity when it reproduced), and `ulimit -n` is **1048576**, so neither
`D-116-DISK` nor descriptor exhaustion explains it. The remaining correlate is concurrency: the
failure appears when the whole 1844-test `--lib` binary runs with the default thread count and
not when a 33-test subset does.

**Why this matters beyond one red run.** `CLAUDE.md` § *Development Workflow* states that
"Tests run with `--test-threads=1` (race condition prevention)" — but `make test-unit` does **not**
pass that flag. So the documented CI invariant and the Makefile disagree, and a developer running
`make quality-gate` locally can get a red gate from a pre-existing, environment-dependent panic in
code they never touched. That is the same class of false signal as `D-116-DISK`, with a different
symptom.

**Why `116-04` did not fix it.** It is outside the executor's scope boundary: not a defect in this
plan's output, not missing functionality for its correctness or security, and not a blocker to
completing its tasks — the plan's own suites, `make lint`, `make doc-check`, `pmat quality-gate`,
`cargo semver-checks`, the `wasm32` build and **every other `make quality-gate` stage**
(`fmt-check`, `lint`, `build`, `pmcp-package-gate`, `audit`, `unused-deps`, `check-todos`,
`check-unwraps`, `purity-check`, `comply`) all pass. Fixing it means either changing
`streamable_http.rs`'s `.expect()` into a fallible path or changing the Makefile's test invocation,
both of which are edits to subsystems this phase does not own.

**Proposed owner:** `116-15`, with two candidate resolutions:
1. Align `make test-unit` with the documented CI behaviour by adding `-- --test-threads=1`, which
   would also close the CLAUDE.md/Makefile divergence; or
2. Make `src/shared/streamable_http.rs:458` fall back to a bundled root store (or skip the
   affected tests) instead of `.expect()`-ing on the platform keychain — an `.expect()` on an OS
   trust-store read is a panic in library code reachable from any consumer on a machine whose
   keychain is momentarily unreadable.

**Until then: run `df -h /`, then re-run the failing subset in isolation before treating a
`streamable_http` keychain panic as a regression.**

### RESOLVED by measurement — `116-06`, on a CLEAN volume: 0 failures

`116-06` was the first plan in this phase to run after the orchestrator deleted `target/`
entirely, with **71 GiB free at 15% capacity**. `/usr/bin/make quality-gate` reported:

```
running 1865 tests
test result: ok. 1865 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.98s
✓ Unit tests passed
```

Two independent greps over the whole gate log confirm the mechanism did not merely go quiet:
`grep -c "streamable_http.rs:4"` → **0**, and
`grep -c "Failed to load native root certificates"` → **0**.

**The arithmetic closes exactly, which is what makes this attributable rather than lucky.**

| Plan | Volume state when measured | Total | Passed | Failed |
|---|---|---|---|---|
| `116-04` | filling (29 GiB → 132 Mi across the session) | 1844 | 1830 | **14** |
| `116-05` | full twice (132 Mi, then 532 Mi at 96–99%) | 1849 | 1836 | **13** |
| `116-06` | **clean: 71 GiB free at 15%** | 1865 | **1865** | **0** |

`1849 + 16 = 1865`, and 16 is exactly this plan's new inline test count (10 in
`src/shared/http_body_cap.rs`, plus `src/client/auth.rs` going from 5 inline tests to 11). So the
population grew by precisely this plan's contribution and the 13 failures **disappeared** — they
were not renamed, filtered or skipped.

**Conclusion: `D-116-KEYCHAIN` is an ENVIRONMENT ARTIFACT, not a defect in the tree.** It belongs
with `D-116-DISK`, not beside it: macOS `ioErr (-36)` reading keychain trust settings is a generic
I/O failure, and a volume at 96–99% is an I/O failure waiting to be reported by whichever syscall
asks first. `116-04`'s note that "disk was not the trigger this time (29 GiB free at 29%)" measured
the volume at ONE instant during a session in which `make quality-gate` links ~430 doctest binaries
and `target/debug/incremental` regrew 21–33 GB; `116-05` then measured 132 Mi twice in the same
session. The apparent flakiness (observed passing 3× in `116-04`) is what an intermittent
disk-pressure condition looks like.

**Revised guidance, replacing the two candidate resolutions above.** Neither is required:

1. Do **NOT** change `src/shared/streamable_http.rs:458` on this evidence. The `.expect()` on
   `load_native_certs` is still worth revisiting on its own merits — an `.expect()` on an OS
   trust-store read is a panic in library code — but it is not the cause of the red gate this
   phase kept seeing, and "fixing" it would have masked the real one.
2. The `--test-threads=1` divergence between `CLAUDE.md` and `make test-unit` is real and still
   worth closing, but it is **not** what was failing: the full 1865-test binary ran with the
   default thread count here and passed.
3. **Run `df -h /` before every `make quality-gate`, and again before believing any failure in a
   subsystem the plan did not touch.** This is the same rule `D-116-DISK` already states; this
   entry is now its second, differently-shaped symptom.

Note for `116-15`: the gate consumed **42 GiB** during this single run (71 GiB free → 29 GiB free).
A plan that runs it two or three times will re-enter the failure regime, so the finding is
reproducible in both directions.

**Owner:** `116-15` may close this entry citing the measurement above, or fold it into
`D-116-DISK`. No source change is owed.

---

## D-116-FAILFAST — `cargo nextest run` truncates a negative control, and the truncation looks like a result

**Found during:** `116-05` (Task 2), running the plan's three-break negative control.

**Finding.** `cargo nextest run` **fail-fast is ON by default**. The first negative-control run
reported:

```
Summary [0.025s] 15/54 tests run: 10 passed, 5 failed, 0 skipped
```

Read quickly, that is a five-failure partition. It is not — nextest stopped after the fifth
failure, having run **15 of 54** tests. Re-running the identical command with `--no-fail-fast`
gave the real partition:

```
Summary [0.075s] 54 tests run: 37 passed, 17 failed, 0 skipped
```

**Why it matters more than a cosmetic difference.** A negative control is only evidence when a
named SIBLING still passes — that is what distinguishes an attributable detector from a suite
that fails wholesale. Under fail-fast, the tests that would have demonstrated attribution may
never run at all, so the surviving-sibling argument is unsupported by the log while *looking*
supported. In this case the three D-116-R1 path tests (live / migration / trait) and 12 of the
17 detectors were outside the truncated window; the `15/54` line is the only marker that the
run was partial, and it is easy to skim past.

Note the `15/54` prefix appears ONLY when the run is truncated — a complete run prints
`54 tests run`, with no fraction. That prefix is the tell.

**Guidance for every later plan in this phase.** Run negative controls with
`cargo nextest run --no-fail-fast …`, and assert the reported denominator equals the suite's full
count before reading the partition. This composes with `116-01`'s selector trap (`test(/foo/)`
silently selects zero): both failure modes produce a plausible-looking summary line from a run
that did not do what the reader thinks.

**Proposed owner:** informational; no fix required. `116-15` may wish to fold `--no-fail-fast`
into the phase's written conventions alongside the `binary(...)` selector rule.

---

## D-116-TRIPWIRE — `116-05` left `v2_bounded_reads_tripwire` RED, and nothing in that plan ran it

**Found during:** `116-06` (Task 1), running `binary(v2_bounded_reads_tripwire)` as a regression
check after adding a file to `src/shared/` — which is the directory that tripwire scans.

**Finding.** `every_peer_byte_accumulation_is_reviewed` FAILS at `b573fca2` and at every commit
since `ec80e5b1`:

```
HTTP-09: the reviewed accumulation population changed:
  NEW accumulation site(s): src/shared/credential_store.rs `push_str(` at line(s) [742]
    Bound it, or add an ALLOWLIST entry naming the mechanism that bounds it.
```

`src/shared/credential_store.rs:742` is `key.push_str(&format!(":{port}"))` inside
`normalize_server_key`, added by `116-05` Task 1 (`d03e6be4` / `ec80e5b1`). The other 12 tests in
that binary pass, **including** `no_unbounded_whole_body_read_over_peer_supplied_bytes` — so
`116-06`'s new `src/shared/http_body_cap.rs` is clean and is not named by the failure.

**Attribution.** The tripwire's failure message enumerates every NEW site; it names exactly one,
and that file is entirely `116-05`'s. `116-06` adds a file to the same scanned directory and is
not reported, so removing `116-06`'s file cannot make the `credential_store.rs` entry disappear.

**Why it matters more than one red test.** `make quality-gate` runs `test-all`, which includes the
integration binaries — so this is a **gate-red condition introduced inside this phase**, distinct
from `D-116-KEYCHAIN` (environmental) and `D-116-DISK` (environmental). `116-05`'s summary states
"every OTHER gate stage exits 0", which was measured stage-by-stage and did not include this
binary. It would fail CI.

**The fix is one reviewed exemption, not a code change.** The accumulation is bounded by
construction: `port` is a `u16` rendered by `format!`, so at most six bytes are appended, once.
The tripwire asks for exactly this — "add an ALLOWLIST entry naming the mechanism that bounds it".
`116-06` did **not** make that entry, because the allowlist is a REVIEWED-EXEMPTION register and
adding an entry on behalf of another plan's code, without that plan's author, is the silent
exemption the file's own doc warns against.

**Proposed owner:** `116-15`, or an immediate `116-05` follow-up. Do not let it ride to the end of
the phase — every later plan that touches `src/shared/` will now inherit a red tripwire it did not
cause.

### RESOLVED — the orchestrator made the reviewed entry in `5f1474e2`

`116-16` re-ran `binary(v2_bounded_reads_tripwire)` at `5f1474e2` and after adding
`src/shared/credential_file.rs` to the same scanned directory: **13 tests run, 13 passed**, both
times. The new module introduces **zero** accumulation sites (no `extend_from_slice(`, no
`push_str(`, no `.extend(`, no `.append(`), so the population is unchanged and no further allowlist
entry is owed. No action remains.

---

## D-116-LINT-OAUTH — the authoritative lint compiles NONE of this phase's `oauth`-gated code

**Found during:** `116-16` (Task 1). The third distinct shape of `D-116-LINT`, and the one that
matters most for the plans still to come.

**Finding.** `make lint` — which `D-116-LINT` correctly established as the authoritative clippy
evidence — runs `cargo clippy --features "full" --lib --tests`. **`full` does not contain `oauth`**
(`Cargo.toml:205` lists fifteen features; `oauth` is not among them, and `116-05` deliberately
declined to add it). So `make lint` does not compile, and therefore does not lint, ANY item behind
`#[cfg(feature = "oauth")]` — including all of `src/client/oauth.rs`, all of
`src/shared/credential_file.rs`, and whatever `116-10`/`116-12`/`116-13` add next.

`116-16` ran `make lint`'s command verbatim with `--features "full,oauth"` substituted — same
`RUSTFLAGS="-D warnings"`, same 28-entry `-A` allow-list, same lint groups
(`target/116-verify/116-16-clippy-oauth.raw.log`, exit **101**):

```
29 errors — every one of them in src/client/oauth.rs
 0 errors in src/shared/credential_file.rs
 0 errors in any file 116-16 touched
```

Distribution (`grep -A2 '^error' | grep -oE '^\s*--> [^:]+'`): **29 / 29 in
`src/client/oauth.rs`**. Categories include `doc_markdown` (23×), `needless_continue` (2×),
`map_unwrap_or`, `unnested_or_patterns` and `items_after_statements`.

**Why this is not the same finding as `D-116-LINT`.** That entry is about clause (b) being WEAKER
than `make lint` on the code both compile. This one is about a body of code **neither** command
covers by default: clause (b) does enable `oauth`, but omits `RUSTFLAGS="-D warnings"`, so the
29 errors above appear there only as warnings and clause (b) exits 0. The union of the two
documented commands therefore reports green on 29 hard errors.

**Consequence for `116-10`, `116-12` and `116-13`.** `src/client/oauth.rs` is the file `116-10`
wires `application_type` into and `116-12` changes refresh-scope behaviour in. The first plan to
turn the gate-equivalent command on over that file will inherit 29 pre-existing errors that are
**not** its own. Measure the baseline BEFORE editing, exactly as `116-16` did, or the attribution
argument will be unavailable.

**What `116-16` did instead of fixing it.** Ran both commands and asserted **zero errors
attributable to files this plan touched**, which is the same standard `D-116-DOC`'s 28-error anchor
uses. Fixing 29 lints in `src/client/oauth.rs` is an edit to a file this plan does not own, under
the executor's scope boundary.

**Proposed owner:** `116-15`, with two candidate resolutions:
1. Add a second lint invocation (`--features full,oauth`) to `make lint`, after clearing the 29
   pre-existing errors in `src/client/oauth.rs` — otherwise the gate turns red immediately.
2. Leave `make lint` alone and record the gate-equivalent-with-`oauth` command in
   `116-BASELINES.md` as a per-plan obligation for any plan touching gated code, with the 29-error
   figure as its anchor.

Do **not** simply add `oauth` to the `full` feature: `116-05` declined that on purpose (Pitfall 3),
and it would pull `webbrowser`, `dirs` and `rand` into every `full` build.

---

## D-116-FUZZGATE — `make test-fuzz` runs NOTHING and reports success, on a stable default toolchain

**Found during:** `116-08` (Tasks 1 and 2). Measured inside this plan's own
`make quality-gate` run, not inferred.

**Finding.** `Makefile:241-252`'s `test-fuzz` target — the FUZZ row of
`make validate-always`, and therefore of `make quality-gate` — is:

```make
cd fuzz && $(CARGO) fuzz list | while read target; do \
    timeout 30s $(CARGO) fuzz run $$target || echo "Fuzz target $$target completed"; \
done
```

`CARGO = cargo` (`Makefile:10`), with no `+nightly`. `cargo fuzz` passes `-Zsanitizer=address`, so
on a machine whose **default toolchain is stable** every invocation dies immediately:

```
error: the option `Z` is only accepted on the nightly compiler
```

In this plan's gate run that happened **21 times out of 21 targets**, and each one printed
`Fuzz target <name> completed` — because the `|| echo` swallows the failure — followed by
`✓ Fuzz testing completed` and, ultimately, `QUALITY_GATE_EXIT=0`.

So the ALWAYS-FUZZ gate is green over a stage that executed **zero fuzzing iterations**. It is not
merely weak: it cannot fail. A target that does not compile, or one that crashes on its first
input, produces exactly the same output as one that ran clean.

**Why this is not `116-08`'s to fix.** The `Makefile` is not in this plan's `files_modified`, and
the plan's own verification is explicit that `fuzz/` is workspace-EXCLUDED (`D-115-AB`) and must be
built and run EXPLICITLY — which is what this plan did, with `cargo +nightly fuzz`, recording run
counts and an empty artifacts directory. Changing the shared gate for the whole repo is a different
decision, and one with a CI dimension this plan cannot measure.

**Note the interaction with `fuzz/README.md`,** which already states "Rust nightly toolchain:
`rustup install nightly`" as a REQUIREMENT. The repo therefore documents that nightly is needed and
then invokes `cargo` without it. Both `pkce_helper.rs` and the two targets `116-08` adds give the
plain `cargo fuzz run <name>` form in their module docs, matching the Makefile — that convention is
correct for a machine whose default toolchain IS nightly, and is the reason the discrepancy is easy
to miss.

**Proposed owner:** `116-15`, with three candidate resolutions:
1. Use `cargo +nightly fuzz` in `test-fuzz` and drop the `|| echo`, so a failing target is a red
   gate. Measure first: this will surface any target that no longer builds.
2. Keep the `|| echo` but detect the nightly error explicitly and fail with a clear message, so the
   stage is honest about being unable to run.
3. Leave the Makefile alone and record in `116-BASELINES.md` that `make test-fuzz` is a smoke
   target, with per-plan explicit `cargo +nightly fuzz build` / `run` as the real FUZZ obligation —
   which is the standard `116-08` actually met.

Do **not** close the ALWAYS-FUZZ row on `make quality-gate`'s exit code alone.

---

## D-116-SLASH — the trailing-slash rule is now DIFFERENT in the two halves of discovery, on purpose, and it is operator-visible

**Found during:** `116-07` (Task 2), by a test that FAILED against a correct implementation — the
same shape as `116-04`'s `https:///path` finding.

**Finding.** The URL DERIVATION normalises a trailing slash away: `116-04` decided that
`https://as.example/` and `https://as.example` must derive the same candidate list, because a
trailing slash is a formatting difference and not a path component. The RFC 8414 §3.3 ANCHOR does
**not** normalise, and must not — `116-04` pinned four normalisation rows as `false` precisely so a
lenient comparison cannot be exploited.

The two rules therefore disagree by design, and the consequence is visible to operators. Measured,
against the implementation as shipped:

```
Discovery document fetched from http://…/us-east-1_TEST/.well-known/openid-configuration
declares issuer `http://…/us-east-1_TEST`, but the URL was built from issuer
`http://…/us-east-1_TEST/`. RFC 8414 section 3.3 … require these to be identical, so the
metadata is NOT used and is NOT cached.
```

An operator who configures `https://as.example/pool/` against a provider whose document declares
`https://as.example/pool` now gets a hard refusal where, before this plan, they got a working
provider. This is what RFC 8414 §3.3 requires, and it is the whole point of `T-116-09`/`T-116-23` —
but it is a BEHAVIOUR CHANGE, not merely a new check.

**Why it is very unlikely to bite in practice.** Real trailing-slash issuers declare the slash.
Auth0's issuer is `https://tenant.auth0.com/` and its discovery document declares
`"issuer": "https://tenant.auth0.com/"`, so `GenericOidcConfig::auth0` — which builds
`format!("https://{domain}/")` — matches byte-for-byte. `CognitoProvider::new`,
`GenericOidcConfig::google`, `::okta` and `::entra` all produce slash-free issuers, matching their
providers' documents. The exposure is a hand-written `GenericOidcConfig::new` whose issuer string
carries a slash the provider does not declare.

**Why `116-07` did not soften it.** Normalising the anchor would delete the fence this phase exists
to install. Normalising the CONFIGURED issuer before comparing would be the same defect wearing a
different hat: it would make `https://attacker.example/` and `https://attacker.example` equivalent
anchors, and the specification's no-normalisation rule exists exactly to stop that reasoning.
Instead the behaviour is pinned by a deliberate test —
`a_trailing_slash_issuer_still_needs_a_byte_identical_document_issuer` in `cognito.rs` — so it is
documented rather than discovered in production, and the refusal names BOTH values so the fix is a
one-character config edit.

**Proposed owner:** `116-13` (release notes / version bump). This needs one CHANGELOG line under
"behaviour changes", not a code change: *"OIDC discovery now enforces RFC 8414 §3.3. If your
configured issuer differs from the `issuer` your provider's discovery document declares — most
commonly by a trailing slash — discovery will refuse it and name both values."*

`116-15` may cite this entry rather than re-deriving it. No source change is owed.

---

## D-116-LINT — two more measurements from `116-07`, both in TEST code

Appended here rather than opening a new entry, because it is the same finding for the seventh and
eighth time. `116-07` ran `make lint` per the standing obligation and got **exit 101** twice on code
the phase's clause-(b) command accepts:

| Lint | Site | Why clause (b) missed it |
|---|---|---|
| `clippy::doc_markdown` | `IdP` unbackticked in `tests/oauth_provider_discovery.rs`'s module doc | pedantic; only a warning without `RUSTFLAGS="-D warnings"` |
| `clippy::duration_suboptimal_units` | `Duration::from_secs(3600)` in a `cognito.rs` test helper — the fix is `Duration::from_hours(1)` | nursery; same reason |
| `clippy::items_after_statements` | `const ATTEMPTS` declared mid-function in a `cognito.rs` test | pedantic; same reason |

All three are in **test** code, which reconfirms `116-04`'s note that `make lint` covers
`--lib --tests` and therefore gates new test files too. The gate-equivalent-with-`oauth` command
(`D-116-LINT-OAUTH`) was also run: **29 errors, all 29 in `src/client/oauth.rs`**, exactly the
recorded anchor, and **0** attributable to any file `116-07` touched.

---

## D-116-GREP — two of `116-09`'s own acceptance greps cannot pass as written, at any HEAD

**Found during:** `116-09` (Tasks 1 and 2), running the plan's `<acceptance_criteria>` literally.
The same shape as `116-07`'s trailing-slash finding: a check that *looks* like a detector and is
not one.

**Finding 1 — `grep -n 'pub iss' src/client/oauth.rs` "shows no new public field on OAuthConfig".**
`OAuthConfig` has had `pub issuer: Option<String>` since the type existed, and `pub iss` is a
PREFIX of `pub issuer`. The grep therefore matches at `b2bf9157`, at this HEAD, and at every commit
in between. Taken as written the criterion is unsatisfiable; taken as a *reader's* check it is a
false alarm.

The invariant it reaches for is real and load-bearing (`OAuthConfig` is all-pub-field and not
`#[non_exhaustive]`, so a new field is `constructible_struct_adds_field` = MAJOR). `116-09` therefore
asserts **the exact eight-field set by name**, parsed out of the struct body, in
`oauth_config_gained_no_public_iss_field` — plus a second assertion that no field name starts with
`iss_`. That version FAILED on its first run against the plan's literal wording, which is how the
defect was found rather than papered over.

**Finding 2 — `grep -n 'validate_authorization_response' src/client/oauth.rs` "shows exactly one
call site".** The count is **2**: line 38 is the `use` declaration, line 1064 is the call. A plan
that greps for a bare symbol name will always also match its import. The measured, meaningful form
is `grep -c '<symbol>(' ` or reading the hits — `116-09` reports both hits explicitly and names the
line numbers so the claim is checkable.

**Why this matters beyond two greps.** `116-06` already recorded that a module's own PROSE can trip
an acceptance grep. This is the third and fourth instance in the phase of the same class:
**a grep-based acceptance criterion that measures something other than what its sentence says.**
`116-01`'s nextest-selector trap and `D-116-FAILFAST` are the same failure mode in a different tool.

**Proposed owner:** `116-15`, when it reconciles the phase's evidence. The cheap systemic fix is a
written convention: *an acceptance grep must be RUN against the pre-change tree when the plan is
written, and its expected count recorded* — a grep whose baseline count is unknown cannot
distinguish a regression from a prefix collision. No source change is owed.

---

## D-116-FALLBACK — a security refusal used to be downgraded into "no supported OAuth flow available"

**Found during:** `116-09` (Task 2), writing the plan's eleven behaviour rows. Fixed there under
Rule 2; recorded because the same shape may exist on other fallback paths this phase does not touch.

**Finding.** Both callers of the authorization-code flow (`get_access_token` and
`authorize_with_details`) wrapped ANY failure: they logged it, then either fell back to the
device-code grant or replaced it with the fixed string *"No supported OAuth flow available."* Once
`116-09` made the flow return `Error::iss_mismatch` / `Error::state_mismatch`, that wrapper
did two harmful things at once:

1. **It destroyed the stable programmatic identity.** `116-02` built the three marker-const error
   identities precisely so callers branch on `err.is_iss_mismatch()` instead of on message text.
   A caller of `authorize_with_details()` would have received a generic internal error and been
   pushed straight back to substring matching — with a substring that does not even mention `iss`.
2. **It re-attempted authentication after detecting an attack.** Falling back to device code after
   a mix-up or CSRF refusal offers the same adversary a second attempt through a different grant.

**Fix applied in `116-09`:** `OAuthHelper::is_terminal_authorization_refusal` — an `iss` or `state`
mismatch propagates verbatim from both callers; every other failure keeps its existing fallback
behaviour untouched.

**What is deliberately NOT covered, and is the deferred half.** The refusals that do NOT carry one
of the two markers — a duplicated security parameter, a query over `MAX_CALLBACK_QUERY_BYTES`, an
oversize request line, an unparseable request target — are still wrapped by the generic message.
They are all still *refusals* (the tests assert `is_err()` and `expect(0)` on `/token`, and both
hold), but a caller cannot tell them apart from "the authorization endpoint was unreachable". The
clean resolution is a fourth marker identity, or a `Error::callback_refused` wrapper, so that
"the callback arrived and was refused" is programmatically distinguishable from "the flow never
ran". `116-09` did not add one: a new public error identity is `116-02`'s subsystem, and inventing
one here would fork the convention that plan established.

**Proposed owner:** `116-15`, or a `116-02` follow-up. No behaviour is wrong today; the gap is in
what a caller can *observe*.

---

## D-116-LINT-OAUTH — the TEST-side twin: `make quality-gate` runs ZERO of `116-09`'s 25 tests

**Found during:** `116-09`, reading its own `make quality-gate` log and noticing a run of
`0 passed; … N filtered out` lines. Appended to `D-116-LINT-OAUTH` rather than opening a new
entry: it is the same root cause (`full` does not contain `oauth`) with a different, worse
consequence.

`D-116-LINT-OAUTH` established that `make lint` compiles none of this phase's `oauth`-gated code.
The same is true of `make test-all`, and `116-09` is the first plan whose tests are affected.
Measured at `c03cfe87`:

| Suite | `--features full` | `--features full,oauth` | Gated on |
|---|---|---|---|
| `oauth_discovery_validation` + `oauth_provider_discovery` (`116-06`, `116-07`) | **34** | 34 | `http-client` |
| `oauth_iss_integration` + `oauth_state_csrf` (**`116-09`**) | **0** | **25** | `oauth` |

`116-06` and `116-07` could gate on `http-client` because their subjects (`OidcDiscoveryClient`,
the two server-side providers) live behind that feature. `116-09`'s subject is `OAuthHelper`, which
is behind `oauth`, and the tests construct one — the whole point is that they drive the REAL
interactive flow through the `BrowserLauncher` seam. **There is no gating choice that makes them
reachable by the current gate.** So `make quality-gate` exits 0 having run none of AUTH-01's
end-to-end evidence, including every `expect(0)`-on-`/token` proof.

This is the "command that reports success while measuring nothing" shape again — the sixth
instance in this phase, and the first where the un-measured thing is a security proof rather than a
lint.

**Not fixed by `116-09`** because the fix is a `Makefile` change (a second test invocation with
`--features full,oauth`) plus a CI job change, neither of which is in this plan's `files_modified`,
and because turning `oauth` on in the gate would immediately surface the 24 pre-existing clippy
errors in `src/client/oauth.rs` and turn the gate red — the exact interaction `D-116-LINT-OAUTH`
already warns about. The two changes must land together.

**Proposed owner:** `116-15`. The resolutions now have to be taken as a PAIR:
1. clear the remaining pre-existing `src/client/oauth.rs` clippy errors (**24** at `c03cfe87`, down
   from the 29 anchor — `116-09` removed 5 by rewriting the doc comments it touched and added
   none), THEN
2. add `--features "full,oauth"` invocations to both `make lint` and the gate's test stage.

Doing (2) without (1) turns the gate red. Doing neither leaves 25 security tests outside CI.

### `116-10` re-measured both halves, and the numbers MOVED in the right direction

Measured at `87f1f648`, with `make lint`'s command run verbatim under `--features "full,oauth"`:

| Tree | `^error` count | Distribution |
|---|---|---|
| `86fbb70a` (this plan's baseline, pristine) | **24** | 24 / 24 in `src/client/oauth.rs` |
| `87f1f648` (after both `116-10` tasks) | **21** | 21 / 21 in `src/client/oauth.rs` |

**ZERO new errors attributable**, compared as a multiset of
`(error message, offending source-line text)` rather than by line number, since every line in the
file moved again. The three that DISAPPEARED were not fixed as a side quest — each sat on a line
this plan had to rewrite anyway: `items_after_statements` on the function-local
`const MAX_DCR_RESPONSE_BYTES` (hoisted to module scope so the rejection path and the success path
name one number), `map(<f>).unwrap_or_else(<g>)` on `granted_scopes` (rewritten as an explicit
two-branch `match` so RFC 6749 §5.1's omission rule is named at the branch that applies it), and a
`doc_markdown` hit on `TokenResponse` in a doc comment the signature change forced.

**The anchor for `116-11` and `116-12` is therefore 21, not 24 and not 29.** It has now moved twice
in three plans, always downward and always as a side effect of rewriting the surrounding line — so
a plan that measures against a stale figure will report phantom "fixes".

The test-side twin was re-measured too, on this plan's own suites:

| Suite | `--features full` | `--features full,oauth` |
|---|---|---|
| `oauth_dcr_integration` + the two new inline `client::oauth` modules (`116-10`) | **0** | **38** |

`cargo nextest run --features full -E '<this plan's selector>'` reports
`Starting 0 tests across 2 binaries` and then `error: no tests to run`. `make quality-gate` exited
**0** at this HEAD with `test-unit` at **1880 passed** — the identical figure `116-09` recorded,
which is itself the proof: this plan added **14** new inline lib tests and the gate's population did
not move by one, because every one of them is behind `oauth`. Running the gate is not evidence that
this plan's tests passed; it is evidence that they never ran.

---

## D-116-GREP — a fourth instance, from `116-10`'s own acceptance criteria

**Found during:** `116-10` (Task 2), running the plan's `<acceptance_criteria>` literally.

**Finding.** The criterion is *"`grep -n 'retry' src/client/oauth.rs` shows no automatic
`application_type` retry was added"*. It returns **3** hits at `87f1f648` — lines 310–312, all three
of them `///` doc lines inside `registration_rejected`, which say:

```
/// SEP-837's OPTIONAL retry with an adjusted `application_type` is also
/// deliberately not implemented. The specification says clients MAY retry; an
/// automatic retry would silently register the client under a type its operator
/// did not choose, which is the opposite of surfacing a meaningful error.
```

So the ONLY way to satisfy the criterion as written is to delete the documentation that records the
non-adoption — and recording that non-adoption is required by the same task's `<action>`. The two
instructions are in direct conflict when the grep is read literally.

This is the same shape `116-06` first recorded (a module's own PROSE trips its audit grep) and the
fifth instance in the phase overall. The meaningful check, which `116-10` performed instead: every
hit is a `///` line, and there is no control-flow retry — `do_dynamic_client_registration` issues
exactly one `POST` and returns `registration_rejected(...)` on any non-success status, with no loop
and no second `apply_application_type` call.

**Proposed owner:** `116-15`. The convention `D-116-GREP` already proposes (measure an acceptance
grep's baseline count when the plan is written) would have caught this at planning time. A second
half is worth adding: **an audit grep over a file that documents its own non-adoptions must exclude
comment lines** (`grep -n 'retry' … | grep -v '^\s*[0-9]*:\s*///'`), or the plan should assert the
absence of the CODE construct rather than the absence of the word. No source change is owed.

---

## D-116-PRM — RFC 9728 is a NAMED DEPENDENCY of two things this phase shipped, not just a deferred nicety

**Found during:** `116-11` (Tasks 1 and 2), writing the D-116-R1 collision test and D-18's detection.
Recorded because "deferred by owner decision" understates what is actually blocked.

**Finding.** `pmcp` derives the authorization server from the MCP base URL directly —
`get_metadata_with_extras` → `discover_metadata_with_extras` → `extract_base_url` — and `116-07`
then enforces RFC 8414 §3.3 anchoring, so the fetched document must declare exactly the issuer the
URL was built from. **Two consequences follow, and both are load-bearing for AUTH-03:**

1. **The D-116-R1 collision is not CONSTRUCTIBLE through the live flow.** Two MCP servers at two
   different origins always resolve two different issuers, and two MCP servers at ONE origin
   normalize to one `server` key (`normalize_server_key` drops the path). So "two MCP servers
   sharing one authorization server and one account" — the case AUTH-03's amended text was written
   for, and the case the third key component exists to keep apart — cannot arise until the
   authorization server is discovered independently of the MCP origin. That is RFC 9728 Protected
   Resource Metadata. `116-11`'s collision test therefore SEEDS the second server's entry, which is
   recorded in the test file's own module doc rather than left for a reader to infer. The key shape
   is correct and proven; the scenario it defends is currently reachable only by a platform that
   drives the store itself.

2. **D-18's detection is narrower than the specification's.** The specification describes an
   authorization-server change as one "detected via updated protected resource metadata". `116-11`
   compares the issuer discovery RESOLVED for a server URL against the one last recorded for it,
   which catches a server that starts pointing somewhere else but cannot catch a change announced
   only through protected resource metadata, because nothing reads that. This is written into
   `announce_authorization_server_change`'s rustdoc in place, and `T-116-43`'s disposition is
   `accept`.

**Why this is not `116-11`'s to fix.** RFC 9728 discovery is a new network surface with its own
threat register, its own `.well-known` probe order and its own caching rules. It is DEFERRED by
owner decision (2026-08-02, `116-CONTEXT.md` § Deferred Ideas). Implementing it inside a wiring plan
would be the architectural change Rule 4 exists to stop.

**Proposed owner:** `116-15` to record it as a named dependency with an owner, rather than as a
generic deferral. The two items above should be quotable when AUTH-03 is booked: the key shape is
delivered and proven at the store, the trait and the helper; the SCENARIO in consequence 1 has no
end-to-end coverage and cannot have any until RFC 9728 lands.

---

## D-116-PLANCONFLICT — a plan `<action>` whose two instructions cannot both be satisfied, and the measurement that settles it

**Found during:** `116-11` (Task 1). The same family as `D-116-GREP` — an instruction that reads
correctly in isolation and is unsatisfiable against the tree — but in the `<action>` rather than in
the acceptance criteria, so a grep convention would not have caught it.

**Finding.** `116-11`'s `<action>` says to resolve the default store "honoring `config.cache_file`
when set and `default_credential_path()` otherwise". Its `<behavior>` says
`~/.pmcp/oauth-tokens.json` "is NEVER opened for reading" and "The file is left in place for the
user to delete". **Both cannot hold**, because the two in-repo callers pass exactly that file as
`cache_file`:

```
crates/mcp-tester/src/main.rs:594     Some(default_cache_path())
cargo-pmcp/src/commands/auth.rs:76    Some(default_cache_path())
```

Pointing a `FileCredentialStore` at `cache_file` therefore (a) parses the legacy flat document —
which `parse_credential_snapshot` rejects for having no `schema_version`, so every existing user's
first call errors — and (b) overwrites it on the first save, which is the opposite of leaving it in
place.

A second, independent conflict in the same sentence: `cache_file: None` means **do not cache**.
Every previous cache read and write in `src/client/oauth.rs` was guarded by
`if let Some(ref cache_file) = self.config.cache_file`, and `cargo-pmcp/src/commands/auth.rs:73`
sets the field to `None` precisely when `--no-cache` is passed. Resolving a default store in that
case would silently defeat the flag — and, measurably, would have made every `oauth`-gated
integration test in this phase write real credential documents into the developer's
`~/.pmcp/oauth-cache.json` under an `O_EXCL` lock shared by ~10 parallel nextest processes.

**Resolution applied in `116-11`** (both recorded as Rule 1 deviations, both tested):

| Rule | Behaviour |
|---|---|
| `cache_file` names the legacy file's DIRECTORY | the store is `<that directory>/oauth-cache.json`, i.e. `default_credential_path()`'s file name beside the legacy one. `cargo-pmcp`/`mcp-tester` land on exactly `~/.pmcp/oauth-cache.json` |
| `cache_file: None` and no injected store | NO persistence, as before |
| a caller who wants a specific store path | `with_credential_store(Arc::new(FileCredentialStore::new(path)))`, which is strictly better because it also admits a non-file store |

**Proposed owner:** `116-15`. The convention worth adding to `D-116-GREP`'s: **when a plan's
`<action>` names a configuration field, the plan should record what the in-repo callers actually
pass to it.** One `grep -rn '<field>' src crates cargo-pmcp` at planning time would have surfaced
this. `116-13` must also carry the CHANGELOG line — an existing `~/.pmcp/oauth-tokens.json` is
discarded, one re-login is required, and the file is left on disk.

---

## D-116-LINT-OAUTH — `116-11` re-measured BOTH halves; the anchor moved again, 21 → 17

Appended to `D-116-LINT-OAUTH` rather than opening a new entry.

**The clippy half.** Measured with `make lint`'s command run verbatim under `--features "full,oauth"`:

| Tree | `^error` count | Distribution |
|---|---|---|
| `70dc259f` (`116-11`'s pristine baseline) | **21** | 21 / 21 in `src/client/oauth.rs` |
| `3b2a61e1` (after both `116-11` tasks) | **17** | 17 / 17 in `src/client/oauth.rs` |

**ZERO new errors attributable**, compared as a multiset of `(error message, offending source-line
text)` rather than by line number, since every line in the file moved again. The four that
DISAPPEARED all sat on lines this plan had to rewrite or delete:

- `map(<f>).unwrap_or(<a>)` on `let now = SystemTime::now()` in `build_auth_result` — replaced by
  the module-level `unix_now_secs()`, which the device-code path now shares, so a second copy of
  the same lint was never introduced.
- **three** `doc_markdown` hits on one line — `/// Full artifacts (refresh_token, expires_at,
  scopes, issuer, client_id) are` — inside `authorization_code_flow`, the `String`-returning wrapper
  this plan deleted once both entry points went through `authorize_with_fallback`. One deleted line,
  three errors, because `doc_markdown` fires once per unbackticked item.

**The anchor for `116-12` is therefore 17**, not 21, not 24 and not 29. It has now moved three times
in four plans, always downward and always as a side effect of rewriting a surrounding line.

**The test half — a THIRD independent measurement, and the population is still 1880.** Measured at
`3b2a61e1`:

| Suite | `--features full` | `--features full,oauth` |
|---|---|---|
| `binary(oauth_store_wiring)` | **0** (`Starting 0 tests across 1 binary`, then `error: no tests to run`) | **18** |
| inline `client::oauth::credential_store_wiring_tests` | **0** (`1880 filtered out`) | **6** |

`make quality-gate`'s `test-unit` population is **1880** — byte-identical to `116-09`'s and
`116-10`'s — although this plan added **6** new inline lib tests. So the gate's own number has now
failed to move for three consecutive plans that added inline tests, which is as direct a proof as
the finding can have. **A green gate is not evidence that any of `116-11`'s 24 tests ran.**

The paired resolution is unchanged and now cheaper: clear the **17**, then add `--features
"full,oauth"` to `make lint` AND to the gate's test stage. **81** tests from `116-09`, `116-10` and
`116-11` are outside CI.

---

## D-116-KEYCHAIN — REOPENED by `116-12`, and the disk theory is refuted

`116-06` closed this by measurement (1865 passed / 0 failed on a clean volume) and `116-16` and
`116-11` reconfirmed it clean. **It reproduced during `116-12`'s final gate run**, and the
circumstances rule out both previous explanations.

**The observation.** `make quality-gate` exit **2** at `test-unit`: `1866 passed; 14 failed`. All
14 are in `shared::streamable_http::tests`, a module `116-12` never touched. Every one panics at
the same pre-existing line:

```
panicked at src/shared/streamable_http.rs:458:18:
Failed to load native root certificates: Custom { kind: NotFound, error:
  "no native root CA certificates found (errors: [
     Error { context: \"failed to load user trust settings\",   kind: Os(Error { code: -36, message: \"I/O error.\" }) },
     Error { context: \"failed to load admin trust settings\",  kind: Os(Error { code: -36, message: \"I/O error.\" }) },
     Error { context: \"failed to load system trust settings\", kind: Os(Error { code: -36, message: \"I/O error.\" }) }])" }
```

`14` is exactly the count `116-04` measured, so this is the same phenomenon and not a new one.

**Three measurements that settle the attribution.**

| Measurement | Result |
|---|---|
| An EARLIER `make quality-gate` in the same session, on the identical tree | `test result: ok. **1880 passed; 0 failed**` — the same commit passed minutes before it failed |
| `df -h /` at the moment of failure | **92 GiB free, 12% used** — the volume was nowhere near full |
| The same 14 tests run against the **PRE-PLAN** `src/client/oauth.rs` (`git show 73d95880:...`, the tree whose own summary records `make quality-gate` exit 0) | `81 passed; **14 failed**` — byte-identical failure with none of this plan's code present |

**What this changes.** `D-116-DISK` is NOT the mechanism. The mechanism is the macOS Security
framework returning `ioErr` (`-36`) for user, admin AND system trust settings at once — a
transient keychain/`securityd` condition that a full volume can PROVOKE but does not require.
`116-06`'s resolution should be read as "it did not reproduce that day", not as "it is fixed".

**The real defect is the `.expect`.** `src/shared/streamable_http.rs:458` unwraps a
`Result<ConnectorBuilder, io::Error>` from `hyper_rustls`, so a transient OS condition becomes a
PANIC inside a transport constructor. That is a library-code panic a caller cannot catch, in the
same family as the `duration_since(UNIX_EPOCH).unwrap()` `116-11` removed from `oauth.rs`. It
should return an `Error`, and the tests should build their transports through a fixture that does
not need the platform trust store at all.

**Do not** "fix" this by pinning `rustls` to `webpki-roots`: that changes which CAs the SDK trusts
in production to work around a test-environment fault.

**Proposed owner:** `116-15`, as a NAMED item rather than a generic deferral — it is a
gate-red condition reproducible on a healthy machine and it will fail CI on any macOS runner that
hits the same keychain state.

---

## D-116-GREP — fifth instance, and the first where the plan's own grep hid real violations

`116-12` Task 3's acceptance criterion is
`grep -c '\.text()\.await\|\.bytes()\.await\|\.json()\.await\|\.json::<' src/client/oauth.rs` = 0,
and the plan's `<action>` names **three** remaining whole-body reads on that basis.

**There were six.** `rustfmt` splits a long chain across lines, so the three below never matched a
single-line pattern and were invisible to every audit this phase ran:

| Site | Rendered form | Why the grep missed it |
|---|---|---|
| DCR SUCCESS body | `let bytes = response\n    .bytes()\n    .await` | `.bytes()` and `.await` on different lines |
| device-code POLL body | `let body = response\n    .text()\n    .await` | same |
| refresh SUCCESS body | `response\n    .json()\n    .await` | same |

Note that the grep returned **0 both before and after** three of the six were fixed, so it could
never have distinguished a clean file from a dirty one.

**The check that actually holds** is multi-line aware, and it is what `116-12` used:

```python
re.finditer(r'\.\s*(text|bytes|json)\s*(::<[^>]*>)?\s*\(\s*\)\s*\n?\s*\.\s*await', src)
```

`116-14`'s fence must use a form like this, not a line-oriented `grep`, or a single `cargo fmt`
run can silently reopen every site the fence claims to guard. The same shape appeared twice more
in this plan and is worth stating as a general rule: **a line-oriented count over a
`rustfmt`-formatted file is not a reliable audit.** Two further examples measured here:

- `grep -c '^error'` over a clippy log counts the `could not compile ... due to N previous errors`
  summary line as an error, so the honest count is one lower than the grep (`116-11`'s 17 and this
  plan's raw 18 are the SAME number).
- `grep -c '^warning'` over a `cargo build` log counts the `generated N warnings` summary line the
  same way (93 vs the anchor's 92).

**Proposed owner:** `116-14` for the fence itself; `116-15` to fold the rule into the phase
conventions.

---

## D-116-LINT-OAUTH — `116-12` is the FOURTH consecutive plan whose inline tests the gate never runs

| Measurement | `116-09` | `116-10` | `116-11` | **`116-12`** |
|---|---|---|---|---|
| gate `test-unit` population | 1880 | 1880 | 1880 | **1880** |
| inline lib tests the plan ADDED | yes | yes | 6 | **6** |
| the plan's own tests the gate SELECTED | 0 of 25 | 0 of 38 | 0 of 24 | **0 of 27** |

`116-12`'s numbers, measured rather than inferred: `cargo nextest run --features full -E
'binary(oauth_refresh)'` reports `Starting 0 tests across 1 binary` then `error: no tests to run`;
`cargo test --lib --features full token_fingerprint` reports `0 passed … 1880 filtered out`. Under
`full,oauth` the same two are **21** and **6**. The gate's own population has now failed to move
across four consecutive plans that each added inline tests, which is as direct a proof as the
finding can have.

**The clippy half is unchanged at 17** (`116-12` added zero, having fixed two of its own —
`format_collect` and `items_after_statements` — rather than allowing them). **102 tests** from
`116-09`, `116-10`, `116-11` and `116-12` are now outside CI.

Owner: `116-15`, unchanged — clear the 17, then add `--features "full,oauth"` to `make lint` AND
to the gate's test stage, as a PAIR.
