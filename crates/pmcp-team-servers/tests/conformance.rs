//! Wire-level conformance harness (TEAM-06, D-17/D-19).
//!
//! Drives the exportable [`run_fixtures`] runner against all four reference
//! servers over an in-memory [`DuplexTransport`], proving **advertised ==
//! enforced** at the wire for every advertised tool and every guard, using the
//! v2 fixtures under `contracts/team-servers/fixtures/**`.
//!
//! Each independent case runs against a FRESH deterministic server instance (so
//! the `mem-001…`/`appr-001…` id seams replay exactly); stateful sequences
//! (`write→read`, `add→get→search`, `ask→resolve→get`) are declared as ordered
//! `scenario` groups and run against a single shared instance with
//! capture/substitution.
//!
//! The `tools_list.json` fixtures (exact advertised name set + per-tool input
//! schema) are generated from the live servers by the `#[ignore]`d
//! [`regenerate_tools_list_fixtures`] test; the normal conformance tests then
//! REPLAY them, so any surface/schema drift fails the suite. The negative
//! harness tests that prove the runner itself catches drift live in
//! `src/conformance/runner.rs`.

#![cfg(feature = "conformance")]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use pmcp::types::protocol::RequestMeta;
use pmcp::types::sampling::{
    CreateMessageParams, CreateMessageResultWithTools, SamplingMessageContent,
};
use pmcp::types::{CallToolResult, Role, ToolInfo};

use pmcp_team_servers::conformance::runner::{
    assert_conformant, run_fixtures, CallError, ClientTarget, ConformanceTarget,
};
use pmcp_team_servers::DuplexTransport;

// team-fs
use pmcp_team_servers::fs::backend::TeamFsBackend;
use pmcp_team_servers::fs::local::LocalDirBackend;
use pmcp_team_servers::fs::server::{build_team_fs_server, FS_TOOL_NAMES};
// mem-mcp
use pmcp_team_servers::mem::backend::{InMemoryMemoryBackend, TeamMemoryBackend};
use pmcp_team_servers::mem::server::{build_mem_mcp_server, MEM_TOOL_NAMES};
// approval-mcp
use pmcp_package::package::HumanRole;
use pmcp_team_servers::approval::channels::{ApprovalChannel, ConsoleChannel};
use pmcp_team_servers::approval::repository::ApprovalRepository;
use pmcp_team_servers::approval::server::build_approval_mcp_server;
// team-mcp
use pmcp_agent::{
    CompletionError, CompletionSource, CompletionSourceFactory, FixedSourceFactory,
    ProgrammaticBuilder, ResolvedAgentConfig,
};
use pmcp_package::reference::ComponentType;
use pmcp_package::slot::SlotType;
use pmcp_package::{AgentPackage, ComponentRef, ConfigSlot};
use pmcp_team_servers::team::identity::{MemberId, MemberTaskForwarding};
use pmcp_team_servers::team::member::{resolve_member_factory, MemberHandle};
use pmcp_team_servers::team::server::build_team_mcp_server;

// ===========================================================================
// Fixture location (CARGO_MANIFEST_DIR-anchored → repo-root contracts dir)
// ===========================================================================

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/team-servers/fixtures")
}

// ===========================================================================
// team-fs target: a fresh LocalDirBackend over a temp dir kept alive by the
// target (so the workspace/review roots outlive the client hop).
// ===========================================================================

struct FsTarget {
    inner: ClientTarget<DuplexTransport>,
    _tmp: tempfile::TempDir,
}

#[async_trait]
impl ConformanceTarget for FsTarget {
    async fn initialize(&mut self) -> Result<(), String> {
        self.inner.initialize().await
    }
    async fn list_tools(&mut self) -> Result<Vec<ToolInfo>, String> {
        self.inner.list_tools().await
    }
    async fn call(
        &mut self,
        name: &str,
        args: Value,
        meta: RequestMeta,
        task: bool,
    ) -> Result<CallToolResult, CallError> {
        self.inner.call(name, args, meta, task).await
    }
}

