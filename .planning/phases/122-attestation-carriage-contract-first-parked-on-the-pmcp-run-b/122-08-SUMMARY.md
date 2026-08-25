---
phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b
plan: 08
subsystem: release
tags: [versioning, semver, release-order, pmcp-package, cargo-pmcp, scaffold-pin, tripwire]
status: in-progress

# Dependency graph
requires:
  - phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b
    provides: "the four source-breaking changes this version must name (plans 122-02, 122-03, 122-05, 122-06, 122-07)"
  - phase: 121-local-round-trip-e2e
    provides: "CR-01 — the path-only `pmcp-package` dev-dep constraint in `crates/pmcp-openapi-server`, and its enforcing tripwire"
provides:
  - "A MEASURED emitter inventory of every in-repo `pmcp-package` and `cargo-pmcp` version literal (Task 1)"
affects: [124]

key-files:
  created: []
  modified: []

requirements-completed: []

duration: in-progress
completed: 2026-08-25
---

# Phase 122 Plan 08: The Version Decision and Its Propagation Summary

> **Status: IN PROGRESS.** Task 1 (the measured emitter inventory) is complete and
> recorded below. Task 2 is a `checkpoint:decision` — the version number is a
> one-way call and is being put to the developer with this inventory as its blast
> radius. Tasks 3 and 4 have not run.

## Task 1 — the measured emitter inventory

Produced BEFORE the checkpoint, per the plan's `<precondition>`. Every command below
was executed through absolute binary paths (`/usr/bin/grep`), not through the `rtk`
proxy, because this phase has recorded three separate instances of that proxy
truncating or misreporting evidence (122-01 deviation 2, 122-05 issues, 122-07 issues).

**This task changed no source file.** `git status --porcelain` at the start of the
task printed nothing; at its end it lists only this SUMMARY.

### Commands run, verbatim

```
$ /usr/bin/grep -rn 'pmcp-package' --include='Cargo.toml' crates cargo-pmcp . | /usr/bin/grep -v '^\./target'
$ /usr/bin/grep -rn 'pmcp-package' --include='*.rs' crates cargo-pmcp src
$ cargo metadata --format-version 1 --no-deps | jq -r '.packages[] | . as $p | .dependencies[]
    | select(.name=="pmcp-package") | "\($p.name) \($p.version) | kind=\(.kind // "normal") | req=\(.req)"'
$ cargo metadata --manifest-path crates/pmcp-package/Cargo.toml --format-version 1 --no-deps | jq -r '.packages[] | "\(.name) \(.version)"'
$ /usr/bin/grep -rn 'cargo-pmcp' --include='Cargo.toml' crates cargo-pmcp . | /usr/bin/grep -v '^\./target'
$ /usr/bin/grep -rn 'CARGO_PMCP.*VERSION\|cargo-pmcp = ' --include='*.rs' crates cargo-pmcp src
$ /usr/bin/grep -rn 'CARGO_PKG_VERSION' --include='*.rs' cargo-pmcp/src
$ /usr/bin/grep -n '^version' cargo-pmcp/Cargo.toml crates/pmcp-{agent,team-servers,cfn-renderer,openapi-server,package}/Cargo.toml
```

### `cargo metadata` reconciliation (root workspace)

```
pmcp-agent          0.3.0  | kind=normal | req=^0.2
pmcp-cfn-renderer   0.2.0  | kind=normal | req=^0.2
pmcp-team-servers   0.2.0  | kind=normal | req=^0.2
pmcp-openapi-server 0.1.1  | kind=dev    | req=*      <- path-only, CR-01
cargo-pmcp          0.22.0 | kind=normal | req=^0.2
```

Workspace-excluded crate, separately: `cargo metadata --manifest-path crates/pmcp-package/Cargo.toml` → `pmcp-package 0.2.0`.

**Reconciliation result: `cargo metadata` and the greps agree exactly.** Five manifest
entries, no requirement the greps missed and no grep hit that is not a real dependency
entry. The `req=*` on the dev-dep is Cargo's rendering of "no version requirement" —
the CR-01 shape, not a wildcard someone typed.

