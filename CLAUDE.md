# PMCP SDK Development Standards

## Toyota Way Quality System - ZERO TOLERANCE FOR DEFECTS

We have ZERO tolerance for defects. Your "clippy warnings won't..." is a P0 problem.

## Quality Gate Enforcement

### Pre-Commit Quality Gates (MANDATORY)
**ALL commits are blocked until quality gates pass:**
- Pre-commit hook automatically runs Toyota Way quality checks
- Format checking: `cargo fmt --all -- --check`  
- Clippy analysis: Zero warnings allowed
- Build verification: Must compile successfully
- Doctest validation: All doctests must pass

**To commit code:**
```bash
make quality-gate  # Run before any commit
git add -A
git commit -m "message"  # Will be blocked if quality fails
```

### CI Quality Gates (PR-blocking, added Phase 75 Wave 5)

**PRs are blocked from merging if PMAT detects new cognitive-complexity violations.**

The check runs in `.github/workflows/ci.yml` `quality-gate` job:

```bash
pmat quality-gate --fail-on-violation --checks complexity
```

PMAT version is pinned to `3.15.0` (matches `.github/workflows/quality-badges.yml`; see Phase 75 Wave 0 Task 3 for rationale). The `gate` aggregate job lists `quality-gate` in its `needs:` array, so a PMAT failure propagates to the org-required `gate` status check and blocks merge.

**If your PR fails this check:**

1. Run locally to see which functions exceed cog 25:
   ```bash
   pmat analyze complexity --format json --max-cognitive 25 \
     | jq '.violations[] | select(.path | startswith("src/"))'
   ```
2. Apply one of the 6 refactor techniques (P1–P6) documented in `.planning/phases/75-fix-pmat-issues/75-RESEARCH.md` Architecture Patterns.
3. If the function is irreducibly complex (parser, AST walker, protocol dispatch), apply a `// Why:` annotated `#[allow(clippy::cognitive_complexity)]` per the template in `.planning/phases/75-fix-pmat-issues/75-00-PLAN.md`. Hard cap is cog 50 (D-03).
4. Re-push and the gate re-runs.

**DO NOT** disable, weaken, or remove this gate without explicit Phase-level approval — it is the mechanism that keeps the README "Quality Gate: passing" badge accurate.

Pre-commit `make quality-gate` covers fmt/clippy/build/test/audit but does **not** run PMAT (per Phase 75 D-07: PMAT runs only in CI to keep the dev loop fast).

### PMAT Quality-Gate Proxy Mode (REQUIRED DURING DEVELOPMENT)

**MANDATORY: Use pmat quality-gate proxy via MCP during development**

All code changes MUST go through pmat quality-gate proxy before writing:

```bash
# Start pmat MCP server with quality-gate proxy
pmat mcp-server --enable-quality-proxy

# In Claude Code, use quality_proxy MCP tool for all file operations:
# - write operations
# - edit operations  
# - append operations
```

**Quality Proxy Enforcement Modes:**
- **Strict Mode** (default): Reject code that doesn't meet quality standards
- **Advisory Mode**: Warn about quality issues but allow changes
- **Auto-Fix Mode**: Automatically refactor code to meet standards

**Quality Checks Applied:**
- Cognitive complexity limits (≤25 per function)
- Zero SATD (Self-Admitted Technical Debt) comments
- Comprehensive documentation requirements
- Lint violation prevention
- Automatic refactoring suggestions

## Task Management - PDMT Style

**MANDATORY: Use PDMT (Pragmatic Deterministic MCP Templating) for all todos**

### PDMT Todo Generation
Use the `pdmt_deterministic_todos` MCP tool for creating quality-enforced todo lists:

```bash
# Generate PDMT todos with quality enforcement
pdmt_deterministic_todos --requirement "implement feature X" --mode strict --coverage-target 80
```

**PDMT Todo Features:**
- **Quality Gates Built-in**: Each todo includes validation commands
- **Success Criteria**: Clear, measurable completion requirements  
- **Test Coverage**: Enforce 80%+ coverage targets
- **Zero SATD**: No technical debt tolerance
- **Complexity Limits**: Automatic complexity validation
- **Documentation**: Comprehensive docs required

