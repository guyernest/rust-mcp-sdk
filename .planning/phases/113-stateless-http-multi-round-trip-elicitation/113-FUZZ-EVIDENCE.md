# Phase 113 — libFuzzer campaign evidence: `subscription_listen_frames`

Closes gap item 5 of `113-VERIFICATION.md`:

> "An actual libFuzzer run (not just `cargo check`) against
> `subscription_listen_frames`, matching the rigor already applied to
> `fuzz_request_state` (20 000 runs, 0 artifacts)."

Precedent and format: `113-03-SUMMARY.md` § Verification, which recorded
`cargo fuzz run fuzz_request_state -- -runs=20000` → **exit 0, artifacts/ EMPTY**
(cov 559, 78-entry corpus) for the phase's OTHER untrusted-input decoder.

**Verdict: PASS — 20 000 runs, exit 0, zero crash artifacts, overflow branch covered.**

---

## 1. What was run, exactly

| | |
|---|---|
| Target | `subscription_listen_frames` (`fuzz/fuzz_targets/subscription_listen_frames.rs`) |
| Code under test | `pmcp::client::subscriptions` frame decoder + the SHARED `SseParser` (`src/shared/sse_parser.rs`) |
| Repo commit | **`e37c381ab5fe5b223183896c4ddf6ec737172f9c`** (`git rev-parse HEAD` at run time; `git status --porcelain -- src fuzz` was EMPTY, so the campaign ran against exactly this committed tree) |
| Branch | `fix/mcp-publisher-oidc-audience` |
| Date | 2026-07-26 |

### Commands, in the order they were run

```bash
# 1. stable, no sanitizer — the "does it still compile for everyone" check
cargo fuzz build --sanitizer=none subscription_listen_frames        # exit 0

# 2. nightly, AddressSanitizer — the build the campaign actually uses
cargo +nightly fuzz build subscription_listen_frames                # exit 0

# 3. the campaign, from an EMPTY corpus
cargo +nightly fuzz run subscription_listen_frames -- -runs=20000   # exit 0
```

`+nightly` is load-bearing, not decoration: `cargo fuzz run` passes
`-Zsanitizer=address`, which stable rustc rejects. `RUSTUP_TOOLCHAIN=nightly`
was NOT needed here (113-12's `rustup which cargo` caveat did not bite), but
remains the fallback if the `rustc` proxy honours a vendored
`rust-toolchain.toml`.

libFuzzer's own invocation line, verbatim from the run:

```
Running `fuzz/target/aarch64-apple-darwin/release/subscription_listen_frames
  -artifact_prefix=<repo>/fuzz/artifacts/subscription_listen_frames/
  -runs=20000
  <repo>/fuzz/corpus/subscription_listen_frames`
```

### Toolchain

| Component | Version |
|-----------|---------|
| Campaign compiler | `rustc 1.97.0-nightly (bf4fbfb7a 2026-04-11)` |
| Campaign cargo | `cargo 1.97.0-nightly (eb94155a9 2026-04-09)` |
| Stable compiler (build check only) | `rustc 1.97.1 (8bab26f4f 2026-07-14)` |
| `cargo-fuzz` | `0.13.1` |
| Host | `aarch64-apple-darwin` (Darwin 25.5.0) |

---

## 2. Counters

Starting state — proven, not assumed:

```
INFO:        0 files found in <repo>/fuzz/corpus/subscription_listen_frames
INFO: -max_len is not provided; libFuzzer will not generate inputs larger than 4096 bytes
INFO: A corpus is not provided, starting from an empty corpus
INFO: Seed: 872967294
#2	INITED cov: 130 ft: 130 corp: 1/1b exec/s: 0 rss: 67Mb
```

Final line:

```
#20000	DONE   cov: 232 ft: 823 corp: 182/3497b lim: 63 exec/s: 0 rss: 103Mb
Done 20000 runs in 0 second(s)
```

| Counter | Value |
|---------|-------|
| Iterations | **20 000** (`-runs=20000`, reached `#20000 DONE`) |
| Process exit code | **0** |
| Coverage | `cov: 232` (edges), `ft: 823` (features) |
| Corpus | `corp: 182/3497b` in libFuzzer's accounting; **180 files on disk** after the run (libFuzzer's count includes the synthetic empty input and the in-flight unit) |
| Max RSS | 103 MB |
| Wall clock | < 1 s (`exec/s` collapses to 0 in the final line because the whole run fits inside one reporting second) |
| Crash / timeout / OOM artifacts | **none** |

