---
quick_id: 260527-ttn
status: complete
date: 2026-05-27
---

# Quick Task 260527-ttn — Fix the three n51 GCR critical review findings

## Outcome

The three CRITICAL findings from the multi-angle code review of n51 are fixed
and verified. `cargo-pmcp` full test suite: **1178 passed / 0 failed** (n51
had 1169; 9 new tests added — 3 + 4 + 2). `make lint` (the real CI gate):
**✓ No lint issues**. fmt clean; `clippy::all` clean on `cargo-pmcp`.

## The three findings, fixed

### Fix #1 — GCR init wrote AWS-shape deploy.toml

**Was:** `cargo pmcp deploy init --target-type google-cloud-run` called
`DeployConfig::default_for_server` (AWS shape: required `[aws]`, no `[gcp]`,
`memory_mb=512`), then patched only `target.target_type`. The GCR-aware
`default_for_cloud_run_server` constructor existed but was dead code on the
dispatch path. `save_if_missing` cemented the bad file on every re-init.

**Now:** Extracted `pub(crate) fn default_config_for_target(target_id, …)` in
`cargo-pmcp/src/commands/deploy/mod.rs` that branches on `target_id`:
- `"google-cloud-run"` → `default_for_cloud_run_server` with empty `project_id`
  (operator fills `[gcp].project_id`; `deploy::resolve_params` already falls
  back to `gcloud config get-value project` when the field is empty/placeholder).
- everything else → existing `default_for_server` (legacy AWS shape, correct
  for `pmcp-run` / `cloudflare-workers` / etc.).

**Tests:** `google_cloud_run_target_produces_gcp_shape` (asserts gcp.is_some,
aws.is_none, target_type, region), `pmcp_run_target_produces_aws_shape`,
`cloudflare_workers_target_produces_aws_shape`.

### Fix #2 — GCR destroy/outputs/logs/metrics ignored config.gcp

**Was:** All four lifecycle methods read `project_id` from
`auth::get_project_id()` (gcloud config) and `region` from
`std::env::var("CLOUD_RUN_REGION")`, completely bypassing `config.gcp`.
`deploy()` correctly read `config.gcp` first; these methods did not — so
operator deploying to "prod-billing" per `deploy.toml` while gcloud was
logged into "dev-sandbox" would `destroy()` against dev-sandbox, silently
deleting a same-named unrelated service or returning "service not found"
in the wrong project. `metrics()` even had `let _project_id = ...`, dead
computation hiding the routing miss.

**Now:** Extracted `fn resolve_project_and_region(config: &DeployConfig)
-> Result<(String, String)>` in
`cargo-pmcp/src/deployment/targets/google_cloud_run/mod.rs`, mirroring
`deploy::resolve_params`'s precedence: `config.gcp.{project_id,region}`
(skipping empty + `"your-gcp-project-id"` placeholder) → `CLOUD_RUN_REGION`
env (region only) → `gcloud config get-value project` / `run/region`.
Routed `destroy()`, `outputs()`, `logs()` through it (`logs` keeps region
as `_region` since gcloud-logging filters by service-name label, not
region — future enhancement). `metrics()` is a stub today (just prints a
Cloud Console URL); the three dead `_`-prefixed bindings are dropped
entirely (was masking the bug); `config` parameter renamed `_config`.

**Tests:** `prefers_config_gcp_over_env`,
`region_falls_back_to_env_when_gcp_region_empty`,
`config_gcp_region_wins_over_env`, `rejects_placeholder_project_id`. Tests
use an RAII `EnvGuard` for safe save/restore of `CLOUD_RUN_REGION`; CI
runs `--test-threads=1` so env mutation is serialized.

### Fix #3 — configure resolver never read `[gcp]`

**Was:** The resolver in `cargo-pmcp/src/commands/configure/resolver.rs`
only read `d.aws.as_ref().map(|a| a.region.clone())` for the region source
of `google-cloud-run` and hardcoded `None` as the deploy_config source for
`gcp_project`. n51 had patched the resolver for compile-success when `aws`
became `Option<>` but never wired in the symmetric GcpConfig reader, so
`cargo pmcp configure show` against a Cloud Run deploy.toml displayed
blank deploy-config columns for region/project_id even though the values
were plainly in the file.

**Now:**
- Region source reads `d.aws.region OR d.gcp.region` (the two shapes are
  mutually exclusive per `target.target_type`, so the OR-chain is safe).
- `gcp_project` deploy_config source reads
  `d.gcp.as_ref().map(|g| g.project_id.clone()).filter(non-empty)` (was `None`).

**Tests:** `resolve_target_falls_back_to_deploy_toml_for_gcp_region`,
`resolve_target_falls_back_to_deploy_toml_for_gcp_project`. Use existing
`run_isolated` harness + `TargetConfigV1::empty()` named-target pattern.
Assert `TargetSource::DeployToml` is surfaced for both. Import line
widened to include `GoogleCloudRunEntry`.

## Verification
- `cargo clippy -p cargo-pmcp --all-targets --all-features -- -D clippy::all` → 0 errors
- `cargo fmt -p cargo-pmcp --check` → clean
- `cargo test -p cargo-pmcp --all-features -- --test-threads=1` → **1178 passed, 0 failed, 2 ignored** (the docker-gated integration tests, expected)
- `make lint` (the real CI gate; lints root `pmcp` with the project allow-list — config.rs is shared via the lib shim) → **✓ No lint issues**

## Commits (atomic, one per finding)
- `3205b409` fix(260527-ttn): GCR init writes [gcp]-shape deploy.toml (was AWS-shape)
- `90280ca7` fix(260527-ttn): GCR destroy/outputs/logs route through config.gcp not gcloud ambient
- `5a8846be` fix(260527-ttn): configure resolver reads [gcp].region + [gcp].project_id

## Remaining review findings (not in this task)
The original code review surfaced 15 ranked findings; this task fixed the top
3 CRITICALs. Remaining (per severity order, see the review thread for
details):
- HIGH #4 find_deploy_root climbs to ancestor's `.pmcp/deploy.toml`
- HIGH #5 render_set_env_vars no escape for `,`/`=`/`\n`
- HIGH #6 init unconditionally overwrites Dockerfile/.dockerignore/cloudbuild.yaml
- HIGH #7 allow_unauthenticated defaults to TRUE (public-by-default Cloud Run)
- MEDIUM #8–13: layout.primary sanitize divergence + empty-sanitize, blocking
  Command in async fn, cloudbuild.yaml YAML injection, --target shadowing,
  --manifest-path file form, login-before-init regression
- LOW–MED #14–15: AWS fields polluting GCR serialize, guard_init_root `(None,None)`

Recommend tackling #4–#7 as the next quick task before any live GCR deploy.
