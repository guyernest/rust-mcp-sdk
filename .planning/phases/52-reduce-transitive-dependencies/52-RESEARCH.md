# Phase 52: Reduce Transitive Dependencies - Research

**Researched:** 2026-03-18
**Domain:** Rust dependency management, feature gating, Cargo workspace optimization
**Confidence:** HIGH

## Summary

The pmcp crate currently resolves 249 unique production dependencies with default features and 203 with no default features. The heaviest contributor is `reqwest` (172 transitive deps), which is a non-optional platform dependency even though most MCP server authors (the primary consumers) never need an outbound HTTP client. The second-largest waste comes from `jsonschema` using default features that enable `resolve-http`, which redundantly pulls in reqwest+rustls through jsonschema's own dependency path.

Three dependencies (`lazy_static`, `pin-project`, `rayon`) are listed in Cargo.toml but never imported or used anywhere in the codebase. Additionally, `jsonschema` and `garde` are declared as optional deps behind the `validation` feature, but no code in the entire workspace uses `cfg(feature = "validation")` or imports either crate -- they are phantom dependencies.

The primary optimization strategy is: (1) remove genuinely unused deps, (2) set `default-features = false` on `jsonschema`, (3) make `reqwest` optional behind an `http-client` feature, and (4) slim `tokio` from `features = ["full"]` to explicit feature list. Combined, these changes would reduce default-feature production deps from 249 to approximately 180-190 and no-feature deps from 203 to approximately 150-160.

**Primary recommendation:** Make reqwest optional behind `http-client` feature, set `jsonschema = { default-features = false }`, remove unused deps (lazy_static, pin-project), and slim tokio features.

## Standard Stack

### Core Tools

| Tool | Version | Purpose | Why Standard |
|------|---------|---------|--------------|
| `cargo tree` | (built-in) | Dependency analysis | Standard Cargo tool for understanding dep tree |
| `cargo machete` | latest | Detect unused deps | False positives on optional deps but catches real waste |
| `cargo tree -d` | (built-in) | Find duplicate crate versions | Identifies version unification opportunities |
| `cargo tree -e no-dev` | (built-in) | Production-only deps | Separates dev from prod dependencies |

### Key Commands

```bash
# Count unique production deps (default features)
cargo tree -p pmcp -e no-dev --prefix none | sort -u | wc -l

# Count unique production deps (no default features)
cargo tree -p pmcp --no-default-features -e no-dev --prefix none | sort -u | wc -l

# Find unused deps
cargo machete

# Find duplicate crate versions
cargo tree --workspace -d | grep "^[a-z]"

# Check resolved features for a dep
cargo tree -p pmcp -e no-dev -f '{p} {f}' --prefix none | grep "^<dep_name>"
```

## Architecture Patterns

### Feature Flag Design Pattern

The standard Rust pattern for optional heavy dependencies:

```toml
# In Cargo.toml
[dependencies]
heavy-dep = { version = "X", optional = true, default-features = false, features = ["needed"] }

[features]
feature-name = ["dep:heavy-dep"]
```

```rust
// In source code
#[cfg(feature = "feature-name")]
use heavy_dep::Thing;

#[cfg(feature = "feature-name")]
pub fn uses_heavy_dep() { /* ... */ }
```

### Feature Implication Pattern

When multiple features need the same dep, use feature implication:

```toml
[features]
http-client = ["dep:reqwest"]
oauth = ["http-client", "dep:webbrowser", "dep:dirs", "dep:rand"]
jwt-auth = ["http-client", "dep:jsonwebtoken"]
sse = ["http-client", "dep:bytes"]
```

### Anti-Patterns to Avoid

- **Feature flag explosion:** Don't create a separate feature for every dep. Group logically.
- **Breaking default features:** Removing deps from default features is a semver-breaking change if downstream crates rely on them. The `validation` feature is safe to change since nothing uses it.
- **Overly granular cfg gates:** Don't scatter `#[cfg(feature = "X")]` across dozens of files. Gate at module boundaries where possible.