### PDMT Todo Structure
```
## Todo: [ID] Implementation Task
**Quality Gate**: `cargo test --coverage && cargo clippy`
**Success Criteria**: 
- [ ] Feature implemented with 80%+ test coverage
- [ ] Zero clippy warnings
- [ ] Comprehensive documentation with examples
- [ ] Property tests included
- [ ] Integration tests passing
**Validation Command**: `make quality-gate && make test-coverage`
```

## Development Workflow (MANDATORY)

### 1. Planning Phase
- Use `pdmt_deterministic_todos` for task breakdown
- Set quality targets: 80%+ coverage, zero SATD, complexity ≤25

### 2. Development Phase  
- **ALL code changes via pmat quality-gate proxy**
- Use MCP `quality_proxy` tool for file operations
- Continuous quality validation during development

### 3. Pre-Commit Phase
- Pre-commit hook enforces Toyota Way quality gates
- **Cannot commit** without passing all quality checks
- Zero tolerance: formatting, clippy, build, tests

### 4. CI/CD Phase
- Tests run with `--test-threads=1` (race condition prevention)
- Full quality gate validation
- Documentation coverage verification

## ALWAYS Requirements for New Features (MANDATORY)

**Every new feature MUST include ALL of the following - NO EXCEPTIONS:**

### 1. FUZZ Testing (ALWAYS REQUIRED)
```bash
# Property-based fuzzing for robustness
cargo fuzz run fuzz_target_name
# OR using proptest for property-based testing
cargo test proptest
```

### 2. PROPERTY Testing (ALWAYS REQUIRED)
```bash
# Invariant verification with quickcheck/proptest
cargo test property_tests
# Comprehensive property-based testing coverage
```

### 3. UNIT Testing (ALWAYS REQUIRED)
```bash
# Comprehensive unit test coverage (80%+ required)
cargo test unit_tests
# All functions must have unit tests
```

### 4. EXAMPLE Demonstration (ALWAYS REQUIRED)
```bash
# Working example that demonstrates the feature
cargo run --example feature_name
# Must include real-world usage scenario
```

### Additional Testing Requirements:
- **Integration Tests**: Full client-server integration scenarios
- **Doctests**: All public APIs with working examples
- **Performance Tests**: Benchmarks for performance-critical features
- **Security Tests**: Security validation for auth/transport features

## Toyota Way Development Workflow (Updated)

### Feature Development Kata (The "Always" Process)

**For EVERY new feature, follow this exact sequence:**

#### Step 1: PLANNING (PDMT Required)
```bash
# Generate deterministic todos with quality gates
pdmt_deterministic_todos --requirement "implement feature X" --mode strict --coverage-target 80
```

#### Step 2: IMPLEMENTATION (ALWAYS Include)
1. **Write Property Tests FIRST** - Define the invariants
2. **Write Unit Tests** - Cover all edge cases
3. **Implement Feature** - Meet the test requirements
4. **Add Fuzz Testing** - Verify robustness
5. **Create Example** - Demonstrate real usage

#### Step 3: QUALITY VALIDATION (ALWAYS Required)
```bash
# MANDATORY validation before any commit
make quality-gate     # All quality checks
make test-fuzz          # Fuzz testing
make test-property      # Property tests  
make test-unit          # Unit tests
make test-examples      # Example verification
make test-integration   # Integration tests
```

#### Step 4: DOCUMENTATION (ALWAYS Required)
- **API Documentation**: Comprehensive rustdoc with examples
- **Usage Examples**: Real-world scenarios in examples/
- **Integration Guide**: How to use with existing systems
- **Property Documentation**: What invariants are maintained

## Quality Standards Summary

✅ **Zero tolerance for defects**
✅ **Pre-commit quality gates enforced**  
✅ **PMAT quality-gate proxy mandatory during development**
✅ **PDMT style todos with built-in quality gates**
✅ **Toyota Way principles: Jidoka, Genchi Genbutsu, Kaizen**
✅ **80%+ test coverage with quality doctests**
✅ **Cognitive complexity ≤25 per function**
✅ **Zero SATD comments allowed**
✅ **Comprehensive documentation required**
✅ **ALWAYS requirements: fuzz, property, unit, cargo run --example**