### Inventory: `pmcp-package` version emitters

Current version of the crate itself: **0.2.0**.

| # | File | Line | Current literal | Bucket | Guarded by |
|---|------|------|-----------------|--------|------------|
| 1 | `crates/pmcp-package/Cargo.toml` | 10 | `version = "0.2.0"` | **test-guarded** | `pmcp_package_resolved_crate_is_on_the_0_2_line` (openapi-server tripwire) — this is the SOURCE OF TRUTH every other row is compared against |
| 2 | `crates/pmcp-agent/Cargo.toml` | 18 | `pmcp-package = { version = "0.2", path = … }` | **compiler-guarded** | `cargo build --workspace` cannot resolve if stale |
| 3 | `crates/pmcp-team-servers/Cargo.toml` | 24 | `pmcp-package = { version = "0.2", path = … }` | **compiler-guarded** | `cargo build --workspace` |
| 4 | `crates/pmcp-cfn-renderer/Cargo.toml` | 10 | `pmcp-package = { version = "0.2", path = … }` | **compiler-guarded** | `cargo build --workspace` |
| 5 | `cargo-pmcp/Cargo.toml` | 87 | `pmcp-package = { version = "0.2", path = … }` | **compiler-guarded AND test-guarded** | `cargo build --workspace` + `pmcp_package_pin_is_the_expected_caret_line` |
| 6 | `crates/pmcp-openapi-server/Cargo.toml` | 123 | `pmcp-package = { path = "../pmcp-package" }` — **no version key** | **test-guarded** | `pmcp_package_dev_dep_is_path_only` — **MUST NOT MOVE** (Phase 121 CR-01) |
| 7 | `cargo-pmcp/tests/pmcp_package_pin.rs` | 38 | `const EXPECTED_PIN: &str = "0.2"` | **test-guarded (it IS the guard)** | itself — goes red against a moved row 5 |
| 8 | `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs` | 87 | `const EXPECTED_VERSION_LINE: &str = "0.2."` | **test-guarded (it IS the guard)** | itself — goes red against a moved row 1 |
| 9 | `cargo-pmcp/src/templates/agent.rs` | 61 | `const PMCP_PACKAGE_VERSION_REQ: &str = "0.2"` | **test-guarded ONLY** | `emitted_package_requirement_matches_workspace_major_minor_line` (`cargo test -p cargo-pmcp --lib`) — **invisible to `cargo build`** |

**Nine rows.** The plan's frontmatter and its `bump-0-3-0` option name **seven**
emitters. **The count differs, and per the plan's acceptance criterion that is
reported as a finding rather than quietly reconciled.** The difference is arithmetic,
not substantive: the plan's prose says "the crate manifest, four consuming manifests,
the scaffold template constant, and both pin tripwires' constants", which is
1 + 4 + 1 + 2 = **8** distinct constants across 8 files, and calls that "seven".
Row 6 — the path-only dev-dep that must NOT move — is the ninth, counted here because
an inventory of emitters that omits the one emitter whose correct action is *do
nothing* is exactly how someone "helpfully" adds a version key to it. Nothing in the
plan's list is absent from this table; the table adds row 6 and resolves the 7-vs-8
slip.

### Inventory: `cargo-pmcp` version emitters

Current version: **0.22.0** (matches the value the plan recorded at planning time).

| # | File | Line | Current literal | Bucket |
|---|------|------|-----------------|--------|
| C1 | `cargo-pmcp/Cargo.toml` | 3 | `version = "0.22.0"` | source of truth; nothing in-repo asserts it |
| C2 | `cargo-pmcp/fuzz/Cargo.toml` | 12 | `cargo-pmcp = { path = ".." }` | path-only, **no version key** — nothing to move |

**The `cargo-pmcp` half is stated explicitly rather than omitted, as the plan
requires: there is NO scaffold-pin constant for `cargo-pmcp`'s own version, and no
in-repo crate declares a version requirement on it.**

