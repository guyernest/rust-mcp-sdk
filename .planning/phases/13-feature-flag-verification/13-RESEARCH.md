# Phase 13: Feature Flag Verification - Research

**Researched:** 2026-02-23
**Domain:** Rust feature flag compilation verification, `cfg` attribute isolation, cross-feature contamination detection, CI integration for `pmcp-tasks` crate
**Confidence:** HIGH

## Summary

Phase 13 verifies that the `pmcp-tasks` crate compiles correctly under all four feature flag combinations: no features (InMemory only), `dynamodb` only, `redis` only, and both `dynamodb` + `redis`. This is a verification-and-fix phase, not a new implementation phase. The empirical investigation performed during this research revealed that **all four combinations already compile, pass clippy, pass tests, and pass doctests**. However, there are **broken doc-links** (`rustdoc::broken_intra_doc_links`) that surface across all feature combinations, with two being feature-flag-specific (references to `DynamoDbBackend` and `RedisBackend` in module-level doc comments that resolve to absent modules when those features are disabled).

The verification work for this phase involves three concerns: (1) fixing the feature-flag-specific doc-link issues so doc generation works cleanly per-feature, (2) fixing pre-existing doc-link issues unrelated to features (references to `SequentialWorkflow`, `WorkflowStep`, `WorkflowProgress`, `WORKFLOW_PROGRESS_KEY`, and `generic::GenericTaskStore` that are broken across all feature combinations), and (3) creating an automated verification script or CI job that tests all four feature flag combinations for ongoing regression prevention.

**Primary recommendation:** Create a single plan that fixes all broken doc-links (both feature-specific and pre-existing), then adds an automated feature-flag matrix verification script to the Makefile and/or CI workflow.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| TEST-04 | Feature flag compilation verification (each backend compiles independently) | Empirical testing confirms all 4 combinations compile, pass clippy, and pass tests. Doc-link issues need fixing. Automated verification script needed for regression prevention. See Architecture Patterns section for the verification matrix and Code Examples for the script. |
</phase_requirements>

## Standard Stack

### Core

| Tool | Version | Purpose | Why Standard |
|------|---------|---------|--------------|
| `cargo check` | (stable) | Fast compilation verification without codegen | Standard Rust approach for "does it compile" checks |
| `cargo clippy` | (stable) | Lint verification per feature combination | Zero-warning policy per project CLAUDE.md |
| `cargo test --no-run` | (stable) | Test compilation verification (tests compile per feature) | Ensures test code compiles under each feature |
| `cargo test --doc` | (stable) | Doctest verification per feature combination | Ensures doc examples compile per feature |
| `cargo doc` | (stable) | Documentation generation with broken-link detection | With `RUSTDOCFLAGS="-D warnings"` catches broken intra-doc links |

### Supporting

| Tool | Version | Purpose | When to Use |
|------|---------|---------|-------------|
| `make` | (system) | Automation target for feature-flag matrix check | Existing Makefile pattern in project |
| `cargo test` | (stable) | Run unit tests per feature | Verify no feature-gated test regression |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Shell script in Makefile | `cargo hack --feature-powerset` | cargo-hack automates powerset testing but adds a dev dependency; simple shell loop sufficient for 4 combinations |
| CI matrix job | Local Makefile target | Both should exist -- local for developer verification, CI for automated regression |

## Architecture Patterns

### Feature Flag Matrix

The `pmcp-tasks` crate has the following feature flags:

```toml
[features]
dynamodb = ["dep:aws-sdk-dynamodb", "dep:aws-config"]
dynamodb-tests = ["dynamodb"]
redis = ["dep:redis"]
redis-tests = ["redis"]
```

The **four required verification combinations** (from success criteria):

