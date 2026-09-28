---
phase: 128-secure-by-default-input-validation-for-config-driven-servers
plan: 11
subsystem: infra
tags: [release, semver, publish-order, cargo-publish, drift-tests, changelog, documentation]

# Dependency graph
requires:
  - phase: 128-01
    provides: "`pmcp::server::schema_validation::{validate_input, render_refusal, InputViolation}` and the `schema-validation` feature split — the core public surface this release publishes"
  - phase: 128-02
    provides: "`validate_path_placeholder`, `validate_resolved_path`, `PlaceholderRules`, `PlaceholderRefusal`, `PLACEHOLDER_MAX_LENGTH`, `check_input_schema_compiles` — the surface the docs page documents"
  - phase: 128-03
    provides: "the D2 `ParamDecl` vocabulary, the D3 position-scoped cap, `[server.validation]`'s four keys and `ServerConfig::lint()` — the config surface the CHANGELOG's forward-incompatibility note and the docs page's opt-out section describe"
  - phase: 128-04
    provides: "E3 garde on `TypedTool`, the `garde` `derive` feature, and `#[deprecated(since = \"2.21.0\")]` on `server::validation` — the deprecation this release's version number had to match"
  - phase: 128-05
    provides: "D-09's breaking `HttpExecutor` contract change, the `schema-validation` feature edge on `pmcp-code-mode` (H1) that CREATED the FORK-1 deadlock, and the narrowed `?` rule the rollout note had to describe correctly"
  - phase: 128-06
    provides: "D4 on the curated surface, `Parameter`'s three new fields, and `ConfigValidationError::MalformedPathTemplateSegment` — the boot-blocking change the rollout note leads with"
  - phase: 128-07
    provides: "`cargo pmcp validate config`, the direct `cargo-pmcp -> pmcp-server-toolkit` edge (the eighth pin), the ninth `pmcp-workbook-compiler` pin, and the SC-3 wording deviation this plan amends"
  - phase: 128-08
    provides: "`HttpCodeExecutor::with_schema`/`has_schema` and `ServerConfig::lint_against_spec` — the additive toolkit API behind the MINOR bump"
  - phase: 128-09
    provides: "E1/E2 (`RequestPolicy`, `ArgumentValidator`, `ToolkitHooks`), `build_server`'s breaking fifth parameter, and the verbatim startup-log line formats the docs page reproduces"
  - phase: 128-10
    provides: "SC-6's sweep, SC-7's fuzz invariants, and the reassigned D-15 CHANGELOG obligation this plan discharges"
provides:
  - "Twelve published crate versions, every manifest pin and eight scaffold-emitted literals moved in ONE commit (D-14), with `cargo build --workspace` and `./scripts/check-release-coverage.sh` green"
  - "FORK 1 exit (b): root `Cargo.toml`'s `pmcp-code-mode` and `pmcp-code-mode-derive` dev-deps are PATH-ONLY, and `release.yml` publishes `pmcp` AHEAD of `pmcp-code-mode`"
  - "`tests/root_dev_dep_path_only.rs` — the path-only tripwire, a negative control on `pmcp-macros`, and a structural assertion that `s41`'s `required-features` are not satisfiable from `default`"
  - "`scripts/check-release-coverage.sh`'s second bounded order region (`BEGIN PHASE-128 ORDER ASSERTION`), proven able to fail"
  - "Five new MAJOR.MINOR scaffold drift tests for `sql_server.rs` and `openapi_server.rs`, closing five previously-unguarded version emitters"
  - "`pmcp_server_toolkit::VERSION` and the SC-3 toolkit-version banner on both lint surfaces"
  - "`CHANGELOG.md`'s 2.21.0 entry: the whole behaviour change, the WIDENED D-15 forward-incompatibility, and all eleven deviations"
  - "`docs/architecture/input-validation.md` — the three layers, the two UMLS worked examples, the warn-vs-refuse asymmetry, the two-engine regex table, `format`, the startup log and the two message surfaces a team owns"
  - "`.planning/ROADMAP.md` SC-3 amended to a version-scoped claim with the original wording retained"
  - "`CLAUDE.md` items 2, 3, 4 and 15a corrected: the false `code-mode`-feature rationale, the new publish order, the stale `cargo-pmcp` version, and both `cargo-pmcp -> pmcp-server-toolkit` pins"
affects: [gsd-ship, next-release, phase-129]

# Actuals (#2632) — chars/4 over the realized diff, NOT a harness token count.
actuals:
  tokens: 30831
  tasks: 4
  commits: 5
  plan_head_before: bfc4e0fe5b1d1aa97614692555b2bf9462a51626
  # `tokens`: `git diff bfc4e0fe..HEAD | wc -c` == 123322 at the four-code/docs-commit
  # point, /4 — the same estimateTokens scale the plan's `estimate: 65000` used, so the
  # two are comparable. The plan over-estimated by ~53%; NOT rounded toward the estimate.
  #
  # `commits: 5` is MEASURED as `git rev-list --count bfc4e0fe..HEAD` AT THE PLAN TIP:
  # 4 production commits (396b6500, 6c1d4946, 18e4c87a, 11c86aff) + the single close-out
  # commit (cae924b2) carrying this SUMMARY together with STATE.md / ROADMAP.md /
  # WINDOWS.md.
  #
  # First written as **6** on the assumption — recorded by plans 09 and 10 — that the
  # SDK's `query commit` writes the metadata as its OWN commit separate from the
  # SUMMARY's. Measured, it did NOT here: all four files went into one commit. Corrected
  # to 5 by AMENDING it into that commit rather than adding a sixth, so the number does
  # not chase itself. `/gsd-verify-work` re-measuring from `plan_head_before` with the
  # same instrument will see 5.
  # `tasks: 4` counts Task 1 (the checkpoint, resolved before this executor ran) plus
  # Tasks 2-4, which are this executor's work.

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "A published release order is machine-checked by a SECOND deliberately-named bounded region, not by widening a scan the prior region explicitly declined to generalize"
    - "A path-only dev-dep is the mechanism, and a tripwire test carrying a NEGATIVE CONTROL (the sibling that must KEEP its version key) is what stops the rule being over-applied"
    - "A scaffold-emitted version literal is guarded at MAJOR.MINOR against the workspace package version, never exactly — an exact guard forces the scaffold to pin an unpublished patch every release cycle"
    - "A version-emitter drift test's non-vacuity is proven by reverting the literal and counting the failures, not asserted"
    - "A lint result prints the version of the implementation that produced it, so a clean result cannot read as a guarantee about a different deployment"

key-files:
  created:
    - tests/root_dev_dep_path_only.rs
    - docs/architecture/input-validation.md
  modified:
    - Cargo.toml
    - server.json
    - .github/workflows/release.yml
    - scripts/check-release-coverage.sh
    - crates/pmcp-server-toolkit/src/lib.rs
    - cargo-pmcp/src/commands/validate.rs
    - cargo-pmcp/src/templates/sql_server.rs
    - cargo-pmcp/src/templates/openapi_server.rs
    - CHANGELOG.md
    - CLAUDE.md
    - .planning/ROADMAP.md