- `/usr/bin/grep -rn 'CARGO_PMCP.*VERSION\|cargo-pmcp = ' --include='*.rs' crates cargo-pmcp src` → **exit 1, no match**.
- Every in-source consumer of cargo-pmcp's own version reads `env!("CARGO_PKG_VERSION")`
  — five sites (`pentest/sarif.rs:266`, `loadtest/client.rs:100,430`,
  `commands/schema.rs:307,589`). These are **derived, never restated**, so they cannot
  go stale. This is the pattern `PMCP_AGENT_VERSION` deliberately does not use, which
  is why that one needs a drift test and these do not.

So a `cargo-pmcp` bump moves exactly **one** line (C1) and fires no tripwire.

### UNGUARDED emitters — called out separately, per the plan

Two hits are **unguarded**: nothing fails if they go stale. Both are already stale,
which is the point.

**Finding U1 — `cargo-pmcp/tests/support/scaffold_patch.rs:59`**

```rust
/// - `pmcp-package`       → `<repo>/crates/pmcp-package`          (0.1.0, agent scaffold's manifest type, NOT yet on crates.io)
```

The doc comment says **0.1.0**. `pmcp-package` has been **0.2.0** since Phase 120.
The comment has been wrong for two phases and nothing noticed, because it is prose.

- **What would break:** nothing functional. The TOML this helper actually *emits*
  (line 94) is `pmcp-package = {{ path = "{package}" }}` — **path-only, carrying no
  version literal at all**. So this is a stale-prose emitter, not a stale-requirement
  emitter, and it cannot ship a broken scaffold.
- **How it would be noticed:** only by a human reading the comment and being misled
  about which version the `[patch.crates-io]` closure is standing in for.

**Finding U2 — `cargo-pmcp/tests/scaffold_agent.rs:17` and `:97`**

Both say `pmcp-agent`/`pmcp-package 0.1.0`. Same class, same staleness, same
harmlessness: the patch section they describe is path-based.

**Why U1/U2 matter to this decision.** They are the measured, in-repo demonstration
that an unguarded version literal in this repository *does* rot, and rots silently
across a bump — which is the entire argument for moving row 9
(`PMCP_PACKAGE_VERSION_REQ`) deliberately rather than trusting a green build. Row 9
differs from U1/U2 in exactly one respect, and it is the respect that matters: row 9's
value is **emitted into a generated project's manifest**, so its staleness ships to a
user rather than merely misleading a reader. Row 9 has a drift test *because* Phase
120 already shipped it stale once (its own rustdoc at `agent.rs:57` records that it
"sat on `0.1` after `pmcp-package` had gone 0.2.0").

### Finding F1 (NEW, not in the plan) — the four pins cannot move alone without breaking the PUBLISHED build

This was measured while bucketing row 5, and it is the reason the row count is not the
only thing worth reading in this table.

`cargo-pmcp` depends on `pmcp-package` **directly** (row 5) *and* **transitively**
through three crates that each carry their own `pmcp-package` requirement (rows 2, 3,
4). Locally this is invisible: every one of those entries carries a `path`, and
**`path` wins locally** — the whole workspace unifies on the single in-tree copy, so
`cargo build --workspace` is green no matter what the `version` keys say. That is not
a hypothesis; `cargo-pmcp/Cargo.toml:65-67` already documents the class in prose:

> `path` wins locally, but a published cargo-pmcp 0.19.0 claiming compatibility with an older pmcp would not compile

At **publish** time the `version` keys are all that is left. If `pmcp-package` goes
`0.3.0` and only rows 2–5 move to `"0.3"` while `pmcp-agent`, `pmcp-team-servers` and
`pmcp-cfn-renderer` keep their **current version numbers** (0.3.0 / 0.2.0 / 0.2.0),
then `release.yml` **skips** those three as already-published (the workflow skips
crates whose version already exists), and the published `cargo-pmcp` resolves:

- `pmcp-package ^0.3` → **0.3.0** (its own direct requirement), and
- `pmcp-agent 0.3.0` (from crates.io, pinning `pmcp-package ^0.2`) → **0.2.0**