## Release & Publish Workflow

### Workspace Crates (publish order)

**The numbered list below records RATIONALE (who depends on whom, and why a
crate sits where it does). `.github/workflows/release.yml` is the AUTHORITY on
the actual order — see the flat ledger at the end of this section, which mirrors
it step for step. Where the two have ever disagreed, the workflow was right and
the prose was wrong (items 12 and 2 below are both corrections of exactly that).
Numbering is left dense rather than renumbered so existing "item N"
cross-references stay valid.**

1. `pmcp-widget-utils` (leaf, no internal deps)
1a. `pmcp-macros-support` (leaf proc-macro support crate; `pmcp` depends on it, so it
   must publish BEFORE item 2). **This entry was missing from this list until
   2026-08-23** — it was in `release.yml` the whole time, so CI published it
   correctly and only the prose was silent. A releaser following the prose to bump
   versions before a tag push would have skipped it, shipping a stale
   `pmcp-macros-support` with the core SDK.
1b. `pmcp-macros` (the derive crate; depends on `pmcp-macros-support`, and `pmcp`
   depends on it, so it publishes after item 1a and before item 2). **Also missing
   from this list until 2026-08-23**, same reason.
2. `pmcp` (core SDK, depends on widget-utils). **Corrected 2026-08-23:** this list
   put `pmcp` AHEAD of items 3 and 4, which inverts the real order —
   `release.yml` publishes `pmcp-code-mode` and `pmcp-code-mode-derive` FIRST,
   then `pmcp`. That is not an accident to be "fixed": `pmcp-code-mode` pins
   `pmcp = ">=2.2.0"`, which an already-published `pmcp` satisfies, so the
   code-mode crates can go first — and they must, because `pmcp`'s own
   `code-mode` feature reaches them. A releaser who trusted the old prose and
   reordered `release.yml` to publish `pmcp` first would reintroduce the class of
   bug PR #303 fixed. The numbers stay as they are because "item 2" is
   cross-referenced throughout this file.
