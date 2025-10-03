//! Workflow step with chainable builder
//!
//! Provides a type-safe, ergonomic API for building workflow steps.

use super::{
    data_source::DataSource,
    error::WorkflowError,
    handles::ToolHandle,
    newtypes::{ArgName, BindingName, StepName},
};
use indexmap::IndexMap;

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

    /// Validate the step
    ///
    /// Checks that all referenced bindings are available
    pub fn validate(&self, available_bindings: &[BindingName]) -> Result<(), WorkflowError> {
        // Check that all step output references exist
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