fn fs_target() -> FsTarget {
    let tmp = tempfile::tempdir().expect("tempdir");
    let backend =
        Arc::new(LocalDirBackend::new(tmp.path()).expect("backend")) as Arc<dyn TeamFsBackend>;
    let server = build_team_fs_server(backend).expect("team-fs server");
    FsTarget {
        inner: ClientTarget::in_memory(server),
        _tmp: tmp,
    }
}

// ===========================================================================
// mem-mcp target: deterministic in-memory backend (mem-001…).
// ===========================================================================

fn mem_target() -> ClientTarget<DuplexTransport> {
    let backend = Arc::new(InMemoryMemoryBackend::deterministic()) as Arc<dyn TeamMemoryBackend>;
    ClientTarget::in_memory(build_mem_mcp_server(backend).expect("mem-mcp server"))
}

// ===========================================================================
// approval-mcp target: deterministic repository (appr-001…) + console channel.
// ===========================================================================

fn approval_roles() -> Vec<HumanRole> {
    vec![
        HumanRole {
            role: "release-manager".to_string(),
            description: "Approves releases".to_string(),
            responsibilities: vec![],
            channel_hints: vec![],
        },
        HumanRole {
            role: "security-reviewer".to_string(),
            description: "Approves security-sensitive changes".to_string(),
            responsibilities: vec![],
            channel_hints: vec![],
        },
    ]
}

fn approval_target() -> ClientTarget<DuplexTransport> {
    let repo = Arc::new(ApprovalRepository::deterministic());
    let channel = Arc::new(ConsoleChannel::new()) as Arc<dyn ApprovalChannel>;
    ClientTarget::in_memory(
        build_approval_mcp_server(&approval_roles(), channel, repo).expect("approval-mcp server"),
    )
}

// ===========================================================================
// team-mcp target: one `reviewer` member backed by a deterministic, offline
// FixedSource (no live LLM / no network), max_team_depth = 3.
// ===========================================================================

struct EndTurnMock;

#[async_trait]
impl CompletionSource for EndTurnMock {
    async fn create_message(
        &self,
        _params: CreateMessageParams,
    ) -> std::result::Result<CreateMessageResultWithTools, CompletionError> {
        Ok(CreateMessageResultWithTools::new(
            "test-model",
            Role::Assistant,
            vec![SamplingMessageContent::Text {
                text: "ok".to_string(),
                meta: None,
            }],
        )
        .with_stop_reason("end_turn"))
    }
}

fn member_ref(name: &str) -> ComponentRef {
    ComponentRef::Range {
        name: name.to_string(),
        range: semver::VersionReq::parse("^1").unwrap(),
        component_type: ComponentType::Agent,
    }
}

fn member_pkg(name: &str) -> AgentPackage {
    AgentPackage {
        name: name.to_string(),
        version: semver::Version::parse("1.0.0").unwrap(),
        instructions: "You are a helpful team member. Be brief.".to_string(),
        llm: ConfigSlot {
            slot: SlotType::LlmProvider {
                name: "primary-llm".to_string(),
                tested_value: "test-model".to_string(),
            },
        },
        max_tokens: 4096,
        max_iterations: 5,
        connectors: vec![],
        tool_selection: None,
        input_schema: None,
        output_schema: None,
        importance: None,
        finalizer_role: None,
        budget_defaults: vec![],
    }
}

