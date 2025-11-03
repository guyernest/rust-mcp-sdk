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
#[derive(Clone, Debug)]
pub struct WorkflowStep {
    /// Step identifier
    name: StepName,
    /// Tool to invoke
    tool: ToolHandle,
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
    /// Create a new workflow step
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
            tool,
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

    /// Get tool handle
    pub fn tool(&self) -> &ToolHandle {
        &self.tool
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
    /// Checks that all referenced bindings are available
    pub fn validate(&self, available_bindings: &[BindingName]) -> Result<(), WorkflowError> {
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
        for (_var_name, source) in &self.template_bindings {
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
        assert_eq!(step.tool().name(), "greet");
        assert!(step.arguments().is_empty());
        assert!(step.binding().is_none());
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
}