key-decisions:
  - "FORK 1 resolved by exit (b) exactly as specified, on the operator's verbatim `exit-b-as-specified`. Both root code-mode dev-deps path-only, `release.yml` reordered, and BOTH guards landed in the same commit."
  - "SC-3's mixed-version gap resolved by the operator's `Version banner in lint output`: `pmcp_server_toolkit::VERSION` is new additive toolkit API, both lint surfaces print it, and neither hard-errors on a mismatch — the CLI cannot know which toolkit the deployment runs, so refusing would be refusing on a guess."
  - "`server.json` 2.20.4 -> 2.21.0 folded INTO the release commit rather than followed up. Its own pin test's failure message requires the same commit, and a follow-up would leave one commit where `release.yml`'s publish-mcp job would 400 on a duplicate version."
  - "`input-validation` was NOT added to `openapi-code-mode`'s feature list (H-08b). Widening a published feature's list is a compatibility decision with no need behind it: the off-half is safe and `input-validation` is already in the toolkit's `default`."
  - "Contract YAML for the eleven new public symbols was NOT authored. `../provable-contracts` is present but its documented `contracts/<crate>/<name>.yaml` tree does not exist (one commit, README only), so authoring them would invent a format — which the plan's precondition forbids in substance."
  - "Three scaffold templates outside the plan's four named files carry `pmcp` requirements stale by a major or more. Recorded in WINDOWS.md, deliberately NOT fixed — pre-existing, unrelated to this release, and `mcp_app.rs` carries an exact-string golden assertion that a sweep would have to move too."

patterns-established:
  - "Pattern 1: prove a version-emitter guard non-vacuous by reverting every literal it guards at once and asserting the failure COUNT equals the guard count"
  - "Pattern 2: prove a publish-order gate non-vacuous by running it against a reconstructed copy of the workflow in the OLD order and reading its error"
  - "Pattern 3: prove a manifest-shape change by diffing the ACTUAL published manifest out of `cargo package`'s `.crate`, not by reasoning about Cargo's stripping rules"
  - "Pattern 4: when folding a missed change into an already-made commit, reconstruct with `git reset --soft` + re-staged file sets from `git diff-tree`, never `--hard`, and prove the reconstruction by `git diff <old-tip> HEAD` naming only the intended file"

requirements-completed: [SC-2, SC-3, SC-8]

coverage:
  - id: D1
    description: "Twelve crate versions, fifteen manifest pins and eight scaffold literals land in ONE commit and the workspace builds"
    requirement: "SC-8"
    verification:
      - kind: automated
        ref: "RUSTFLAGS='' cargo build --workspace (exit 0, zero `error[`, zero 'failed to select a version')"
        status: pass
      - kind: automated
        ref: "per-manifest enumeration of all twelve expected versions -> '12/12 versions match'"
        status: pass
      - kind: automated
        ref: "./scripts/check-release-coverage.sh (all 25 publishable members have a step)"
        status: pass
    human_judgment: false
  - id: D2
    description: "FORK 1 exit (b): root's two code-mode dev-deps are path-only and `release.yml` publishes `pmcp` first, both guarded"
    requirement: "SC-8"
    verification:
      - kind: unit
        ref: "tests/root_dev_dep_path_only.rs#root_code_mode_dev_deps_are_path_only (+ the pmcp-macros negative control and the s41 required-features assertion) — 3 passed"
        status: pass
      - kind: automated
        ref: "scripts/check-release-coverage.sh PHASE-128 region run against a copy of release.yml in the OLD order -> exit 1 naming both ordinals"
        status: pass
      - kind: automated
        ref: "cargo package -p pmcp --no-verify: the published manifest retains [dev-dependencies.pmcp-macros] and contains NO pmcp-code-mode / pmcp-code-mode-derive / pmcp-agent entry"
        status: pass
    human_judgment: false
  - id: D3
    description: "Five previously-unguarded scaffold version emitters now carry MAJOR.MINOR drift tests"
    requirement: "SC-8"
    verification:
      - kind: unit
        ref: "cargo test -p cargo-pmcp --bins templates -- --test-threads=1 (40 -> 45 passed)"
        status: pass
      - kind: automated
        ref: "mutation: all five literals reverted -> exactly 5 failed, 40 passed"
        status: pass
    human_judgment: false
  - id: D4
    description: "SC-3's toolkit-version banner on both lint surfaces, with a negative on the paths that lint nothing"
    requirement: "SC-3"
    verification:
      - kind: unit
        ref: "cargo-pmcp/src/commands/validate.rs#toolkit_lint_banner_{names_the_linked_toolkit_version,states_that_a_clean_result_is_version_scoped} — 2 passed"
        status: pass
      - kind: integration
        ref: "cargo-pmcp/tests/validate_server_config.rs (12 -> 16 passed), incl. validate_deploy_prints_no_banner_when_there_is_no_toolkit_config"
        status: pass
    human_judgment: false
  - id: D5
    description: "The CHANGELOG 2.21.0 rollout note: the behaviour change, the WIDENED D-15 forward-incompatibility, and all eleven deviations"
    requirement: "SC-2"
    verification:
      - kind: automated
        ref: "grep -c 'server.validation' CHANGELOG.md == 4; grep -c 'validate config' == 3; grep -c 'FORK 1|path-only' == 4"
        status: pass
    human_judgment: true
    rationale: "Whether an operator can actually reconstruct the FORK-1 argument and plan a rollback from this prose is a judgment a grep cannot make."
  - id: D6
    description: "`docs/architecture/input-validation.md` — seven sections including the two-engine regex table and the warn-vs-refuse asymmetry"
    requirement: "SC-2"
    verification:
      - kind: automated
        ref: "git ls-files --error-unmatch (tracked); grep -c U+3000 == 2; warn_on_schema_mismatch == 2; RequestPolicy == 9"
        status: pass
      - kind: automated
        ref: "RUSTFLAGS='' make doc-check (exit 0, zero rustdoc warnings)"
        status: pass
    human_judgment: true
    rationale: "Whether a team can read one page and know which layer a rule belongs in is exactly the question automation cannot answer."
  - id: D7
    description: "The three ledger corrections in CLAUDE.md and SC-3's ROADMAP amendment"
    requirement: "SC-3"
    verification:
      - kind: automated
        ref: "grep -c '0.23.0' CLAUDE.md == 0; 'code-mode feature' == 0; 'retained dev-dep|retained in the published manifest' 2 -> 4; 'pmcp-server-toolkit' 10 -> 16"
        status: pass
      - kind: automated
        ref: "RUSTFLAGS='' make lint-plans (exit 0)"
        status: pass
    human_judgment: false

# Metrics
duration: 108min
completed: 2026-09-28
status: complete
---

# Phase 128 Plan 11: The twelve-crate release, FORK 1's exit, and the rollout note Summary

**Twelve crate versions, fifteen pins, eight scaffold literals, a thirteenth version emitter the plan never enumerated, and a reversed publish order — all in one commit, with both halves of the reversal machine-checked and each guard proven able to fail.**