async fn team_target() -> ClientTarget<DuplexTransport> {
    let r = member_ref("reviewer");
    let id = MemberId::from_ref(&r);
    let pkg = member_pkg("reviewer");

    // Injected FixedSource override (D-15): no slot resolution, fully offline.
    let injected: Arc<dyn CompletionSourceFactory> = Arc::new(FixedSourceFactory::new(Arc::new(
        EndTurnMock,
    )
        as Arc<dyn CompletionSource>));
    let resolver = ProgrammaticBuilder::new();
    let factory = resolve_member_factory(&pkg, &resolver, Some(injected))
        .await
        .expect("override factory");
    let config = ResolvedAgentConfig::new("Be a helpful team member.", "test-model", 10_000, 5);
    let handle = MemberHandle::spawn_from_package(
        id.clone(),
        pkg,
        config,
        factory,
        MemberTaskForwarding::Synthesize,
    )
    .await
    .expect("member spawns");

    ClientTarget::in_memory(
        build_team_mcp_server(vec![handle], 3, vec![id]).expect("team-mcp server"),
    )
}

// ===========================================================================
// Conformance runs: every server proven at zero failures.
// ===========================================================================

#[tokio::test]
async fn team_fs_is_conformant() {
    let dir = fixtures_root().join("team-fs");
    let report = run_fixtures(|| async { fs_target() }, &dir).await;
    assert_conformant(&report);
    assert!(
        report.passed >= 11,
        "expected ≥11 team-fs cases: {report:?}"
    );
}

#[tokio::test]
async fn mem_mcp_is_conformant() {
    let dir = fixtures_root().join("mem-mcp");
    let report = run_fixtures(|| async { mem_target() }, &dir).await;
    assert_conformant(&report);
    assert!(report.passed >= 6, "expected ≥6 mem-mcp cases: {report:?}");
}

#[tokio::test]
async fn approval_mcp_is_conformant() {
    let dir = fixtures_root().join("approval-mcp");
    let report = run_fixtures(|| async { approval_target() }, &dir).await;
    assert_conformant(&report);
    assert!(
        report.passed >= 5,
        "expected ≥5 approval-mcp cases: {report:?}"
    );
}

#[tokio::test]
async fn team_mcp_is_conformant() {
    let dir = fixtures_root().join("team-mcp");
    let report = run_fixtures(|| async { team_target().await }, &dir).await;
    assert_conformant(&report);
    assert!(report.passed >= 7, "expected ≥7 team-mcp cases: {report:?}");
}

// ===========================================================================
// Every-tool + every-guard coverage: a missing fixture fails the test.
// ===========================================================================

/// Load every fixture JSON under a server subdir.
fn load_dir(server: &str) -> Vec<Value> {
    let dir = fixtures_root().join(server);
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("read {server} dir: {e}")) {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read fixture");
        out.push(serde_json::from_str(&text).expect("parse fixture"));
    }
    out
}

/// The set of `request.name`s exercised by `tool_call` fixtures for a server.
fn exercised_tools(server: &str) -> Vec<String> {
    load_dir(server)
        .iter()
        .filter_map(|v| v["request"]["name"].as_str().map(str::to_string))
        .collect()
}

/// The advertised name set frozen in the server's `tools_list.json`.
fn advertised_surface(server: &str) -> Vec<String> {
    let fixture = load_dir(server)
        .into_iter()
        .find(|v| v["kind"].as_str() == Some("tools_list"))
        .unwrap_or_else(|| panic!("{server} missing a tools_list fixture (run the generator)"));
    fixture["expect"]["tools_list_schema"]
        .as_object()
        .expect("tools_list_schema object")
        .keys()
        .cloned()
        .collect()
}

#[test]
fn team_fs_covers_every_tool() {
    let exercised = exercised_tools("team-fs");
    for name in FS_TOOL_NAMES {
        assert!(
            exercised.iter().any(|n| n == name),
            "team-fs has no tool_call fixture for `{name}`"
        );
    }
    // Frozen surface equals the 11 contracted names.
    let surface = advertised_surface("team-fs");
    assert_eq!(
        surface.len(),
        11,
        "team-fs surface must be exactly 11 tools"
    );
    for name in FS_TOOL_NAMES {
        assert!(surface.iter().any(|n| n == name), "surface missing {name}");
    }
}

