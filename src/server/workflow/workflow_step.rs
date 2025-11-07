//! Workflow step with chainable builder
//!
//! Provides a type-safe, ergonomic API for building workflow steps.

use super::{
    data_source::DataSource,
    error::WorkflowError,
    handles::{ResourceHandle, ToolHandle},
    newtypes::{ArgName, BindingName, StepName},
};
use indexmap::IndexMap;
use std::collections::HashMap;

/// A single step in a workflow
///
/// A workflow step can either:
/// - Execute a tool (with optional resources for context)
/// - Fetch resources only (no tool execution)
///
/// # Examples
///
/// Tool execution step:
/// ```ignore
/// WorkflowStep::new("get_data", ToolHandle::new("fetch_user"))
///     .bind("user")
/// ```
///
/// Resource-only step:
/// ```ignore
/// WorkflowStep::fetch_resources("fetch_guide")
///     .with_resource("docs://guide/{topic}")
///     .with_template_binding("topic", prompt_arg("topic"))
/// ```
#[derive(Clone, Debug)]
pub struct WorkflowStep {
    /// Step identifier
    name: StepName,
    /// Tool to invoke (None for resource-only steps)
    tool: Option<ToolHandle>,
    /// Argument mappings (tool arg name -> data source)
    /// `IndexMap` for deterministic iteration
    arguments: IndexMap<ArgName, DataSource>,
    /// Optional binding name for step output
    binding: Option<BindingName>,
    /// Optional guidance for client LLM about what this step should accomplish
    ///
    /// When provided, this guidance is rendered as an assistant message in the
    /// conversation trace. It helps the client understand the step's intent,
    /// especially when the server cannot execute it deterministically.
    ///
    /// Guidance text supports argument substitution using `{arg_name}` syntax.
    /// For example: `"Find the page matching '{project}' in the list above"`
    guidance: Option<String>,
    /// Resources to fetch and embed before executing this step
    ///
    /// Resources are fetched server-side and their content is embedded in the
    /// conversation trace as user messages. This enables hybrid execution where
    /// the server provides all necessary context (tool results + resource content)
    /// before the client LLM continues.
    resources: Vec<ResourceHandle>,
    /// Template variable bindings for resource URI interpolation
    ///
    /// Maps template variable names to data sources. Template variables in resource
    /// URIs (using `{var_name}` syntax) are resolved from these bindings at execution time.
    ///
    /// # Example
    /// ```ignore
    /// WorkflowStep::new("read", ResourceHandle::new("docs://{doc_id}"))
    ///     .with_template_binding("doc_id", field("query_result", "document_id"))
    /// ```
    template_bindings: HashMap<String, DataSource>,
}

impl WorkflowStep {
    /// Create a new workflow step that executes a tool
    ///
    /// For resource-only steps (no tool execution), use [`WorkflowStep::fetch_resources`] instead.
    ///
    /// # Example
    /// ```
    /// use pmcp::server::workflow::{WorkflowStep, ToolHandle};
    ///
    /// let step = WorkflowStep::new("step1", ToolHandle::new("greet"));
    /// ```
    #[must_use]
    pub fn new(name: impl Into<StepName>, tool: ToolHandle) -> Self {
        Self {
            name: name.into(),
            tool: Some(tool),
            arguments: IndexMap::new(),
            binding: None,
            guidance: None,
            resources: Vec::new(),
            template_bindings: HashMap::new(),
        }
    }

    /// Create a new resource-only workflow step (no tool execution)
    ///
    /// Resource-only steps fetch resources and embed their content in the conversation
    /// without executing any tools. This is useful when you need to provide context
    /// from resources based on previous step results.
    ///
    /// # Example
    /// ```
    /// use pmcp::server::workflow::{WorkflowStep, DataSource};
    ///
    /// let step = WorkflowStep::fetch_resources("fetch_guide")
    ///     .with_resource("docs://guide/intro")
    ///     .expect("Valid resource URI");
    /// ```
    ///
    /// # With Dynamic Resource URIs
    /// ```
    /// use pmcp::server::workflow::{WorkflowStep, DataSource};
    ///
    /// let step = WorkflowStep::fetch_resources("fetch_walkthrough")
    ///     .with_resource("game://walkthrough/{game_id}")
    ///     .expect("Valid resource URI")
    ///     .with_template_binding("game_id", DataSource::from_step_field("progress", "game_id"));
    /// ```
    ///
    /// # Validation
    ///
    /// Resource-only steps must have at least one resource URI. Validation will fail if:
    /// - No resources are specified
    /// - Tool arguments are provided (use `.arg()` only with tool steps)
    #[must_use]
    pub fn fetch_resources(name: impl Into<StepName>) -> Self {
        Self {
            name: name.into(),
            tool: None,
            arguments: IndexMap::new(),
            binding: None,
            guidance: None,
            resources: Vec::new(),
            template_bindings: HashMap::new(),
        }
    }

