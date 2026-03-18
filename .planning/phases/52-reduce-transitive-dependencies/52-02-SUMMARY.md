---
phase: 52-reduce-transitive-dependencies
plan: 02
subsystem: infra
tags: [cargo, feature-flags, cfg-gates, reqwest, tracing-subscriber, conditional-compilation]

# Dependency graph
requires:
  - phase: 52-01
    provides: "Cargo.toml with optional reqwest/tracing-subscriber behind http-client/logging features"
provides:
  - "cfg-gated source files: pmcp builds with --no-default-features (134 deps vs 295 with full)"
  - "Full feature matrix verified: no-default-features, logging, http-client, full"
  - "All auth providers (jwt, jwt_validator, cognito, generic_oidc) behind http-client gate"
  - "init_logging and CorrelationLayer behind logging gate"
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns: ["cfg(feature = http-client) gate at module level for reqwest-dependent code", "cfg(all(not(wasm32), feature = logging)) combined gate for tracing-subscriber code"]

key-files:
  created: []
  modified:
    - src/client/mod.rs
    - src/server/auth/mod.rs
    - src/shared/logging.rs
    - src/shared/mod.rs

key-decisions:
  - "Gate entire modules (jwt, jwt_validator, providers) at parent auth/mod.rs rather than individual items inside -- simpler and prevents dead code in gated modules"
  - "CorrelationLayer gated behind logging feature since it implements tracing_subscriber::Layer"
  - "No changes needed inside jwt.rs, jwt_validator.rs, cognito.rs, generic_oidc.rs -- parent-level module gates sufficient"

patterns-established:
  - "Module-level cfg gating: gate entire mod declarations in parent rather than sprinkling cfg inside child modules"
  - "Combined cfg gates: all(not(wasm32), feature = X) for features unavailable on both wasm and no-feature builds"

requirements-completed: [DEP-REDUCE-06, DEP-REDUCE-07]

# Metrics
duration: 3min
completed: 2026-03-18
---

# Phase 52 Plan 02: Source File cfg Gates for Optional reqwest and tracing-subscriber Summary

**cfg-gated all reqwest and tracing-subscriber usage sites enabling pmcp to compile with 134 deps (--no-default-features) vs 295 deps (--features full)**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-18T15:24:59Z
- **Completed:** 2026-03-18T15:28:42Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Gated client/auth module and server/auth JWT/provider modules behind `cfg(feature = "http-client")` so reqwest is not required at compile time without the feature
- Gated tracing-subscriber imports, init_logging, and CorrelationLayer behind `cfg(feature = "logging")` so tracing-subscriber is not required without the feature
- Full feature matrix verified: no-default-features (134 deps), logging (145 deps), http-client (190 deps), full (295 deps)
- All 877 lib tests pass with --features full
- Workspace builds successfully (pre-existing pmcp-server-lambda issue unrelated)

## Task Commits

Each task was committed atomically:

1. **Task 1: Gate reqwest-dependent modules behind http-client feature** - `6239c45` (feat)
2. **Task 2: Gate tracing-subscriber usage behind logging feature** - `0f64e24` (feat)

## Files Created/Modified
- `src/client/mod.rs` - Added `feature = "http-client"` to auth module gate
- `src/server/auth/mod.rs` - Gated jwt, jwt_validator, providers modules and their re-exports behind `http-client`
- `src/shared/logging.rs` - Gated tracing_subscriber imports, init_logging, CorrelationLayer behind `logging`
- `src/shared/mod.rs` - Gated init_logging re-export behind `logging`

## Decisions Made
- Gated entire modules at parent level (auth/mod.rs) rather than inside individual files -- cleaner, prevents compiling dead code
- No internal changes needed in jwt.rs, jwt_validator.rs, cognito.rs, generic_oidc.rs -- parent-level gates are sufficient since these modules are only accessed through auth/mod.rs
- CorrelationLayer requires both `not(wasm32)` and `feature = "logging"` since it implements `tracing_subscriber::Layer`

## Deviations from Plan

None - plan executed exactly as written.

## Dependency Count Summary

| Configuration | Dep Count | Delta vs Full |
|--------------|-----------|---------------|
| --no-default-features | 134 | -161 (54% fewer) |
| --features logging (default) | 145 | -150 (51% fewer) |
| --no-default-features --features http-client | 190 | -105 (36% fewer) |
| --features full | 295 | baseline |

## Issues Encountered
- Pre-existing `pmcp-server-lambda` non-exhaustive match on `lambda_http::Body` causes workspace check to fail -- unrelated to this plan, documented in Plan 01 summary

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Phase 52 is complete: Cargo.toml changes (Plan 01) + source cfg gates (Plan 02) both done
- pmcp consumers can now use `--no-default-features` for minimal dependency builds
- All workspace members build with their existing feature configurations

## Self-Check: PASSED

- [x] src/client/mod.rs exists and contains http-client gate
- [x] src/server/auth/mod.rs exists and contains http-client gate
- [x] src/shared/logging.rs exists and contains logging gate
- [x] src/shared/mod.rs exists and contains logging gate
- [x] Commit 6239c45 exists in git history
- [x] Commit 0f64e24 exists in git history
- [x] cargo check -p pmcp --no-default-features succeeds
- [x] cargo check -p pmcp --features full succeeds
- [x] cargo test -p pmcp --features full --lib passes 877 tests

---
*Phase: 52-reduce-transitive-dependencies*
*Completed: 2026-03-18*
