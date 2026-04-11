---
phase: 67-docs-rs-pipeline-and-feature-flags
plan: 06
type: execute
wave: 5
depends_on:
  - 67-01
  - 67-02
  - 67-03
  - 67-04
  - 67-05
files_modified: []
autonomous: true
requirements:
  - DRSD-01
  - DRSD-02
  - DRSD-03
  - DRSD-04
tags:
  - rust
  - verification
  - integration
must_haves:
  truths:
    - "`make quality-gate` exits 0 (the canonical pre-PR gate matches what CI runs)"
    - "`make doc-check` exits 0 (the new rustdoc zero-warnings gate)"
    - "`cargo package --list --allow-dirty` includes `CRATE-README.md` (crate ships with the include_str! source file)"
    - "Cargo.toml `[package.metadata.docs.rs]` features list is byte-identical to Makefile `doc-check` features list (single-source-of-truth invariant)"
    - "Zero manual `doc(cfg(...))` annotations remain in src/ (Plan 02 invariant preserved)"
    - "`src/lib.rs:70` still contains `#![cfg_attr(docsrs, feature(doc_cfg))]` (D-01 amended invariant)"
    - "`pmcp` crate version is still 2.3.0 (D-28: no version bump)"
    - "All 5 ROADMAP.md Phase 67 success criteria are satisfied (with SC#1 interpreted per the D-01 amendment — `feature(doc_cfg)` replaces the factually-outdated `feature(doc_auto_cfg)` text)"
  artifacts: []
  key_links:
    - from: "Cargo.toml [package.metadata.docs.rs] features"
      to: "Makefile doc-check --features"
      via: "byte-identical feature list (single source of truth)"
      pattern: "composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket"
---

<objective>
Final integration verification for Phase 67. This plan mutates **no files** — it runs the aggregate checks that prove every prior plan landed correctly and the phase is ready for verification + commit.

Purpose: Catches integration regressions that individual plan verifications might miss — file-level checks can pass while invariants between files drift. This plan specifically verifies the single-source-of-truth invariant between Cargo.toml, Makefile, and CRATE-README.md feature lists, and the canonical `make quality-gate` check that CI runs on every PR.

Output: Zero file changes. A verification report (as the plan summary) confirming all invariants hold. The phase is then ready for `/gsd-verify-work` and the final commit.
</objective>

<execution_context>
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/workflows/execute-plan.md
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-CONTEXT.md
@.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-RESEARCH.md
@.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-VALIDATION.md
@.planning/ROADMAP.md
@Cargo.toml
@Makefile
@CRATE-README.md
@src/lib.rs
@.github/workflows/ci.yml

<interfaces>
<!-- The 5 ROADMAP success criteria for Phase 67 as currently written. -->
<!-- SC#1 text is factually outdated per CONTEXT.md D-01 amendment; interpretation below. -->

From `.planning/ROADMAP.md:727-732`:

1. `src/lib.rs` contains `#![cfg_attr(docsrs, feature(doc_auto_cfg))]` and all ~145 feature-gated items on docs.rs display automatic feature availability badges
   - **AMENDED:** `src/lib.rs` contains `#![cfg_attr(docsrs, feature(doc_cfg))]` (NOT `doc_auto_cfg`, which was removed in Rust 1.92.0). RFC 3631 now makes `feature(doc_cfg)` provide automatic feature badges. Verification is GREP against the actual line + the Plan 02 invariant (zero manual annotations).
2. `Cargo.toml` `[package.metadata.docs.rs]` uses an explicit feature list (~13 user-facing features) instead of `all-features = true` — the exact count is 15 per D-16.
3. A feature flag table in `lib.rs` doc comments documents all user-facing features — per D-04, this table lives in `CRATE-README.md` (which `lib.rs` includes via `include_str!`) so `rg '## Cargo Features' src/lib.rs` returns 0 matches; the table is in `CRATE-README.md`.
4. `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps` exits with zero warnings — **AMENDED:** per D-25 we use the explicit D-16 feature list, not `--all-features`. The gate command is `make doc-check`.
5. CI includes a `make doc-check` target that enforces zero rustdoc warnings on every PR — Plan 05 Task 2 satisfies this.

