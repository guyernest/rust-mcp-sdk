//! `PromptHandler` implementation for `SequentialWorkflow`
//!
//! Enables workflows to be registered as prompts in the server.

use super::{
    conversion::{ExpansionContext, ResourceInfo, ToolInfo},
    sequential::SequentialWorkflow,
};
use crate::error::Result;
use crate::server::cancellation::RequestHandlerExtra;
use crate::server::PromptHandler;
use crate::types::{GetPromptResult, PromptArgument, PromptInfo};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// `PromptHandler` implementation for `SequentialWorkflow`
///
/// Wraps a validated workflow and provides prompt handler functionality.
/// The workflow's instructions are returned as the prompt messages.
///
/// # Metadata Implementation
///
/// This handler properly implements `PromptHandler::metadata()` to expose:
/// - **name**: The workflow name from `SequentialWorkflow::name()`
/// - **description**: The workflow description from `SequentialWorkflow::description()`
/// - **arguments**: All workflow arguments with their descriptions and required flags
///
/// When registered via `ServerBuilder::prompt_workflow()`, the metadata is automatically
/// available in `prompts/list` responses.
///
/// # Example
///
/// ```rust,no_run
/// use pmcp::Server;
/// use pmcp::server::workflow::{SequentialWorkflow, InternalPromptMessage};
/// use pmcp::types::Role;
///
/// let workflow = SequentialWorkflow::new(
///     "add_task",
///     "Add a task to a project"
/// )
/// .argument("project", "Project name", true)
/// .argument("task", "Task description", true);
///
/// let server = Server::builder()
///     .name("server")
///     .version("1.0.0")
///     .prompt_workflow(workflow)?
///     .build()?;
///
/// // The workflow metadata is now available in prompts/list
/// # Ok::<(), pmcp::Error>(())
/// ```
#[derive(Debug)]
pub struct WorkflowPromptHandler {
    /// The workflow definition
    workflow: SequentialWorkflow,
    /// Tool registry for handle expansion
    tools: HashMap<Arc<str>, ToolInfo>,
    /// Resource registry for handle expansion
    resources: HashMap<Arc<str>, ResourceInfo>,
}

impl WorkflowPromptHandler {
    /// Create a new workflow prompt handler
    ///
    /// # Arguments
    ///
    /// * `workflow` - The workflow definition (should be validated before passing)
    /// * `tools` - Tool registry for expanding tool handles
    /// * `resources` - Resource registry for expanding resource handles
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::server::workflow::{SequentialWorkflow, WorkflowPromptHandler};
    /// use std::collections::HashMap;
    ///
    /// let workflow = SequentialWorkflow::new("my_workflow", "A test workflow");
    /// let handler = WorkflowPromptHandler::new(workflow, HashMap::new(), HashMap::new());
    /// ```
    pub fn new(
        workflow: SequentialWorkflow,
        tools: HashMap<Arc<str>, ToolInfo>,
        resources: HashMap<Arc<str>, ResourceInfo>,
    ) -> Self {
        Self {
            workflow,
            tools,
            resources,
        }
    }
}