## Performance

- **Duration:** ~108 min
- **Started:** 2026-09-28T02:17Z (HEAD `bfc4e0fe`)
- **Completed:** 2026-09-28T04:05Z
- **Tasks:** 4 of 4 (Task 1 was the resolved checkpoint; Tasks 2-4 executed here)
- **Files modified:** 27 (2 created, 25 modified), plus SUMMARY / STATE.md / ROADMAP.md

## The two operator decisions, recorded VERBATIM

Task 1 was a `checkpoint:decision` (gate: blocking) on the one-way FORK-1 change, reached
and answered before this executor ran. The operator's response, verbatim:

```
exit-b-as-specified
```

That selected exit (b) exactly as the plan specifies it: make root `Cargo.toml`'s
`pmcp-code-mode` and `pmcp-code-mode-derive` dev-deps **path-only**, move `release.yml`'s
`Publish pmcp (core SDK)` step **ahead of** `Publish pmcp-code-mode`, and add **both**
guards. All three landed, in one commit, unmodified from the specification.

The second decision, the SC-3 resolution, verbatim:

```
Version banner in lint output
```

`cargo pmcp validate config` now always prints which `pmcp-server-toolkit` performed the
lint, and does **not** hard-error on a mismatch. Plan 07 did not implement it (checked:
`validate.rs` had no version reference at all), so it was built here — see Deviation 1.

**Scope of each decision.** `exit-b-as-specified` authorizes FORK 1 only; `Version banner in
lint output` authorizes the SC-3 surface only. The other three decisions carried in the
dispatch (`publish-as-specified` from plan 01, `breaking-newtype` and
`Narrow the '?' rule only` from plan 05) name option ids this plan's gates do not, and were
treated as authority for nothing here.

## The `s41` safety reading — the one measured way exit (b) could fail, and it CLEARS

`cargo publish -p pmcp` runs a verify build, and Cargo strips a path-only dev-dep at publish
time. So a code-mode-consuming example reachable from the verify build would fail to compile
against the stripped dep. Measured before the edit and re-asserted structurally after:

| Fact | Reading |
|---|---|
| Examples consuming those dev-deps | **exactly one** — `grep -rln 'pmcp_code_mode' examples/` returns `examples/s41_code_mode_graphql.rs` alone |
| Its `[[example]]` stanza | `Cargo.toml` `name` / `path` / **`required-features = ["full"]`** |
| Root `default` | `["logging", "v1-compat"]` |
| Does `default` satisfy `full`? | **NO** — `full` is a SIBLING of `default`, never reachable from it |
| Consequence | the publish verify build **SKIPS `s41`** rather than failing on the stripped dep |

This is the same condition the in-tree `pmcp-agent` / `s53` comment at root `Cargo.toml`
already records as making its own path-only entry safe. It is now also asserted rather than
trusted: `s41_example_requires_a_feature_default_does_not_satisfy` expands `default` one
level (with a positive control that the expansion actually found `logging`, or the
"not in the default set" assertion would pass on an empty set) and fails if every one of
`s41`'s required features becomes satisfiable from `default`.

**And the end-to-end fact, MEASURED rather than argued.** `cargo package -p pmcp --no-verify`
was run and the published manifest extracted from the `.crate`:

```
[dev-dependencies.pmcp-macros]     version = "0.6.1"   <- RETAINED (correct: publishes ahead of pmcp)
pmcp-code-mode                     (absent)            <- STRIPPED
pmcp-code-mode-derive              (absent)            <- STRIPPED
pmcp-agent                         (absent)            <- STRIPPED (pre-existing path-only entry)
```

`cargo package -p pmcp` and `cargo package -p pmcp-server-toolkit` both exit 0.

## The twelve versions — measured before/after

Every "From" value was read and compared to the plan's table before any write. **All twelve
matched**, so nothing drifted between planning and execution.

| # | Manifest | From (measured) | To | Axis |
|---|---|---|---|---|
| 1 | `Cargo.toml` (`pmcp`) | 2.20.4 | **2.21.0** | minor — D1 entry point, D-04 feature split, D-03's deprecation |
| 2 | `crates/pmcp-code-mode` | 0.5.4 | **0.6.0** | breaking — D-09's `HttpExecutor` contract |
| 3 | `crates/pmcp-code-mode-derive` | 0.3.0 | **0.3.1** | patch (Q4) — only a dev-dep requirement changed |
| 4 | `crates/pmcp-server-toolkit` | 0.1.3 | **0.2.0** | breaking on 0.x — the set-driver |
| 5 | `crates/pmcp-toolkit-postgres` | 0.1.0 | **0.2.0** | re-exposes `SqlConnector` publicly |
| 6 | `crates/pmcp-toolkit-mysql` | 0.1.1 | **0.2.0** | same |
| 7 | `crates/pmcp-toolkit-athena` | 0.1.0 | **0.2.0** | same |
| 8 | `crates/pmcp-sql-server` | 0.1.1 | **0.2.0** | binary; own config surface changed |
| 9 | `crates/pmcp-openapi-server` | 0.1.2 | **0.2.0** | same, plus `build_server`'s breaking 5th param |
| 10 | `crates/pmcp-workbook-server` | 0.1.1 | **0.2.0** | same |
| 11 | `crates/pmcp-workbook-compiler` | 0.1.3 | **0.2.0** | lib + binary; pins the toolkit |
| 12 | `cargo-pmcp` | 0.24.3 | **0.25.0** | minor — `validate config` + the lint banner |
| **13** | **`server.json`** | **2.20.4** | **2.21.0** | **NOT in the plan's ledger — see Deviation 2** |

## Every pin that moved, and every pin deliberately NOT moved

### Moved (15 manifest pins)