Two semver-incompatible copies of `pmcp-package` in one dependency graph. Cargo permits
that; the **type checker does not**, wherever a `pmcp-package` type crosses between
them. Measured crossings in production (non-test) code:

| Crossing | Evidence |
|---|---|
| `cargo-pmcp` → `pmcp-cfn-renderer` | `cargo-pmcp/src/deployment/stack_routing.rs:93` returns `pmcp_package::package::DeployDescriptor`; `cargo-pmcp/src/deployment/targets/pmcp_run/deploy.rs:316` passes `&descriptor` to `pmcp_cfn_renderer::render`, whose signature is `render(descriptor: &DeployDescriptor, …)` over `use pmcp_package::package::DeployDescriptor` (`crates/pmcp-cfn-renderer/src/lib.rs:88,119-122`) |
| `cargo-pmcp` → `pmcp-team-servers` | `cargo-pmcp/src/commands/team/dev.rs` imports both `pmcp_package::{AgentPackage, ComponentRef, ConfigSlot, TeamPackage}` (:43-46) and `pmcp_team_servers::compose::resolver::{LocalDirPackageResolver, PackageResolver}` (:48); that trait's method returns `Result<pmcp_package::AgentPackage, ResolveError>` (`crates/pmcp-team-servers/src/compose/resolver.rs:108-109`) |
| `cargo-pmcp` → `pmcp-agent` | `cargo-pmcp/src/commands/agent/dev.rs:29` imports `pmcp_package::AgentPackage` alongside `pmcp_agent::*` (:25); `pmcp-agent` takes `pmcp_package` types across its own surface (`crates/pmcp-agent/src/adapter/server.rs:47`, `src/config/resolver.rs:24`) |

**Consequence for the decision:** under `bump-0-3-0`, moving the four pins is
necessary but **not sufficient**. `pmcp-agent`, `pmcp-team-servers` and
`pmcp-cfn-renderer` must ALSO receive their own version bumps (so `release.yml`
republishes them carrying the `^0.3` requirement), and `cargo-pmcp`'s pins on those
three — `pmcp-agent = "0.3"` (`cargo-pmcp/Cargo.toml:79`), `pmcp-team-servers = "0.2"`
(:83), `pmcp-cfn-renderer = "0.2"` (:91) — must move with them. That is **six more
lines** than the plan's action text enumerates, in three files the plan already lists
plus `cargo-pmcp/Cargo.toml` which it also already lists.

This is exactly the class of defect this plan exists to prevent, arriving one level
further out than the plan looked: an emitter whose staleness `cargo build --workspace`
cannot see. It is reported here rather than fixed, because the correct scope of the
fix depends on which option the developer ratifies.

**It is also consistent with CLAUDE.md's own stated rule**, under *Version Bump
Rules*: "Downstream crates that pin a bumped dependency must also be bumped." F1 is
that rule applied to `pmcp-package`, and the reason it applies here rather than being
optional is the type-crossing table above.

### Prose and assertion-message sites (not separate emitters, but must move with their constants)

Recorded so Task 3 does not leave a constant that moved and a message that did not.
These carry no independent authority — they are text inside rows 1, 5, 7, 8 and 9.

- `cargo-pmcp/tests/pmcp_package_pin.rs` — module docs (`:4-6`), the two dep-shape
  comments (`:49`, `:51`), and the assertion message naming the rejected forms
  `=0.2.0` and `0.2.0` (`:61-66`, `:74-75`).
- `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs` — module docs (`:9-12`),
  the shorthand example (`:96`), and the failure message (`:158`).
- `cargo-pmcp/Cargo.toml:84-86` — the comment naming the caret `"0.2"` literal and the
  test that asserts it.
- `cargo-pmcp/src/templates/agent.rs:57` and `:304` — the rustdoc and test comment
  recording that this constant "sat on `0.1` after `pmcp-package` had gone 0.2.0".

---

<!-- Tasks 2-4 append below. -->
