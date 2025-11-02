use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

/// Represents a test scenario file that defines a sequence of MCP operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestScenario {
    /// Name of the test scenario
    pub name: String,

    /// Description of what the scenario tests
    pub description: Option<String>,

    /// Timeout for the entire scenario (in seconds)
    #[serde(default = "default_timeout")]
    pub timeout: u64,

    /// Whether to stop on first failure
    #[serde(default = "default_stop_on_failure")]
    pub stop_on_failure: bool,

    /// Variables that can be used in the steps
    #[serde(default)]
    pub variables: HashMap<String, Value>,

    /// Setup steps to run before the test
    #[serde(default)]
    pub setup: Vec<TestStep>,

    /// The main test steps
    pub steps: Vec<TestStep>,

    /// Cleanup steps to run after the test
    #[serde(default)]
    pub cleanup: Vec<TestStep>,
}

/// Represents a single test step in a scenario
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestStep {
    /// Name/description of this step
    pub name: String,

    /// The type of operation
    pub operation: Operation,

    /// Optional timeout for this specific step (in seconds)
    pub timeout: Option<u64>,

    /// Whether to continue if this step fails
    #[serde(default)]
    pub continue_on_failure: bool,

    /// Store the result in a variable for later use
    pub store_result: Option<String>,

    /// Assertions to validate the response
    #[serde(default)]
    pub assertions: Vec<Assertion>,
}

/// Types of operations that can be performed
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Operation {
    /// Call a tool with arguments
    #[serde(rename = "tool_call")]
    ToolCall {
        tool: String,
        #[serde(default)]
        arguments: Value,
    },

    /// List available tools
    #[serde(rename = "list_tools")]
    ListTools,

    /// List available resources
    #[serde(rename = "list_resources")]
    ListResources,

    /// Read a resource
    #[serde(rename = "read_resource")]
    ReadResource { uri: String },

    /// List available prompts
    #[serde(rename = "list_prompts")]
    ListPrompts,

    /// Get a prompt
    #[serde(rename = "get_prompt")]
    GetPrompt {
        name: String,
        #[serde(default)]
        arguments: Value,
    },

    /// Send a custom JSON-RPC request
    #[serde(rename = "custom")]
    Custom {
        method: String,
        #[serde(default)]
        params: Value,
    },

    /// Wait for a specified duration
    #[serde(rename = "wait")]
    Wait { seconds: f64 },

    /// Set a variable
    #[serde(rename = "set_variable")]
    SetVariable { name: String, value: Value },
}

/// Types of assertions that can be made on responses
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Assertion {
    /// Check if a field equals a specific value
    #[serde(rename = "equals")]
    Equals {
        path: String,
        value: Value,
        #[serde(default)]
        ignore_case: bool,
    },

    /// Check if a field contains a substring
    #[serde(rename = "contains")]
    Contains {
        path: String,
        value: String,
        #[serde(default)]
        ignore_case: bool,
    },

    /// Check if a field matches a regex pattern
    #[serde(rename = "matches")]
    Matches { path: String, pattern: String },

    /// Check if a field exists (is not null/undefined)
    #[serde(rename = "exists")]
    Exists { path: String },

    /// Check if a field does not exist (is null/undefined)
    #[serde(rename = "not_exists")]
    NotExists { path: String },

    /// Check if response indicates success (no error field)
    #[serde(rename = "success")]
    Success,

    /// Check if response indicates failure (has error field)
    #[serde(rename = "failure")]
    Failure,

    /// Check array length
    #[serde(rename = "array_length")]
    ArrayLength {
        path: String,
        #[serde(flatten)]
        comparison: Comparison,
    },

    /// Check numeric value
    #[serde(rename = "numeric")]
    Numeric {
        path: String,
        #[serde(flatten)]
        comparison: Comparison,
    },

    /// Custom JSONPath assertion
    #[serde(rename = "jsonpath")]
    JsonPath {
        expression: String,
        expected: Option<Value>,
    },
}