**Feature-list canonicalization command (Task 1 uses this verbatim):**

```bash
# Extract the Cargo.toml docs.rs features list (sorted, one per line).
cargo_list=$(awk '/^\[package.metadata.docs.rs\]/,/^\[/{if (/^\[/ && !/docs.rs/) exit; print}' Cargo.toml \
  | awk '/features = \[/,/\]/' \
  | grep -oE '"[a-z0-9-]+"' \
  | tr -d '"' \
  | sort)

# Extract the Makefile doc-check target features list (sorted, one per line).
make_list=$(awk '/^\.PHONY: doc-check/,/^\.PHONY:/{if (/^\.PHONY:/ && !/doc-check/) exit; print}' Makefile \
  | grep -oE 'features [a-z0-9,-]+' \
  | head -1 \
  | sed 's/features //' \
  | tr ',' '\n' \
  | sort)

# Diff them. Exit 0 if identical.
diff <(printf '%s\n' "$cargo_list") <(printf '%s\n' "$make_list")
```
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Aggregate integration verification — make quality-gate + make doc-check + package list + single-source-of-truth invariant</name>
  <files></files>
  <read_first>
    - .planning/ROADMAP.md lines 723–733 (the 5 Phase 67 success criteria)
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-CONTEXT.md (D-01 amendment — SC#1 interpretation; D-28 — no version bump)
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-VALIDATION.md (task 67-06-01, 67-06-02, 67-06-03 for the exact commands)
    - Cargo.toml (verify version, [package.metadata.docs.rs] block, exclude list)
    - Makefile (verify doc-check target exists)
    - CRATE-README.md (verify file exists at root)
    - src/lib.rs (verify include_str! line, feature(doc_cfg) line, no doc_auto_cfg, no manual annotations)
    - .github/workflows/ci.yml (verify Check rustdoc zero-warnings step)
  </read_first>
  <action>
Run the full integration check sequence. **No files are modified by this task — this is a verification gate only.** If any command fails, the task fails and the executor must stop, diagnose which prior plan's invariant was violated, and return to that plan for a fix-up.

**Check 1: `make quality-gate` passes end-to-end.**

```
make quality-gate
```

Expected: exit 0. This runs the canonical CI-matching command (fmt, clippy pedantic+nursery, build, test, audit). If this fails, one of the upstream plans introduced a regression — most likely Plan 04 (warning cleanup may have accidentally touched non-doc code) or Plan 03 (include_str! flip may have exposed a broken doctest).

Timeout: up to 10 minutes.

**Check 2: `make doc-check` passes.**

```
make doc-check
```

Expected: exit 0. This is the new target from Plan 05 running `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features <D-16 list>`.

**Check 3: `cargo package --list --allow-dirty` includes CRATE-README.md.**

```
cargo package --list --allow-dirty | grep -E '^CRATE-README\.md$'
```

Expected: prints `CRATE-README.md` on exit 0. Confirms Plan 03's new file actually ships with the published crate (required for `include_str!` to resolve on crates.io/docs.rs).

**Check 4: Single-source-of-truth invariant — Cargo.toml features list equals Makefile doc-check features list.**

Run the canonicalization commands from the `<interfaces>` block above:

```
cargo_list=$(awk '/^\[package.metadata.docs.rs\]/,/^\[/{if (/^\[/ && !/docs.rs/) exit; print}' Cargo.toml \
  | awk '/features = \[/,/\]/' \
  | grep -oE '"[a-z0-9-]+"' \
  | tr -d '"' \
  | sort)

make_list=$(awk '/^\.PHONY: doc-check/,/^\.PHONY:/{if (/^\.PHONY:/ && !/doc-check/) exit; print}' Makefile \
  | grep -oE 'features [a-z0-9,-]+' \
  | head -1 \
  | sed 's/features //' \
  | tr ',' '\n' \
  | sort)

diff <(printf '%s\n' "$cargo_list") <(printf '%s\n' "$make_list")
```

Expected: no output, exit 0 (lists identical). If different, a drift exists between Plan 01 (Cargo.toml) and Plan 05 (Makefile). Fix the drift in whichever file is wrong — the canonical authority per CONTEXT.md D-16 is Cargo.toml.

Verify CRATE-README.md feature table has 16 individual features — the 15 from Cargo.toml docs.rs metadata plus `logging` (D-13 amendment: CRATE-README.md documents `logging` as its own row even though Cargo.toml omits it because `default = ["logging"]` auto-includes it). It also has 2 meta rows for `default` and `full`, making 18 total data rows:

```
readme_features=$(awk '/^## Cargo Features/,/^## /{if (/^## / && !/Cargo Features/) exit; print}' CRATE-README.md \
  | grep -oE '^\| `[a-z0-9-]+`' \
  | sed 's/^| `//; s/`$//' \
  | grep -v -E '^(default|full)$' \
  | sort)

