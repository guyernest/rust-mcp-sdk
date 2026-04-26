# Changelog

All notable changes to the `cargo-pmcp` crate will be documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).

## [0.11.0] - 2026-04-XX

### Added
- `cargo pmcp configure {add,use,list,show}` command group for managing named deployment targets (dev / prod / staging / …) — modeled on `aws configure`. Targets are defined in `~/.pmcp/config.toml` (typed-per-variant TOML schema with `pmcp-run`, `aws-lambda`, `google-cloud-run`, `cloudflare-workers` variants). A workspace selects a target via `.pmcp/active-target` (single-line marker file). Resolution precedence: `PMCP_TARGET` env > `--target` flag > `.pmcp/active-target` > none.
- New global `--target <name>` flag on the top-level `Cli` for one-off named-target overrides.
- Header banner emitted to stderr before each AWS API / CDK / upload call: `→ Using target: <name> (<type>)` + fixed-order field block (api_url / aws_profile / region / source). Suppressible with `--quiet` (except the `PMCP_TARGET` override note, which always fires as a safety signal).
- `~/.pmcp/config.toml` parser fuzz target (`pmcp_config_toml_parser`) and property tests for atomic-write round-trip and field-precedence resolution.
- Working example: `cargo run --example multi_target_monorepo -p cargo-pmcp` — demonstrates a monorepo with two sibling servers (one `pmcp-run`, one `aws-lambda`) each carrying their own `.pmcp/active-target`.

### Changed
- **DEPRECATED**: `cargo pmcp deploy --target <type>` (where `<type>` is `aws-lambda`, `cloudflare-workers`, `pmcp-run`, `google-cloud-run`) — renamed to `--target-type <type>`. The old spelling continues to work via `#[arg(alias = "target")]` for one release cycle and will be removed in 0.12.0. The bare `--target <name>` flag now refers to a NAMED target from `~/.pmcp/config.toml`.

### Security
- `configure add` rejects raw-credential patterns (AWS access keys `AKIA[0-9A-Z]{16}`, AWS session keys `ASIA[0-9A-Z]{16}`, GitHub tokens `ghp_*` / `github_pat_*`, Stripe live keys `sk_live_*`, Google API keys `AIza*`) at insertion time with actionable error messages pointing at AWS profile names, env-var references, or Secrets Manager ARNs. `~/.pmcp/config.toml` itself never stores raw secrets — only references.
- Config writes are atomic (`tempfile::NamedTempFile::persist`); on Unix, file mode is `0o600` and parent directory is `0o700`.

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
