//! Attachment wiring: turn an [`AttachmentSet`] into an attached, running
//! reference-server stack in ONE process over in-memory transports (D-01/D-04).
//!
//! [`TeamRuntimeBuilder`] collects every seam the runtime needs and
//! [`TeamRuntimeBuilder::build`] (a.k.a. `TeamRuntime::start`) composes the
//! derive-selected servers:
//!
//! 1. [`derive_attachment`](crate::compose::derive::derive_attachment) decides
//!    which servers attach (team-mcp iff ≥2 members, approval-mcp iff ≥1 human
//!    role, plus the opt-in team-fs/mem-mcp extras).
//! 2. Each member `ComponentRef` is resolved to an
//!    [`AgentPackage`](pmcp_package::AgentPackage) via the injected
//!    [`PackageResolver`](crate::compose::resolver::PackageResolver), and each
//!    member `AgentServer` is built with the runtime's config/invoker/store
//!    seams. Member LLM factories come from the SHARED
//!    [`resolve_member_factory`](crate::team::member::resolve_member_factory) —
//!    the same helper the team-mcp dev binary uses.
//! 3. Every server is hosted over an in-memory
//!    [`DuplexTransport`](crate::transport::DuplexTransport) pair with an
//!    initialized [`pmcp::Client`] on the near side — NO sockets.
//!
//! # Feature gating & fail-closed policy (D-06)
//!
//! Each attachment branch is `#[cfg]`-gated on its server feature so selective
//! feature builds still compile the runtime skeleton, and every builder field
//! whose TYPE lives inside a feature-gated module is per-field `#[cfg]`-gated
//! (e.g. the `approval-mcp` channel, the `team-mcp` forwarding contract). A
//! requested-but-uncompiled server, or an unknown opt-in name, FAILS CLOSED with
//! [`RuntimeError::UnsupportedServer`] — it is never silently ignored.
//!
//! # Lifecycle
//!
//! Startup is transactional: hosting tasks are tracked as they spawn, and any
//! later failure aborts every already-spawned task before returning the error
//! (no leak). Teardown is explicit — prefer [`TeamRuntime::shutdown`]; the
//! [`Drop`] impl is a safety net that aborts any still-tracked hosting task.

use std::sync::Arc;

use tokio::task::JoinHandle;

use pmcp::Client;
use pmcp_package::TeamPackage;

use crate::compose::derive::{derive_attachment, AttachmentSet};
use crate::transport::DuplexTransport;

// ---- team-mcp (member-wiring) subsystem imports ---------------------------
// Member wiring — package resolution, LLM factory resolution, AgentServer
// construction, and the member hop — is the team-mcp subsystem, so its inputs
// and types are gated behind the `team-mcp` feature.
#[cfg(feature = "team-mcp")]
use async_trait::async_trait;
#[cfg(feature = "team-mcp")]
use pmcp_agent::{
    resolve_agent, AgentServer, CompletionSourceFactory, ConversationStore, InMemoryStore,
    SlotResolver, ToolCall, ToolCallResult, ToolInvoker,
};
#[cfg(feature = "team-mcp")]
use serde_json::json;

#[cfg(feature = "team-mcp")]
use crate::compose::resolver::PackageResolver;
#[cfg(feature = "team-mcp")]
use crate::team::identity::{MemberId, MemberTaskForwarding};
#[cfg(feature = "team-mcp")]
use crate::team::member::{resolve_member_factory, MemberHandle};
#[cfg(feature = "team-mcp")]
use crate::team::server::build_team_mcp_server;

// ---- approval-mcp imports -------------------------------------------------
#[cfg(feature = "approval-mcp")]
use crate::approval::channels::{ApprovalChannel, ConsoleChannel};
#[cfg(feature = "approval-mcp")]
use crate::approval::repository::ApprovalRepository;
#[cfg(feature = "approval-mcp")]
use crate::approval::server::build_approval_mcp_server;