## Current State Analysis

### Dependency Counts (Production Only)

| Feature Configuration | Unique Deps | Notes |
|----------------------|-------------|-------|
| Default (`validation`) | 249 | Current default for consumers |
| No default features | 203 | Minimum with reqwest still required |
| Full features | 303 | Used by CI quality gate |

### Heavy Dependencies by Transitive Count

| Dependency | Transitive Deps | Status | Gating |
|------------|-----------------|--------|--------|
| reqwest 0.13.2 | 172 | Non-optional (platform dep) | `cfg(not(wasm32))` only |
| tracing-subscriber 0.3.22 | 48 | Non-optional | None |
| tokio 1.48.0 | 31 | Non-optional (platform dep) | `cfg(not(wasm32))` |
| futures 0.3.31 | 21 | Non-optional | None |
| dashmap 6.1.0 | 19 | Non-optional | None |
| chrono 0.4.42 | 14 | Non-optional | None |
| jsonschema 0.45.0 | ~218 total | Optional (validation feature) | But uses default features pulling reqwest |

### Unused Dependencies (Confirmed by cargo-machete + manual verification)

| Dependency | Evidence | Action |
|------------|----------|--------|
| `lazy_static` | Zero imports in `src/`. Codebase uses `std::sync::LazyLock` (MSRV 1.83 supports it) | Remove |
| `pin-project` | Zero imports in `src/`. No `pin_project` macro usage found | Remove |
| `rayon` | Zero imports in `src/`. Already optional (`dep:rayon`) but listed as direct dep | Already optional, no action |
| `jsonschema` | Optional dep behind `validation` feature, but NO code uses `cfg(feature = "validation")` | Phantom feature -- keep dep but fix default-features |
| `garde` | Same as jsonschema -- behind `validation` but never cfg-gated in code | Phantom feature -- investigate or remove |

### Reqwest Usage Map

| File | Feature Gate | Purpose |
|------|-------------|---------|
| `src/shared/sse_optimized.rs` | `feature = "sse"` | SSE client transport |
| `src/server/auth/providers/cognito.rs` | `cfg(not(wasm32))` | OAuth provider HTTP calls |
| `src/server/auth/providers/generic_oidc.rs` | `cfg(not(wasm32))` | OIDC discovery + token exchange |
| `src/server/auth/jwt.rs` | `cfg(not(wasm32))` | JWKS fetching |
| `src/server/auth/jwt_validator.rs` | `cfg(not(wasm32))` | JWKS fetching |
| `src/client/auth.rs` | `cfg(not(wasm32))` | Client-side OIDC/token exchange |
| `src/client/oauth.rs` | `feature = "oauth"` | Client OAuth flow |

All reqwest usage falls into auth/OAuth (server + client) and SSE transport. None of these are needed for a basic stdio MCP server.

### Tokio Feature Bloat

Current: `tokio = { features = ["full"] }`

Resolved features: `bytes, default, fs, full, io-std, io-util, libc, macros, mio, net, parking_lot, process, rt, rt-multi-thread, signal, signal-hook-registry, socket2, sync, time, tokio-macros`

Actually used tokio modules in `src/`:
- `tokio::fs` (read_to_string, create_dir_all, write) -- in client/oauth.rs only
- `tokio::io::stdin`, `tokio::io::stdout` -- in shared/stdio.rs
- `tokio::net` -- TCP listeners in transport
- `tokio::spawn`, `tokio::select!` -- core async runtime
- `tokio::sync::{mpsc, oneshot, RwLock}` -- core channels
- `tokio::time::{sleep, timeout, interval}` -- timers
- `tokio::task::spawn_blocking` -- blocking task offload

NOT used: `tokio::process`, `tokio::signal` (adds `signal-hook-registry` dep)

Recommended minimal set: `rt-multi-thread, macros, net, io-util, io-std, fs, sync, time`

### Hyper/Hyper-util Feature Bloat

