//! Domain-specific newtypes for workflow system
//!
//! These newtypes prevent type confusion and encode domain invariants.
//! All use Arc<str> for O(1) cloning.

use super::error::WorkflowError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::Arc;

// Helper module for Arc<str> serialization
mod arc_str_serde {
    use super::{Arc, Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S>(arc: &Arc<str>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(arc)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Arc<str>, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer).map(|s| Arc::from(s.as_str()))
    }
}

/// Workflow step identifier
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StepName(#[serde(with = "arc_str_serde")] Arc<str>);

impl StepName {
    /// Create a new step name
    pub fn new(name: impl AsRef<str>) -> Self {
        Self(Arc::from(name.as_ref()))
    }

    /// Get the step name as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for StepName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for StepName {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for StepName {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// Output variable binding name
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BindingName(#[serde(with = "arc_str_serde")] Arc<str>);

impl BindingName {
    /// Create a new binding name
    pub fn new(name: impl AsRef<str>) -> Self {
        Self(Arc::from(name.as_ref()))
    }

    /// Get the binding name as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for BindingName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for BindingName {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for BindingName {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// Argument name
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ArgName(#[serde(with = "arc_str_serde")] Arc<str>);

impl ArgName {
    /// Create a new argument name
    pub fn new(name: impl AsRef<str>) -> Self {
        Self(Arc::from(name.as_ref()))
    }

    /// Get the argument name as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ArgName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for ArgName {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for ArgName {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// Resource URI with validation
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Uri(#[serde(with = "arc_str_serde")] Arc<str>);

impl Uri {
    /// Create a new URI with validation
    ///
    /// URIs must start with "resource://" or "file://"
    pub fn new(uri: impl AsRef<str>) -> Result<Self, WorkflowError> {
        let uri_str = uri.as_ref();

        // Validate URI format (basic check)
        if !uri_str.starts_with("resource://") && !uri_str.starts_with("file://") {
            return Err(WorkflowError::InvalidUri {
                uri: uri_str.to_string(),
            });
        }

        Ok(Self(Arc::from(uri_str)))
    }

    /// Get the URI as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Uri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_name() {
        let name = StepName::new("step1");
        assert_eq!(name.as_str(), "step1");
        assert_eq!(name.to_string(), "step1");
    }

    #[test]
    fn test_step_name_from_str() {
        let name: StepName = "step1".into();
        assert_eq!(name.as_str(), "step1");
    }

    #[test]
    fn test_binding_name() {
        let binding = BindingName::new("result");
        assert_eq!(binding.as_str(), "result");
    }

    #[test]
    fn test_arg_name() {
        let arg = ArgName::new("input");
        assert_eq!(arg.as_str(), "input");
    }

    #[test]
    fn test_uri_valid() {
        let uri = Uri::new("resource://test/path").unwrap();
        assert_eq!(uri.as_str(), "resource://test/path");

        let uri = Uri::new("file:///path/to/file").unwrap();
        assert_eq!(uri.as_str(), "file:///path/to/file");
    }

    #[test]
    fn test_uri_invalid() {
        let result = Uri::new("http://example.com");
        assert!(result.is_err());

        let result = Uri::new("invalid-uri");
        assert!(result.is_err());
    }

    #[test]
    fn test_newtypes_are_cheap_to_clone() {
        let name1 = StepName::new("step1");
        let name2 = name1.clone();

        // Arc pointer equality - same underlying data
        assert_eq!(name1.as_str(), name2.as_str());
    }

    #[test]
    fn test_step_name_equality() {
        let name1 = StepName::new("step1");
        let name2 = StepName::new("step1");
        let name3 = StepName::new("step2");

        assert_eq!(name1, name2);
        assert_ne!(name1, name3);
    }

    #[test]
    fn test_step_name_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(StepName::new("step1"));
        set.insert(StepName::new("step1")); // Duplicate
        set.insert(StepName::new("step2"));

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_step_name_display() {
        let name = StepName::new("step1");
        assert_eq!(format!("{}", name), "step1");
    }

    #[test]
    fn test_step_name_from_string() {
        let name: StepName = String::from("step1").into();
        assert_eq!(name.as_str(), "step1");
    }

    #[test]
    fn test_binding_name_equality() {
        let b1 = BindingName::new("result");
        let b2 = BindingName::new("result");
        let b3 = BindingName::new("output");

        assert_eq!(b1, b2);
        assert_ne!(b1, b3);
    }

    #[test]
    fn test_binding_name_display() {
        let binding = BindingName::new("result");
        assert_eq!(format!("{}", binding), "result");
    }

    #[test]
    fn test_arg_name_equality() {
        let a1 = ArgName::new("input");
        let a2 = ArgName::new("input");
        let a3 = ArgName::new("output");

        assert_eq!(a1, a2);
        assert_ne!(a1, a3);
    }

    #[test]
    fn test_arg_name_display() {
        let arg = ArgName::new("input");
        assert_eq!(format!("{}", arg), "input");
    }

    #[test]
    fn test_uri_display() {
        let uri = Uri::new("resource://test/path").unwrap();
        assert_eq!(format!("{}", uri), "resource://test/path");
    }

    #[test]
    fn test_uri_equality() {
        let u1 = Uri::new("resource://test/path").unwrap();
        let u2 = Uri::new("resource://test/path").unwrap();
        let u3 = Uri::new("file:///different/path").unwrap();

        assert_eq!(u1, u2);
        assert_ne!(u1, u3);
    }

    #[test]
    fn test_uri_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Uri::new("resource://test/path").unwrap());
        set.insert(Uri::new("resource://test/path").unwrap()); // Duplicate
        set.insert(Uri::new("file:///different").unwrap());

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_uri_validation_http_rejected() {
        let result = Uri::new("http://example.com");
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("http://example.com"));
        }
    }

    #[test]
    fn test_uri_validation_https_rejected() {
        let result = Uri::new("https://example.com");
        assert!(result.is_err());
    }

    #[test]
    fn test_uri_validation_relative_path_rejected() {
        let result = Uri::new("./relative/path");
        assert!(result.is_err());
    }

    #[test]
    fn test_uri_file_scheme_accepted() {
        let uri = Uri::new("file:///absolute/path");
        assert!(uri.is_ok());
    }

    #[test]
    fn test_newtypes_with_empty_strings() {
        let step = StepName::new("");
        assert_eq!(step.as_str(), "");

        let binding = BindingName::new("");
        assert_eq!(binding.as_str(), "");

        let arg = ArgName::new("");
        assert_eq!(arg.as_str(), "");
    }

    #[test]
    fn test_newtypes_with_special_characters() {
        let step = StepName::new("step-1_test.foo");
        assert_eq!(step.as_str(), "step-1_test.foo");

        let binding = BindingName::new("my$result");
        assert_eq!(binding.as_str(), "my$result");
    }

    #[test]
    fn test_newtypes_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<StepName>();
        assert_send_sync::<BindingName>();
        assert_send_sync::<ArgName>();
        assert_send_sync::<Uri>();
    }
}