// ---- team-fs imports ------------------------------------------------------
#[cfg(feature = "team-fs")]
use crate::fs::backend::TeamFsBackend;
#[cfg(feature = "team-fs")]
use crate::fs::local::LocalDirBackend;
#[cfg(feature = "team-fs")]
use crate::fs::server::build_team_fs_server;
#[cfg(feature = "team-fs")]
use std::path::PathBuf;

// ---- mem-mcp imports ------------------------------------------------------
#[cfg(feature = "mem-mcp")]
use crate::mem::backend::{InMemoryMemoryBackend, TeamMemoryBackend};
#[cfg(feature = "mem-mcp")]
use crate::mem::server::build_mem_mcp_server;

/// A factory that mints a fresh per-member conversation store.
#[cfg(feature = "team-mcp")]
type StoreFactory = Arc<dyn Fn() -> Arc<dyn ConversationStore> + Send + Sync>;

/// The default per-member conversation store (an in-memory store, D-12).
#[cfg(feature = "team-mcp")]
fn default_store_factory() -> StoreFactory {
    Arc::new(|| Arc::new(InMemoryStore::new()) as Arc<dyn ConversationStore>)
}

/// A no-op [`ToolInvoker`] — the reference members drive an end-turn source and
/// never dispatch downstream tools (the same default the dev binary uses).
#[cfg(feature = "team-mcp")]
struct DefaultInvoker;

#[cfg(feature = "team-mcp")]
#[async_trait]
impl ToolInvoker for DefaultInvoker {
    async fn invoke(&self, call: ToolCall) -> ToolCallResult {
        ToolCallResult::ok(call.id, json!({}))
    }
}

/// Why the in-process runtime failed to start.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    /// A member `ComponentRef` could not be resolved to an `AgentPackage`, or
    /// its config slots could not be resolved.
    #[error("resolving a member package/config failed: {0}")]
    Resolve(String),
    /// A server was requested (by derivation or opt-in) but is not compiled or
    /// is not permitted by the enabled-server policy — the runtime FAILS CLOSED
    /// rather than silently omitting it.
    #[error("server '{name}' was requested but is not compiled/enabled (fail closed)")]
    UnsupportedServer {
        /// The requested server name that could not be satisfied.
        name: String,
    },
    /// A server or member `AgentServer` failed to build.
    #[error("building a server failed: {0}")]
    Build(String),
    /// A server failed to spawn or its client failed to `initialize`.
    #[error("spawning/initializing a server failed: {0}")]
    Spawn(String),
}

/// The opt-in servers the runtime is permitted to attach.
///
/// Combined with per-branch `#[cfg]` gating, this is the runtime's fail-closed
/// policy: an opt-in whose name is absent from this set is rejected with
/// [`RuntimeError::UnsupportedServer`] even if its feature is compiled.
#[derive(Debug, Clone)]
pub struct EnabledServers {
    allowed: std::collections::BTreeSet<String>,
}

impl EnabledServers {
    /// Permit every known opt-in server (`team-fs`, `mem-mcp`).
    #[must_use]
    pub fn all() -> Self {
        let mut allowed = std::collections::BTreeSet::new();
        allowed.insert("team-fs".to_string());
        allowed.insert("mem-mcp".to_string());
        Self { allowed }
    }

    /// Permit no opt-in servers (every opt-in fails closed).
    #[must_use]
    pub fn none() -> Self {
        Self {
            allowed: std::collections::BTreeSet::new(),
        }
    }

    /// Permit the named opt-in server.
    #[must_use]
    pub fn with(mut self, name: impl Into<String>) -> Self {
        self.allowed.insert(name.into());
        self
    }

    /// Withdraw permission for the named opt-in server.
    #[must_use]
    pub fn without(mut self, name: &str) -> Self {
        self.allowed.remove(name);
        self
    }

    /// Whether the policy permits the named opt-in server.
    #[must_use]
    pub fn permits(&self, name: &str) -> bool {
        self.allowed.contains(name)
    }
}

impl Default for EnabledServers {
    fn default() -> Self {
        Self::all()
    }
}

