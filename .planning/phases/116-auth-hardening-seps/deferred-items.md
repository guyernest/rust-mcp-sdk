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