---

## 3. Artifacts-empty proof

The directory **exists and is empty** — `cargo fuzz` creates it from
`-artifact_prefix` regardless of outcome, so "absent" was NOT the observed case:

```console
$ /bin/ls -A fuzz/artifacts/subscription_listen_frames/ | /usr/bin/wc -l
0
$ test -d fuzz/artifacts/subscription_listen_frames && echo EXISTS
EXISTS
```

Absolute binary paths are deliberate. The repo's `rtk` shell proxy rewrites and
summarises command output; the *same* check written as
`ls -A ... | wc -l` returned a spurious `1` through the proxy while the
directory was demonstrably empty. Any future re-verification should use
`/bin/ls` and `/usr/bin/wc` (or `find`) for this proof.

**Out-of-scope observation, recorded rather than acted on:** a pre-existing crash
artifact for a DIFFERENT target,
`fuzz/artifacts/auth_flows/crash-e29e9da4b8b23e9e48def2fd1201ea339341fc89`
(8 bytes, dated 2025-09-12), is present in the repo working tree. It predates
Phase 113 by ten months, belongs to the `auth_flows` target, and is out of this
plan's scope fence. It is logged in this phase's `deferred-items.md`.

---

## 4. The campaign exercised the BOUNDED parser — and that took a correction

Plan 113-15 gave `SseParser` a line-buffer bound with a latching `overflowed()`
flag. The riskiest new branch is the discard-and-latch path, because it
manipulates buffer state on hostile input. A campaign that never reaches it
would be evidence of nothing.

The target therefore drives `SseParser::with_max_buffer_size` (through the
`decode_listen_chunks_for_fuzz` seam) rather than the default constructor, and
feeds each input as successive 16-byte "body frames" so the SSE line buffer and
the undecoded-UTF-8 tail carry across chunks exactly as they do in
`read_next_frame`.

**First attempt used a single 64-byte bound, and covered the branch ZERO times.**
Measured, not suspected: libFuzzer ramps its length limit (`len_control`), and
inside a 20 000-run budget it only reached 38-byte inputs — the retained corpus
topped out at 53 bytes, with **0 entries over 64 bytes**. An input that short can
never push a 64-byte buffer past its bound.

The target now decodes every input **once per bound, `[64, 8]`**:

- **64** — the ordinary path (tokenize / incremental UTF-8 / JSON-RPC classify).
- **8** — the overflow path, tripped by any newline-free chunk of 9+ bytes, i.e.
  by nearly every input libFuzzer generates from its first run. No special flags
  and no seeded corpus are required — which matters, because `fuzz/.gitignore`
  ignores `corpus`, so a seed would not survive for the next reader.

Coverage rose from `cov: 226` (64-only) to `cov: 232` (both bounds).

### Branch-coverage proof

Any corpus entry whose first ≤16-byte chunk is longer than 8 bytes and contains
no `\n` *necessarily* executes the discard-and-latch branch at the 8-byte bound
(that is the enforcement condition in `SseParser::feed`, verbatim). Counting
them over the retained corpus:

```console
$ python3 - <<'PY'
import os
d = "fuzz/corpus/subscription_listen_frames"
files = sorted(os.listdir(d))
trips = [n for n in files
         if len(open(os.path.join(d, n), 'rb').read()[:16]) > 8
         and b"\n" not in open(os.path.join(d, n), 'rb').read()[:16]]
sizes = [os.path.getsize(os.path.join(d, n)) for n in files]
print("corpus entries on disk:", len(files))
print("entries driving the overflow branch on chunk 1:", len(trips))
print("entries larger than 64 bytes:", sum(1 for s in sizes if s > 64),
      "| max entry size:", max(sizes))
PY
corpus entries on disk: 180
entries driving the overflow branch on chunk 1: 50
entries larger than 64 bytes: 0 | max entry size: 61
```