| # | Features Enabled | Available Backends | Status (verified) |
|---|------------------|--------------------|-------------------|
| 1 | (none) | InMemoryBackend | Compiles, clippy clean, 592 tests compile, 76 doctests pass |
| 2 | `dynamodb` | InMemory + DynamoDB | Compiles, clippy clean, 612 tests compile, 81 doctests pass |
| 3 | `redis` | InMemory + Redis | Compiles, clippy clean, 597 tests compile, 81 doctests pass |
| 4 | `dynamodb,redis` | All backends | Compiles, clippy clean, 617 tests compile, 86 doctests pass |

### Pattern 1: Feature-Gated Module Declaration

The existing pattern is clean and correct:

```rust
// store/mod.rs
pub mod backend;
#[cfg(feature = "dynamodb")]
pub mod dynamodb;
pub mod generic;
pub mod memory;
#[cfg(feature = "redis")]
pub mod redis;
```

```rust
// lib.rs
#[cfg(feature = "dynamodb")]
pub use store::dynamodb::DynamoDbBackend;
#[cfg(feature = "redis")]
pub use store::redis::RedisBackend;
```

**Verdict:** Correctly gated. No cross-contamination in code.

### Pattern 2: Doc-Link Issues Requiring Fixes

**Feature-specific doc-link issues** (broken only when features are disabled):

In `store/mod.rs` module doc-comment, lines 26-30:
```rust
//! - [`DynamoDbBackend`](crate::store::dynamodb::DynamoDbBackend) -- ...
//! - [`RedisBackend`](crate::store::redis::RedisBackend) -- ...
```
These doc-links resolve to nonexistent modules when `dynamodb`/`redis` features are disabled.

**Fix:** Wrap the doc-comment links in feature-conditional text, or convert the links from intra-doc links to plain text/backtick-only references that don't attempt resolution. The simplest fix is to use backtick-only references (`DynamoDbBackend`) without a link target, or use conditional `#[cfg_attr(feature = "dynamodb", doc = "...")]` (complex) vs just using backticks (simple).

**Pre-existing doc-link issues** (broken in ALL combinations, including all-features):

| File | Broken Link | Root Cause |
|------|-------------|------------|
| `types/workflow.rs:3` | `SequentialWorkflow` | References a type from `pmcp` parent crate, not in scope in `pmcp-tasks` |
| `types/workflow.rs:160` | `WorkflowStep` | Same -- references parent crate type |
| `router.rs:317` | `WorkflowProgress` | References a type not re-exported into scope |
| `router.rs:318` | `WORKFLOW_PROGRESS_KEY` | References a constant not in scope at that location |
| `lib.rs:35-36` (via store module doc) | `generic::GenericTaskStore` | Should be `crate::store::generic::GenericTaskStore` |

### Pattern 3: Automated Verification Script

Add a Makefile target that runs all 4 combinations:

```makefile
.PHONY: test-feature-flags
test-feature-flags:
	@echo "Verifying feature flag combinations for pmcp-tasks..."
	@echo "1/4: No features (InMemory only)..."
	cargo check -p pmcp-tasks --no-default-features
	cargo clippy -p pmcp-tasks --no-default-features -- -D warnings
	cargo test -p pmcp-tasks --no-default-features --no-run
	@echo "2/4: dynamodb only..."
	cargo check -p pmcp-tasks --features dynamodb
	cargo clippy -p pmcp-tasks --features dynamodb -- -D warnings
	cargo test -p pmcp-tasks --features dynamodb --no-run
	@echo "3/4: redis only..."
	cargo check -p pmcp-tasks --features redis
	cargo clippy -p pmcp-tasks --features redis -- -D warnings
	cargo test -p pmcp-tasks --features redis --no-run
	@echo "4/4: dynamodb + redis..."
	cargo check -p pmcp-tasks --features "dynamodb,redis"
	cargo clippy -p pmcp-tasks --features "dynamodb,redis" -- -D warnings
	cargo test -p pmcp-tasks --features "dynamodb,redis" --no-run
	@echo "All feature flag combinations verified."
```

### Anti-Patterns to Avoid