#[test]
fn mem_mcp_covers_every_tool() {
    let exercised = exercised_tools("mem-mcp");
    for name in MEM_TOOL_NAMES {
        assert!(
            exercised.iter().any(|n| n == name),
            "mem-mcp has no tool_call fixture for `{name}`"
        );
    }
    let surface = advertised_surface("mem-mcp");
    assert_eq!(surface.len(), 6, "mem-mcp surface must be exactly 6 tools");
}

#[test]
fn approval_mcp_covers_static_and_dynamic_tools() {
    let exercised = exercised_tools("approval-mcp");
    assert!(exercised.iter().any(|n| n == "resolve_approval"));
    assert!(exercised.iter().any(|n| n == "get_approval"));
    assert!(
        exercised
            .iter()
            .any(|n| n.starts_with("team_approval__ask_")),
        "approval-mcp must exercise at least one ask_<role> tool"
    );
    // Frozen surface: 2 unnamespaced statics + 2 ask-per-role tools.
    let surface = advertised_surface("approval-mcp");
    assert!(surface.iter().any(|n| n == "resolve_approval"));
    assert!(surface.iter().any(|n| n == "get_approval"));
    assert_eq!(
        surface
            .iter()
            .filter(|n| n.starts_with("team_approval__ask_"))
            .count(),
        2,
        "one ask tool per configured human role"
    );
}

#[test]
fn team_mcp_covers_every_guard() {
    let case_ids: Vec<String> = load_dir("team-mcp")
        .iter()
        .filter_map(|v| v["case_id"].as_str().map(str::to_string))
        .collect();
    for guard in [
        "positive-related-task-meta",
        "negative-unknown-member",
        "negative-self-call",
        "negative-malformed-depth",
        "negative-excessive-depth",
        "negative-ancestor-cycle",
        "negative-invalid-args",
    ] {
        assert!(
            case_ids.iter().any(|c| c.contains(guard)),
            "team-mcp missing a `{guard}` fixture"
        );
    }
    // The dynamic member family is frozen to exactly one member tool.
    let surface = advertised_surface("team-mcp");
    assert_eq!(
        surface.len(),
        1,
        "team-mcp surface must advertise 1 member tool"
    );
    assert!(surface[0].starts_with("team_mcp__"));
}

// ===========================================================================
// Fixture generator (run once, on demand): freezes the EXACT advertised surface
// + per-tool input schema of each live server into `<server>/tools_list.json`.
// Re-run intentionally when a server's surface legitimately changes:
//   cargo test -p pmcp-team-servers --test conformance --all-features \
//     regenerate_tools_list_fixtures -- --ignored --nocapture
// ===========================================================================

#[tokio::test]
#[ignore = "regenerates tools_list.json fixtures from live servers; run with --ignored"]
async fn regenerate_tools_list_fixtures() {
    write_tools_list("team-fs", fs_target()).await;
    write_tools_list("mem-mcp", mem_target()).await;
    write_tools_list("approval-mcp", approval_target()).await;
    write_tools_list("team-mcp", team_target().await).await;
}

async fn write_tools_list<T: ConformanceTarget>(server: &str, mut target: T) {
    target.initialize().await.expect("initialize");
    let tools = target.list_tools().await.expect("list_tools");
    // BTreeMap → deterministic, sorted key order in the emitted fixture.
    let schema: BTreeMap<String, Value> = tools
        .into_iter()
        .map(|t| (t.name, t.input_schema))
        .collect();
    let fixture = json!({
        "schema_version": "2",
        "kind": "tools_list",
        "case_id": format!("{server}.tools_list.exact-surface"),
        "server": server,
        "expect": { "tools_list_schema": schema }
    });
    let path = fixtures_root().join(server).join("tools_list.json");
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&fixture).expect("serialize"),
    )
    .expect("write");
    eprintln!("wrote {}", path.display());
}
