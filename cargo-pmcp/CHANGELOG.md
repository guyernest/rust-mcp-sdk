# Changelog

All notable changes to the `cargo-pmcp` crate will be documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).

## [0.10.0] — 2026-04

### Added
- **Declarative IAM in `.pmcp/deploy.toml`** (Phase 76, closes pmcp-run CR
  `CLI_IAM_CHANGE_REQUEST.md`). New optional `[iam]` section with three
  repeated tables: `[[iam.tables]]`, `[[iam.buckets]]`, `[[iam.statements]]`.
  Sugar keywords `read` / `write` / `readwrite` map to per-CR action lists;
  `[[iam.statements]]` is passthrough after validation. See
  `DEPLOYMENT.md#iam-declarations-iam-section`.
- **Stable `McpRoleArn` CfnOutput.** Both `pmcp-run` and `aws-lambda` stack
  templates now emit `new cdk.CfnOutput(this, 'McpRoleArn', ...)` with
  `exportName: pmcp-${serverName}-McpRoleArn`. Bolt-on CDK stacks can switch
  from `iam.Role.fromRoleName` to `iam.Role.fromRoleArn(Fn.importValue(...))`.
- **`cargo pmcp validate deploy` subcommand.** Pre-flights `.pmcp/deploy.toml`
  and rejects IAM footguns (wildcard Allow on `*:*`, malformed actions, bad
  effects, sugar-keyword typos).
- **`examples/deploy_with_iam.rs`** — runnable walkthrough of parse →
  validate → render, plus a demonstration of the validator rejecting an
  Allow-*-* configuration.
- **`fuzz_iam_config` libfuzzer target** covering `toml::from_str::<DeployConfig>`
  + `cargo_pmcp::deployment::iam::validate` with three corpus seeds (empty,
  cost-coach, wildcard). 10-second smoke run: 170K executions, zero panics.

### Changed
- `cargo pmcp deploy` now runs `iam::validate(&config.iam)` immediately after
  `DeployConfig::load`. Deploys with invalid IAM fail-fast before touching AWS.
- `aws-lambda` stack template now imports `aws-cdk-lib/aws-iam`. Pure addition;
  existing templates unaffected when no `[iam]` section is present.
- Library-visible surface widened: `deployment::config` and `deployment::iam`
  are now `pub` through a narrow `#[path]`-mounted view in `src/lib.rs` so the
  fuzz target and the `deploy_with_iam` example can compile against the real
  public API. The rest of `deployment::*` remains bin-only.

### Fixed
- N/A

### Security
- T-76-02 (wildcard escalation) mitigated by hard-error validation of
  Allow + `*:*` + `*` in any `[[iam.statements]]` entry (blocks
  `cargo pmcp deploy` fail-closed).
- T-76-03 (parser DoS) mitigated by the new `fuzz_iam_config` libfuzzer
  target — continuous coverage of `toml::from_str::<DeployConfig>()` plus
  `iam::validate()` on arbitrary UTF-8 input.