- **Testing only `--all-features`:** The whole point is to verify individual feature isolation. `--all-features` hides compilation failures that occur when a feature is disabled.
- **Relying on `cargo check` alone:** Also need clippy (zero-warning policy) and test compilation verification (`--no-run`).
- **Skipping doctests:** Doctests are compiled per-feature and can contain feature-gated code examples.
- **Ignoring doc-link warnings:** Broken intra-doc links indicate types that reference feature-gated modules, which is a form of cross-contamination even if it doesn't block compilation.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Feature powerset testing | Manual combinatorial test scripts | Simple loop over 4 known combinations | Only 4 combinations; powerset tools like `cargo-hack` are overkill |
| Doc-link conditional compilation | Complex `#[cfg_attr(feature=..., doc=...)]` per line | Plain backtick references without link targets | Simpler, works across all feature states, no maintenance burden |

**Key insight:** This is a finite, enumerable verification problem (4 combinations). The complexity is not in automation but in identifying and fixing the actual cross-contamination issues (doc-links).

## Common Pitfalls

### Pitfall 1: Doc-Links to Feature-Gated Types
**What goes wrong:** Module-level documentation references types like `crate::store::dynamodb::DynamoDbBackend` which don't exist when the `dynamodb` feature is off. `cargo doc` with `-D warnings` fails.
**Why it happens:** When writing docs for a module that has multiple backends, it's natural to reference all backends even though some are conditionally compiled.
**How to avoid:** Use plain backtick references (`DynamoDbBackend`) without intra-doc link targets for types that are behind feature flags. Or note "(requires `dynamodb` feature)" in plain text.
**Warning signs:** `RUSTDOCFLAGS="-D warnings" cargo doc -p pmcp-tasks --no-default-features` fails.

### Pitfall 2: Cross-Crate Doc-Links
**What goes wrong:** `pmcp-tasks` doc-comments reference types from the parent `pmcp` crate (e.g., `SequentialWorkflow`, `WorkflowStep`) using intra-doc links. These types exist in `pmcp` but are not in scope in `pmcp-tasks`.
**Why it happens:** The types were referenced during development when thinking about the full ecosystem, not just the crate boundary.
**How to avoid:** Either use fully-qualified paths with `pmcp::` prefix (if re-exported) or use plain backtick text without link resolution.
**Warning signs:** `cargo doc -p pmcp-tasks` fails even with all features enabled.

### Pitfall 3: Test Code Depending on Feature-Gated Imports
**What goes wrong:** Integration test files in `tests/` import feature-gated types unconditionally.
**Why it happens:** Test files are compiled separately from lib code and need their own `#[cfg]` guards.
**How to avoid:** Check that no test file in `tests/` imports `DynamoDbBackend` or `RedisBackend` without a `#[cfg(feature = "...")]` guard.
**Warning signs:** `cargo test -p pmcp-tasks --no-default-features --no-run` fails.
**Current status:** Verified clean -- no `tests/` files reference feature-gated types.

### Pitfall 4: Clippy Warnings Appearing Only in Specific Feature Combinations
**What goes wrong:** Code paths only compiled under certain features may have clippy warnings that `--all-features` masks or reveals differently.
**Why it happens:** Clippy analyzes what's compiled; different features compile different code paths.
**How to avoid:** Run clippy per-combination, not just `--all-features`.
**Current status:** Verified clean -- all 4 combinations pass clippy.

## Code Examples

### Verification Script (Shell)

Verified pattern for testing all 4 feature combinations:

```bash
#!/usr/bin/env bash
set -euo pipefail

CRATE="pmcp-tasks"
COMBOS=(
  ""
  "dynamodb"
  "redis"
  "dynamodb,redis"
)

for combo in "${COMBOS[@]}"; do
  if [ -z "$combo" ]; then
    echo "==> Testing: no features"
    FLAGS="--no-default-features"
  else
    echo "==> Testing: --features $combo"
    FLAGS="--features $combo"
  fi

  cargo check -p "$CRATE" $FLAGS
  cargo clippy -p "$CRATE" $FLAGS -- -D warnings
  cargo test -p "$CRATE" $FLAGS --no-run
  cargo test -p "$CRATE" $FLAGS --doc
  echo "==> PASS: $combo"
done

echo "All feature flag combinations verified."
```

