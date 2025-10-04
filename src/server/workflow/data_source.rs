//! Data source types for workflow argument mapping
//!
//! Defines where workflow step arguments get their values from.

use super::newtypes::{ArgName, BindingName};
use serde_json::Value;

/// Source of data for workflow step arguments
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum DataSource {
    /// Value from prompt arguments (user-provided)
    PromptArg(ArgName),

    /// Value from a previous step's output (by binding name)
    ///
    /// The `step` field refers to the **binding name** set via `.bind()`,
    /// not the step name passed to `WorkflowStep::new()`.
    StepOutput {
        /// The binding name of the step whose output to use
        step: BindingName,
        /// Optional field to extract from step output
        /// If None, use entire output
        field: Option<String>,
    },

    /// Constant value
    Constant(Value),
}

impl DataSource {
    /// Create a data source from a prompt argument
    pub fn prompt_arg(name: impl Into<ArgName>) -> Self {
        Self::PromptArg(name.into())
    }

    /// Create a data source from a step's entire output (by binding name)
    ///
    /// The `step` parameter should be the **binding name** set via `.bind()`,
    /// not the step name passed to `WorkflowStep::new()`.
    pub fn from_step(step: impl Into<BindingName>) -> Self {
        Self::StepOutput {
            step: step.into(),
            field: None,
        }
    }

    /// Create a data source from a step's output field (by binding name)
    ///
    /// The `step` parameter should be the **binding name** set via `.bind()`,
    /// not the step name passed to `WorkflowStep::new()`.
    pub fn from_step_field(step: impl Into<BindingName>, field: impl Into<String>) -> Self {
        Self::StepOutput {
            step: step.into(),
            field: Some(field.into()),
        }
    }

    /// Create a constant data source
    pub fn constant(value: Value) -> Self {
        Self::Constant(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_prompt_arg_source() {
        let source = DataSource::prompt_arg("input");
        match source {
            DataSource::PromptArg(name) => {
                assert_eq!(name.as_str(), "input");
            },
            _ => panic!("Expected PromptArg variant"),
        }
    }

    #[test]
    fn test_from_step_source() {
        let source = DataSource::from_step("step1");
        match source {
            DataSource::StepOutput { step, field } => {
                assert_eq!(step.as_str(), "step1");
                assert!(field.is_none());
            },
            _ => panic!("Expected StepOutput variant"),
        }
    }

    #[test]
    fn test_from_step_field_source() {
        let source = DataSource::from_step_field("step1", "result");
        match source {
            DataSource::StepOutput { step, field } => {
                assert_eq!(step.as_str(), "step1");
                assert_eq!(field.as_deref(), Some("result"));
            },
            _ => panic!("Expected StepOutput variant"),
        }
    }

    #[test]
    fn test_constant_source() {
        let value = json!({"key": "value"});
        let source = DataSource::constant(value.clone());
        match source {
            DataSource::Constant(v) => {
                assert_eq!(v, value);
            },
            _ => panic!("Expected Constant variant"),
        }
    }

    #[test]
    fn test_constant_with_primitives() {
        let source = DataSource::constant(json!(42));
        assert!(matches!(source, DataSource::Constant(_)));

        let source = DataSource::constant(json!("hello"));
        assert!(matches!(source, DataSource::Constant(_)));

        let source = DataSource::constant(json!(true));
        assert!(matches!(source, DataSource::Constant(_)));
    }

    #[test]
    fn test_data_source_equality() {
        let s1 = DataSource::prompt_arg("input");
        let s2 = DataSource::prompt_arg("input");
        let s3 = DataSource::prompt_arg("output");

        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
    }

    #[test]
    fn test_data_source_clone() {
        let source = DataSource::from_step_field("step1", "result");
        let cloned = source.clone();
        assert_eq!(source, cloned);
    }

    #[test]
    fn test_data_source_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DataSource>();
    }

    #[test]
    fn test_data_source_debug() {
        let source = DataSource::prompt_arg("input");
        let debug = format!("{:?}", source);
        assert!(debug.contains("PromptArg"));
    }
}