/// Collects every seam the in-process runtime needs, then composes them.
///
/// Server-specific fields whose TYPE lives behind a feature (the `approval-mcp`
/// channel, the `team-mcp` member-wiring seams) are per-field `#[cfg]`-gated so
/// a reduced-feature build (e.g. `--no-default-features --features team-fs`)
/// still compiles the struct.
pub struct TeamRuntimeBuilder {
    /// Resolves a member `ComponentRef` to its `AgentPackage`.
    #[cfg(feature = "team-mcp")]
    resolver: Arc<dyn PackageResolver>,
    /// Resolves each member `AgentPackage`'s config slots (incl. the LLM model).
    #[cfg(feature = "team-mcp")]
    slot_resolver: Arc<dyn SlotResolver>,
    /// An explicit completion-source override (tests/CI inject `FixedSource`;
    /// production passes `None` and the mandatory llm slot is resolved).
    #[cfg(feature = "team-mcp")]
    completion_override: Option<Arc<dyn CompletionSourceFactory>>,
    /// The shared member tool invoker seam (default: a no-op invoker).
    #[cfg(feature = "team-mcp")]
    invoker: Arc<dyn ToolInvoker>,
    /// Mints a fresh conversation store per member (default: `InMemoryStore`).
    #[cfg(feature = "team-mcp")]
    store_factory: StoreFactory,
    /// How the member hop forwards a task-augmented call.
    #[cfg(feature = "team-mcp")]
    forwarding: MemberTaskForwarding,
    /// The data root for the team-fs `LocalDirBackend`.
    #[cfg(feature = "team-fs")]
    data_root: PathBuf,
    /// The approval-notification channel (default: `ConsoleChannel`).
    #[cfg(feature = "approval-mcp")]
    approval_channel: Arc<dyn ApprovalChannel>,
    /// Which opt-in servers the runtime is permitted to attach.
    enabled_servers: EnabledServers,
}

impl TeamRuntimeBuilder {
    /// Create a builder with the two required member-wiring seams and documented
    /// defaults for the rest (no completion override, no-op invoker, in-memory
    /// stores, `Synthesize` forwarding, `.` data root, `ConsoleChannel`, all
    /// opt-ins permitted).
    #[cfg(feature = "team-mcp")]
    #[must_use]
    pub fn new(resolver: Arc<dyn PackageResolver>, slot_resolver: Arc<dyn SlotResolver>) -> Self {
        Self {
            resolver,
            slot_resolver,
            completion_override: None,
            invoker: Arc::new(DefaultInvoker),
            store_factory: default_store_factory(),
            forwarding: MemberTaskForwarding::default(),
            #[cfg(feature = "team-fs")]
            data_root: PathBuf::from("."),
            #[cfg(feature = "approval-mcp")]
            approval_channel: Arc::new(ConsoleChannel::new()),
            enabled_servers: EnabledServers::all(),
        }
    }