echo "$readme_features" | diff - <(printf '%s\n' "$cargo_list")
```

Expected: the diff shows **exactly one line of difference** — `logging` is present in CRATE-README.md but absent from Cargo.toml docs.rs metadata (per D-13 amendment: CRATE-README.md documents `logging` as its own row; Cargo.toml omits `logging` because `default = ["logging"]` auto-includes it). The invariant is NOT "identical lists" — it is "CRATE-README.md individual features minus Cargo.toml docs.rs features = {logging}" exactly. Any other diff line (extra or missing feature beyond `logging`) is a regression. In `diff` default (BSD) output the extra line appears as `< logging`; in unified-diff (`diff -u`) it appears as `-logging`. Either way, exactly one differing feature name, and that name must be `logging`:
```
+ logging
```
This is the only allowed asymmetry — verify it, then move on.

**Check 5: D-01 amended invariant — `feature(doc_cfg)` present, `doc_auto_cfg` absent.**

```
grep -c '^#!\[cfg_attr(docsrs, feature(doc_cfg))\]$' src/lib.rs  # must be 1
grep -c 'doc_auto_cfg' src/lib.rs  # must be 0
```

**Check 6: Plan 02 invariant — zero manual annotations.**

```
rg '#\[cfg_attr\(docsrs, doc\(cfg' src/ -c  # must return no matches (count 0)
```

**Check 7: Plan 03 invariant — include_str! wired, no leftover `//!` module doc.**

```
grep -c '^#!\[doc = include_str!("../CRATE-README.md")\]$' src/lib.rs  # must be 1
grep -c '^//!' src/lib.rs  # must be 0
```

**Check 8: D-28 invariant — no version bump.**

```
grep -c '^version = "2.3.0"$' Cargo.toml  # must be 1 (top-level package version unchanged)
```

**Check 9: D-29 invariant — no pmcp-macros edits.**

```
git status --porcelain pmcp-macros/ 2>/dev/null | grep -v '^??' | wc -l  # must be 0 (no pmcp-macros files in the phase diff)
```
(If the executor started from a clean working tree when the phase began, this check correctly ensures zero tracked-file modifications in pmcp-macros/. If the working tree was dirty at phase start, use git diff against the phase-start SHA instead.)

**Check 10: CI workflow has the new step.**

```
grep -c 'Check rustdoc zero-warnings' .github/workflows/ci.yml  # must be 1
grep -A 1 'Check rustdoc zero-warnings' .github/workflows/ci.yml | grep -c 'run: make doc-check'  # must be 1
```

