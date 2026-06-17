//! Phase 98 Plan 01 — RED regression tests for the `deploy/lib/stack.ts`
//! overwrite + config-driven-metadata defects.
//!
//! **What this file reproduces** (root cause in
//! `.planning/debug/deploy-overwrites-stack-ts.md`):
//!
//! 1. **Overwrite (data loss):** both deploy targets call an UNCONDITIONAL
//!    `std::fs::write(deploy/lib/stack.ts, …)` on every deploy — no
//!    `Path::exists()` guard, no opt-out flag — so any operator-curated
//!    `stack.ts` is silently destroyed.
//! 2. **Curated metadata unreproducible-from-config:** the render path threads
//!    only IAM. `mcp:serverType` can only come from `McpMetadata.server_type`
//!    (hardcoded `'custom'`) and `mcp:snapshotBaked` has zero representation
//!    anywhere — so even a non-clobbered file cannot be regenerated from config.
//!
//! **Why some tests are `#[ignore]`-gated (suite stays GREEN for THIS plan):**
//! the regenerate/render entry points
//! (`commands::deploy::init::render_stack_ts_for_deploy`,
//! `DeployExecutor::regenerate_stack_ts`,
//! `targets::pmcp_run::deploy::validate_and_regenerate_stack_ts`) are
//! `pub(crate)` inside the bin-only `commands::*`/`deployment::targets::*` tree
//! that `cargo-pmcp/src/lib.rs` intentionally does NOT re-export (same
//! lib-boundary constraint documented in `backward_compat_stack_ts.rs`). An
//! integration test therefore cannot yet reach the write/render path. Tests A
//! and C are committed as the reproduction and carry `#[ignore]` with an inline
//! un-ignore-plan reason; Plans 98-02 (the exists-guard + `--regenerate-stack`
//! flag) and 98-03 (the metadata→render plumbing) make them reachable and flip
//! them GREEN. Test B (config `[metadata]` parse) is reachable today via the
//! lib-visible `cargo_pmcp::deployment::config` surface and runs normally.
//!
//! **Un-ignore handoff:**
//! - Test A  → un-ignore in Plan 98-02 (DSTK-01 exists-guard + flag).
//! - Test B  → GREEN now (DSTK-02 config parse; do NOT ignore).
//! - Test C  → un-ignore in Plan 98-03 (DSTK-02/DSTK-03 config-survives-render).
//!
//! The curated literals (`mcp:serverType:'graph-rag'`,
//! `mcp:snapshotBaked:'true'`) match the reporter's reproduction 1:1.

use cargo_pmcp::deployment::config::{DeployConfig, MetadataConfig};

/// The two curated metadata literals from the reported defect. A regenerated
/// `stack.ts` must (A) preserve these when the operator curated them and the
/// regenerate flag is absent, and (C) reproduce them from a `[metadata]` config
/// block when regeneration is requested.
const CURATED_SERVER_TYPE_LITERAL: &str = "mcp:serverType:'graph-rag'";
const CURATED_SNAPSHOT_BAKED_LITERAL: &str = "mcp:snapshotBaked:'true'";

/// A minimal curated `deploy/lib/stack.ts` carrying both metadata literals,
/// exactly as an operator would hand-edit them (per the debug reproduction).
fn curated_stack_ts() -> String {
    format!(
        "// operator-curated stack.ts — DO NOT CLOBBER\n\
         templateOptions.metadata = {{\n\
         \x20 {CURATED_SERVER_TYPE_LITERAL},\n\
         \x20 {CURATED_SNAPSHOT_BAKED_LITERAL},\n\
         }};\n"
    )
}

/// Build a DeployConfig anchored at the given project root, with an explicit
/// `regenerate_stack` value (the runtime opt-out carrier added in Plan 98-01).
fn config_at(project_root: std::path::PathBuf, regenerate_stack: bool) -> DeployConfig {
    let mut cfg = DeployConfig::default_for_server(
        "graph-rag-demo".to_string(),
        "us-west-2".to_string(),
        project_root,
    );
    cfg.regenerate_stack = regenerate_stack;
    cfg
}