| File:line (measured) | From | To | Why |
|---|---|---|---|
| `Cargo.toml:271` `pmcp-code-mode` | `version = "0.5.3"` + `path` | **PATH-ONLY** | FORK 1 exit (b) |
| `Cargo.toml:272` `pmcp-code-mode-derive` | `version = "0.3.0"` + `path` | **PATH-ONLY** | FORK 1 exit (b) — see the supersession note below |
| `crates/pmcp-code-mode/Cargo.toml:47` `pmcp` | `>=2.2.0` | `>=2.21.0` | plan 05's H1 hand-off |
| `crates/pmcp-code-mode-derive/Cargo.toml:27` `pmcp-code-mode` | `0.5.0` | `0.6` | `^0.5` does not admit 0.6.0 |
| `crates/pmcp-server-toolkit/Cargo.toml:23` `pmcp` | `2.9.0` | `2.21` | must reach `validate_input` |
| `crates/pmcp-server-toolkit/Cargo.toml:24` `pmcp-code-mode` | `0.5.3` | `0.6` | 0.x incompatible |
| `crates/pmcp-toolkit-postgres/Cargo.toml:28` toolkit | `0.1.0` | `0.2` | |
| `crates/pmcp-toolkit-mysql/Cargo.toml:28` toolkit | `0.1.0` | `0.2` | |
| `crates/pmcp-toolkit-athena/Cargo.toml:28` toolkit | `0.1.0` | `0.2` | |
| `crates/pmcp-sql-server/Cargo.toml:33` toolkit | `0.1.0` | `0.2` | |
| `crates/pmcp-sql-server/Cargo.toml:34` `pmcp-toolkit-postgres` | `0.1.0` | `0.2` | **absent from D-13's ledger** |
| `crates/pmcp-sql-server/Cargo.toml:35` `pmcp-toolkit-mysql` | `0.1.0` | `0.2` | **absent from D-13's ledger** |
| `crates/pmcp-sql-server/Cargo.toml:36` `pmcp-toolkit-athena` | `0.1.0` | `0.2` | **absent from D-13's ledger** |
| `crates/pmcp-openapi-server/Cargo.toml:47` toolkit | `0.1.2` | `0.2` | `^0.1.2` also fails |
| `crates/pmcp-workbook-server/Cargo.toml:50` toolkit | `0.1.0` | `0.2` | plan said `:43` |
| `crates/pmcp-workbook-compiler/Cargo.toml:113` toolkit | `0.1.0` | `0.2` | plan said `:107` |
| `cargo-pmcp/Cargo.toml:101` toolkit | `0.1.0` | `0.2` | the EIGHTH pin (128-07 H1) |
| `cargo-pmcp/Cargo.toml:75` `pmcp-workbook-compiler` | `0.1.3` | `0.2` | the NINTH pin (128-07 H2) |

**Line numbers were matched BY CRATE NAME, never by the plan's numbers.** Four of the plan's
line references were stale (`code-mode:29`→`:47`, `workbook-server:43`→`:50`,
`compiler:107`→`:113`, root `:263`/`:264`→`:271`/`:272`), and the exact-line-match helper
would have failed loudly rather than editing the wrong line.

### Deliberately NOT moved, each with its reason

- **`Cargo.toml:270` `pmcp-macros = { version = "0.6.1", path = ... }` KEEPS its version
  key.** `pmcp-macros` publishes at `release.yml` ahead of `pmcp` (CLAUDE.md item 1b), so the
  requirement resolves at publish time and states a real compatibility claim. The FORK-1 rule
  is about publish ORDER, not about making every dev-dep look alike, and
  `root_pmcp_macros_dev_dep_still_carries_its_version_requirement` is the negative control
  that stops the rule being over-applied.
- **`crates/pmcp-server-toolkit/fuzz/Cargo.toml:13`** is `{ path = ".." }`, still path-only.
  Verified after the commit. Do not add a key.
- **Every other `pmcp` requirement in the tree.** Re-measured after the release commit
  (§ *The re-measured caret readings* below): all are carets on the 2.x line and all admit
  2.21.0, so CLAUDE.md's caret exception covers the `pmcp` half. It does **not** cover the
  `pmcp-code-mode` or toolkit halves, which are minor bumps on pre-1.0 lines.
- **`input-validation` was NOT added to `openapi-code-mode`'s feature list** (128-08 H-08b).
  My call, as the hand-off said it would be. Widening a published feature's list is a
  compatibility decision, and there is no need behind it: the off-half of
  `spec_placeholder_rules` is safe (floor and cap still run through `pmcp-code-mode`'s
  unconditional `pmcp/schema-validation` forward) and `input-validation` is already in the
  toolkit's `default`.

### The SUPERSEDED instruction about root `Cargo.toml`'s derive dev-dep — explicitly

An earlier draft of this plan said root's `pmcp-code-mode-derive` dev-dep does **not** move,
"per Q4 / the caret exception". **That instruction is SUPERSEDED and the entry DID move to
path-only.** The old reasoning is correct about the VERSION axis — a patch bump needs no
downstream repin — and **irrelevant to FORK 1**, which is about the `version` key EXISTING at
all. That entry is the same retained-dev-dep shape as its sibling, so leaving its key would
have kept `cargo publish -p pmcp` blocked on an unpublished `pmcp-code-mode-derive 0.3.1` and
exit (b) would have unblocked nothing.

Recorded here, in the CHANGELOG's deviation list (item 10) and in a `# Why:` comment on the
entries themselves, so a future releaser cannot restore the pin on the strength of the old
note.

## The re-measured `pmcp` caret readings

Read AFTER the release commit, so these are the shipped values:

| Requirement | Admits 2.21.0? |
|---|---|
| `crates/mcp-tester:21` `2.19.0` | ✅ |
| `crates/pmcp-agent:17`, `:61` `2.17.0` | ✅ |
| `crates/pmcp-code-mode-derive:26` `>=2.2.0` (dev-dep) | ✅ (open lower bound) |
| `crates/pmcp-code-mode:47` `>=2.21.0` | ✅ (moved by this plan) |
| `crates/pmcp-openapi-server:40` `2.9.0` | ✅ |
| `crates/pmcp-server-toolkit:23` `2.21` | ✅ (moved by this plan) |
| `crates/pmcp-server:30` `2.8.1` | ✅ |
| `crates/pmcp-sql-server:32` `2.9.0` | ✅ |
| `crates/pmcp-tasks:11`, `:37` `2.8.1` | ✅ |
| `crates/pmcp-team-servers:22`, `:167` `2.17.0` | ✅ |
| `crates/pmcp-workbook-server:29` `2.9.0` | ✅ |
| **`cargo-pmcp:68` `2.20.3` — the TIGHTEST in the tree** | ✅ `^2.20.3` admits 2.21.0 |
| `pmcp-macros:28` `>=1.20.0` | ✅ |

**No further `pmcp` pin moves.** The caret exception holds, measured rather than reasoned —
this is the axis this repo's own memory records as one where prose reasoning is unreliable.

## The scaffold literals — EIGHT, not the nine the plan counted

Measured across the four template files the plan names. Every one moved; the three that
already carried a drift test, and the five that did not and now do:

| Literal | From | To | Drift-tested BEFORE | Drift-tested NOW |
|---|---|---|---|---|
| `workbook_server.rs:53` `PMCP_VERSION` | `"2.20.4"` | `"2.21.0"` | YES (exact) | YES |
| `workbook_server.rs:59` `TOOLKIT_VERSION` | `"0.1.3"` | `"0.2.0"` | YES (exact) | YES |
| `workspace.rs:33` `PMCP_VERSION_REQ` | `"2.20"` | `"2.21"` | YES (MAJOR.MINOR) | YES |
| `sql_server.rs:57` emitted `pmcp` | `"2.8.1"` | `"2.21"` | **NO** | **YES** (new) |
| `sql_server.rs:58` emitted toolkit | `"0.1.0"` | `"0.2"` | **NO** | **YES** (new) |
| `openapi_server.rs:73` emitted `pmcp` | `"2.8.1"` | `"2.21"` | **NO** | **YES** (new) |
| `openapi_server.rs:74` emitted toolkit | `"0.1.0"` | `"0.2"` | **NO** | **YES** (new) |
| `openapi_server.rs:78` emitted `pmcp-openapi-server` | `"0.1.0"` | `"0.2"` | **NO** | **YES** (new) |