**Check 11: CRATE-README.md table has 18 data rows.**

```
grep -c '^| `' CRATE-README.md  # must be 18 (2 meta + 16 individual)
```

**Check 12: ROADMAP.md success criterion #1 interpretation.**

Manual verification step (no command): confirm the D-01 amendment in `67-CONTEXT.md` explains why the existing `feature(doc_cfg)` line satisfies the intent of SC#1 even though the literal text "doc_auto_cfg" appears in ROADMAP.md. Per planning_context "do NOT edit ROADMAP.md in this phase" — the amendment lives in CONTEXT.md and is absorbed by the plan. This check is a documentation-consistency confirmation, not a command.

**If ALL 12 checks pass, the phase is ready for verify and commit.** If any check fails:

1. Identify the failing check's owning plan (Check 1 → widest; Checks 2–5 → Plans 01/05, Plan 04, Plan 03, Plans 02/01; Check 6 → Plan 02; Check 7 → Plan 03; Check 8 → Plan 01; Check 9 → Plan 02/04; Check 10 → Plan 05; Check 11 → Plan 03).
2. Stop this plan's execution.
3. Reopen the failing plan's commit, apply the fix, re-run the failing check.
4. Resume this plan from Check 1.

Do NOT edit any files in this task other than to fix an identified regression in an upstream plan. The default outcome of this task is zero file mutations — it's a pure verification gate.
  </action>
  <verify>
    <automated>make quality-gate && make doc-check && cargo package --list --allow-dirty 2>/dev/null | grep -qE '^CRATE-README\.md$' && grep -c '^#!\[cfg_attr(docsrs, feature(doc_cfg))\]$' src/lib.rs | grep -qx 1 && [ "$(grep -c 'doc_auto_cfg' src/lib.rs)" = "0" ] && [ "$(rg '#\[cfg_attr\(docsrs, doc\(cfg' src/ --count-matches | awk -F: '{s+=$2} END {print s+0}')" = "0" ] && grep -q '^#!\[doc = include_str!("../CRATE-README.md")\]$' src/lib.rs && [ "$(grep -c '^//!' src/lib.rs)" = "0" ] && grep -q '^version = "2.3.0"$' Cargo.toml && grep -q 'Check rustdoc zero-warnings' .github/workflows/ci.yml && [ "$(grep -c '^| `' CRATE-README.md)" = "18" ]</automated>
  </verify>
  <acceptance_criteria>
    - `make quality-gate` exits 0 (the canonical CI-matching gate)
    - `make doc-check` exits 0 (zero rustdoc warnings on D-16 feature list)
    - `cargo package --list --allow-dirty 2>/dev/null | grep -qE '^CRATE-README\.md$'` succeeds (file ships with the crate)
    - `grep -c '^#!\[cfg_attr(docsrs, feature(doc_cfg))\]$' src/lib.rs` returns `1`
    - `grep -c 'doc_auto_cfg' src/lib.rs` returns `0`
    - `rg '#\[cfg_attr\(docsrs, doc\(cfg' src/` returns nothing (no manual annotations remain)
    - `grep -c '^#!\[doc = include_str!("../CRATE-README.md")\]$' src/lib.rs` returns `1`
    - `grep -c '^//!' src/lib.rs` returns `0`
    - `grep -c '^version = "2.3.0"$' Cargo.toml` returns `1` (D-28)
    - `grep -c 'Check rustdoc zero-warnings' .github/workflows/ci.yml` returns `1`
    - `grep -A 1 'Check rustdoc zero-warnings' .github/workflows/ci.yml | grep -c 'run: make doc-check'` returns `1`
    - `grep -c '^| `' CRATE-README.md` returns `18` (2 meta + 16 individual rows)
    - `git status --porcelain pmcp-macros/ 2>/dev/null | grep -v '^??' | wc -l` returns `0` (no pmcp-macros edits)
    - Single-source-of-truth diff: Cargo.toml features list equals Makefile doc-check features list (exact match after sort/normalize)
    - CRATE-README.md table has exactly the 15 features from Cargo.toml plus one additional row for `logging` (allowed asymmetry per D-13)
    - No new `#![allow(rustdoc::...)]` suppressions in src/: `grep -rc '#!\[allow(rustdoc::' src/` returns `0`
    - This task modified zero files: `git status --porcelain | wc -l` at task start equals `git status --porcelain | wc -l` at task end (ignoring ignore this check if debugging and the executor had to make upstream fixes — in that case, document the upstream plan that was amended)
  </acceptance_criteria>
  <done>