    /// Add an argument mapping (chainable)
    ///
    /// # Example
    /// ```
    /// use pmcp::server::workflow::{WorkflowStep, ToolHandle, DataSource};
    ///
    /// let step = WorkflowStep::new("step1", ToolHandle::new("greet"))
    ///     .arg("name", DataSource::prompt_arg("user_name"))
    ///     .arg("greeting", DataSource::constant(serde_json::json!("Hello")));
    /// ```
    #[must_use]
    pub fn arg(mut self, name: impl Into<ArgName>, source: DataSource) -> Self {
        self.arguments.insert(name.into(), source);
        self
    }

    /// Set the output binding name (chainable)
    ///
    /// # Example
    /// ```
    /// use pmcp::server::workflow::{WorkflowStep, ToolHandle};
    ///
    /// let step = WorkflowStep::new("step1", ToolHandle::new("greet"))
    ///     .bind("greeting_result");
    /// ```
    #[must_use]
    pub fn bind(mut self, binding: impl Into<BindingName>) -> Self {
        self.binding = Some(binding.into());
        self
    }

    /// Add guidance for the client LLM about what this step should accomplish (chainable)
    ///
    /// Guidance is rendered as an assistant message and helps the client understand
    /// the step's intent, especially when the server cannot execute it deterministically.
    ///
    /// Guidance text supports argument substitution using `{arg_name}` syntax.
    /// At runtime, `{arg_name}` will be replaced with the actual argument value.
    ///
    /// # Example
    /// ```
    /// use pmcp::server::workflow::{WorkflowStep, ToolHandle};
    ///
    /// let step = WorkflowStep::new("match_project", ToolHandle::new("add_task"))
    ///     .with_guidance("Find the page name from the list above that best matches '{project}'");
    /// ```
    ///
    /// # Use Cases
    ///
    /// Use guidance when:
    /// - The step requires LLM reasoning (fuzzy matching, context-aware decisions)
    /// - The server cannot resolve all parameters deterministically
    /// - You want to enable hybrid execution (server starts, client continues)
    #[must_use]
    pub fn with_guidance(mut self, guidance: impl Into<String>) -> Self {
        self.guidance = Some(guidance.into());
        self
    }