**`workspace.rs:33` was the hard blocker the plan predicted**: its MAJOR.MINOR drift test
derives the expectation from root's `[package].version`, so the release commit was
uncreatable until the constant moved.

**The count is EIGHT, not nine.** The plan's prose says "nine literals... six are not
drift-tested" while its own table lists eight rows with three tested — 8/5, not 9/6. An
exhaustive scan of the four files confirms eight: three constants plus five inline literals.
Three further lines matched the scan and are correctly excluded — `workbook_server.rs:89`,
`:99` and `workspace.rs:63` interpolate `{PMCP_VERSION}` / `{TOOLKIT_VERSION}` /
`{PMCP_VERSION_REQ}`, so they derive from the constants and cannot go stale independently.
The `version = "0.1.0"` lines in each template are the SCAFFOLDED PROJECT's own version and
must not move.

### The final scaffold-literal negative grep, and its positive control

The plan's raw expression **over-fires**, as the plan itself warned it might. Measured:

```bash
# RAW (the plan's form) — matches each template's own `version = "0.1.0"`; prints 4, not 0.
grep -cE '"(2\.8\.1|2\.20\.4|0\.1\.0|0\.1\.3|2\.20)"' $TPL | grep -v ':0$' | grep -c .
```

The FINAL expression anchors on the dependency name or the constant name:

```bash
TPL="cargo-pmcp/src/templates/workbook_server.rs cargo-pmcp/src/templates/workspace.rs \
     cargo-pmcp/src/templates/sql_server.rs cargo-pmcp/src/templates/openapi_server.rs"

grep -hE '^(pmcp|pmcp-server-toolkit|pmcp-openapi-server) *=|^const (PMCP_VERSION|PMCP_VERSION_REQ|TOOLKIT_VERSION):' $TPL \
  | grep -cE '"(2\.8\.1|2\.20\.4|2\.20|0\.1\.0|0\.1\.2|0\.1\.3)"'
```

**Reading: 0.** And the POSITIVE CONTROL — the identical scan against the NEW values —
returns **8**, so the zero is a statement about the tree and not a broken scan.

### Non-vacuity of the five new drift tests, MEASURED

All five literals reverted at once, then `cargo test -p cargo-pmcp --bins templates`:

```
test result: FAILED. 40 passed; 5 failed
  templates::openapi_server::tests::emitted_openapi_server_requirement_matches_workspace_major_minor_line ... FAILED
  templates::openapi_server::tests::emitted_pmcp_requirement_matches_workspace_major_minor_line       ... FAILED
  templates::openapi_server::tests::emitted_toolkit_requirement_matches_workspace_major_minor_line    ... FAILED
  templates::sql_server::tests::emitted_pmcp_requirement_matches_workspace_major_minor_line           ... FAILED
  templates::sql_server::tests::emitted_toolkit_requirement_matches_workspace_major_minor_line        ... FAILED
```

Exactly five failures for five reverted literals, then restored and re-verified green at
**45 passed**. `--bins templates` was used, not `--lib templates` (which selects 15 from two
`#[path]` leaves and MISSES `sql_server`/`openapi_server` entirely — a wrong-scope pass, worse
than a zero) and not `--lib templates::` (which selects 0 and exits 0).

## The publish order after the reorder — MEASURED

```
$ grep -n 'cargo publish -p' .github/workflows/release.yml | head -6
141:  cargo publish -p pmcp-widget-utils
159:  cargo publish -p pmcp-macros-support
177:  cargo publish -p pmcp-macros
220:  cargo publish -p pmcp                     <-- MOVED (was :231, after both code-mode steps)
238:  cargo publish -p pmcp-code-mode           <-- was :195
256:  cargo publish -p pmcp-code-mode-derive    <-- was :213
```

**Every step body is byte-identical to before the move.** Proven, not claimed: the
comment-stripped, SORTED line multiset of the whole file is **unchanged (0 lines differ)**
against `HEAD~`, while the unsorted form differs — i.e. only the order moved. The one added
content is a 26-line comment block above the moved step naming Phase 128 FORK 1, the
`schema-validation` requirement that forces the order, the path-only prerequisite, and both
guards.

## The coverage-gate order region, proven able to fail

`scripts/check-release-coverage.sh` gained `BEGIN PHASE-128 ORDER ASSERTION` … `END …`, a
second bounded region using `step_line_re` with the `( |$)` boundary form (`pmcp` is a prefix
of ten-plus crate names; a boundary-less match returns a silently WRONG ordinal, the worse
failure). It is framed in its own comment as a second deliberately-named cluster, NOT a
widening of the scan the D-10 region explicitly declines to generalize.

- Against the real workflow: **exit 0**, `release-coverage: all 25 publishable workspace
  members have a publish step.`
