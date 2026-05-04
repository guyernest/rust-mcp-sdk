# Changelog

All notable changes to the `cargo-pmcp` crate will be documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).

## [0.12.1] - 2026-05-03

### Fixed
- **Widget pre-build no longer hard-crashes on raw-HTML / CDN-import widgets.**
  Fixes the regression where `cargo pmcp deploy` aborted with raw `io::Error`
  ("No such file or directory", `os error 2`) when the auto-detected `widget/`
  or `widgets/` directory contained only `*.html` files importing the SDK from
  a CDN (e.g. `https://esm.sh/@modelcontextprotocol/ext-apps`). The Phase 45
  zero-build MCP Apps archetype is now correctly detected as raw-HTML and the
  Node pipeline (`npm install`, `npm run build`) is skipped entirely.
- **Eliminates the `npm install` parent-walk side effect.** Previously, when
  `package.json` was missing from `widgets/`, the CLI still spawned `npm
  install`; npm walked UP the directory tree, found a `package.json` in a
  parent workspace, and audited 1839 packages — risking writes to
  `node_modules/` or `package-lock.json` outside the project. The fix bails
  before any subprocess is spawned.
- **Defense-in-depth diagnostic** at `verify_build_script_exists`: missing
  `package.json` now produces an actionable error naming the widget directory
  and three remediation paths (add `package.json`, configure `[[widgets]]`
  with a non-Node build, or remove the override). Replaces the unactionable
  raw OS error.

### Notes
- HIGH-C1 invariant preserved: raw-HTML widget directories are still appended
  to `PMCP_WIDGET_DIRS` (colon-list), so the generated `build.rs`
  `cargo:rerun-if-changed` chain still rebuilds the binary on `*.html` edits.
- HIGH-G1 invariant preserved: `discover_local_widget_dirs()` build.rs
  fallback unchanged.
- HIGH-G2 invariant preserved: `ROLLBACK_REJECT_MESSAGE` unchanged.
- Reference reproduction fixed: `~/projects/mcp/Scientific-Calculator-MCP-App`
  (UAT Test 3, severity: major).

## [0.12.0] - 2026-05-03

### Added
- **`cargo pmcp deploy` now pre-builds widgets and post-deploy-verifies the live endpoint** (Phase 79).