    /// Add a resource to fetch and embed before executing this step (chainable)
    ///
    /// Resources are fetched server-side during workflow execution and their content
    /// is embedded in the conversation trace as user messages. This ensures the client
    /// LLM has all necessary context (tool results + resource content) when it continues
    /// execution.
    ///
    /// Multiple resources can be added by calling this method multiple times.
    ///
    /// # Example
    /// ```
    /// use pmcp::server::workflow::{WorkflowStep, ToolHandle};
    ///
    /// let step = WorkflowStep::new("add_task", ToolHandle::new("add_journal_task"))
    ///     .with_guidance("Format the task as shown in the guide")
    ///     .with_resource("docs://logseq/task-format")
    ///     .expect("Valid resource URI");
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `WorkflowError` if the URI is invalid (must use resource:// or file:// scheme)
    ///
    /// # Use Cases
    ///
    /// Use resources when:
    /// - The step requires documentation or context that's stored as a resource
    /// - You want to provide formatting guides, examples, or schemas to the client LLM
    /// - The resource content is needed for the client to make informed decisions
    pub fn with_resource(mut self, uri: impl AsRef<str>) -> Result<Self, WorkflowError> {
        let handle = ResourceHandle::new(uri)?;
        self.resources.push(handle);
        Ok(self)
    }

    /// Bind a template variable for resource URI interpolation (chainable)
    ///
    /// Template variables in resource URIs (using `{var_name}` syntax) are resolved
    /// from these bindings at execution time. This enables dynamic resource fetching
    /// based on values from previous workflow steps or prompt arguments.
    ///
    /// # Example
    /// ```
    /// use pmcp::server::workflow::{WorkflowStep, ToolHandle, DataSource};
    ///
    /// // Dynamic resource URI based on previous step's result
    /// let step = WorkflowStep::new("read_guide", ToolHandle::new("read"))
    ///     .with_resource("docs://{doc_id}")
    ///     .expect("Valid resource URI")
    ///     .with_template_binding("doc_id", DataSource::from_step_field("query", "id"));
    ///
    /// // Multiple template variables
    /// let step2 = WorkflowStep::new("read_project", ToolHandle::new("read"))
    ///     .with_resource("project://{org}/{repo}/config")
    ///     .expect("Valid resource URI")
    ///     .with_template_binding("org", DataSource::from_step_field("project", "organization"))
    ///     .with_template_binding("repo", DataSource::from_step_field("project", "repository"));
    /// ```
    ///
    /// # Use Cases
    ///
    /// Use template bindings when:
    /// - Resource URIs depend on values from previous workflow steps
    /// - Resource URIs are constructed from user-provided prompt arguments
    /// - You need to fetch different resources based on workflow state
    ///
    /// # Template Syntax
    ///
    /// Template variables use `{var_name}` syntax, matching the format used in guidance messages.
    /// At execution time, each `{var_name}` is replaced with the value resolved from the corresponding
    /// `DataSource`.
    #[must_use]
    pub fn with_template_binding(
        mut self,
        var_name: impl Into<String>,
        source: DataSource,
    ) -> Self {
        self.template_bindings.insert(var_name.into(), source);
        self
    }

    /// Get step name
    pub fn name(&self) -> &StepName {
        &self.name
    }

    /// Get tool handle (None for resource-only steps)
    pub fn tool(&self) -> Option<&ToolHandle> {
        self.tool.as_ref()
    }

    /// Check if this is a resource-only step
    pub fn is_resource_only(&self) -> bool {
        self.tool.is_none()
    }

    /// Get arguments
    pub fn arguments(&self) -> &IndexMap<ArgName, DataSource> {
        &self.arguments
    }

    /// Get binding name if set
    pub fn binding(&self) -> Option<&BindingName> {
        self.binding.as_ref()
    }

    /// Get guidance text if set
    pub fn guidance(&self) -> Option<&str> {
        self.guidance.as_deref()
    }

    /// Get resources to fetch for this step
    pub fn resources(&self) -> &[ResourceHandle] {
        &self.resources
    }

    /// Get template bindings for resource URI interpolation
    pub fn template_bindings(&self) -> &HashMap<String, DataSource> {
        &self.template_bindings
    }

    /// Validate the step
    ///
    /// Checks that:
    /// - All referenced bindings are available
    /// - Resource-only steps have at least one resource
    /// - Resource-only steps don't have tool arguments
    pub fn validate(&self, available_bindings: &[BindingName]) -> Result<(), WorkflowError> {
        // Validate resource-only steps
        if self.is_resource_only() {
            // Must have at least one resource
            if self.resources.is_empty() {
                return Err(WorkflowError::InvalidMapping {
                    step: self.name.to_string(),
                    reason: "Resource-only steps must have at least one resource. Use .with_resource() to add resources.".to_string(),
                });
            }

            // Resource-only steps cannot have tool arguments
            if !self.arguments.is_empty() {
                return Err(WorkflowError::InvalidMapping {
                    step: self.name.to_string(),
                    reason: "Resource-only steps cannot have tool arguments. Remove .arg() calls or use WorkflowStep::new() instead.".to_string(),
                });
            }

            // Resource-only steps cannot have bindings (no tool output to bind)
            if self.binding.is_some() {
                return Err(WorkflowError::InvalidMapping {
                    step: self.name.to_string(),
                    reason: "Resource-only steps cannot have output bindings. Remove .bind() call."
                        .to_string(),
                });
            }
        }

        // Check that all step output references exist in arguments
        for (_arg_name, source) in &self.arguments {
            if let DataSource::StepOutput { step, .. } = source {
                // Convert step name to binding name for lookup
                let binding = BindingName::new(step.as_str());
                if !available_bindings.contains(&binding) {
                    return Err(WorkflowError::UnknownBinding {
                        step: self.name.to_string(),
                        binding: binding.to_string(),
                    });
                }
            }
        }

        // Check that all step output references exist in template bindings
        for source in self.template_bindings.values() {
            if let DataSource::StepOutput { step, .. } = source {
                // Convert step name to binding name for lookup
                let binding = BindingName::new(step.as_str());
                if !available_bindings.contains(&binding) {
                    return Err(WorkflowError::UnknownBinding {
                        step: self.name.to_string(),
                        binding: binding.to_string(),
                    });
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_workflow_step_creation() {
        let step = WorkflowStep::new("step1", ToolHandle::new("greet"));
        assert_eq!(step.name().as_str(), "step1");
        assert_eq!(step.tool().unwrap().name(), "greet");
        assert!(step.arguments().is_empty());
        assert!(step.binding().is_none());
        assert!(!step.is_resource_only());
    }

    #[test]
    fn test_workflow_step_with_args() {
        let step = WorkflowStep::new("step1", ToolHandle::new("greet"))
            .arg("name", DataSource::prompt_arg("user_name"))
            .arg("greeting", DataSource::constant(json!("Hello")));

        assert_eq!(step.arguments().len(), 2);
        assert!(step.arguments().contains_key(&ArgName::new("name")));
        assert!(step.arguments().contains_key(&ArgName::new("greeting")));
    }

    #[test]
    fn test_workflow_step_with_binding() {
        let step = WorkflowStep::new("step1", ToolHandle::new("greet")).bind("result");

        assert!(step.binding().is_some());
        assert_eq!(step.binding().unwrap().as_str(), "result");
    }

    #[test]
    fn test_workflow_step_chainable_builder() {
        let step = WorkflowStep::new("create_content", ToolHandle::new("create_content"))
            .arg("topic", DataSource::prompt_arg("topic"))
            .arg("style", DataSource::constant(json!("formal")))
            .bind("content");

        assert_eq!(step.name().as_str(), "create_content");
        assert_eq!(step.arguments().len(), 2);
        assert_eq!(step.binding().unwrap().as_str(), "content");
    }

    #[test]
    fn test_workflow_step_validation_success() {
        let step = WorkflowStep::new("step2", ToolHandle::new("process"))
            .arg("input", DataSource::from_step("step1"));

        let available = vec![BindingName::new("step1")];
        assert!(step.validate(&available).is_ok());
    }

    #[test]
    fn test_workflow_step_validation_failure() {
        let step = WorkflowStep::new("step2", ToolHandle::new("process"))
            .arg("input", DataSource::from_step("step1"));

        let available = vec![]; // step1 not available
        let result = step.validate(&available);
        assert!(result.is_err());

        if let Err(WorkflowError::UnknownBinding { step: s, binding }) = result {
            assert_eq!(s, "step2");
            assert_eq!(binding, "step1");
        } else {
            panic!("Expected UnknownBinding error");
        }
    }

    #[test]
    fn test_workflow_step_deterministic_arg_order() {
        let step = WorkflowStep::new("step1", ToolHandle::new("tool"))
            .arg("z_arg", DataSource::prompt_arg("z"))
            .arg("a_arg", DataSource::prompt_arg("a"))
            .arg("m_arg", DataSource::prompt_arg("m"));

        // IndexMap preserves insertion order
        let keys: Vec<_> = step.arguments().keys().map(|k| k.as_str()).collect();
        assert_eq!(keys, vec!["z_arg", "a_arg", "m_arg"]);
    }

    #[test]
    fn test_workflow_step_clone() {
        let step = WorkflowStep::new("step1", ToolHandle::new("greet"))
            .arg("name", DataSource::prompt_arg("user"))
            .bind("result");

        let cloned = step.clone();
        assert_eq!(cloned.name().as_str(), step.name().as_str());
        assert_eq!(cloned.arguments().len(), step.arguments().len());
    }

    #[test]
    fn test_workflow_step_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<WorkflowStep>();
    }

    #[test]
    fn test_workflow_step_with_step_output_field() {
        let step = WorkflowStep::new("step2", ToolHandle::new("process"))
            .arg("data", DataSource::from_step_field("step1", "output"));

        let arg_source = step.arguments().get(&ArgName::new("data")).unwrap();
        match arg_source {
            DataSource::StepOutput { step, field } => {
                assert_eq!(step.as_str(), "step1");
                assert_eq!(field.as_deref(), Some("output"));
            },
            _ => panic!("Expected StepOutput"),
        }
    }

    // Tests for template bindings

    #[test]
    fn test_workflow_step_with_template_binding() {
        let step = WorkflowStep::new("read", ToolHandle::new("read"))
            .with_resource("docs://{doc_id}")
            .expect("Valid resource URI")
            .with_template_binding("doc_id", DataSource::from_step_field("query", "id"));

        assert_eq!(step.template_bindings().len(), 1);
        assert!(step.template_bindings().contains_key("doc_id"));

        let binding = step.template_bindings().get("doc_id").unwrap();
        match binding {
            DataSource::StepOutput { step, field } => {
                assert_eq!(step.as_str(), "query");
                assert_eq!(field.as_deref(), Some("id"));
            },
            _ => panic!("Expected StepOutput"),
        }
    }

    #[test]
    fn test_workflow_step_with_multiple_template_bindings() {
        let step = WorkflowStep::new("read", ToolHandle::new("read"))
            .with_resource("project://{org}/{repo}/config")
            .expect("Valid resource URI")
            .with_template_binding(
                "org",
                DataSource::from_step_field("project", "organization"),
            )
            .with_template_binding("repo", DataSource::from_step_field("project", "repository"));

        assert_eq!(step.template_bindings().len(), 2);
        assert!(step.template_bindings().contains_key("org"));
        assert!(step.template_bindings().contains_key("repo"));
    }

    #[test]
    fn test_workflow_step_template_binding_from_prompt_arg() {
        let step = WorkflowStep::new("read", ToolHandle::new("read"))
            .with_resource("dataset://{dataset_id}")
            .expect("Valid resource URI")
            .with_template_binding("dataset_id", DataSource::prompt_arg("dataset"));

        let binding = step.template_bindings().get("dataset_id").unwrap();
        match binding {
            DataSource::PromptArg(arg_name) => {
                assert_eq!(arg_name.as_str(), "dataset");
            },
            _ => panic!("Expected PromptArg"),
        }
    }

    #[test]
    fn test_workflow_step_template_binding_from_constant() {
        let step = WorkflowStep::new("read", ToolHandle::new("read"))
            .with_resource("api://{version}/endpoint")
            .expect("Valid resource URI")
            .with_template_binding("version", DataSource::constant(json!("v1")));

        let binding = step.template_bindings().get("version").unwrap();
        match binding {
            DataSource::Constant(val) => {
                assert_eq!(val, &json!("v1"));
            },
            _ => panic!("Expected Constant"),
        }
    }

    #[test]
    fn test_workflow_step_validation_with_template_bindings() {
        let step = WorkflowStep::new("read", ToolHandle::new("read"))
            .with_resource("docs://{doc_id}")
            .expect("Valid resource URI")
            .with_template_binding("doc_id", DataSource::from_step_field("query", "id"));

        // Validation should pass when binding is available
        let available = vec![BindingName::new("query")];
        assert!(step.validate(&available).is_ok());

        // Validation should fail when binding is not available
        let empty: Vec<BindingName> = vec![];
        let result = step.validate(&empty);
        assert!(result.is_err());
    }

    #[test]
    fn test_workflow_step_empty_template_bindings() {
        let step = WorkflowStep::new("read", ToolHandle::new("read"))
            .with_resource("docs://static-page")
            .expect("Valid resource URI");

        assert_eq!(step.template_bindings().len(), 0);
        assert!(step.template_bindings().is_empty());
    }

    #[test]
    fn test_workflow_step_chainable_with_template_bindings() {
        let step = WorkflowStep::new("complex", ToolHandle::new("tool"))
            .arg("input", DataSource::prompt_arg("data"))
            .with_resource("guide://{guide_id}")
            .expect("Valid resource URI")
            .with_template_binding("guide_id", DataSource::from_step_field("config", "guide"))
            .with_guidance("Follow the guide carefully")
            .bind("result");

        assert_eq!(step.name().as_str(), "complex");
        assert_eq!(step.arguments().len(), 1);
        assert_eq!(step.resources().len(), 1);
        assert_eq!(step.template_bindings().len(), 1);
        assert!(step.guidance().is_some());
        assert!(step.binding().is_some());
    }

    // Tests for resource-only steps

    #[test]
    fn test_resource_only_step_creation() {
        let step = WorkflowStep::fetch_resources("fetch_guide")
            .with_resource("docs://guide/intro")
            .expect("Valid resource URI");

        assert_eq!(step.name().as_str(), "fetch_guide");
        assert!(step.tool().is_none());
        assert!(step.is_resource_only());
        assert_eq!(step.resources().len(), 1);
        assert!(step.arguments().is_empty());
        assert!(step.binding().is_none());
    }

    #[test]
    fn test_resource_only_step_with_template_bindings() {
        let step = WorkflowStep::fetch_resources("fetch_walkthrough")
            .with_resource("game://walkthrough/{game_id}")
            .expect("Valid resource URI")
            .with_template_binding(
                "game_id",
                DataSource::from_step_field("progress", "game_id"),
            );

        assert!(step.is_resource_only());
        assert_eq!(step.template_bindings().len(), 1);
        assert!(step.template_bindings().contains_key("game_id"));

        // Validation should pass when binding is available
        let available = vec![BindingName::new("progress")];
        assert!(step.validate(&available).is_ok());
    }

    #[test]
    fn test_resource_only_step_with_multiple_resources() {
        let step = WorkflowStep::fetch_resources("fetch_docs")
            .with_resource("docs://guide/intro")
            .expect("Valid resource URI")
            .with_resource("docs://guide/advanced")
            .expect("Valid resource URI");

        assert!(step.is_resource_only());
        assert_eq!(step.resources().len(), 2);
    }

    #[test]
    fn test_resource_only_step_validation_requires_resource() {
        let step = WorkflowStep::fetch_resources("fetch_nothing");

        let result = step.validate(&[]);
        assert!(result.is_err());
        match result {
            Err(WorkflowError::InvalidMapping { step: s, reason }) => {
                assert_eq!(s, "fetch_nothing");
                assert!(reason.contains("at least one resource"));
            },
            _ => panic!("Expected InvalidMapping error"),
        }
    }

    #[test]
    fn test_resource_only_step_validation_rejects_tool_arguments() {
        let step = WorkflowStep::fetch_resources("bad_step")
            .with_resource("docs://guide")
            .expect("Valid resource URI")
            .arg("invalid", DataSource::prompt_arg("arg"));

        let result = step.validate(&[]);
        assert!(result.is_err());
        match result {
            Err(WorkflowError::InvalidMapping { step: s, reason }) => {
                assert_eq!(s, "bad_step");
                assert!(reason.contains("cannot have tool arguments"));
            },
            _ => panic!("Expected InvalidMapping error"),
        }
    }

    #[test]
    fn test_resource_only_step_validation_rejects_binding() {
        let step = WorkflowStep::fetch_resources("bad_step")
            .with_resource("docs://guide")
            .expect("Valid resource URI")
            .bind("output");

        let result = step.validate(&[]);
        assert!(result.is_err());
        match result {
            Err(WorkflowError::InvalidMapping { step: s, reason }) => {
                assert_eq!(s, "bad_step");
                assert!(reason.contains("cannot have output bindings"));
            },
            _ => panic!("Expected InvalidMapping error"),
        }
    }

    #[test]
    fn test_resource_only_step_with_guidance() {
        let step = WorkflowStep::fetch_resources("fetch_guide")
            .with_resource("docs://guide/intro")
            .expect("Valid resource URI")
            .with_guidance("I'll fetch the introductory guide for you...");

        assert!(step.is_resource_only());
        assert!(step.guidance().is_some());
        assert_eq!(
            step.guidance().unwrap(),
            "I'll fetch the introductory guide for you..."
        );
    }

    #[test]
    fn test_workflow_step_is_send_sync_with_optional_tool() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<WorkflowStep>();
    }
}
