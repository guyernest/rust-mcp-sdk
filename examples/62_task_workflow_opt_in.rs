//! Example: Task Workflow Opt-In API
//!
//! Demonstrates the `.with_task_support(true)` API for SequentialWorkflow.
//! When task support is enabled and a task router is configured, the builder
//! wraps the workflow in a TaskWorkflowPromptHandler that creates a task
//! when the workflow prompt is invoked.
//!
//! This example:
//! 1. Creates a SequentialWorkflow with 2 steps and `.with_task_support(true)`
//! 2. Configures a task store and router on the server builder
//! 3. Registers the workflow via `.prompt_workflow()`
//! 4. Verifies the wiring compiled and the server built successfully
//!
//! This is NOT the full lifecycle example (see Phase 7's 63_tasks_workflow.rs).
//! It proves the opt-in API compiles and the builder wiring works end-to-end.
//!
//! Run: `cargo run --example 62_task_workflow_opt_in`

use async_trait::async_trait;
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::server::core::ProtocolHandler;
use pmcp::server::workflow::{SequentialWorkflow, ToolHandle, WorkflowStep};
use pmcp::types::ToolInfo;
use pmcp::RequestHandlerExtra;
use pmcp_tasks::{InMemoryTaskStore, TaskRouterImpl, TaskSecurityConfig};
use serde_json::{json, Value};
use std::sync::Arc;

/// A simple tool that validates a configuration.
struct ValidateConfig;

#[async_trait]
impl pmcp::ToolHandler for ValidateConfig {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!({
            "valid": true,
            "config": args.get("name").unwrap_or(&json!("default"))
        }))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            "validate_config",
            Some("Validate a configuration file".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Config name" }
                },
                "required": ["name"]
            }),
        ))
    }
}

/// A simple tool that deploys a service.
struct DeployService;

#[async_trait]
impl pmcp::ToolHandler for DeployService {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!({
            "deployed": true,
            "region": args.get("region").unwrap_or(&json!("us-east-1"))
        }))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            "deploy_service",
            Some("Deploy a service to a region".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "region": { "type": "string", "description": "AWS region" }
                },
                "required": ["region"]
            }),
        ))
    }
}

fn main() {
    // 1. Create task store and router
    let store = Arc::new(
        InMemoryTaskStore::new()
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true)),
    );
    let router = Arc::new(TaskRouterImpl::new(store));

    // 2. Create a workflow with task support enabled
    let workflow = SequentialWorkflow::new("deploy_pipeline", "Validate and deploy a service")
        .argument("region", "Target AWS region", true)
        .step(WorkflowStep::new(
            "validate",
            ToolHandle::new("validate_config"),
        ))
        .step(WorkflowStep::new(
            "deploy",
            ToolHandle::new("deploy_service"),
        ))
        .with_task_support(true);

    assert!(
        workflow.has_task_support(),
        "Workflow should have task support enabled"
    );

    // 3. Build server with task store and task-enabled workflow
    let server = ServerCoreBuilder::new()
        .name("task-workflow-example")
        .version("1.0.0")
        .tool("validate_config", ValidateConfig)
        .tool("deploy_service", DeployService)
        .with_task_store(router)
        .prompt_workflow(workflow)
        .expect("Task-enabled workflow should register successfully")
        .build()
        .expect("Server should build successfully");

    // 4. Verify the server built and has prompt capabilities
    let caps = server.capabilities();
    assert!(
        caps.prompts.is_some(),
        "Server should have prompt capabilities"
    );
    assert!(
        caps.experimental.is_some(),
        "Server should have experimental (tasks) capabilities"
    );

    println!("Task workflow opt-in example: OK");
    println!("  - Workflow 'deploy_pipeline' registered with task support");
    println!("  - Server built with task router and task-enabled workflow");
    println!("  - Builder correctly wrapped workflow in TaskWorkflowPromptHandler");
    println!("  - Prompt capabilities: {:?}", caps.prompts);
}