Current: `hyper = { features = ["full"] }` (optional, behind "http" feature)
Resolved: `client, default, http1, http2, server`

pmcp uses hyper only for server-side HTTP (streamable HTTP transport). The `client` and `http2` features are unnecessary for pmcp's direct use. However, when reqwest is in the tree, it forces `http2` on hyper regardless.

Current: `hyper-util = { features = ["full"] }` (optional)
Resolved: `client, client-legacy, client-proxy, default, http1, http2, tokio`

pmcp only needs: `tokio, http1, server`

### jsonschema Default Features

jsonschema 0.45.0 default = `["resolve-http", "resolve-file", "tls-aws-lc-rs"]`

- `resolve-http` pulls in reqwest + rustls (the entire HTTP + TLS stack)
- `resolve-file` enables resolving `$ref` from local files
- `tls-aws-lc-rs` selects the aws-lc-rs TLS backend

pmcp uses jsonschema for local-only schema validation (no remote `$ref` resolution needed). Setting `default-features = false` is safe and drops the entire reqwest chain from jsonschema's path.

### Duplicate Crate Versions in Workspace

| Crate | Versions | Cause |
|-------|----------|-------|
| reqwest | 0.12, 0.13 | oauth2 crate pins reqwest 0.12; pmcp uses 0.13 |
| rand | 0.8, 0.9, 0.10 | oauth2 uses 0.8; proptest uses 0.9; pmcp uses 0.10 |
| getrandom | 0.2, 0.3, 0.4 | Various consumers across versions |
| console | 0.15, 0.16 | insta (dev-dep) uses 0.15; cargo-pmcp uses 0.16 |
| base64 | 0.21, 0.22 | Transitive vs direct |
| hashbrown | 0.14, 0.16 | dashmap uses 0.14; indexmap uses 0.16 |

The reqwest 0.12/0.13 duplication comes from `oauth2 v5.0` (in cargo-pmcp) pinning reqwest 0.12. This is a workspace-level issue, not a pmcp core issue.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Dep tree analysis | Custom scripts to parse Cargo.lock | `cargo tree` with various flags | Built-in, handles feature resolution correctly |
| Unused dep detection | Manual grep | `cargo machete` | Handles proc macros, optional deps (with caveats) |
| Feature testing | Manual builds | CI matrix with feature combinations | Catches cfg gate mismatches early |

## Common Pitfalls

### Pitfall 1: Breaking Downstream Consumers
**What goes wrong:** Removing a dep from default features causes compile errors for consumers who relied on it being present.
**Why it happens:** Cargo features are additive and consumers may depend on transitive items.
**How to avoid:** Check what's actually exported in the public API. The `validation` feature is safe because no code uses it. Making reqwest optional is also safe because reqwest is not re-exported. But `pub use providers::{CognitoProvider, GenericOidcProvider}` in auth/mod.rs exports types that contain reqwest fields -- these need feature gating.
**Warning signs:** `pub use` of types that contain optional-dep types.

### Pitfall 2: cfg Gate Mismatches Between Code and Tests
**What goes wrong:** Code compiles with `--no-default-features` but tests fail because test code doesn't have matching cfg gates.
**Why it happens:** Tests often import things unconditionally.
**How to avoid:** After changing features, build with: `cargo check --no-default-features`, `cargo check --features X` for each feature, and `cargo test --features full`.
**Warning signs:** CI only tests with `--all-features`, never catches missing-feature issues.

### Pitfall 3: Feature Unification Surprise
**What goes wrong:** A dep's features get unified across the workspace, pulling in more than expected.
**Why it happens:** Cargo unifies features for a crate across all dependents. If workspace member A enables feature X on dep D, and member B also depends on D, B gets feature X too in the workspace build.
**How to avoid:** Test the pmcp crate in isolation (`cargo check -p pmcp --no-default-features`) not just as part of the workspace.
**Warning signs:** `cargo tree -f '{p} {f}'` shows unexpected features on deps.