    /// Create a builder for a reduced-feature build without the `team-mcp`
    /// member-wiring subsystem. Such a runtime can only attach opt-in servers;
    /// any team with members fails closed at [`build`](Self::build).
    #[cfg(not(feature = "team-mcp"))]
    #[must_use]
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "team-fs")]
            data_root: PathBuf::from("."),
            #[cfg(feature = "approval-mcp")]
            approval_channel: Arc::new(ConsoleChannel::new()),
            enabled_servers: EnabledServers::all(),
        }
    }

    // (A `Default` impl for this reduced-feature constructor lives below.)

    /// Inject an explicit completion-source override for every member (the
    /// dependency-injection seam tests use to bind a `FixedSource`).
    #[cfg(feature = "team-mcp")]
    #[must_use]
    pub fn with_completion_override(mut self, factory: Arc<dyn CompletionSourceFactory>) -> Self {
        self.completion_override = Some(factory);
        self
    }

    /// Override the shared member tool invoker seam.
    #[cfg(feature = "team-mcp")]
    #[must_use]
    pub fn with_invoker(mut self, invoker: Arc<dyn ToolInvoker>) -> Self {
        self.invoker = invoker;
        self
    }

    /// Override the per-member conversation-store factory seam.
    #[cfg(feature = "team-mcp")]
    #[must_use]
    pub fn with_store_factory(mut self, factory: StoreFactory) -> Self {
        self.store_factory = factory;
        self
    }

    /// Override the member task-forwarding contract.
    #[cfg(feature = "team-mcp")]
    #[must_use]
    pub fn with_forwarding(mut self, forwarding: MemberTaskForwarding) -> Self {
        self.forwarding = forwarding;
        self
    }

    /// Set the data root for the team-fs `LocalDirBackend`.
    #[cfg(feature = "team-fs")]
    #[must_use]
    pub fn with_data_root(mut self, data_root: impl Into<PathBuf>) -> Self {
        self.data_root = data_root.into();
        self
    }

    /// Override the approval-notification channel.
    #[cfg(feature = "approval-mcp")]
    #[must_use]
    pub fn with_approval_channel(mut self, channel: Arc<dyn ApprovalChannel>) -> Self {
        self.approval_channel = channel;
        self
    }

    /// Set the enabled-server (opt-in) policy.
    #[must_use]
    pub fn with_enabled_servers(mut self, enabled: EnabledServers) -> Self {
        self.enabled_servers = enabled;
        self
    }

    /// Build a member `AgentServer` handle for every roster member, using the
    /// SHARED resolver + `resolve_member_factory` + `resolve_agent` helpers.
    ///
    /// Returns the live handles and the roster (member ids). Any error drops the
    /// handles built so far; their in-memory transports close, so the member
    /// tasks self-terminate (no leak).
    #[cfg(feature = "team-mcp")]
    async fn build_member_handles(
        &self,
        pkg: &TeamPackage,
    ) -> Result<(Vec<MemberHandle>, Vec<MemberId>), RuntimeError> {
        let mut handles: Vec<MemberHandle> = Vec::with_capacity(pkg.members.len());
        let mut roster: Vec<MemberId> = Vec::with_capacity(pkg.members.len());

        for member in &pkg.members {
            let agent_pkg = self
                .resolver
                .resolve_agent(&member.agent)
                .await
                .map_err(|e| RuntimeError::Resolve(e.to_string()))?;
            let config = resolve_agent(&agent_pkg, &*self.slot_resolver)
                .await
                .map_err(|e| RuntimeError::Resolve(e.to_string()))?;
            let factory = resolve_member_factory(
                &agent_pkg,
                &*self.slot_resolver,
                self.completion_override.clone(),
            )
            .await
            .map_err(|e| RuntimeError::Build(e.to_string()))?;
            let store = (self.store_factory)();
            let agent =
                AgentServer::builder(agent_pkg, config, factory, self.invoker.clone(), store)
                    .build()
                    .map_err(|e| RuntimeError::Build(e.to_string()))?;

            let id = MemberId::from_ref(&member.agent);
            let handle = MemberHandle::spawn(id.clone(), agent, self.forwarding)
                .await
                .map_err(|e| RuntimeError::Spawn(e.to_string()))?;

            roster.push(id);
            handles.push(handle);
        }

        Ok((handles, roster))
    }

    /// Compose the runtime (a.k.a. `TeamRuntime::start`).
    ///
    /// Resolves the attachment set, wires the attached servers + members over
    /// in-memory transports, and returns a live [`TeamRuntime`]. Startup is
    /// transactional: any failure aborts every hosting task spawned so far.
    ///
    /// # Errors
    /// - [`RuntimeError::Resolve`] — a member package/config could not resolve.
    /// - [`RuntimeError::UnsupportedServer`] — a requested server is not
    ///   compiled or not permitted (fail closed).
    /// - [`RuntimeError::Build`] / [`RuntimeError::Spawn`] — a server failed to
    ///   build, spawn, or initialize.
    pub async fn build(self, pkg: &TeamPackage) -> Result<TeamRuntime, RuntimeError> {
        match self.wire(pkg).await {
            Ok(runtime) => Ok(runtime),
            Err((err, tasks)) => {
                // Transactional abort: no spawned hosting task outlives a failed
                // startup.
                for task in &tasks {
                    task.abort();
                }
                Err(err)
            },
        }
    }

    /// The wiring body. Owns the accumulating hosting-task list so a failure can
    /// hand it back to [`build`](Self::build) for a transactional abort.
    #[allow(unused_mut, unused_variables)]
    async fn wire(
        self,
        pkg: &TeamPackage,
    ) -> Result<TeamRuntime, (RuntimeError, Vec<JoinHandle<()>>)> {
        let attachment = derive_attachment(pkg);
        let mut tasks: Vec<JoinHandle<()>> = Vec::new();

        let mut team_mcp_client: Option<Arc<Client<DuplexTransport>>> = None;
        let mut approval_client: Option<Arc<Client<DuplexTransport>>> = None;
        let mut team_fs_client: Option<Arc<Client<DuplexTransport>>> = None;
        let mut mem_client: Option<Arc<Client<DuplexTransport>>> = None;
        #[cfg(feature = "team-mcp")]
        let mut solo_member: Option<MemberHandle> = None;

        // ---- Members + team-mcp (D-05) -----------------------------------
        #[cfg(feature = "team-mcp")]
        {
            let (mut handles, roster) = match self.build_member_handles(pkg).await {
                Ok(v) => v,
                Err(e) => return Err((e, tasks)),
            };
            if attachment.team_mcp {
                let server = match build_team_mcp_server(handles, pkg.limits.max_team_depth, roster)
                {
                    Ok(s) => s,
                    Err(e) => return Err((RuntimeError::Build(e.to_string()), tasks)),
                };
                team_mcp_client = Some(match host(server, &mut tasks).await {
                    Ok(c) => c,
                    Err(e) => return Err((e, tasks)),
                });
            } else {
                // Team-of-one (or zero): keep the sole member; no dispatch server.
                solo_member = handles.pop();
            }
        }
        #[cfg(not(feature = "team-mcp"))]
        {
            // Member wiring is the team-mcp subsystem; without it no member
            // AgentServer can be brought up. Fail closed rather than silently
            // dropping the roster.
            if !pkg.members.is_empty() {
                return Err((
                    RuntimeError::UnsupportedServer {
                        name: "team-mcp".to_string(),
                    },
                    tasks,
                ));
            }
        }

        // ---- approval-mcp (iff ≥1 human role, D-05) ----------------------
        if attachment.approval_mcp {
            #[cfg(feature = "approval-mcp")]
            {
                let repo = Arc::new(ApprovalRepository::new());
                let server = match build_approval_mcp_server(
                    &pkg.human_roles,
                    self.approval_channel.clone(),
                    repo,
                ) {
                    Ok(s) => s,
                    Err(e) => return Err((RuntimeError::Build(e.to_string()), tasks)),
                };
                approval_client = Some(match host(server, &mut tasks).await {
                    Ok(c) => c,
                    Err(e) => return Err((e, tasks)),
                });
            }
            #[cfg(not(feature = "approval-mcp"))]
            {
                return Err((
                    RuntimeError::UnsupportedServer {
                        name: "approval-mcp".to_string(),
                    },
                    tasks,
                ));
            }
        }

        // ---- Opt-in team-fs / mem-mcp (D-06, fail closed) ----------------
        // Why(clippy::never_loop): under a reduced-feature build where neither
        // opt-in server is compiled, every match arm fails closed with an early
        // return, so the loop provably runs at most once — that is the intended
        // fail-closed behavior, not a bug. Under `--all-features` the arms host
        // servers and the loop iterates normally.
        #[allow(clippy::never_loop)]
        for opt in &attachment.opt_ins {
            match opt.name() {
                "team-fs" => {
                    if !self.enabled_servers.permits("team-fs") {
                        return Err((
                            RuntimeError::UnsupportedServer {
                                name: "team-fs".to_string(),
                            },
                            tasks,
                        ));
                    }
                    #[cfg(feature = "team-fs")]
                    {
                        let backend = match LocalDirBackend::new(&self.data_root) {
                            Ok(b) => Arc::new(b) as Arc<dyn TeamFsBackend>,
                            Err(e) => return Err((RuntimeError::Build(e.to_string()), tasks)),
                        };
                        let server = match build_team_fs_server(backend) {
                            Ok(s) => s,
                            Err(e) => return Err((RuntimeError::Build(e.to_string()), tasks)),
                        };
                        team_fs_client = Some(match host(server, &mut tasks).await {
                            Ok(c) => c,
                            Err(e) => return Err((e, tasks)),
                        });
                    }
                    #[cfg(not(feature = "team-fs"))]
                    {
                        return Err((
                            RuntimeError::UnsupportedServer {
                                name: "team-fs".to_string(),
                            },
                            tasks,
                        ));
                    }
                },
                "mem-mcp" => {
                    if !self.enabled_servers.permits("mem-mcp") {
                        return Err((
                            RuntimeError::UnsupportedServer {
                                name: "mem-mcp".to_string(),
                            },
                            tasks,
                        ));
                    }
                    #[cfg(feature = "mem-mcp")]
                    {
                        let backend =
                            Arc::new(InMemoryMemoryBackend::new()) as Arc<dyn TeamMemoryBackend>;
                        let server = match build_mem_mcp_server(backend) {
                            Ok(s) => s,
                            Err(e) => return Err((RuntimeError::Build(e.to_string()), tasks)),
                        };
                        mem_client = Some(match host(server, &mut tasks).await {
                            Ok(c) => c,
                            Err(e) => return Err((e, tasks)),
                        });
                    }
                    #[cfg(not(feature = "mem-mcp"))]
                    {
                        return Err((
                            RuntimeError::UnsupportedServer {
                                name: "mem-mcp".to_string(),
                            },
                            tasks,
                        ));
                    }
                },
                other => {
                    // Unknown opt-in name: fail closed.
                    return Err((
                        RuntimeError::UnsupportedServer {
                            name: other.to_string(),
                        },
                        tasks,
                    ));
                },
            }
        }

        Ok(TeamRuntime {
            attachment,
            team_mcp: team_mcp_client,
            approval: approval_client,
            team_fs: team_fs_client,
            mem: mem_client,
            #[cfg(feature = "team-mcp")]
            solo_member,
            tasks,
            shut: false,
        })
    }
}