/// Numeric comparison operators
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparison {
    Equals(f64),
    NotEquals(f64),
    GreaterThan(f64),
    GreaterThanOrEqual(f64),
    LessThan(f64),
    LessThanOrEqual(f64),
    Between { min: f64, max: f64 },
}

/// Result of running a test scenario
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioResult {
    pub scenario_name: String,
    pub success: bool,
    pub duration: Duration,
    pub steps_completed: usize,
    pub steps_total: usize,
    pub step_results: Vec<StepResult>,
    pub error: Option<String>,
}

/// Result of running a single test step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_name: String,
    pub success: bool,
    pub duration: Duration,
    pub response: Option<Value>,
    pub assertion_results: Vec<AssertionResult>,
    pub error: Option<String>,
}

/// Result of evaluating an assertion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    pub assertion: String,
    pub passed: bool,
    pub actual_value: Option<Value>,
    pub expected_value: Option<Value>,
    pub message: Option<String>,
}

impl TestScenario {
    /// Load a test scenario from a YAML file
    pub fn from_yaml_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read scenario file: {:?}", path.as_ref()))?;
        serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse YAML scenario: {:?}", path.as_ref()))
    }

    /// Load a test scenario from a JSON file
    pub fn from_json_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read scenario file: {:?}", path.as_ref()))?;
        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse JSON scenario: {:?}", path.as_ref()))
    }

    /// Load a test scenario from a file (auto-detect format)
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();
        match path_ref.extension().and_then(|s| s.to_str()) {
            Some("yaml") | Some("yml") => Self::from_yaml_file(path),
            Some("json") => Self::from_json_file(path),
            _ => {
                // Try YAML first, then JSON
                Self::from_yaml_file(path_ref)
                    .or_else(|_| Self::from_json_file(path_ref))
                    .context("Failed to parse scenario file as YAML or JSON")
            },
        }
    }

    /// Validate the scenario structure
    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            anyhow::bail!("Scenario name cannot be empty");
        }

        if self.steps.is_empty() {
            anyhow::bail!("Scenario must have at least one step");
        }

        // Validate variable references
        for step in &self.steps {
            if let Some(var_name) = &step.store_result {
                if var_name.is_empty() {
                    anyhow::bail!("Variable name for storing result cannot be empty");
                }
            }
        }

        Ok(())
    }
}

fn default_timeout() -> u64 {
    60
}

fn default_stop_on_failure() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_scenario() {
        let yaml = r#"
name: Simple Tool Test
description: Test basic tool functionality
steps:
  - name: List available tools
    operation:
      type: list_tools
    assertions:
      - type: success
      - type: exists
        path: tools
        
  - name: Call echo tool
    operation:
      type: tool_call
      tool: echo
      arguments:
        message: "Hello, World!"
    assertions:
      - type: success
      - type: contains
        path: result
        value: "Hello, World!"
"#;

        let scenario: TestScenario = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(scenario.name, "Simple Tool Test");
        assert_eq!(scenario.steps.len(), 2);
        scenario.validate().unwrap();
    }

    #[test]
    fn test_parse_complex_scenario() {
        let yaml = r#"
name: Complex Scenario
timeout: 120
variables:
  test_message: "Test message"
  expected_count: 5

setup:
  - name: Initialize test data
    operation:
      type: set_variable
      name: test_id
      value: "test_123"

steps:
  - name: Test with variable
    operation:
      type: tool_call
      tool: process
      arguments:
        id: "${test_id}"
        message: "${test_message}"
    store_result: process_result
    assertions:
      - type: success
      - type: numeric
        path: count
        greater_than_or_equal: 5

cleanup:
  - name: Clean up test data
    operation:
      type: tool_call
      tool: cleanup
      arguments:
        id: "${test_id}"
"#;

        let scenario: TestScenario = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(scenario.name, "Complex Scenario");
        assert_eq!(scenario.timeout, 120);
        assert_eq!(scenario.setup.len(), 1);
        assert_eq!(scenario.steps.len(), 1);
        assert_eq!(scenario.cleanup.len(), 1);
        scenario.validate().unwrap();
    }
}