### Pitfall 4: Phantom Features
**What goes wrong:** A feature flag is defined in Cargo.toml and deps are gated behind it, but no source code actually uses `#[cfg(feature = "...")]` -- the deps compile but do nothing.
**Why it happens:** Feature was added for future use, or the code that used it was refactored away.
**How to avoid:** Search for `cfg(feature = "feature_name")` for every defined feature. If zero hits, the feature is phantom.
**Warning signs:** cargo-machete reports optional deps as unused.

### Pitfall 5: auth Module Export Breakage
**What goes wrong:** Making reqwest optional breaks `pub use providers::{CognitoProvider, GenericOidcProvider}` which are unconditionally exported from `server::auth`.
**Why it happens:** These types have `http_client: reqwest::Client` fields gated on `cfg(not(wasm32))` but NOT on any feature flag.
**How to avoid:** Add `cfg(feature = "http-client")` to both the field definitions AND the `pub use` re-exports. Add the providers re-export behind the same feature gate.
**Warning signs:** Build fails on `cargo check --no-default-features` after making reqwest optional.

## Recommended Changes (Priority Order)

### Priority 1: Remove Unused Dependencies (Risk: LOW, Impact: LOW-MEDIUM)

```toml
# REMOVE from [dependencies]:
# lazy_static = "1.5"      # Unused -- codebase uses std::sync::LazyLock
# pin-project = "1.1"      # Unused -- never imported

# ALSO verify and potentially remove from validation feature:
# validation = ["dep:jsonschema", "dep:garde"]
# Neither crate is imported anywhere. The validation feature is phantom.
```

Estimated savings: 3-4 deps (pin-project + pin-project-internal proc macros; lazy_static is also pulled by tracing-subscriber so net saving is 0 for that one in the resolved tree).

### Priority 2: Set jsonschema default-features = false (Risk: LOW, Impact: HIGH)

```toml
# FROM:
jsonschema = { version = "0.45", optional = true }

# TO:
jsonschema = { version = "0.45", optional = true, default-features = false }
```

This prevents jsonschema from pulling in reqwest+rustls through `resolve-http`. pmcp only validates schemas locally, never resolves remote `$ref` URIs.

Estimated savings: When validation feature is enabled, avoids ~170 deps from reqwest through jsonschema's path. (Many overlap with pmcp's direct reqwest, but matters when reqwest itself becomes optional.)

### Priority 3: Make reqwest Optional (Risk: MEDIUM, Impact: HIGH)

```toml
# FROM:
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
reqwest = { version = "0.13", default-features = false, features = ["json", "rustls", "form"] }

# TO:
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
reqwest = { version = "0.13", optional = true, default-features = false, features = ["json", "rustls", "form"] }

[features]
http-client = ["dep:reqwest"]
# Update existing features that need reqwest:
oauth = ["http-client", "dep:webbrowser", "dep:dirs", "dep:rand"]
jwt-auth = ["http-client", "dep:jsonwebtoken"]
sse = ["http-client", "dep:bytes"]
```

Source code changes required:
1. `src/client/auth.rs` -- gate behind `#[cfg(feature = "http-client")]`
2. `src/client/mod.rs` -- gate `pub mod auth` behind `#[cfg(all(not(target_arch = "wasm32"), feature = "http-client"))]`
3. `src/server/auth/providers/cognito.rs` -- add `#[cfg(feature = "http-client")]` to reqwest fields and methods
4. `src/server/auth/providers/generic_oidc.rs` -- same
5. `src/server/auth/jwt.rs` -- reqwest fields already behind `cfg(not(wasm32))`, add `feature = "http-client"`
6. `src/server/auth/jwt_validator.rs` -- same
7. `src/server/auth/mod.rs` -- gate `pub use providers::{CognitoProvider, GenericOidcProvider}` behind `http-client`
8. `src/shared/sse_optimized.rs` -- already behind `feature = "sse"` which will imply `http-client`