/// Default for the reduced-feature (`not team-mcp`) builder — equivalent to
/// [`TeamRuntimeBuilder::new`].
#[cfg(not(feature = "team-mcp"))]
impl Default for TeamRuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Host a [`pmcp::Server`] over an in-memory [`DuplexTransport`] pair, spawn its
/// serving task (tracked for transactional abort), and return an initialized
/// near-side [`pmcp::Client`].
#[cfg(any(
    feature = "team-mcp",
    feature = "approval-mcp",
    feature = "team-fs",
    feature = "mem-mcp"
))]
async fn host(
    server: pmcp::Server,
    tasks: &mut Vec<JoinHandle<()>>,
) -> Result<Arc<Client<DuplexTransport>>, RuntimeError> {
    use pmcp::types::ClientCapabilities;

    let (client_t, server_t) = DuplexTransport::pair();
    // Track the hosting task BEFORE `initialize` so a failed handshake still
    // aborts it transactionally.
    let task = tokio::spawn(async move {
        let _ = server.run(server_t).await;
    });
    tasks.push(task);

    let mut client = Client::new(client_t);
    client
        .initialize(ClientCapabilities::default())
        .await
        .map_err(|e| RuntimeError::Spawn(e.to_string()))?;
    Ok(Arc::new(client))
}

