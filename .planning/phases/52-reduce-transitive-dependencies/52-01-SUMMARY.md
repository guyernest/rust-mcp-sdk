---
phase: 52-reduce-transitive-dependencies
plan: 01
subsystem: infra
tags: [cargo, dependencies, feature-flags, reqwest, tokio, hyper, tracing-subscriber]

# Dependency graph
requires: []
provides:
  - Updated Cargo.toml with slimmed dependency features and optional reqwest/tracing-subscriber
  - New http-client and logging feature flags
  - Feature implication chain (oauth/jwt-auth/sse -> http-client -> dep:reqwest)
affects: [52-02-PLAN (cfg gates for source code)]

# Tech tracking
tech-stack:
  added: []
  patterns: [feature-implication for optional heavy deps, explicit tokio feature list, default-features = false on jsonschema]

key-files:
  created: []
  modified: [Cargo.toml]

key-decisions:
  - "default feature changed from validation to logging -- validation was phantom (no cfg gates in code)"
  - "reqwest gated behind http-client feature with implication from oauth, jwt-auth, and sse"
  - "tracing-subscriber gated behind logging feature for consumers who bring their own subscriber"
  - "tokio slimmed from full to explicit features (drops process and signal)"
  - "hyper/hyper-util slimmed from full to http1+server only"
  - "jsonschema set to default-features = false (drops resolve-http reqwest chain)"
  - "chrono slimmed to clock+serde+std (drops iana-time-zone)"

patterns-established:
  - "Feature implication: features needing reqwest imply http-client rather than dep:reqwest directly"
  - "Explicit tokio feature lists instead of full for production crates"

requirements-completed: [DEP-REDUCE-01, DEP-REDUCE-02, DEP-REDUCE-03, DEP-REDUCE-04, DEP-REDUCE-05]

# Metrics
duration: 4min
completed: 2026-03-18
---

# Phase 52 Plan 01: Reduce Transitive Dependencies - Cargo.toml Changes Summary

**Removed lazy_static/pin-project, made reqwest and tracing-subscriber optional behind http-client/logging features, slimmed tokio/hyper/chrono/jsonschema feature sets**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-18T15:18:04Z
- **Completed:** 2026-03-18T15:22:11Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Removed 2 unused dependencies (lazy_static, pin-project)
- Made reqwest optional behind new `http-client` feature flag, reducing default dep count significantly
- Made tracing-subscriber optional behind new `logging` feature flag
- Slimmed tokio from `full` to explicit feature list (drops process/signal)
- Slimmed hyper from `full` to `http1, server` and hyper-util from `full` to `tokio, http1, server-auto`
- Set `jsonschema` to `default-features = false` (prevents pulling reqwest through jsonschema's resolve-http)
- Slimmed chrono to `clock, serde, std` (drops iana-time-zone)
- Changed default features from `validation` (phantom) to `logging`
- Verified all workspace members (pmcp, mcp-tester, mcp-preview, cargo-pmcp) build successfully with full features

## Task Commits

Each task was committed atomically:

1. **Task 1: Remove unused deps, slim features, make reqwest and tracing-subscriber optional** - `7f1c293` (chore)
2. **Task 2: Verify feature matrix builds** - verification-only task, no code changes

## Files Created/Modified
- `Cargo.toml` - Removed unused deps, slimmed features, added http-client/logging features, made reqwest/tracing-subscriber optional

## Decisions Made
- Changed default feature from `validation` to `logging` -- validation was phantom (no code uses `cfg(feature = "validation")`)
- Features that need reqwest (oauth, jwt-auth, sse) imply `http-client` rather than declaring `dep:reqwest` directly, providing a single toggle point
- tokio slimmed to `rt-multi-thread, macros, net, io-util, io-std, fs, sync, time` -- drops unused `process`, `signal`, `parking_lot` features
- hyper slimmed to `http1, server` -- pmcp only does HTTP/1.1 server; when reqwest is enabled it unifies to include http2 anyway
- jsonschema set to `default-features = false` -- pmcp only validates local schemas, never resolves remote `$ref` URIs

## Deviations from Plan

None - plan executed exactly as written.

## Files Needing cfg Gates in Plan 02

The following files have unconditional `use reqwest::*` or `use tracing_subscriber::*` statements that cause build failures when those deps are not enabled:

**reqwest (needs `#[cfg(feature = "http-client")]`):**
- `src/client/auth.rs` -- reqwest::Client usage in OidcAuthClient
- `src/server/auth/jwt.rs` -- reqwest::Client for JWKS fetching
- `src/server/auth/jwt_validator.rs` -- reqwest::Client for JWKS fetching
- `src/server/auth/providers/cognito.rs` -- reqwest::Client field and methods
- `src/server/auth/providers/generic_oidc.rs` -- reqwest::Client for OIDC discovery

**tracing-subscriber (needs `#[cfg(feature = "logging")]`):**
- `src/shared/logging.rs` -- entire module uses tracing_subscriber types

## Issues Encountered
- `cargo check --workspace` fails on `pmcp-server-lambda` due to pre-existing non-exhaustive match on `lambda_http::Body` -- unrelated to this plan's changes, not addressed
- `cargo check -p pmcp` (default features = logging only) fails with 57 errors in reqwest-using files -- this is expected and will be fixed in Plan 02 with cfg gates

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Cargo.toml changes are complete and verified with full features
- Plan 02 must add cfg gates to 6 source files listed above before the `--no-default-features` and default-features builds will work
- All workspace members already build correctly since they enable features that imply http-client

## Self-Check: PASSED

- [x] Cargo.toml exists and contains all expected changes
- [x] 52-01-SUMMARY.md created
- [x] Commit 7f1c293 exists in git history
- [x] lazy_static removed from Cargo.toml
- [x] pin-project removed from Cargo.toml
- [x] http-client feature defined
- [x] logging feature defined
- [x] reqwest marked optional
- [x] default features set to logging

---
*Phase: 52-reduce-transitive-dependencies*
*Completed: 2026-03-18*
