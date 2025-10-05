//! DSL helpers for ergonomic workflow construction
//!
//! Provides convenient functions to reduce boilerplate when building workflows.

use super::{
    data_source::DataSource,
    newtypes::{ArgName, BindingName},
};
use serde_json::Value;

/// Create a data source from a prompt argument
///
/// # Example
/// ```
/// use pmcp::server::workflow::dsl::prompt_arg;
///
/// let source = prompt_arg("user_name");
/// ```
pub fn prompt_arg(name: impl Into<ArgName>) -> DataSource {
    DataSource::prompt_arg(name)
}

/// Create a data source from a step's entire output
///
/// The `step` parameter should be the **binding name** set via `.bind()`,
/// not the step name passed to `WorkflowStep::new()`.
///
/// # Example
/// ```
/// use pmcp::server::workflow::dsl::from_step;
///
/// // If you have: WorkflowStep::new("my_step", ...).bind("output")
/// // Reference it as:
/// let source = from_step("output");  // Use binding name, not "my_step"
/// ```
pub fn from_step(step: impl Into<BindingName>) -> DataSource {
    DataSource::from_step(step)
}

/// Create a data source from a step's output field
///
/// The `step` parameter should be the **binding name** set via `.bind()`,
/// not the step name passed to `WorkflowStep::new()`.
///
/// # Example
/// ```
/// use pmcp::server::workflow::dsl::field;
///
/// // If you have: WorkflowStep::new("my_step", ...).bind("output")
/// // Reference a field as:
/// let source = field("output", "result");  // Use binding name, not "my_step"
/// ```
pub fn field(step: impl Into<BindingName>, field: impl Into<String>) -> DataSource {
    DataSource::from_step_field(step, field)
}

/// Create a constant data source
///
/// # Example
/// ```
/// use pmcp::server::workflow::dsl::constant;
/// use serde_json::json;
///
/// let source = constant(json!("Hello"));
/// ```
pub fn constant(value: Value) -> DataSource {
    DataSource::constant(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_prompt_arg_helper() {
        let source = prompt_arg("input");
        match source {
            DataSource::PromptArg(name) => {
                assert_eq!(name.as_str(), "input");
            },
            _ => panic!("Expected PromptArg"),
        }
    }

    #[test]
    fn test_from_step_helper() {
        let source = from_step("step1");
        match source {
            DataSource::StepOutput { step, field } => {
                assert_eq!(step.as_str(), "step1");
                assert!(field.is_none());
            },
            _ => panic!("Expected StepOutput"),
        }
    }

    #[test]
    fn test_field_helper() {
        let source = field("step1", "output");
        match source {
            DataSource::StepOutput { step, field: f } => {
                assert_eq!(step.as_str(), "step1");
                assert_eq!(f.as_deref(), Some("output"));
            },
            _ => panic!("Expected StepOutput"),
        }
    }

    #[test]
    fn test_constant_helper() {
        let source = constant(json!({"key": "value"}));
        match source {
            DataSource::Constant(v) => {
                assert_eq!(v, json!({"key": "value"}));
            },
            _ => panic!("Expected Constant"),
        }
    }

    #[test]
    fn test_constant_with_primitives() {
        let s1 = constant(json!(42));
        let s2 = constant(json!("hello"));
        let s3 = constant(json!(true));

        assert!(matches!(s1, DataSource::Constant(_)));
        assert!(matches!(s2, DataSource::Constant(_)));
        assert!(matches!(s3, DataSource::Constant(_)));
    }

    #[test]
    fn test_dsl_in_workflow_step() {
        use crate::server::workflow::{ToolHandle, WorkflowStep};

        let step = WorkflowStep::new("step1", ToolHandle::new("create"))
            .arg("topic", prompt_arg("topic"))
            .arg("style", constant(json!("formal")))
            .arg("previous", from_step("step0"))
            .arg("specific", field("step0", "title"))
            .bind("result");

        assert_eq!(step.arguments().len(), 4);
    }
}