All 12 integration checks pass. Phase 67 is ready for `/gsd-verify-work` and the final commit. No files were modified by this plan under normal execution.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

None introduced — this plan performs read-only verification.

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-67-06-01 | Information disclosure | `cargo package --list` output | accept | The output includes every file to be published; this plan only greps for `CRATE-README.md`. No sensitive data exposed. |
| T-67-06-02 | Integrity | Single-source-of-truth drift | mitigate | Check 4's diff between Cargo.toml and Makefile feature lists is the last-line defense against drift. Fails loudly on any asymmetry. |
| T-67-06-03 | Integrity | Accidental pmcp-macros edit | mitigate | Check 9 (`git status --porcelain pmcp-macros/`) catches any accidental Plan 02 or Plan 04 overreach into the pmcp-macros subcrate. |

No runtime attack surface. Plan is read-only verification.
</threat_model>

<verification>
**Wave 5 placement:** This plan depends on every prior plan (01, 02, 03, 04, 05). It is the final gate before `/gsd-verify-work`. No file modifications under normal execution — if any invariant check fails, the executor diagnoses which upstream plan is at fault and reopens that plan for fix-up.

**Failure recovery:** If Check N fails, the executor must NOT patch the failure locally in Plan 06. Instead, amend the upstream plan (identified by the check → plan mapping in the action) and re-run Plan 06 from Check 1. This preserves plan-level atomicity in the git history.

**Manual-only verification (NOT part of this plan's automated checks):** The visual auto-badge fidelity check on nightly — `RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --no-deps --features <D-16 list>` — is called out in 67-VALIDATION.md as a Manual-Only Verification step. It is NOT automated in this plan because it requires a nightly toolchain and a browser visit to inspect rendered HTML. The `/gsd-verify-work` reviewer handles this manual check separately.
</verification>

<success_criteria>
- `make quality-gate` exits 0
- `make doc-check` exits 0
- `cargo package --list --allow-dirty` includes CRATE-README.md
- Cargo.toml `[package.metadata.docs.rs]` features list byte-identical to Makefile `doc-check` features list
- Zero manual `doc(cfg(...))` annotations in src/
- `#![cfg_attr(docsrs, feature(doc_cfg))]` at `src/lib.rs` unchanged; no `doc_auto_cfg` anywhere
- `#![doc = include_str!("../CRATE-README.md")]` at `src/lib.rs`; zero `//!`-prefixed module doc lines
- `pmcp` version still `2.3.0` (D-28)
- CI workflow has `Check rustdoc zero-warnings` step
- CRATE-README.md has 18 data rows in feature table (2 meta + 16 individual including logging)
- No pmcp-macros edits (D-29)
- No new rustdoc suppressions
- All 5 ROADMAP success criteria satisfied (SC#1 interpreted per D-01 amendment)
</success_criteria>

<output>
After completion, create `.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-06-SUMMARY.md` with:
- Full 12-check report with pass/fail per check
- `make quality-gate` duration
- `make doc-check` duration
- Files ultimately published (count from `cargo package --list | wc -l`)
- Confirmation zero files were modified by this plan (unless an upstream regression required fix-up, in which case document which plan was amended)
- Status: READY FOR VERIFY AND COMMIT
</output>