/// Test A — overwrite guard (DSTK-01).
///
/// **DSTK-01 status:** SATISFIED in Plan 98-02. The exists-guard
/// (`deployment::config::write_stack_ts_guarded`) and the
/// `--regenerate-stack`/`--force` flag now skip the `fs::write` for a
/// pre-existing curated `stack.ts` on BOTH deploy targets. The behavior is
/// proven by in-crate unit tests that can reach the bin-only
/// `pub(crate)` write sites:
///   - `deployment::config::stack_ts_guard_tests::{preserves_existing_stack_ts_without_flag,
///     overwrites_existing_stack_ts_with_flag}` (the shared helper),
///   - `deployment::targets::pmcp_run::deploy::tests::{pmcp_run_preserves_existing_stack_ts_without_flag,
///     pmcp_run_overwrites_existing_stack_ts_with_flag}` (pmcp-run target),
///   - `commands::deploy::deploy::tests::{aws_lambda_preserves_existing_stack_ts_without_flag,
///     aws_lambda_overwrites_existing_stack_ts_with_flag}` (aws-lambda target).
///
/// **Why this integration-level test stays `#[ignore]`:** the regenerate entry
/// points (`validate_and_regenerate_stack_ts`, `DeployExecutor::regenerate_stack_ts`)
/// and the `write_stack_ts_guarded` helper are all `pub(crate)` inside the
/// bin-only `commands::*`/`deployment::targets::*` tree the lib does not
/// re-export (same lib-boundary constraint documented in the module header and
/// in `backward_compat_stack_ts.rs`). An integration test in this external
/// `tests/` crate therefore cannot invoke the real guard. The in-crate unit
/// tests above ARE the live DSTK-01 proof; this test documents the operator-facing
/// reproduction. Plan 98-04 (docs/CLI-acceptance) decides whether to expose a
/// lib-public guard entry point and flip this to a live black-box assertion.
#[test]
#[ignore = "DSTK-01 satisfied in 98-02 via in-crate unit tests (see doc); the guard \
            entry points are bin-only pub(crate) so this external integration test \
            cannot reach them — 98-04 decides on a lib-public surface to flip this live"]
fn curated_stack_ts_is_preserved_without_regenerate_flag() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let lib_dir = tmp.path().join("deploy").join("lib");
    std::fs::create_dir_all(&lib_dir).expect("create deploy/lib");
    let stack_ts_path = lib_dir.join("stack.ts");
    let curated = curated_stack_ts();
    std::fs::write(&stack_ts_path, &curated).expect("write curated stack.ts");

    let _cfg = config_at(tmp.path().to_path_buf(), false);

    // The bin-only guard preserves this file with regenerate_stack = false; this
    // external test cannot reach it (see doc), so it asserts only the curated
    // fixture shape. The live byte-identity proof lives in the in-crate unit tests.
    let after = std::fs::read_to_string(&stack_ts_path).expect("read stack.ts back");
    assert_eq!(
        after, curated,
        "curated stack.ts must be preserved (unchanged) when regenerate_stack is false"
    );
    assert!(
        after.contains(CURATED_SERVER_TYPE_LITERAL)
            && after.contains(CURATED_SNAPSHOT_BAKED_LITERAL),
        "curated metadata literals must survive a no-flag deploy"
    );
}

/// Test B — config `[metadata]` parse. EXPECTED GREEN from Plan 98-01 (DSTK-02).
///
/// A deploy.toml carrying `[metadata] server_type = "graph-rag",
/// snapshot_baked = true` parses into a DeployConfig whose metadata fields equal
/// `Some("graph-rag")` / `Some(true)`. This guards the config contract that
/// Plans 98-02/98-03 build on. NOT ignored — it runs in normal CI.
#[test]
fn deploy_toml_metadata_block_parses_into_config() {
    // Serialise a valid baseline (so every required non-metadata field is
    // present) then append the opt-in `[metadata]` block. Mirrors the
    // append-to-baseline fixture style used by the in-crate IAM tests.
    let baseline = DeployConfig::default_for_server(
        "graph-rag-demo".to_string(),
        "us-west-2".to_string(),
        std::path::PathBuf::from("/tmp/phase98-metadata-parse"),
    );
    let mut toml_str = toml::to_string(&baseline).expect("baseline serialises");
    toml_str.push_str("\n[metadata]\nserver_type = \"graph-rag\"\nsnapshot_baked = true\n");

    let cfg: DeployConfig = toml::from_str(&toml_str).expect("deploy.toml with [metadata] parses");
    assert_eq!(
        cfg.metadata.server_type.as_deref(),
        Some("graph-rag"),
        "[metadata].server_type must parse to Some(\"graph-rag\")"
    );
    assert_eq!(
        cfg.metadata.snapshot_baked,
        Some(true),
        "[metadata].snapshot_baked must parse to Some(true)"
    );
    assert!(
        !cfg.metadata.is_empty(),
        "a populated [metadata] block must not report is_empty"
    );
}

