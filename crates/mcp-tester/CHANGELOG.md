# Changelog

All notable changes to the `mcp-tester` crate will be documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).

## [0.7.0] - 2026-05-16

### Changed

- **pmcp dep bumped 2.6.0 → 2.8.0** (transitive: pulls in the
  `AuthProvider::on_unauthorized()` hook + transport retry-once-on-401 from
  pmcp 2.8.0, and the RUSTSEC-2026-0098/0099/0104 rustls-webpki fixes from
  pmcp 2.7.0).
- **MSRV bumped to Rust 1.91** to align with the workspace; CI MSRV gate
  pinned to `dtolnay/rust-toolchain@1.91`.

### Internal

- **`app_validator::strip_js_comments`** cog 59 → ~10 via P1 helper extraction
  per Phase 75 patterns: split the 6-state JS parser body into four focused
  per-state helpers (`step_js_outside`, etc.). Functional behavior preserved —
  the 10 existing `strip_js_comments` unit tests still pass.
- Minor stylistic cleanups from `clippy::unnecessary_duration_constructor`
  fired by the MSRV bump (Duration::from_secs round-number → from_mins/
  from_hours rewrites). Semantically equivalent.

### Notes

- Bump from 0.6.0 → 0.7.0 is **minor** because the pmcp dep major-line
  remained on `2.x` but the SDK gained a new public trait method
  (`on_unauthorized`); downstream callers of `mcp-tester` see no API change.

## [0.6.0] - 2026-05-03

### Added
- **`mcp_tester::post_deploy_report::PostDeployReport`** — new machine-readable
  contract for `cargo pmcp test {check, conformance, apps} --format=json` output,
  consumed by the Phase 79 post-deploy verifier (Plan 79-05).
- `PostDeployReport`, `TestCommand`, `TestOutcome`, `FailureDetail` types with
  `Serialize + Deserialize` and `schema_version: "1"` for forward-compatibility.
- New `--format=<pretty|json>` flag on `cargo pmcp test {check, conformance, apps}`
  (default `pretty`, behavior unchanged for human users).

### Changed
- N/A (additive minor bump per CLAUDE.md release rules).

### Notes
- Bump from 0.5.3 → 0.6.0 is **minor** because `post_deploy_report` is a new
  public module — additive surface that does not break any existing consumer.
- `cargo-pmcp 0.12.0` consumes this contract via the typed JSON path; older
  versions of cargo-pmcp continue to work because the `--format` flag defaults
  to `pretty` (the pre-0.6.0 behavior).