/// A running small-team stack: the derive-selected servers + members wired
/// together in one process over in-memory transports (D-01/D-04).
///
/// Access each attached server's client via the accessors; drive teardown with
/// [`TeamRuntime::shutdown`] (the [`Drop`] impl is a safety net).
pub struct TeamRuntime {
    attachment: AttachmentSet,
    team_mcp: Option<Arc<Client<DuplexTransport>>>,
    approval: Option<Arc<Client<DuplexTransport>>>,
    team_fs: Option<Arc<Client<DuplexTransport>>>,
    mem: Option<Arc<Client<DuplexTransport>>>,
    /// The sole member of a team-of-one (no team-mcp dispatch server is built).
    #[cfg(feature = "team-mcp")]
    solo_member: Option<MemberHandle>,
    tasks: Vec<JoinHandle<()>>,
    shut: bool,
}

impl TeamRuntime {
    /// Start a runtime from a builder and package (alias for
    /// [`TeamRuntimeBuilder::build`]).
    ///
    /// # Errors
    /// Propagates any [`RuntimeError`] from [`TeamRuntimeBuilder::build`].
    pub async fn start(
        builder: TeamRuntimeBuilder,
        pkg: &TeamPackage,
    ) -> Result<Self, RuntimeError> {
        builder.build(pkg).await
    }