`entries larger than 64 bytes: 0` is the same measurement stated twice: it is
simultaneously the proof that the 8-byte bound was needed and the proof that a
64-byte-only campaign of this size covers nothing of the overflow path.

Single-input replay of one such entry, i.e. the same shape a crash reproducer
would take:

```console
$ ./fuzz/target/aarch64-apple-darwin/release/subscription_listen_frames \
    fuzz/corpus/subscription_listen_frames/cad27335a716667593237bb43673d4a1a565b257
Executed ... in 0 ms
# exit 0
```

### Invariants the campaign asserted on every input

1. **Never panics** (T-113-67) — arbitrary bytes must not unwind a client, and
   the incremental UTF-8 buffer must not wedge on an invalid sequence.
2. **Never cross-delivers** (T-113-66) — a frame reaches the caller only if the
   input carried THIS subscription's id. The check is skipped for inputs
   containing a backslash: a `\u`-escaped id decodes to the SAME id without those
   bytes appearing literally, so asserting there would have produced a SPURIOUS
   crash artifact (verification finding WR-08, closed by this plan).
3. **The overflow latch never clears** (T-113-73) — once `overflowed()` is true
   it stays true for the rest of the stream. `read_next_frame` polls it once per
   body frame and ends the stream on the first `true`; a clearable flag would let
   a peer hide a discarded line behind a subsequent well-formed one.

---

## 5. Why this was run directly and not through `make quality-gate`

**D-113-G.** The gate's `test-fuzz` stage sets `CARGO = cargo` (stable), while
`cargo fuzz` requires nightly for `-Zsanitizer=address`. Every one of the 17
targets therefore fails to build, and each failure is swallowed by `|| echo`
before the gate prints "ALL ALWAYS requirements validated". Confirmed again
during this plan — the gate's own log carries 17 copies of:

```
Error: failed to build fuzz script: ... -Zsanitizer=address ... (exit status: 1)
```

and still exits 0.

D-113-G is a real defect with no owner. It is a Makefile/tooling concern, NOT a
Phase 113 requirement, and this plan deliberately did **not** edit the Makefile.
The fix shape for whoever picks it up is recorded in this phase's
`deferred-items.md`. Until then, a green gate is not evidence that anything was
fuzzed — which is exactly the repudiation failure (T-113-77) this file exists to
close: commit SHA, toolchain, seed and counters above are checkable, not
asserted.

---

## 6. Reproducing this run

```bash
git checkout e37c381ab5fe5b223183896c4ddf6ec737172f9c
rustup toolchain install nightly            # if absent
rm -rf fuzz/corpus/subscription_listen_frames fuzz/artifacts/subscription_listen_frames
cargo +nightly fuzz run subscription_listen_frames -- -runs=20000
/bin/ls -A fuzz/artifacts/subscription_listen_frames/ | /usr/bin/wc -l   # expect 0
```

Counters will not match byte-for-byte — libFuzzer picks a fresh random seed each
run (this run: `Seed: 872967294`; add `-seed=872967294` to replay this exact
one). What must reproduce is the shape: `#20000 DONE`, exit 0, an EMPTY
artifacts directory, and a corpus containing entries that trip the overflow
branch.

---

## 7. Scope

No manifest changed: `git diff --name-only -- Cargo.toml Cargo.lock fuzz/Cargo.toml`
is empty (the target was registered by 113-13; this plan added no dependency and
installed no package — only a rustup toolchain, per T-113-SC).

No Makefile changed. No requirement checkbox flipped — HTTP-01..05 and
CLNT-01..02 stay `[~]` under the `113-SPEC-RECHECK.md` recorded exception.