### Fixing Feature-Gated Doc-Links

Convert intra-doc links to plain backtick references for feature-gated types:

```rust
// BEFORE (broken when dynamodb feature is off):
//! - [`DynamoDbBackend`](crate::store::dynamodb::DynamoDbBackend) -- DynamoDB
//!   backend for production AWS/Lambda deployments.

// AFTER (works in all feature states):
//! - `DynamoDbBackend` -- DynamoDB backend for production AWS/Lambda
//!   deployments. Available behind the `dynamodb` feature flag.
```

### Fixing Cross-Crate Doc-Links

Convert cross-crate intra-doc links to plain backtick references:

```rust
// BEFORE (broken -- SequentialWorkflow is in pmcp, not pmcp-tasks):
//! These types track the execution state of a [`SequentialWorkflow`] that is

// AFTER:
//! These types track the execution state of a `SequentialWorkflow` that is
```

### Fixing In-Crate Doc-Link Path

Fix the `generic::GenericTaskStore` reference to use the correct path:

```rust
// BEFORE (broken -- `generic` is not in scope at module root):
//! 2. **[`GenericTaskStore<B>`](generic::GenericTaskStore)** -- All domain

// AFTER (correct full path):
//! 2. **[`GenericTaskStore<B>`](crate::store::generic::GenericTaskStore)** -- All domain
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual "does it compile" checks | Feature-flag CI matrix | Standard practice | Prevents regression when adding new feature-gated code |
| `cargo-hack --feature-powerset` | Known-combination loop | N/A | For 4 combinations, explicit loop is clearer than powerset |

**Deprecated/outdated:**
- Nothing in this domain is deprecated. Feature flags are a stable Rust feature.

## Open Questions

1. **Should doc generation be part of the verification matrix?**
   - What we know: `RUSTDOCFLAGS="-D warnings" cargo doc` catches broken doc-links that are a form of feature contamination
   - What's unclear: Whether the project wants to enforce clean doc generation per-feature as a CI gate, or only fix the currently broken links
   - Recommendation: Fix all broken doc-links and add `cargo doc --no-deps` to the verification matrix. Doc-link correctness is cheap to verify and prevents documentation degradation.

2. **Should the CI workflow be updated in this phase?**
   - What we know: The current CI (`.github/workflows/ci.yml`) only tests `--all-features`. It does not test individual feature combinations.
   - What's unclear: Whether CI changes are in scope for TEST-04 or a follow-up
   - Recommendation: Add a CI matrix job for the 4 feature combinations. This is the natural home for ongoing regression prevention and directly fulfills TEST-04's intent of "each backend compiles independently."

## Sources

### Primary (HIGH confidence)
- Direct empirical verification via `cargo check`, `cargo clippy`, `cargo test`, `cargo doc` on all 4 feature combinations (local machine, 2026-02-23)
- `crates/pmcp-tasks/Cargo.toml` -- feature flag definitions
- `crates/pmcp-tasks/src/store/mod.rs` -- `#[cfg(feature = "...")]` module declarations
- `crates/pmcp-tasks/src/lib.rs` -- `#[cfg(feature = "...")]` re-exports
- `.github/workflows/ci.yml` -- current CI configuration

### Secondary (MEDIUM confidence)
- Rust Reference on conditional compilation: https://doc.rust-lang.org/reference/conditional-compilation.html

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All tools are standard Rust toolchain (`cargo check/clippy/test/doc`)
- Architecture: HIGH - Empirically verified all 4 combinations; issues are enumerated with specific file/line references
- Pitfalls: HIGH - All pitfalls discovered through direct testing, not hypothetical

**Research date:** 2026-02-23
**Valid until:** 90 days (feature flags are stable Rust; patterns don't change)
