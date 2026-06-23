# Phase 100 Deferred Items

Out-of-scope discoveries logged during execution (per executor SCOPE BOUNDARY rule).
These are NOT fixed by the discovering plan — they pre-date this plan's changes.

| Plan | File:Line | Issue | Notes |
|------|-----------|-------|-------|
| 100-02 | crates/pmcp-workbook-runtime/src/render/mod.rs:~944 | clippy::unnecessary_map_or — `.map_or(false, \|rest\| ...)` should be `.is_some_and(...)` | Pre-existing (not in 100-02 diff). `pmcp-workbook-runtime` is NOT covered by `make lint` (which lints only root `pmcp --features full`), so it does not block the CI clippy gate. Out of scope for WBVER-01 (writer formula-or-value change). Fix opportunistically in a render/mod.rs-touching plan. |