- Against a reconstructed copy of `release.yml` in the OLD order: **exit 1** —
  `::error::pmcp publishes AT OR AFTER pmcp-code-mode … (comment-stripped ordinals: pmcp=178,
  pmcp-code-mode=142)` plus the load-bearing reason and a fix that explicitly refuses the
  wrong remedy ("Do NOT instead give root Cargo.toml's code-mode dev-deps their version keys
  back").
- Sentinel present: `grep -v '^\s*#' scripts/check-release-coverage.sh | grep -c 'BEGIN
  PHASE-128 ORDER ASSERTION'` → **1**.

## Accumulated hand-offs — reconciled against the dispatch list

Every item in the dispatch's accumulated-handoff list was addressed. Two were **absent from
D-13's own ledger** and one was absent from the plan's ledger entirely:

| Hand-off | Status |
|---|---|
| 128-05 H1 — `code-mode`'s `pmcp` requirement → a version carrying `schema-validation` | ✅ `>=2.21.0` at `:47` (plan said `:29`) |
| 128-05 H2 — `pmcp-code-mode` 0.6.0 + its three pins as ONE set | ✅ two repinned, root's made path-only |
| 128-05 H3 — rollout note AS NARROWED; `jsonschema` always compiled | ✅ CHANGELOG deviation 3 + the `pmcp-code-mode` note; the body-param migration is explicitly called FALSE of what ships |
| 128-02 — trailing-`/` refusal | ✅ CHANGELOG "an empty placeholder value is now refused" |
| 128-06 — `MalformedPathTemplateSegment` makes a third-party config refuse to BOOT | ✅ its own CHANGELOG section, stated plainly as a loud failure replacing a silent one |
| 128-07 (a) eighth toolkit pin, (b) ninth compiler pin, (c) `cargo-pmcp` 0.25.0, (d) CLAUDE.md 15a, (e) SC-3 ROADMAP | ✅ all five |
| 128-08 H-08a — toolkit MINOR for additive API | ✅ 0.2.0 (breaking anyway, so the minor is subsumed) |
| 128-08 H-08b — `input-validation` NOT added to `openapi-code-mode` | ✅ decided: still not added, with the reason recorded |
| 128-09 — `build_server`'s breaking 5th param; new toolkit public API | ✅ called out in its own CHANGELOG paragraph |
| 128-04 — `#[deprecated(since = "2.21.0")]`; `garde` `derive` | ✅ **CONFIRMED** correct against the 2.21.0 set (read at `src/server/mod.rs:231`); `derive` CHANGELOGged as an additive dependency change |
| 128-10 — D-15 CHANGELOG obligation, BOTH halves | ✅ its own section naming the six `ParamDecl` keys AND `[server.validation]`, with `ServerSection`/`ServerConfig`'s `deny_unknown_fields` as the reason |
| 128-10 — `ci.yml` nightly/cargo-fuzz provisioning | ✅ recorded as a release blocker in `.planning/WINDOWS.md` #84 and below |
| 128-03 — 19 asserting `format` names + the typo warning | ✅ docs page § 5, with the warning as a blockquote |
| own `must_haves` — warn-vs-refuse asymmetry with its reason | ✅ docs page § 3, naming `output_validation::warn_on_schema_mismatch` |
| own `must_haves` — `\s` as TWO columns, one per regex engine | ✅ docs page § 4, all 19 codepoints |

**Items my ledger LACKED and which are reported here as the dispatch instructed:**

1. **`server.json`** — a thirteenth version emitter. In no plan, no hand-off, no D-13 entry.
   Found by the root suite, not by reasoning. See Deviation 2.
2. **The `VERSION` constant the SC-3 banner needs did not exist.** The banner decision was
   handed to this plan as "check whether plan 07 implemented it"; it had not, and the toolkit
   exposed no version at all, so `pmcp_server_toolkit::VERSION` is new public API this plan
   added. See Deviation 1.

## Task Commits

1. **SC-3's version banner** — `396b6500` (`feat`): `pmcp_server_toolkit::VERSION`,
   `toolkit_lint_banner()`, both lint surfaces, 2 unit + 4 integration tests
   (`validate_server_config` 12 → 16).
2. **Task 2 — the release set, ONE commit (D-14)** — `6c1d4946` (`chore`): twelve versions +
   `server.json`, fifteen pins, eight scaffold literals, five new drift tests,
   `tests/root_dev_dep_path_only.rs`, the `release.yml` reorder, the coverage-gate region.
3. **Task 3 — the rollout note and three ledger corrections** — `18e4c87a` (`docs`):
   `CHANGELOG.md`, `.planning/ROADMAP.md`, `CLAUDE.md`.
4. **Task 4 — the layers documentation page** — `11c86aff` (`docs`):
   `docs/architecture/input-validation.md`.

**Plan metadata:** the commit carrying this SUMMARY, plus the STATE.md/ROADMAP.md commit.

## Files Created/Modified

**Created**
- `tests/root_dev_dep_path_only.rs` — the FORK-1 tripwire: path-only assertion whose failure
  message names the measured `cargo publish -p pmcp` consequence; a negative control on
  `pmcp-macros`; and the `s41` `required-features` structural assertion with its own positive
  control.
- `docs/architecture/input-validation.md` — the seven-section layers page.

**Modified**
- `Cargo.toml` — 2.21.0; both code-mode dev-deps path-only with a 30-line `# Why:` block.
- `server.json` — 2.21.0 (the thirteenth emitter).
- `.github/workflows/release.yml` — `pmcp` moved ahead of `pmcp-code-mode`, step bodies
  byte-identical, with a comment block recording the whole argument.
- `scripts/check-release-coverage.sh` — the second bounded order region.
- `crates/pmcp-server-toolkit/src/lib.rs` — `pub const VERSION` with the SC-3 reasoning.
- `cargo-pmcp/src/commands/validate.rs` — `toolkit_lint_banner()` + 2 unit tests.
- `cargo-pmcp/tests/validate_server_config.rs` — 4 banner tests including the negative.
- `cargo-pmcp/src/templates/{sql_server,openapi_server}.rs` — literals + 5 drift tests + two
  `pub(crate)` test helpers with the MAJOR.MINOR reasoning.
- `cargo-pmcp/src/templates/{workbook_server,workspace}.rs` — the three tested constants.
- eleven `Cargo.toml` files — versions and pins.
- `CHANGELOG.md`, `CLAUDE.md`, `.planning/ROADMAP.md`.

## Deviations from Plan

### 1. [Rule 2 - Missing critical] The SC-3 banner the operator asked for did not exist, and the toolkit exposed no version to print

- **Found during:** Task 2 preparation (checking plan 07's work, as the dispatch instructed)
- **Issue:** `grep 'CARGO_PKG_VERSION\|toolkit version\|banner' cargo-pmcp/src/commands/validate.rs`
  returned **nothing** — plan 07 did not implement the banner, which its own SUMMARY records
  as "still open, a product question for a human". The operator answered it and assigned it
  here. `pmcp-server-toolkit` also exposed **no** version constant
  (`grep 'pub const VERSION' crates/pmcp-server-toolkit/src/` → 0), so there was nothing to
  print: the only in-crate use of `env!("CARGO_PKG_VERSION")` was a private User-Agent
  builder.
- **Fix:** added `pmcp_server_toolkit::VERSION` (additive public API, subsumed by the crate's
  existing MINOR bump) and `toolkit_lint_banner()` in the CLI. The value comes from the crate
  actually LINKED in, not from a manifest read off disk, so it cannot drift from the
  implementation that just ran. Printed unconditionally in `validate config` and on both
  `validate deploy` branches that report a lint RESULT — and **not** on `Absent` or
  `Unreadable`, where nothing was linted.
- **Files modified:** `crates/pmcp-server-toolkit/src/lib.rs`,
  `cargo-pmcp/src/commands/validate.rs`, `cargo-pmcp/tests/validate_server_config.rs`
- **Verification:** 2 unit tests (one carrying a positive control that `VERSION` is non-empty,
  or the substring assertion would be vacuously true) + 4 integration tests including
  `validate_deploy_prints_no_banner_when_there_is_no_toolkit_config`, without which
  "print it everywhere" would satisfy the three positives.
  `validate_server_config` 12 → **16 passed**.
- **Committed in:** `396b6500` — its OWN commit, deliberately kept out of the D-14 release
  commit so that commit stays purely versions/pins/mechanics.

### 2. [Rule 3 - Blocking] `server.json` is a THIRTEENTH version emitter, and the gate found it

- **Found during:** final verification, after the release commit was already made
- **Issue:** `cargo nextest run --features full --no-fail-fast` → **3362 run, 1 failed**:
  `server_json_version_pin::server_json_version_matches_the_crate_version`. `server.json`
  carried `2.20.4`. Neither D-13, nor any prior plan's hand-off, nor this plan's ledger
  enumerated it, and `cargo build` cannot see it. `release.yml`'s `publish-mcp` job publishes
  this file's version to the MCP Registry, which answers a duplicate with
  `400 invalid version: cannot publish duplicate version` — and it fails **silently**, because
  the crates.io publish succeeds independently and the run's only red job is that one. The
  test's own message demands the fix land "in the SAME commit as the crate bump".
- **Fix:** `server.json` → `2.21.0`, **folded into the release commit** rather than followed
  up. A follow-up would have left one commit in this plan where the tree cannot pass its own
  gate and where a tag would silently 400 — the exact inconsistent-intermediate-state class
  D-14 exists to prevent. The commit was reconstructed with `git reset --soft` (no `--hard`,
  no `clean`, no `stash`), the file sets recovered from `git diff-tree --name-only`, and the
  two docs commits re-applied from their saved messages **unchanged**.
- **Files modified:** `server.json`
- **Verification:** `git diff <old-tip> HEAD` names **`server.json` and nothing else** — the
  reconstruction lost nothing. `cargo test -p pmcp --features full --test
  server_json_version_pin` → 2 passed. Full suite re-run: **3362 run, 3362 passed, 6 skipped,
  exit 0**.
- **Committed in:** `6c1d4946` (the release commit; its message carries a dedicated section on
  this finding and on why it was folded in rather than appended)

### 3. [Rule 1 - Bug in a plan gate] The scaffold-literal negative grep over-fires on each template's own `[package].version`

- **Found during:** Task 2 verification
- **Issue:** the plan's raw expression matches `version = "0.1.0"` — the SCAFFOLDED PROJECT's
  own version — in all four templates, so it prints **4** and can never print 0 no matter how
  correct the release is. The plan anticipated this and instructed that the final expression
  be recorded.
- **Fix:** anchored the scan on the dependency name or the constant name (final expression
  reproduced verbatim above). Reads **0**, and a POSITIVE CONTROL on the same scan against
  the NEW values reads **8**, so the zero discriminates.
- **Files modified:** none (a gate correction)
- **Verification:** both readings above
- **Committed in:** n/a — recorded here

### 4. [Rule 1 - Bug in the plan's own prose] The scaffold-literal count is EIGHT, not nine

- **Found during:** Task 2
- **Issue:** the plan's prose says "nine literals... six are not drift-tested" while its own
  table lists eight rows of which three are tested. 8/5, not 9/6.
- **Fix:** exhaustively scanned all four files for version-shaped literals and confirmed
  **eight** movable emitters (three constants + five inline), plus three interpolated lines
  that derive from the constants and one per-file `version = "0.1.0"` that must NOT move. All
  eight moved; the five untested ones now have tests. The count is corrected in the CHANGELOG
  deviation list (item 11) rather than quietly adopted.
- **Files modified:** none (a measurement correction)
- **Verification:** the positive control returning 8
- **Committed in:** n/a

### 5. [Rule 3 - Blocking] `clippy::needless_collect` on the tripwire test under the real gate

- **Found during:** Task 2 (`make lint`, which is stricter than a bare `cargo clippy -D warnings`)
- **Issue:** `s41_example_requires_a_feature_default_does_not_satisfy` collected `unsatisfied`
  and used it only through `.is_empty()`.
- **Fix:** the assertion message now also formats `unsatisfied`, which both satisfies the lint
  honestly and improves the diagnostic — a reader who trips the test learns WHICH features
  became default-satisfiable. Not `#[allow]`ed.
- **Files modified:** `tests/root_dev_dep_path_only.rs`
- **Verification:** `RUSTFLAGS="" make lint` exit 0, `✓ No lint issues`
- **Committed in:** `6c1d4946`

### 6. [Scope boundary — recorded, NOT fixed] Three scaffold templates outside the plan's four carry `pmcp` requirements stale by a major or more

- **Found during:** Task 2's exhaustive emitter scan
- **Issue:** `cargo-pmcp/src/templates/oauth/proxy.rs:468` and `oauth/authorizer.rs:216` emit
  `pmcp = { version = "0.3", ... }`; `mcp_app.rs:348` emits `pmcp = { version = "1.10", ... }`
  with an exact-string golden assertion at `:897`. All three are unguarded and have been wrong
  for many releases.
- **Fix:** **none.** Out of scope: the plan names four template files, these are pre-existing,
  they are not caused by this release, and `mcp_app.rs` carries a golden assertion a sweep
  would have to move too. Recorded in `.planning/WINDOWS.md` (#85, kind `todo`) so it is
  visible at ship time.
- **Files modified:** none
- **Committed in:** n/a

### 7. [Precondition partially unmet — recorded, NOT fabricated] Contract YAML was not authored

- **Found during:** Task 4
- **Issue:** the plan's precondition is "`../provable-contracts` is present". It IS — but its
  own README documents a `contracts/<crate-name>/<contract-name>.yaml` layout that **does not
  exist**: the repository has **one commit and one tracked file** (`README.md`). There is no
  in-tree contract to copy and no schema for `pmat comply check <path>` to validate against.
- **Fix:** the documentation half was completed in full; the eleven contract YAMLs were
  **not** written, because authoring them would invent a format — which the precondition's
  "do not fabricate contract files" forbids in substance even though the directory check
  passes on its letter. Recorded in `.planning/WINDOWS.md` (#83, kind `deviation`).
- **Files modified:** none
- **Committed in:** n/a (stated in `11c86aff`'s message)

---

**Total deviations:** 7 — 2 missing-critical/blocking code findings (1 the operator's own
decision, 1 a release-breaking emitter the gate caught), 2 plan-gate/prose measurement
corrections, 1 blocking lint, and 2 deliberate non-fixes recorded rather than swept in.
**Impact on plan:** no scope was dropped. Deviation 2 is the one that changed history, and it
did so to PRESERVE D-14's invariant rather than to work around it.

## `pmat comply check` — before and after, as the plan requires

Run before writing the docs page and again after. **Byte-comparable:**

| | Before | After |
|---|---|---|
| `✓` checks | 40 | 40 |
| `✗` checks | 5 | 5 |
| `make comply` exit | 0 | 0 |
| `comply-bindings-check` | all four team-servers bindings resolve | all four resolve |

Every `✗` is a pre-existing project-level advisory unrelated to this phase (`CB-200` TDG grade
gate — 13 functions below grade A; `File Health`; `CB-1204`/`CB-1208`/`CB-1308` contract-ladder
advisories that all report "no `.pmat-work/` directory"). `make comply` pipes the `pmat comply`
failure into an informational note by design (D-07), so a clean run is the target and not a
gate; the non-advisory `comply-bindings-check` leg that follows it is green.

`.pmat/` runtime churn (`deps-cache.json`, `metrics/dependencies.json`, `project.toml`) was
restored with `git checkout -- .pmat/` after every pmat invocation, never committed.

## Verification results

| Gate | Result |
|---|---|
| `RUSTFLAGS="" cargo build --workspace` | exit 0, zero `error[`, zero `failed to select a version` |
| twelve-version per-manifest enumeration | `12/12 versions match` |
| toolkit pins off the 0.1 line | **0** — positive control on the 0.2 line: **8** |
| `crates/pmcp-sql-server` connector pins off `"0.1.0"` | **0** |
| `cargo-pmcp` compiler pin off the 0.1 line | **0** |
| scaffold stale-literal scan (final expression) | **0** — positive control on new values: **8** |
| `cargo test -p cargo-pmcp --bins templates` | **45 passed** (was 40); mutation → exactly 5 fail |
| `cargo test -p pmcp --features full --test root_dev_dep_path_only` | **3 passed** |
| `cargo test -p pmcp --features full --test server_json_version_pin` | **2 passed** |
| `cargo package -p pmcp --no-verify --allow-dirty --list` | exit 0 |
| `cargo package -p pmcp-server-toolkit --no-verify --allow-dirty --list` | exit 0 |
| published `pmcp` manifest | `pmcp-macros` retained; all three path-only dev-deps STRIPPED |
| `grep -n 'cargo publish -p'` order | `pmcp` at 220 **before** `pmcp-code-mode` at 238 |
| `release.yml` step-body fidelity | comment-stripped sorted multiset unchanged (0 lines differ) |
| `./scripts/check-release-coverage.sh` | exit 0, all 25 members; **exit 1** on a reverted-order copy |
| PHASE-128 sentinel present (comments stripped) | **1** |
| `RUSTFLAGS="" make lint` | exit 0, `✓ No lint issues` |
| `RUSTFLAGS="" make lint-plans` | exit 0 |
| `RUSTFLAGS="" make doc-check` | exit 0, `✓ Zero rustdoc warnings` |
| `cargo doc -p pmcp-server-toolkit` | **17** warnings — the pre-existing figure, **no rise** |
| `cargo nextest run --features "full" --no-fail-fast` | **3362 run, 3362 passed, 6 skipped**, exit 0 |
| `cargo test -p pmcp-openapi-server --no-fail-fast` | **49 passed**, 0 failed |
| **`RUSTFLAGS="" make quality-gate`** | **exit 0, and the captured 17 910-line output contains `ALL TOYOTA WAY QUALITY CHECKS PASSED`** |

The five ADVISORY `✗` sub-checks inside the gate are the pre-existing set plans 09 and 10 both
verified are not caused by this phase (file health + the four pmat advisories). The gate still
printed its banner.

## Known Stubs

None. No stub, placeholder, `TODO`, `FIXME`, `#[ignore]` or skipped test was added.

Two deliberate NON-stubs, named so a reader does not mistake them for one: the banner's
"does not hard-error on mismatch" is a recorded operator decision with its reasoning, not an
unfinished check; and the five new drift tests compare at MAJOR.MINOR rather than exactly for
the measured reason `workspace.rs` already records, not as a weakening.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: published_release_mechanic_changed | `.github/workflows/release.yml`, `Cargo.toml` | The publish ORDER and root's published manifest SHAPE both changed. One-way, confirmed at Task 1's checkpoint. Guarded by `tests/root_dev_dep_path_only.rs` and the PHASE-128 coverage region, both inside `make quality-gate` — but note NEITHER guard can see the other's half, and reverting either alone strands the release. There is no single check that covers both. |
| threat_flag: new_public_api | `crates/pmcp-server-toolkit/src/lib.rs` | `pub const VERSION` is new published API. Trivial and additive, but there is no `cargo public-api` gate in this repo, so nothing mechanical will catch a later widening near it. |
| threat_flag: accepted_residual | `cargo-pmcp/src/templates/{oauth/proxy,oauth/authorizer,mcp_app}.rs` | Three unguarded `pmcp` version emitters stale by a major or more, scaffolding projects that cannot resolve. Pre-existing and out of this plan's scope; `WINDOWS.md` #85. |
| threat_flag: unrun_ci_provisioning | `.github/workflows/ci.yml` | **RELEASE BLOCKER, carried from 128-10.** The nightly + `cargo-fuzz` provisioning in the `test-fuzz-strict` leg has NEVER run on a GitHub runner. The first CI run IS the measurement. If `cargo install cargo-fuzz` or `cargo +nightly fuzz build` fails there, the correct response is to fix the provisioning, NOT to relax the leg's CI branch — relaxing it returns the leg to being a gate that cannot fail. `WINDOWS.md` #84. |

## The release's known accepted risk — for whoever pushes the tag

**D-14, stated once, plainly: one tag, `release.yml`'s order, and one failing crate mid-order
strands every crate after it.** The workflow tolerates only an `already exists` failure; any
other failure is `exit 1`. crates.io OWNERSHIP has stranded this repo before, and a 403 only
appears on a NEW version. The remedy is a second tag, which finishes a partial publish because
already-published crates are skipped gracefully.

Two things make this tag riskier than most and must be checked before pushing it:

1. **The publish ORDER is new.** `pmcp` now goes first. Both guards are green locally, but the
   order has never been exercised against crates.io.
2. **`ci.yml`'s fuzz provisioning has never run on a runner** (threat flag above). It is in
   the quality-gate path, so it can fail the gate job on the release commit.

And two reminders from the ledger: a tag without a manifest bump is a SILENT NO-OP, and
`server.json` must be at the crate version or `publish-mcp` 400s on a duplicate — the latter
is now fenced by a test, which is how this plan found it.

**Nothing was tagged and nothing was pushed by this plan.** Work ends at four commits on
`fix/oauth-discovery-optional-fields`.

## Next Phase Readiness

- **The release is assembled and NOT shipped.** Pre-flight before a tag: `rustup update
  stable` (local/CI toolchain mismatch is the #1 CI failure cause), then `make release-sweep`
  or the crates.io API **with a `User-Agent` header** — never `cargo search`/`cargo info`,
  which report the in-tree path override as published state.
- **`/gsd-verify-work` will re-measure `commits` from `plan_head_before`** with the same
  instrument recorded in the frontmatter and see 5 — the figure was first written as 6 and
  corrected by amendment; the frontmatter comment records why.
- **Three open WINDOWS entries** were added by this plan (#83 contracts, #84 the CI fuzz
  provisioning blocker, #85 the three stale templates). #84 is the one that should be read
  before a tag.
- Nothing is blocked by this plan. Phase 128 has no further plans.

---
*Phase: 128-secure-by-default-input-validation-for-config-driven-servers*
*Completed: 2026-09-28*

## Self-Check: PASSED

**Files created — all present on disk and TRACKED:** `tests/root_dev_dep_path_only.rs`,
`docs/architecture/input-validation.md` (both `git ls-files --error-unmatch` exit 0), and this
SUMMARY.

**Commits — all four resolve in `git log --all`:** `396b6500`, `6c1d4946`, `18e4c87a`,
`11c86aff`. `git rev-list --count bfc4e0fe..HEAD` measured at the tip and recorded in the
frontmatter with its base.

**Stub scan over every file this plan touched:** the only `TODO`/`FIXME`/`HACK`/`XXX` hits are
in `CHANGELOG.md` (1) and `cargo-pmcp/src/commands/validate.rs` (7), and
`git diff bfc4e0fe..HEAD` shows **none of them on an added line** — all are pre-existing.

**Tracked working tree clean** apart from the untracked directories present at dispatch;
`.pmat/` churn restored after every pmat invocation and never committed; `target/package/`
removed after the published-manifest measurement.