3. `pmcp-code-mode` (depends on pmcp; publishes BEFORE item 2 — see item 2's note)
4. `pmcp-code-mode-derive` (depends on pmcp-code-mode; also publishes BEFORE item 2)
4a. `pmcp-workbook-dialect` (workbook leaf; publishes between `pmcp-workbook-runtime`
   and item 5). **Missing from this list until 2026-08-23**, same class as items
   1a/1b — present in `release.yml`, absent from the prose.
5. `pmcp-server-toolkit` (runtime library; depends on pmcp + pmcp-code-mode under the default `code-mode` feature)
6. `pmcp-toolkit-postgres` (depends on pmcp-server-toolkit + tokio-postgres + deadpool-postgres)
7. `pmcp-toolkit-mysql` (depends on pmcp-server-toolkit + sqlx)
8. `pmcp-toolkit-athena` (depends on pmcp-server-toolkit + aws-sdk-athena)
9. `pmcp-sql-server` (Shape A pure-config binary; depends on pmcp-server-toolkit + all four connector crates — must publish AFTER items 5–8; no inter-dep with mcp-tester)
9a. `pmcp-workbook-server` (Shape A pure-config WORKBOOK binary; depends on `pmcp-server-toolkit` with the `workbook` + `http` features — and thus transitively on `pmcp-workbook-runtime` — plus `pmcp`. Must publish AFTER `pmcp-server-toolkit` (item 5) and its `pmcp-workbook-runtime` dep. It is a sibling of `pmcp-sql-server` (item 9) but has NO inter-dependency with the SQL connector crates (items 6–8). Its `mcp-tester` link is only a `[dev-dependencies]` parity-test harness — but that entry carries BOTH `path` and `version`, so it IS retained in the published manifest and must resolve on crates.io at publish time, and this crate publishes BEFORE `mcp-tester` (see the CR-01 note under item 9b). NOTE: `pmcp-workbook-runtime` is NOT a numbered item in this list — it is pulled in only transitively, through `pmcp-server-toolkit`'s `workbook` feature (this binary depends on the toolkit directly, never on the runtime crate), and is published out-of-band by its own Phase 91/92 workbook-runtime release ahead of `pmcp-server-toolkit` (item 5). The release workflow skips already-published crates gracefully, so no numbered slot is required here; just ensure the workbook-runtime tree is published before item 5.)
9b. `pmcp-openapi-server` (Shape A pure-config **OpenAPI** binary — point it at a `config.toml`
   plus an optional OpenAPI spec and it serves a production MCP server with no Rust required).
   Depends on `pmcp` and `pmcp-server-toolkit`, so it must publish AFTER item 5. A sibling of
   `pmcp-sql-server` (item 9) and `pmcp-workbook-server` (item 9a) with NO inter-dependency on
   either, and no inter-dep with `mcp-tester`. **This entry was missing until 2026-07-27** — the
   crate has existed at `crates/pmcp-openapi-server/` (a root workspace member, version 0.1.0)
   while being absent from this list, so a release would have silently skipped it. It is the
   proving case for the v2.6 AI-Package portability milestone (PKG-01: a server whose entire
   identity is its config plus its spec).

   **Path-only `pmcp-package` dev-dep constraint (Phase 121 CR-01, 2026-08-24).**
   This crate publishes HERE, ahead of `pmcp-package` at item 13, so any
   dependency it declares on `pmcp-package` — **including a `[dev-dependencies]`
   entry** — must be **path-only**, carrying no version key. Cargo strips a
   dev-dep from the published manifest only when it carries no version
   requirement; one that carries a requirement is retained and must resolve
   against crates.io while `cargo publish` prepares the manifest, which cannot
   succeed at this point in the order (measured: exit 101, "failed to select a
   version for the requirement `pmcp-package = \"^0.2\"`"). The `exclude` list
   does not save it — the failure is at manifest-prep time, and excluding
   `tests/` removes the consumers, not the manifest entry. Enforced by
   `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs`
   (`pmcp_package_dev_dep_is_path_only`), which runs inside `make quality-gate`
   through `test-openapi-server`. Discovered when this crate became the first
   in-repo `pmcp-package` consumer placed BEFORE `release.yml:440` — every other
   pin (`pmcp-agent`, `pmcp-team-servers`, `pmcp-cfn-renderer`, `cargo-pmcp`)
   sits after it. `scripts/check-release-coverage.sh` cannot catch this class: it
   checks only that a publish STEP exists per crate, and is blind to
   workspace-excluded crates besides.

10. `mcp-tester` (depends on pmcp)
11. `mcp-preview` (depends on widget-utils)
12. *(slot retired — `cargo-pmcp` moved to item 15a.)* **Corrected 2026-08-23.** This
   list placed `cargo-pmcp` here, ahead of items 13–15, which is the exact ordering
   bug PR #303 fixed in `release.yml` — `cargo-pmcp` pins `pmcp-agent`,
   `pmcp-team-servers`, `pmcp-package` and `pmcp-cfn-renderer`, so it must publish
   AFTER all of them. `release.yml` has been correct since #303; only this prose was
   wrong, and item 13a's own text ("this must ALSO publish before `cargo-pmcp`")
   contradicted the number in place. Numbering is left dense rather than renumbered
   so existing "item N" cross-references stay valid.
13. `pmcp-package` (the AI-Package format crate at `crates/pmcp-package/`). It is
   standalone / **workspace-excluded** — it has its own `[workspace]` table and is
   NOT a root member, so root `cargo fmt/clippy/test` and `cargo publish -p
   pmcp-package` do NOT reach it; publish via
   `cargo publish --manifest-path crates/pmcp-package/Cargo.toml`. As of Phase 108
   its first in-repo consumer is `pmcp-agent` (item 14), which pins
   `pmcp-package = "0.2"` — so `pmcp-package` must publish **before** `pmcp-agent`,
   hence its slot here just ahead of item 14. It remains an experimental 0.x leaf:
   a failure here must not gate the core SDK release, and it still publishes late
   in the overall order (after the core SDK and toolkit trees).
   Cross-reference (Phase 121 CR-01): because `pmcp-package` publishes HERE, any
   crate publishing EARLIER in this list must declare it **path-only** with no
   version key — see item 9b (`pmcp-openapi-server`) for the mechanism and the
   test that enforces it.
13a. `pmcp-cfn-renderer` (the pure `DeployDescriptor -> CloudFormation` template
   renderer crate at `crates/pmcp-cfn-renderer/`, CFN-renderer extraction). Depends
   on `pmcp-package = "0.2"` (item 13), so it must publish AFTER `pmcp-package`
   — hence its slot here,
   just ahead of `pmcp-agent` (item 14). `cargo-pmcp` (item 15a) pins
   `pmcp-cfn-renderer = "0.2"` (it replaces `npx cdk synth`/`cdk deploy` for
   unmodified scaffolds on the `pmcp-run` and `aws-lambda` deploy targets), so
   this must ALSO publish before `cargo-pmcp` reaches crates.io. 0.x/
   experimental — a failure here must not gate the core SDK release.
14. `pmcp-agent` (the experimental 0.x agent-loop crate at `crates/pmcp-agent/`,
   Phase 108). A regular root workspace member that pins `pmcp = "2.17"` (item 2)
   and `pmcp-package = "0.2"` (item 13) via path deps, so it must publish AFTER
   both. 0.x/experimental — a failure here must not gate the core SDK release. Its
   `openai-compat`/`anthropic`/`url-connector` features are all non-default, so the
   default publish build is reqwest-free and wasm-clean.
15. `pmcp-team-servers` (the experimental 0.x reference-team-server crate at
   `crates/pmcp-team-servers/`, Phase 109). A regular root workspace member that
   pins `pmcp = "2.17"` (item 2), `pmcp-agent = "0.3"` (item 14), and
   `pmcp-package = "0.2"` (item 13) via path deps, so it must publish AFTER all
   three (i.e. after `pmcp-agent`). 0.x/experimental — a failure here must not
   gate the core SDK release. Its `webhook` (reqwest) and `http`
   (`pmcp/streamable-http`) features are non-default, so the default publish
   build is reqwest-free and wasm-clean.
15a. `cargo-pmcp` (depends on pmcp, mcp-tester, mcp-preview — and pins
   `pmcp-package`, `pmcp-cfn-renderer`, `pmcp-agent` and `pmcp-team-servers`, so it
   must publish AFTER items 13, 13a, 14 and 15). Formerly listed as item 12, which
   put it four slots too early; `release.yml` publishes it here, after
   `pmcp-team-servers`.
16. `pmcp-server` (the docs/resources MCP server at `crates/pmcp-server/`). A root
   workspace member pinning `pmcp` (item 2) and `mcp-tester` (item 10), so it must
   publish AFTER both. **This entry was missing from this list until 2026-08-21** —
   it was present in `release.yml` the whole time, so CI published it correctly and
   only the prose order was wrong. Its sibling `pmcp-server-lambda` is
   `publish = []` and never publishes.
17. `pmcp-tasks` (the experimental 0.x MCP-Tasks crate at `crates/pmcp-tasks/`). Pins
   `pmcp` (item 2) only, and NOTHING in this workspace depends on it — so it
   publishes late, like `pmcp-package`, and a failure here must not gate the core
   SDK release. **This entry was missing from BOTH this list and `release.yml`
   until 2026-08-21**, so it had never published at all; pmcp-run's built-in
   servers consume it out-of-repo with `features = ["dynamodb"]` and could not pin
   until 0.1.0 was published by hand. The `release.yml` ledger is now machine-checked
   by `scripts/check-release-coverage.sh` (chained into `make quality-gate` and the CI
   quality-gate job), which is what makes a third recurrence a build failure rather
   than a discovery. This prose list remains hand-maintained, and workspace-excluded
   crates (`pmcp-package`) are a known blind spot of the check until Phase 124 (PKGR-01).

The three per-backend connector crates (`pmcp-toolkit-postgres`, `-mysql`, `-athena`)
have no inter-dependencies — they may publish in any order relative to each other,
but all must publish AFTER `pmcp-server-toolkit`. `pmcp-sql-server` depends on the
toolkit plus all three connector crates (and the SQLite feature), so it must publish
AFTER all of items 5–8; it has no inter-dependency with `mcp-tester` beyond a
`[dev-dependencies]` parity-test harness entry — but note that entry carries BOTH
`path` and `version`, so it IS retained in the published manifest and must resolve
on crates.io at publish time (see the CR-01 note under item 9b). It is safe only
while the pinned `mcp-tester` version is already published.

### Pre-Flight Checklist
Before starting a release, verify:
1. **Update local Rust toolchain** — CI uses `dtolnay/rust-toolchain@stable` (latest stable).
   Local/CI version mismatch is the #1 cause of CI failures (new clippy lints each release).
   ```bash
   rustup update stable
   rustc --version  # Must match or exceed CI's version
   ```
2. **Check crates.io versions** — know what's already published vs what needs bumping:
   ```bash
   cargo search pmcp --limit 5
   cargo search mcp-tester --limit 1
   cargo search mcp-preview --limit 1
   ```
3. **Identify changed crates** — compare against the last release tag:
   ```bash
   git diff --stat vLAST..HEAD -- src/ crates/ cargo-pmcp/
   ```

### Version Bump Rules
- Only bump crates that have changed since their last publish
- Downstream crates that pin a bumped dependency must also be bumped
  (e.g., if `pmcp` bumps, update the `pmcp = { version = "..." }` line in
  `mcp-tester/Cargo.toml` and `cargo-pmcp/Cargo.toml`, and bump their versions)
- Semver: new features = minor bump, breaking changes = major bump, fixes = patch

### Release Steps
```bash
# 1. Update toolchain first
rustup update stable

# 2. Create a release branch
git checkout -b release/pmcp-vX.Y.Z

# 3. Bump version(s) in Cargo.toml files
#    - Root Cargo.toml (pmcp version)
#    - crates/mcp-tester/Cargo.toml (version + pmcp dep version)
#    - crates/mcp-preview/Cargo.toml (version)
#    - cargo-pmcp/Cargo.toml (version + pmcp, mcp-tester, mcp-preview dep versions)

# 4. Run the SAME quality gate CI uses — this is the critical step
#    Do NOT run individual cargo commands; `make quality-gate` matches CI exactly
#    (fmt --all, clippy with pedantic+nursery lints, build, test, audit, etc.)
make quality-gate

# 5. Commit, push, create PR to upstream
git add <changed Cargo.toml files>
git commit -m "chore: bump pmcp vX.Y.Z"
git push -u origin release/pmcp-vX.Y.Z
gh pr create --repo paiml/rust-mcp-sdk --head <your-fork>:release/pmcp-vX.Y.Z --base main

# 6. After PR merges and CI is green, tag and push
git checkout main && git pull upstream main
git tag -a vX.Y.Z -m "pmcp vX.Y.Z - <summary>"
git push upstream vX.Y.Z
```

### Why `make quality-gate` (not individual cargo commands)
CI runs `make quality-gate` which invokes `make lint` with `--features "full"`,
pedantic + nursery clippy lint groups, and workspace-wide `cargo fmt --all`.
Running bare `cargo clippy -- -D warnings` locally is **weaker** than CI and will
miss lints. Always use `make quality-gate` to match CI exactly.

### What Happens Automatically (CI)
Pushing a `v*` tag to upstream triggers `.github/workflows/release.yml`:
1. **Create Release** — GitHub Release from CHANGELOG.md
2. **Publish to crates.io** — publishes in dependency order with 30s waits between
3. **Publish to MCP Registry** — OIDC-authenticated `mcp-publisher`
4. **Release Tester Binary** — cross-platform mcp-tester binaries attached to release

### Tag Convention
- Tags use `v` prefix: `v1.17.0`, `v0.4.1`
- One tag per release — the Release workflow publishes ALL crates that have new versions
- If a crate version already exists on crates.io, the publish step skips it gracefully

## Contract-First Development

All new features and bug fixes must follow provable-contract-first methodology:
1. Write or update the contract YAML in `../provable-contracts/contracts/<crate>/`
2. Run `pmat comply check` to validate compliance
3. Implement the code to satisfy the contract
4. Run `pmat comply check` again to confirm

## Emergency Override (USE WITH EXTREME CAUTION)
```bash
# Only for critical hotfixes - requires justification
git commit --no-verify -m "HOTFIX: critical issue - bypassing quality gates"
```

**Note**: Emergency overrides require immediate follow-up commits to restore quality standards.
- Before pushing a new commit or a PR you need to run `make quality-gate`.