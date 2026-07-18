//! Builds the approval-mcp [`pmcp::Server`] advertising the two UNNAMESPACED
//! legacy static tools (`resolve_approval`, `get_approval`) plus one
//! `team_approval__ask_<role>` per human role (the `approval_tool_surface`
//! equation of `contracts/team-servers-v1.yaml`).
//!
//! # Two stores, two jobs
//!
//! - An [`InMemoryTaskStore`] provides the OBSERVABLE pending→resolved
//!   lifecycle: `ask` mints a `Working` task, `resolve_approval` transitions it
//!   to `Completed`. Clients can watch it via `tasks/get`.
//! - The [`ApprovalRepository`] holds the approval-DOMAIN state a task store
//!   cannot (question, option set, target role, verdict, subject ref) and is the
//!   SOURCE OF TRUTH for [atomic first-writer resolution](ApprovalRepository::resolve).
//!
//! Both are SERVICE-OWNED ([`SERVICE_OWNER`]): the task is minted under a fixed
//! owner and the repository is a shared instance, so any connected client may
//! resolve (D-10). There is no auth in dev — that is deliberate.
//!
//! # ask → notify ordering
//!
//! `ask` creates the repository record (and the pending task) FIRST, THEN
//! notifies the channel. A notify failure is logged and the ask STILL returns
//! the approval id: the approval remains resolvable out-of-band, never
//! unreachable.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde_json::{json, Value};

use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
use pmcp::types::tasks::TaskStatus;
use pmcp::{Error, RequestHandlerExtra, Result, Server, ToolHandler, TypedTool};

use pmcp_package::package::HumanRole;

use crate::approval::channels::{ApprovalAsk, ApprovalChannel};
use crate::approval::repository::{
    ApprovalError, ApprovalRecord, ApprovalRepository, NewApproval, SERVICE_OWNER,
};

/// The UNNAMESPACED legacy resolve tool name.
pub const RESOLVE_APPROVAL_TOOL: &str = "resolve_approval";
/// The UNNAMESPACED legacy get tool name.
pub const GET_APPROVAL_TOOL: &str = "get_approval";
/// The dynamic ask-family prefix; the concrete name is `team_approval__ask_<role>`.
pub const ASK_TOOL_PREFIX: &str = "team_approval__ask_";

type BoxFut = Pin<Box<dyn Future<Output = Result<Value>> + Send>>;

/// The `team_approval__ask_<role>` tool name for a human role.
///
/// The role label is lower-cased and any character outside `[a-z0-9_]` is
/// mapped to `_` so the result is a valid, stable tool name.
#[must_use]
pub fn ask_tool_name(role: &str) -> String {
    let slug: String = role
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("{ASK_TOOL_PREFIX}{slug}")
}

/// Maps an [`ApprovalError`] onto a protocol [`Error`] (never a panic).
fn map_approval_err(err: ApprovalError) -> Error {
    let msg = err.to_string();
    match err {
        ApprovalError::NotFound(_) => Error::not_found(msg),
        ApprovalError::AlreadyResolved { .. }
        | ApprovalError::InvalidDecision { .. }
        | ApprovalError::UnknownRole(_) => Error::validation(msg),
    }
}

/// Reads a required string argument.
fn arg_str(args: &Value, key: &str) -> Result<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| Error::validation(format!("missing required '{key}' string")))
}

/// Reads an optional string argument.
fn arg_str_opt(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::to_string)
}

/// Reads the required, non-empty `options` string array.
fn arg_options(args: &Value) -> Result<Vec<String>> {
    let options: Vec<String> = args
        .get("options")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if options.is_empty() {
        return Err(Error::validation(
            "'options' must be a non-empty array of strings",
        ));
    }
    Ok(options)
}

/// Serializes a record and adds the `approvalId` alias for the wire echo.
fn record_to_json(record: &ApprovalRecord) -> Value {
    let mut v = serde_json::to_value(record).unwrap_or(Value::Null);
    if let Some(obj) = v.as_object_mut() {
        obj.insert("approvalId".to_string(), Value::String(record.id.clone()));
    }
    v
}