Estimated savings: ~20 unique deps for consumers not enabling http-client/oauth/jwt-auth/sse.

### Priority 4: Slim Tokio Features (Risk: LOW, Impact: LOW)

```toml
# FROM:
tokio = { version = "1.46", features = ["full"] }

# TO:
tokio = { version = "1.46", features = ["rt-multi-thread", "macros", "net", "io-util", "io-std", "fs", "sync", "time"] }
```

Drops `process` and `signal` features (unused). Saves `signal-hook-registry` dep and reduces compile scope.

Note: `parking_lot` feature on tokio can also be dropped since pmcp already has its own `parking_lot` dep and tokio's usage is internal optimization.

### Priority 5: Slim Hyper/Hyper-util Features (Risk: LOW, Impact: LOW)

```toml
# FROM:
hyper = { version = "1.6", features = ["full"], optional = true }
hyper-util = { version = "0.1", features = ["full"], optional = true }

# TO:
hyper = { version = "1.6", features = ["http1", "server"], optional = true }
hyper-util = { version = "0.1", features = ["tokio", "http1", "server-auto"], optional = true }
```

Note: When reqwest is in the tree (http-client feature enabled), it will unify hyper features to include http2+client anyway. This change only helps when reqwest is NOT enabled.

### Priority 6: Make tracing-subscriber Optional (Risk: MEDIUM, Impact: LOW)

```toml
# FROM:
tracing-subscriber = { version = "0.3.20", features = ["env-filter"] }

# TO:
tracing-subscriber = { version = "0.3.20", features = ["env-filter"], optional = true }

[features]
logging = ["dep:tracing-subscriber"]  # For SDK-provided logging setup
```

tracing-subscriber is used only in `src/shared/logging.rs` for the SDK's built-in logging configuration. Most consumers set up their own tracing subscriber. Making it optional saves ~5 unique deps.

Source change: gate `src/shared/logging.rs` functions behind `#[cfg(feature = "logging")]`.

### Priority 7: Clean Up chrono Features (Risk: LOW, Impact: NEGLIGIBLE)

```toml
# FROM:
chrono = { version = "0.4", features = ["serde"] }

# TO:
chrono = { version = "0.4", default-features = false, features = ["clock", "serde", "std"] }
```

Drops `iana-time-zone`, `oldtime`, and platform-specific timezone deps. pmcp only uses `Utc::now()` and `DateTime<Utc>` serialization, which only needs `clock` + `serde` + `std`.

Estimated savings: 1-2 deps (iana-time-zone, core-foundation-sys on macOS).

### Priority 8: Address Validation Feature (Risk: LOW, Impact: CLEANUP)

The `validation` feature is phantom -- it's defined, it gates `jsonschema` and `garde` as optional deps, but no code uses `#[cfg(feature = "validation")]`. Options:

1. **Remove the feature entirely** and remove jsonschema + garde from deps (they're unused)
2. **Actually implement validation gating** if there's planned use
3. **Keep as-is** but with `default-features = false` on jsonschema (Priority 2)

Recommendation: Option 1 (remove) unless there are concrete plans to use these crates. This removes jsonschema (218 deps with defaults) and garde (18 deps) from the default build entirely.

## Projected Impact

### Conservative Estimate (Priorities 1-4 only)

| Metric | Before | After | Savings |
|--------|--------|-------|---------|
| Default feature deps | 249 | ~185 | ~64 |
| No-feature deps | 203 | ~160 | ~43 |
| Full feature deps | 303 | ~303 | ~0 (full still pulls everything) |

### Aggressive Estimate (All priorities)

| Metric | Before | After | Savings |
|--------|--------|-------|---------|
| Default feature deps | 249 | ~150 | ~99 |
| No-feature deps | 203 | ~140 | ~63 |
| Full feature deps | 303 | ~300 | ~3 |

The aggressive estimate aligns with the issue's target of "255 -> ~120" for consumers using minimal features.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) + cargo nextest |
| Config file | Makefile (`make quality-gate`) |
| Quick run command | `cargo check --no-default-features -p pmcp` |
| Full suite command | `make quality-gate` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DEP-01 | Build succeeds with no default features | smoke | `cargo check -p pmcp --no-default-features` | N/A (CLI) |
| DEP-02 | Build succeeds with each individual feature | smoke | `cargo check -p pmcp --no-default-features --features X` for each X | N/A (CLI) |
| DEP-03 | Build succeeds with full features | smoke | `cargo check -p pmcp --features full` | N/A (CLI) |
| DEP-04 | Workspace builds with all members | smoke | `cargo build --workspace` | N/A (CLI) |
| DEP-05 | All tests pass with full features | integration | `cargo test --features full -- --test-threads=1` | Existing tests |
| DEP-06 | CI quality gate passes | integration | `make quality-gate` | Existing Makefile |
| DEP-07 | Dep count reduced | manual | `cargo tree -p pmcp -e no-dev --prefix none \| sort -u \| wc -l` | N/A (CLI) |

### Sampling Rate

- **Per task commit:** `cargo check -p pmcp --no-default-features && cargo check -p pmcp --features full`
- **Per wave merge:** `make quality-gate`
- **Phase gate:** Full suite green + dep count verification

### Wave 0 Gaps

- [ ] Add CI job for `cargo check --no-default-features -p pmcp` -- currently CI only tests `--all-features`
- [ ] Add CI job for individual feature builds to catch cfg gate mismatches

## Open Questions

1. **Should the `validation` feature be removed entirely?**
   - What we know: jsonschema and garde are not imported anywhere in the codebase. The validation feature is phantom.
   - What's unclear: Whether there are plans to use these crates in the future.
   - Recommendation: Remove from default features immediately. If there's future use planned, keep as opt-in feature with `default-features = false`. If no plans, remove entirely.

2. **Should `sha2` and `base64` be feature-gated?**
   - What we know: sha2 is used only in OAuth code (server + client). base64 is used in resources, SIMD, composition, and OAuth -- too widespread to gate.
   - What's unclear: Whether the server auth module's sha2 use could be isolated.
   - Recommendation: Keep sha2 and base64 as required for now. The savings are minimal (sha2 adds ~5 deps that overlap with other paths).

3. **Should `toml` be feature-gated?**
   - What we know: Used in observability config and composition module.
   - What's unclear: Whether observability config loading is used by most consumers.
   - Recommendation: Defer. toml has modest dep count (~8 unique) and is broadly useful.

4. **Should downstream crates (mcp-tester, cargo-pmcp) also be optimized?**
   - What we know: cargo-pmcp has oauth2 pulling in reqwest 0.12 alongside pmcp's reqwest 0.13 (duplicate).
   - What's unclear: Whether oauth2 has a newer version that uses reqwest 0.13.
   - Recommendation: Out of scope for this phase. Focus on pmcp core first.

## Sources

### Primary (HIGH confidence)
- Direct `cargo tree` analysis of the workspace (all dep counts verified locally)
- Direct `cargo machete` analysis (unused deps confirmed)
- GitHub issue #175 (https://github.com/paiml/rust-mcp-sdk/issues/175) - detailed analysis from pmat comply check
- Source code grep analysis of all reqwest/tokio/chrono/jsonschema usage

### Secondary (MEDIUM confidence)
- `cargo info jsonschema` - verified default features include resolve-http
- crates.io API - confirmed jsonschema feature definitions

### Tertiary (LOW confidence)
- None -- all findings verified directly from source and tooling

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all tooling verified against current workspace
- Architecture: HIGH - feature gating patterns are well-established Rust idioms
- Pitfalls: HIGH - based on direct analysis of actual code structure and exports
- Impact estimates: MEDIUM - dep counts are exact but savings projections depend on feature unification behavior

**Research date:** 2026-03-18
**Valid until:** 2026-04-18 (stable domain, dep versions may change with updates)