#### Build half (closes Failure Modes A + B)
- Auto-detect `widget/` or `widgets/` at workspace root (NOT `ui/` or `app/`).
- Lockfile-driven package-manager runner (bun > pnpm > yarn > npm).
- New `[[widgets]]` section in `.pmcp/deploy.toml` with explicit `embedded_in_crates`.
- `[[widgets]].build` and `.install` are argv arrays (`Vec<String>`) — REVISION 3 (Codex MEDIUM): replaces the pre-revision string form which broke quoting on flags like `--silent`.
- Yarn PnP detection (`.pnp.cjs` / `.pnp.loader.mjs`) skips the `node_modules` install heuristic — REVISION 3 (Codex MEDIUM).
- **`cargo pmcp app new --embed-widgets`** opts into `include_str!` widget embedding; default scaffold uses `WidgetDir` (run-time file serving). REVISION 3 (Codex MEDIUM scaffold target alignment).
- When `--embed-widgets` is set, `cargo pmcp app new` scaffolds a `build.rs` that consumes **`PMCP_WIDGET_DIRS`** (colon-separated list, Unix `PATH` convention) for multi-widget cache invalidation — REVISION 3 (HIGH-C1 supersession of pre-revision single `PMCP_WIDGET_DIR`).
- The generated `build.rs` includes a **local-discovery fallback** (walks `CARGO_MANIFEST_DIR + ../widget|widgets/dist` up to 3 parents) when `PMCP_WIDGET_DIRS` is unset, restoring the `cargo run` direct-invocation dev loop — REVISION 3 (HIGH-G1 supersession).
- `cargo pmcp doctor` warns when an `include_str!` crate lacks the matching `build.rs`. WidgetDir crates do NOT trigger the warning (run-time file serving doesn't have the Cargo cache problem). REVISION 3 (Codex MEDIUM).
- New flags: `--no-widget-build`, `--widgets-only`.

#### Verify half (closes Failure Mode C)
- After Lambda hot-swap, runs warmup-grace → `cargo pmcp test check` → `cargo pmcp test conformance` → `cargo pmcp test apps --mode claude-desktop` (last only when widgets are present) before reporting deploy success.
- **`cargo pmcp test {check, conformance, apps} --format=json`** (NEW) emits a structured `mcp_tester::PostDeployReport` JSON document. The post-deploy verifier consumes this typed report — NO regex parsing of pretty terminal output — REVISION 3 (HIGH-1 supersession; requires mcp-tester 0.6.0).
- New `[post_deploy_tests]` section in `.pmcp/deploy.toml`.
- `on_failure="fail"` (default): CLI exits **3** (broken-but-live, REVISION 3 HIGH-2 — distinct from infra exit 2 and pre-cutover failures); the deployed broken Lambda revision STAYS LIVE.
- `on_failure="warn"`: CLI exits 0 with banner.
- `on_failure="rollback"` is **HARD-REJECTED** at config-validation time AND at clap parse time with an actionable error — REVISION 3 (HIGH-G2 supersession of pre-revision parse-but-warn behavior).
- **GitHub Actions / GitLab `::error::` annotation** auto-emitted to stderr when `CI=true` env detected — REVISION 3 (HIGH-2 supersession; augments the loud banner, doesn't replace it).
- Distinct exit codes: 0 success, 2 infra-error, 3 broken-but-live (REVISION 3 HIGH-2 renumbered from pre-revision 1).
- Auth pass-through: subprocesses **inherit parent env** (Tokio Command default) and resolve via the existing `AuthMethod::None` path which already supports Phase 74 OAuth cache + automatic refresh. NO parent-side `MCP_API_KEY` injection. NO `--api-key` argv (T-79-04 mitigation, REVISION 3 HIGH-C2 supersession of pre-revision `resolve_auth_token` helper).
- New flags: `--no-post-deploy-test`, `--post-deploy-tests=connectivity,conformance,apps`, `--on-test-failure=warn|fail` (rejects `rollback` at clap parse), `--apps-mode=standard|chatgpt|claude-desktop`.

### Changed
- `cargo pmcp deploy --help` now mentions widgets verbatim and points to the verification suite.
- **Bumped `mcp-tester` dependency to 0.6.0** for the new `PostDeployReport` machine-readable contract.

### Security
- T-79-01 — TOML parser DoS mitigated via `fuzz_widgets_config` (now also exercises OnFailure rollback-rejection path per HIGH-G2).
- T-79-02 — `[[widgets]].path` traversal rejected at validation time.
- T-79-04 — Auth tokens routed via subprocess env inheritance, NOT argv flags or parent-side injection.
- T-79-05 — `OnFailure::Rollback` user-expectation gap eliminated via HARD-REJECT at parse time (REVISION 3 HIGH-G2).
- T-79-17 — argv-array build/install closes the pre-revision whitespace-split injection vector.

### Notes
- Pre-existing projects scaffolded before 0.12.0 should run `cargo pmcp doctor` to learn whether they need the new `build.rs`. WidgetDir scaffolds don't need it.
- Multi-widget projects now correctly invalidate ALL widget output dirs via `PMCP_WIDGET_DIRS` colon-list (REVISION 3 HIGH-C1 fixes the pre-revision last-widget-wins bug).
- Direct `cargo run` / `cargo build` (without the deploy wrapper) now correctly invalidates on widget changes via the build.rs local-discovery fallback (REVISION 3 HIGH-G1).

## [0.11.0] - 2026-04-26

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
