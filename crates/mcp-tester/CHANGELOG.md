# Changelog

All notable changes to the `mcp-tester` crate will be documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).

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