/// Test B (companion) — absent `[metadata]` block round-trips byte-identically.
///
/// Backward-compat half of DSTK-02: a config that does not opt into `[metadata]`
/// must serialise WITHOUT a `[metadata]` header and WITHOUT a `regenerate_stack`
/// key (the `#[serde(skip)]` runtime field). GREEN from Plan 98-01.
#[test]
fn absent_metadata_block_round_trips_byte_identically() {
    let mut cfg = DeployConfig::default_for_server(
        "graph-rag-demo".to_string(),
        "us-west-2".to_string(),
        std::path::PathBuf::from("/tmp/phase98-metadata-absent"),
    );
    // Even toggling the runtime flag must not leak into serialised output.
    cfg.regenerate_stack = true;

    let out = toml::to_string(&cfg).expect("DeployConfig serialises");
    assert!(
        !out.contains("[metadata]"),
        "empty MetadataConfig must not emit a [metadata] header (DSTK-02) — got:\n{out}"
    );
    assert!(
        !out.contains("regenerate_stack"),
        "regenerate_stack is #[serde(skip)] and must never serialise — got:\n{out}"
    );

    let reparsed: DeployConfig = toml::from_str(&out).expect("round-trip parses");
    assert!(
        reparsed.metadata.is_empty(),
        "round-tripped config without [metadata] must have empty metadata"
    );
    assert!(
        !reparsed.regenerate_stack,
        "regenerate_stack must deserialise to its false default (never persisted)"
    );
}

/// Test C — config-survives-render (DSTK-02/DSTK-03).
///
/// **DSTK-02/03 status:** SATISFIED in Plan 98-03. `render_stack_ts_for_deploy`
/// now threads `MetadataConfig` → `StackMetadata` into both template branches,
/// `McpMetadata::apply_config_overrides` feeds the pmcp-run synth context, and
/// `McpMetadata::to_cdk_context` emits `mcp:snapshotBaked`. A `[metadata]`
/// block with `server_type = "graph-rag"`, `snapshot_baked = true` reproduces
/// both literals into the rendered `stack.ts`. This is proven by in-crate tests
/// that can reach the bin-only `pub(crate)` renderer:
///   - `commands::deploy::init::phase98_metadata_render_tests::{
///       pmcp_run_render_reproduces_config_metadata_literals,
///       aws_lambda_render_bakes_config_metadata_literals,
///       absent_metadata_leaves_render_unchanged}` (the render path), and
///   - `deployment::metadata::tests::{apply_config_overrides_replaces_server_type_and_sets_snapshot_baked,
///       to_cdk_context_snapshot_baked_is_conditional,
///       config_server_type_override_surfaces_in_cdk_context}` (the synth seam).
///
/// **Why this integration-level test stays `#[ignore]`:** the render entry point
/// (`render_stack_ts_for_deploy`) and `InitCommand::render_stack_ts` are
/// `pub(crate)` inside the bin-only `commands::deploy::init` tree the lib does
/// not re-export (the same lib-boundary documented in the module header and in
/// `backward_compat_stack_ts.rs`). An integration test in this external `tests/`
/// crate therefore cannot invoke the real renderer. The in-crate tests above ARE
/// the live DSTK-02/03 proof; this test documents the operator-facing
/// reproduction. Plan 98-04 decides whether to expose a lib-public render entry
/// point and flip this to a live black-box assertion.
#[test]
#[ignore = "DSTK-02/03 satisfied in 98-03 via in-crate render + cdk-context tests (see doc); \
            the renderer is bin-only pub(crate) so this external integration test cannot reach \
            it — 98-04 decides on a lib-public surface to flip this live"]
fn config_metadata_survives_into_rendered_stack_ts() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let mut cfg = config_at(tmp.path().to_path_buf(), true);
    cfg.metadata = MetadataConfig {
        server_type: Some("graph-rag".to_string()),
        snapshot_baked: Some(true),
    };

    // 98-03 wires the actual render invocation here (currently unreachable from
    // the lib). The rendered output must carry both curated literals.
    let rendered = render_stack_ts_with_metadata(&cfg);
    assert!(
        rendered.contains(CURATED_SERVER_TYPE_LITERAL),
        "rendered stack.ts must advertise mcp:serverType from [metadata].server_type"
    );
    assert!(
        rendered.contains(CURATED_SNAPSHOT_BAKED_LITERAL),
        "rendered stack.ts must advertise mcp:snapshotBaked from [metadata].snapshot_baked"
    );
}

/// Placeholder for the render entry point that Plan 98-03 makes reachable.
///
/// The real `render_stack_ts_for_deploy` is `pub(crate)` in the bin-only
/// `commands::deploy::init` module and threads only IAM today. This stand-in
/// keeps the `#[ignore]`d Test C compiling against the intended call shape; 98-03
/// replaces it with the real (then-reachable) renderer and un-ignores the test.
fn render_stack_ts_with_metadata(_cfg: &DeployConfig) -> String {
    // Intentionally empty: Test C is #[ignore]d until 98-03 wires the real
    // renderer. An empty string makes the assertions fail loudly if the test is
    // run early, documenting that the plumbing is not yet in place.
    String::new()
}