/// Build the approval-mcp [`Server`].
///
/// Registers `resolve_approval` + `get_approval` UNNAMESPACED plus exactly one
/// `team_approval__ask_<role>` per entry in `human_roles` (computed once here),
/// and wires an [`InMemoryTaskStore`] for the observable lifecycle. `channel` is
/// notify-only; `repo` holds the approval-domain state.
///
/// # Errors
///
/// Propagates any [`Server`] construction error.
pub fn build_approval_mcp_server(
    human_roles: &[HumanRole],
    channel: Arc<dyn ApprovalChannel>,
    repo: Arc<ApprovalRepository>,
) -> Result<Server> {
    let task_store = Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>;

    let mut builder = Server::builder()
        .name("approval-mcp")
        .version(env!("CARGO_PKG_VERSION"))
        .tool_arc(
            RESOLVE_APPROVAL_TOOL,
            resolve_tool(repo.clone(), task_store.clone()),
        )
        .tool_arc(GET_APPROVAL_TOOL, get_tool(repo.clone()));

    for role in human_roles {
        let name = ask_tool_name(&role.role);
        builder = builder.tool_arc(
            &name,
            ask_tool(
                role,
                &name,
                repo.clone(),
                channel.clone(),
                task_store.clone(),
            ),
        );
    }

    builder.task_store(task_store).build()
}

fn resolve_tool(
    repo: Arc<ApprovalRepository>,
    task_store: Arc<dyn TaskStore>,
) -> Arc<dyn ToolHandler> {
    let tool = TypedTool::new_with_schema(
        RESOLVE_APPROVAL_TOOL,
        json!({
            "type": "object",
            "properties": {
                "approvalId": { "type": "string", "description": "The approval id to resolve" },
                "decision": { "type": "string", "description": "The chosen decision (must be one of the ask's options)" }
            },
            "required": ["approvalId", "decision"]
        }),
        move |args: Value, _extra: RequestHandlerExtra| {
            let repo = repo.clone();
            let task_store = task_store.clone();
            Box::pin(async move {
                let id = arg_str(&args, "approvalId")?;
                let decision = arg_str(&args, "decision")?;
                // Atomic first-writer + decision-vs-option-set validation.
                let record = repo.resolve(&id, &decision).map_err(map_approval_err)?;
                // Observable transition (record is the source of truth; a task
                // transition failure is logged, not fatal).
                if let Some(task_id) = &record.task_id {
                    if let Err(e) = task_store
                        .update_status(
                            task_id,
                            SERVICE_OWNER,
                            TaskStatus::Completed,
                            Some(decision.clone()),
                        )
                        .await
                    {
                        tracing::warn!(
                            approval_id = %record.id,
                            error = %e,
                            "task-store transition failed after resolve (record already authoritative)"
                        );
                    }
                }
                Ok(record_to_json(&record))
            }) as BoxFut
        },
    )
    .with_description("Resolve a pending approval with a decision from its option set (any client; D-10).");
    Arc::new(tool)
}

fn get_tool(repo: Arc<ApprovalRepository>) -> Arc<dyn ToolHandler> {
    let tool = TypedTool::new_with_schema(
        GET_APPROVAL_TOOL,
        json!({
            "type": "object",
            "properties": { "approvalId": { "type": "string", "description": "The approval id to fetch" } },
            "required": ["approvalId"]
        }),
        move |args: Value, _extra: RequestHandlerExtra| {
            let repo = repo.clone();
            Box::pin(async move {
                let id = arg_str(&args, "approvalId")?;
                let record = repo
                    .get(&id)
                    .ok_or_else(|| Error::not_found(format!("approval not found: {id}")))?;
                Ok(record_to_json(&record))
            }) as BoxFut
        },
    )
    .with_description("Fetch an approval record (echoes any subject_task_id/subject_ref; D-12).")
    .read_only();
    Arc::new(tool)
}

