//! Error types for workflow system

use thiserror::Error;

/// Errors that can occur during workflow construction and execution
#[derive(Error, Debug)]
pub enum WorkflowError {
    /// A step references a binding that hasn't been defined yet
    #[error("Step '{step}' references unknown binding '{binding}'")]
    UnknownBinding {
        /// The step that has the invalid reference
        step: String,
        /// The binding name that was not found
        binding: String,
    },

    /// A workflow requires a tool that is not registered
    #[error("Workflow '{workflow}' requires unregistered tool '{tool}'")]
    MissingTool {
        /// The workflow that requires the tool
        workflow: String,
        /// The tool name that was not found
        tool: String,
    },

    /// A workflow requires a resource that is not registered
    #[error("Workflow '{workflow}' requires unregistered resource '{resource}'")]
    MissingResource {
        /// The workflow that requires the resource
        workflow: String,
        /// The resource URI that was not found
        resource: String,
    },

    /// Steps have circular dependencies that prevent execution
    #[error("Circular dependency detected in workflow: {cycle}")]
    CircularDependency {
        /// Description of the circular dependency
        cycle: String,
    },

    /// Invalid argument mapping in a workflow step
    #[error("Invalid argument mapping in step '{step}': {reason}")]
    InvalidMapping {
        /// The step with invalid mapping
        step: String,
        /// Why the mapping is invalid
        reason: String,
    },

    /// A resource URI is not in a valid format
    #[error("Invalid URI '{uri}': must start with 'resource://' or 'file://'")]
    InvalidUri {
        /// The invalid URI
        uri: String,
    },

    /// A workflow is missing a required field
    #[error("Workflow '{workflow}' is missing required field: {field}")]
    MissingField {
        /// The workflow that is missing the field
        workflow: String,
        /// The name of the missing field
        field: &'static str,
    },

    /// A step's output field could not be found
    #[error("Step '{step}' output field '{field}' not found in result")]
    OutputFieldNotFound {
        /// The step whose output was queried
        step: String,
        /// The field that was not found
        field: String,
    },

    /// A wrapped error from another part of the system
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

// Convert from crate::Error to WorkflowError
impl From<crate::Error> for WorkflowError {
    fn from(err: crate::Error) -> Self {
        Self::Other(Box::new(err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unknown_binding_error() {
        let err = WorkflowError::UnknownBinding {
            step: "step1".to_string(),
            binding: "result".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("step1"));
        assert!(msg.contains("result"));
        assert!(msg.contains("unknown binding"));
    }

    #[test]
    fn test_missing_tool_error() {
        let err = WorkflowError::MissingTool {
            workflow: "my_workflow".to_string(),
            tool: "greet".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("my_workflow"));
        assert!(msg.contains("greet"));
        assert!(msg.contains("unregistered tool"));
    }

    #[test]
    fn test_missing_resource_error() {
        let err = WorkflowError::MissingResource {
            workflow: "my_workflow".to_string(),
            resource: "resource://test/guide".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("my_workflow"));
        assert!(msg.contains("resource://test/guide"));
        assert!(msg.contains("unregistered resource"));
    }

    #[test]
    fn test_circular_dependency_error() {
        let err = WorkflowError::CircularDependency {
            cycle: "step1 -> step2 -> step1".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Circular dependency"));
        assert!(msg.contains("step1 -> step2 -> step1"));
    }

    #[test]
    fn test_invalid_mapping_error() {
        let err = WorkflowError::InvalidMapping {
            step: "step1".to_string(),
            reason: "field not found".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("step1"));
        assert!(msg.contains("field not found"));
        assert!(msg.contains("Invalid argument mapping"));
    }

    #[test]
    fn test_invalid_uri_error() {
        let err = WorkflowError::InvalidUri {
            uri: "http://example.com".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("http://example.com"));
        assert!(msg.contains("Invalid URI"));
        assert!(msg.contains("resource://"));
        assert!(msg.contains("file://"));
    }

    #[test]
    fn test_missing_field_error() {
        let err = WorkflowError::MissingField {
            workflow: "my_workflow".to_string(),
            field: "name",
        };
        let msg = err.to_string();
        assert!(msg.contains("my_workflow"));
        assert!(msg.contains("name"));
        assert!(msg.contains("missing required field"));
    }

    #[test]
    fn test_output_field_not_found_error() {
        let err = WorkflowError::OutputFieldNotFound {
            step: "step1".to_string(),
            field: "result".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("step1"));
        assert!(msg.contains("result"));
        assert!(msg.contains("not found"));
    }

    #[test]
    fn test_error_conversion_from_crate_error() {
        let crate_err = crate::Error::validation("test error");
        let workflow_err: WorkflowError = crate_err.into();

        match workflow_err {
            WorkflowError::Other(_) => {},
            _ => panic!("Expected Other variant"),
        }
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<WorkflowError>();
    }

    #[test]
    fn test_error_debug_output() {
        let err = WorkflowError::MissingTool {
            workflow: "test".to_string(),
            tool: "greet".to_string(),
        };
        let debug = format!("{:?}", err);
        assert!(debug.contains("MissingTool"));
    }
}
