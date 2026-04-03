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
1. `pmcp-widget-utils` (leaf, no internal deps)
2. `pmcp` (core SDK, depends on widget-utils)
3. `mcp-tester` (depends on pmcp)
4. `mcp-preview` (depends on widget-utils)
5. `cargo-pmcp` (depends on pmcp, mcp-tester, mcp-preview)

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