    /// The derived attachment set that shaped this runtime.
    #[must_use]
    pub fn attachment(&self) -> &AttachmentSet {
        &self.attachment
    }

    /// The team-mcp member-dispatch client, if team-mcp attached (≥2 members).
    #[must_use]
    pub fn team_mcp_client(&self) -> Option<&Arc<Client<DuplexTransport>>> {
        self.team_mcp.as_ref()
    }

    /// The approval-mcp client, if approval-mcp attached (≥1 human role).
    #[must_use]
    pub fn approval_client(&self) -> Option<&Arc<Client<DuplexTransport>>> {
        self.approval.as_ref()
    }

    /// The team-fs client, if the team-fs opt-in was attached.
    #[must_use]
    pub fn team_fs_client(&self) -> Option<&Arc<Client<DuplexTransport>>> {
        self.team_fs.as_ref()
    }

    /// The mem-mcp client, if the mem-mcp opt-in was attached.
    #[must_use]
    pub fn mem_client(&self) -> Option<&Arc<Client<DuplexTransport>>> {
        self.mem.as_ref()
    }

    /// The sole member handle for a team-of-one (no team-mcp dispatch server).
    #[cfg(feature = "team-mcp")]
    #[must_use]
    pub fn solo_member(&self) -> Option<&MemberHandle> {
        self.solo_member.as_ref()
    }

    /// The number of hosted-server tasks the runtime is tracking (one per
    /// attached server). Useful to assert teardown accounted for every task.
    #[must_use]
    pub fn hosted_task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Shut the runtime down explicitly: abort and join every hosting task, then
    /// drop the clients (and any sole member), closing every in-memory transport
    /// so the servers' inner actor tasks reach EOF and end. This is the preferred
    /// teardown path; [`Drop`] is only a safety net.
    ///
    /// Returns the number of hosting tasks that were aborted and joined (so a
    /// caller can assert no tracked task leaked).
    pub async fn shutdown(mut self) -> usize {
        self.shut = true;
        for task in &self.tasks {
            task.abort();
        }
        let mut joined = 0usize;
        for task in self.tasks.drain(..) {
            // Join the aborted task so teardown is observable (a cancelled task
            // resolves to a `JoinError`); either way the task has stopped.
            let _ = task.await;
            joined += 1;
        }
        // Dropping `self` here closes client transports and drops the sole
        // member handle, ending any remaining member/inner-actor tasks.
        joined
    }
}

impl Drop for TeamRuntime {
    fn drop(&mut self) {
        // Safety net: guarantee no hosting task outlives the runtime even if the
        // caller skipped `shutdown`. `Drop` cannot await, so we only abort.
        if !self.shut {
            for task in &self.tasks {
                task.abort();
            }
        }
    }
}