#[async_trait]
impl PromptHandler for WorkflowPromptHandler {
    async fn handle(
        &self,
        _args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> Result<GetPromptResult> {
        // Create expansion context for converting handles to protocol types
        let ctx = ExpansionContext {
            tools: &self.tools,
            resources: &self.resources,
        };

        // Convert workflow instructions to protocol messages
        let mut messages = Vec::new();
        for internal_msg in self.workflow.instructions() {
            let protocol_msg = internal_msg.to_protocol(&ctx).map_err(|e| {
                crate::Error::Internal(format!("Failed to convert workflow message: {}", e))
            })?;
            messages.push(protocol_msg);
        }

        Ok(GetPromptResult {
            description: Some(self.workflow.description().to_string()),
            messages,
        })
    }

    fn metadata(&self) -> Option<PromptInfo> {
        // Convert workflow arguments to prompt arguments
        let arguments = if self.workflow.arguments().is_empty() {
            None
        } else {
            Some(
                self.workflow
                    .arguments()
                    .iter()
                    .map(|(name, spec)| PromptArgument {
                        name: name.to_string(),
                        description: Some(spec.description.clone()),
                        required: spec.required,
                        completion: None,
                    })
                    .collect(),
            )
        };

        Some(PromptInfo {
            name: self.workflow.name().to_string(),
            description: Some(self.workflow.description().to_string()),
            arguments,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::workflow::{InternalPromptMessage, SequentialWorkflow};
    use crate::types::Role;

    #[tokio::test]
    async fn test_workflow_prompt_handler_basic() {
        let workflow = SequentialWorkflow::new("test_workflow", "A test workflow").instruction(
            InternalPromptMessage::new(Role::System, "Process the request"),
        );

        let handler = WorkflowPromptHandler::new(workflow, HashMap::new(), HashMap::new());

        // Test metadata
        let metadata = handler.metadata().expect("Should have metadata");
        assert_eq!(metadata.name, "test_workflow");
        assert_eq!(metadata.description, Some("A test workflow".to_string()));
        assert!(metadata.arguments.is_none());

        // Test handle
        let extra = RequestHandlerExtra {
            cancellation_token: Default::default(),
            request_id: "test-1".to_string(),
            session_id: None,
            auth_info: None,
            auth_context: None,
        };
        let result = handler
            .handle(HashMap::new(), extra)
            .await
            .expect("Should execute successfully");

        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::System);
    }

    #[tokio::test]
    async fn test_workflow_prompt_handler_with_arguments() {
        let workflow = SequentialWorkflow::new("test_workflow", "A test workflow")
            .argument("topic", "The topic to process", true)
            .argument("style", "Writing style", false)
            .instruction(InternalPromptMessage::new(
                Role::System,
                "Process the request",
            ));

        let handler = WorkflowPromptHandler::new(workflow, HashMap::new(), HashMap::new());

        // Test metadata includes arguments
        let metadata = handler.metadata().expect("Should have metadata");
        let args = metadata.arguments.expect("Should have arguments");
        assert_eq!(args.len(), 2);
        assert_eq!(args[0].name, "topic");
        assert!(args[0].required);
        assert_eq!(
            args[0].description,
            Some("The topic to process".to_string())
        );
        assert_eq!(args[1].name, "style");
        assert!(!args[1].required);
        assert_eq!(args[1].description, Some("Writing style".to_string()));
    }

    #[test]
    fn test_workflow_metadata_serialization() {
        use serde_json;

        let workflow = SequentialWorkflow::new(
            "add_project_task",
            "Add a task to a Logseq project with proper task formatting and scheduling",
        )
        .argument(
            "project",
            "The project name (Logseq page) to add the task to",
            true,
        )
        .argument("task", "The task description", true);

        let handler = WorkflowPromptHandler::new(workflow, HashMap::new(), HashMap::new());

        // Get metadata
        let metadata = handler.metadata().expect("Should have metadata");

        // Verify all fields are populated
        assert_eq!(metadata.name, "add_project_task");
        assert_eq!(
            metadata.description,
            Some(
                "Add a task to a Logseq project with proper task formatting and scheduling"
                    .to_string()
            )
        );

        // Verify JSON serialization works correctly
        let json = serde_json::to_value(&metadata).expect("Should serialize to JSON");

        // Check JSON structure matches MCP protocol expectations
        assert_eq!(json["name"], "add_project_task");
        assert_eq!(
            json["description"],
            "Add a task to a Logseq project with proper task formatting and scheduling"
        );

        let json_args = json["arguments"]
            .as_array()
            .expect("arguments should be array");
        assert_eq!(json_args.len(), 2);

        // Verify first argument
        assert_eq!(json_args[0]["name"], "project");
        assert_eq!(
            json_args[0]["description"],
            "The project name (Logseq page) to add the task to"
        );
        assert!(json_args[0]["required"].as_bool().unwrap());

        // Verify second argument
        assert_eq!(json_args[1]["name"], "task");
        assert_eq!(json_args[1]["description"], "The task description");
        assert!(json_args[1]["required"].as_bool().unwrap());

        // Also verify directly on the metadata struct
        let args = metadata.arguments.as_ref().expect("Should have arguments");
        assert_eq!(args.len(), 2);
        assert_eq!(args[0].name, "project");
        assert_eq!(
            args[0].description,
            Some("The project name (Logseq page) to add the task to".to_string())
        );
        assert!(args[0].required);
        assert_eq!(args[1].name, "task");
        assert_eq!(
            args[1].description,
            Some("The task description".to_string())
        );
        assert!(args[1].required);
    }

    #[tokio::test]
    async fn test_workflow_prompt_integration_with_server() {
        use crate::Server;

        // Create workflow
        let workflow = SequentialWorkflow::new(
            "add_project_task",
            "Add a task to a Logseq project with proper task formatting and scheduling",
        )
        .argument(
            "project",
            "The project name (Logseq page) to add the task to",
            true,
        )
        .argument("task", "The task description", true)
        .instruction(InternalPromptMessage::new(
            crate::types::Role::System,
            "Add the task to the project",
        ));

        // Build server with workflow prompt
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .prompt_workflow(workflow)
            .expect("Should register workflow")
            .build()
            .expect("Should build server");

        // Get the server's core to access prompts
        // Note: We need to use the internal API for testing
        // In a real scenario, this would go through the protocol handler

        // Simulate ListPromptsRequest
        let prompts_data = server.prompts;
        let prompts: Vec<_> = prompts_data
            .iter()
            .map(|(name, handler)| {
                if let Some(mut info) = handler.metadata() {
                    info.name.clone_from(name);
                    info
                } else {
                    crate::types::PromptInfo {
                        name: name.clone(),
                        description: None,
                        arguments: None,
                    }
                }
            })
            .collect();

        // Verify we have one prompt
        assert_eq!(prompts.len(), 1);

        // Verify the prompt has all metadata
        let prompt_info = &prompts[0];
        assert_eq!(prompt_info.name, "add_project_task");
        assert!(
            prompt_info.description.is_some(),
            "Description should be present"
        );
        assert_eq!(
            prompt_info.description.as_ref().unwrap(),
            "Add a task to a Logseq project with proper task formatting and scheduling"
        );

        // Verify arguments
        assert!(
            prompt_info.arguments.is_some(),
            "Arguments should be present"
        );
        let args = prompt_info.arguments.as_ref().unwrap();
        assert_eq!(args.len(), 2, "Should have 2 arguments");
        assert_eq!(args[0].name, "project");
        assert!(args[0].required);
        assert_eq!(args[1].name, "task");
        assert!(args[1].required);

        // Verify JSON serialization
        let json = serde_json::to_value(prompt_info).expect("Should serialize");
        assert_eq!(json["name"], "add_project_task");
        assert_eq!(
            json["description"],
            "Add a task to a Logseq project with proper task formatting and scheduling"
        );
        assert!(json["arguments"].is_array());
    }
}