fn ask_tool(
    role: &HumanRole,
    tool_name: &str,
    repo: Arc<ApprovalRepository>,
    channel: Arc<dyn ApprovalChannel>,
    task_store: Arc<dyn TaskStore>,
) -> Arc<dyn ToolHandler> {
    let target_role = role.role.clone();
    let description = if role.description.is_empty() {
        format!("Ask the '{}' role for an approval decision.", role.role)
    } else {
        role.description.clone()
    };
    let tool = TypedTool::new_with_schema(
        tool_name,
        json!({
            "type": "object",
            "properties": {
                "question": { "type": "string", "description": "The question for the human role" },
                "options": {
                    "type": "array",
                    "items": { "type": "string" },
                    "minItems": 1,
                    "description": "The closed set of acceptable decisions"
                },
                "subjectTaskId": { "type": "string", "description": "Optional linked task id (D-12)" },
                "subjectRef": { "type": "string", "description": "Optional linked component/ref (D-12)" }
            },
            "required": ["question", "options"]
        }),
        move |args: Value, _extra: RequestHandlerExtra| {
            let repo = repo.clone();
            let channel = channel.clone();
            let task_store = task_store.clone();
            let target_role = target_role.clone();
            Box::pin(async move {
                let question = arg_str(&args, "question")?;
                let options = arg_options(&args)?;
                let subject_task_id = arg_str_opt(&args, "subjectTaskId");
                let subject_ref = arg_str_opt(&args, "subjectRef");

                // Mint the observable pending task FIRST.
                let task = task_store.create(SERVICE_OWNER, None).await?;

                // Create the domain record (source of truth) BEFORE notifying.
                let record = repo.create(NewApproval {
                    question,
                    options,
                    target_role: target_role.clone(),
                    subject_task_id,
                    subject_ref,
                    task_id: Some(task.task_id.clone()),
                });

                // Notify-only; a failure is NON-BLOCKING — the approval stays
                // resolvable via resolve_approval.
                let ask = ApprovalAsk {
                    approval_id: record.id.clone(),
                    question: record.question.clone(),
                    options: record.options.clone(),
                    target_role: record.target_role.clone(),
                    subject_task_id: record.subject_task_id.clone(),
                    subject_ref: record.subject_ref.clone(),
                };
                if let Err(e) = channel.notify(&ask).await {
                    tracing::warn!(
                        approval_id = %record.id,
                        error = %e,
                        "approval notify failed (non-blocking; approval remains resolvable)"
                    );
                }

                Ok(record_to_json(&record))
            }) as BoxFut
        },
    )
    .with_description(description);
    Arc::new(tool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approval::channels::{ChannelError, ConsoleChannel};
    use crate::transport::DuplexTransport;
    use async_trait::async_trait;
    use pmcp::types::{ClientCapabilities, Content};
    use pmcp::Client;

    fn roles() -> Vec<HumanRole> {
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

    fn build_with(channel: Arc<dyn ApprovalChannel>) -> Server {
        let repo = Arc::new(ApprovalRepository::deterministic());
        build_approval_mcp_server(&roles(), channel, repo).unwrap()
    }

    fn build() -> Server {
        build_with(Arc::new(ConsoleChannel::new()))
    }

    async fn call_json(client: &Client<DuplexTransport>, name: &str, args: Value) -> Value {
        let res = client
            .call_tool(name.to_string(), args)
            .await
            .unwrap_or_else(|e| panic!("call {name} failed: {e}"));
        match &res.content[0] {
            Content::Text { text } => serde_json::from_str(text).expect("json body"),
            other => panic!("expected text content, got {other:?}"),
        }
    }

    async fn connect(server: Server) -> (Client<DuplexTransport>, tokio::task::JoinHandle<()>) {
        let (client_t, server_t) = DuplexTransport::pair();
        let handle = tokio::spawn(async move {
            let _ = server.run(server_t).await;
        });
        let mut client = Client::new(client_t);
        client
            .initialize(ClientCapabilities::default())
            .await
            .expect("initialize");
        (client, handle)
    }

    #[test]
    fn ask_tool_name_is_sanitized() {
        assert_eq!(
            ask_tool_name("release-manager"),
            "team_approval__ask_release_manager"
        );
        assert_eq!(ask_tool_name("Approver"), "team_approval__ask_approver");
    }

    #[tokio::test]
    async fn advertises_exact_surface() {
        let (client, handle) = connect(build()).await;
        let list = client.list_tools(None).await.expect("list_tools");
        let mut names: Vec<&str> = list.tools.iter().map(|t| t.name.as_str()).collect();
        names.sort_unstable();
        let mut expected = vec![
            "get_approval",
            "resolve_approval",
            "team_approval__ask_release_manager",
            "team_approval__ask_security_reviewer",
        ];
        expected.sort_unstable();
        assert_eq!(names, expected, "exact 2 static + N ask surface, no extras");
        handle.abort();
    }

    #[tokio::test]
    async fn ask_resolve_get_lifecycle() {
        let (client, handle) = connect(build()).await;

        let ask = call_json(
            &client,
            "team_approval__ask_release_manager",
            json!({ "question": "Ship?", "options": ["approve", "reject"] }),
        )
        .await;
        let id = ask["approvalId"].as_str().unwrap().to_string();
        assert_eq!(id, "appr-001");
        assert_eq!(ask["status"], "pending");

        let got = call_json(&client, "get_approval", json!({ "approvalId": id })).await;
        assert_eq!(got["status"], "pending");

        let resolved = call_json(
            &client,
            "resolve_approval",
            json!({ "approvalId": id, "decision": "approve" }),
        )
        .await;
        assert_eq!(resolved["status"], "resolved");
        assert_eq!(resolved["verdict"], "approve");

        let got2 = call_json(&client, "get_approval", json!({ "approvalId": id })).await;
        assert_eq!(got2["status"], "resolved");
        assert_eq!(got2["verdict"], "approve");

        handle.abort();
    }

    #[tokio::test]
    async fn subject_ref_round_trips() {
        let (client, handle) = connect(build()).await;
        let ask = call_json(
            &client,
            "team_approval__ask_release_manager",
            json!({
                "question": "Ship?",
                "options": ["approve", "reject"],
                "subjectTaskId": "task-42",
                "subjectRef": "agent://triage@1"
            }),
        )
        .await;
        let id = ask["approvalId"].as_str().unwrap().to_string();
        assert_eq!(ask["subjectTaskId"], "task-42");

        let resolved = call_json(
            &client,
            "resolve_approval",
            json!({ "approvalId": id, "decision": "approve" }),
        )
        .await;
        assert_eq!(resolved["subjectTaskId"], "task-42");
        assert_eq!(resolved["subjectRef"], "agent://triage@1");
        handle.abort();
    }

    #[tokio::test]
    async fn double_resolve_is_rejected() {
        let (client, handle) = connect(build()).await;
        let ask = call_json(
            &client,
            "team_approval__ask_release_manager",
            json!({ "question": "Ship?", "options": ["approve", "reject"] }),
        )
        .await;
        let id = ask["approvalId"].as_str().unwrap().to_string();
        call_json(
            &client,
            "resolve_approval",
            json!({ "approvalId": id, "decision": "approve" }),
        )
        .await;
        let second = client
            .call_tool(
                "resolve_approval".to_string(),
                json!({ "approvalId": id, "decision": "reject" }),
            )
            .await;
        assert!(second.is_err(), "second resolution must be rejected");
        handle.abort();
    }

    #[tokio::test]
    async fn out_of_set_decision_errors() {
        let (client, handle) = connect(build()).await;
        let ask = call_json(
            &client,
            "team_approval__ask_release_manager",
            json!({ "question": "Ship?", "options": ["approve", "reject"] }),
        )
        .await;
        let id = ask["approvalId"].as_str().unwrap().to_string();
        let bad = client
            .call_tool(
                "resolve_approval".to_string(),
                json!({ "approvalId": id, "decision": "maybe" }),
            )
            .await;
        assert!(bad.is_err(), "out-of-option-set decision must error");
        handle.abort();
    }

    #[tokio::test]
    async fn unknown_role_ask_tool_is_not_advertised() {
        let (client, handle) = connect(build()).await;
        let missing = client
            .call_tool(
                "team_approval__ask_nobody".to_string(),
                json!({ "question": "?", "options": ["yes"] }),
            )
            .await;
        assert!(
            missing.is_err(),
            "an unadvertised role tool must error, never panic"
        );
        handle.abort();
    }

    /// A channel whose `notify` always fails, to prove ask stays resolvable.
    struct FailingChannel;

    #[async_trait]
    impl ApprovalChannel for FailingChannel {
        async fn notify(&self, _ask: &ApprovalAsk) -> std::result::Result<(), ChannelError> {
            Err(ChannelError::Transport("simulated outage".to_string()))
        }
    }

    #[tokio::test]
    async fn notify_failure_leaves_approval_resolvable() {
        let (client, handle) = connect(build_with(Arc::new(FailingChannel))).await;
        let ask = call_json(
            &client,
            "team_approval__ask_release_manager",
            json!({ "question": "Ship?", "options": ["approve", "reject"] }),
        )
        .await;
        // ask still succeeded despite the notify failure.
        let id = ask["approvalId"].as_str().unwrap().to_string();
        assert_eq!(ask["status"], "pending");

        // And it is resolvable.
        let resolved = call_json(
            &client,
            "resolve_approval",
            json!({ "approvalId": id, "decision": "approve" }),
        )
        .await;
        assert_eq!(resolved["status"], "resolved");
        handle.abort();
    }
}
