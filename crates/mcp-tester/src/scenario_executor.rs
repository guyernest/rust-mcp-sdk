use anyhow::Result;
use colored::*;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::time::sleep;

use crate::scenario::{
    Assertion, AssertionResult, Comparison, Operation, ScenarioResult, StepResult, TestScenario,
    TestStep,
};
use crate::tester::ServerTester;

/// Executes test scenarios against an MCP server
pub struct ScenarioExecutor<'a> {
    tester: &'a mut ServerTester,
    variables: HashMap<String, Value>,
    verbose: bool,
}

impl<'a> ScenarioExecutor<'a> {
    pub fn new(tester: &'a mut ServerTester, verbose: bool) -> Self {
        Self {
            tester,
            variables: HashMap::new(),
            verbose,
        }
    }

    /// Execute a test scenario
    pub async fn execute(&mut self, scenario: TestScenario) -> Result<ScenarioResult> {
        let start = Instant::now();

        // Validate scenario first
        scenario.validate()?;

        if self.verbose {
            println!(
                "\n{}",
                format!("Executing scenario: {}", scenario.name)
                    .cyan()
                    .bold()
            );
            if let Some(desc) = &scenario.description {
                println!("  {}", desc);
            }
            println!();
        }

        // Initialize variables from scenario
        self.variables = scenario.variables.clone();

        let mut step_results = Vec::new();
        let mut steps_completed = 0;
        let total_steps = scenario.setup.len() + scenario.steps.len() + scenario.cleanup.len();
        let mut scenario_error = None;

        // Execute setup steps
        if !scenario.setup.is_empty() {
            if self.verbose {
                println!("{}", "Setup:".yellow());
            }
            for step in scenario.setup {
                let result = self.execute_step(&step).await?;
                let success = result.success;
                step_results.push(result);
                steps_completed += 1;

                if !success && scenario.stop_on_failure && !step.continue_on_failure {
                    scenario_error = Some("Setup step failed".to_string());
                    break;
                }
            }
        }

        // Execute main steps if setup succeeded
        if scenario_error.is_none() {
            if self.verbose && !scenario.steps.is_empty() {
                println!("\n{}", "Test Steps:".green());
            }

            for step in &scenario.steps {
                let result = self.execute_step(step).await?;
                let success = result.success;
                step_results.push(result);
                steps_completed += 1;

                if !success && scenario.stop_on_failure && !step.continue_on_failure {
                    scenario_error = Some(format!("Step '{}' failed", step.name));
                    break;
                }
            }
        }

        // Always run cleanup steps
        if !scenario.cleanup.is_empty() {
            if self.verbose {
                println!("\n{}", "Cleanup:".yellow());
            }
            for step in scenario.cleanup {
                // Cleanup steps should continue even on failure
                let mut cleanup_step = step;
                cleanup_step.continue_on_failure = true;
                let result = self.execute_step(&cleanup_step).await?;
                step_results.push(result);
                steps_completed += 1;
            }
        }

        let success = scenario_error.is_none()
            && step_results.iter().all(|r| {
                r.success
                    || scenario
                        .steps
                        .iter()
                        .any(|step| step.name == r.step_name && step.continue_on_failure)
            });

        let result = ScenarioResult {
            scenario_name: scenario.name,
            success,
            duration: start.elapsed(),
            steps_completed,
            steps_total: total_steps,
            step_results,
            error: scenario_error,
        };

        if self.verbose {
            self.print_summary(&result);
        }

        Ok(result)
    }

    /// Execute a single test step
    async fn execute_step(&mut self, step: &TestStep) -> Result<StepResult> {
        let start = Instant::now();

        if self.verbose {
            print!("  {} {}... ", "→".cyan(), step.name);
        }

        // Apply timeout if specified
        let timeout = Duration::from_secs(step.timeout.unwrap_or(30));

        // Execute the operation
        let response =
            match tokio::time::timeout(timeout, self.execute_operation(&step.operation)).await {
                Ok(Ok(resp)) => Some(resp),
                Ok(Err(e)) => {
                    let result = StepResult {
                        step_name: step.name.clone(),
                        success: false,
                        duration: start.elapsed(),
                        response: None,
                        assertion_results: vec![],
                        error: Some(e.to_string()),
                    };

                    if self.verbose {
                        println!("{} ({})", "FAILED".red(), e);
                    }

                    return Ok(result);
                },
                Err(_) => {
                    let result = StepResult {
                        step_name: step.name.clone(),
                        success: false,
                        duration: start.elapsed(),
                        response: None,
                        assertion_results: vec![],
                        error: Some(format!("Timeout after {:?}", timeout)),
                    };

                    if self.verbose {
                        println!("{} (timeout)", "FAILED".red());
                    }

                    return Ok(result);
                },
            };

        // Dump full response when verbose mode is enabled
        if self.verbose {
            if let Some(ref resp) = response {
                println!();
                println!("      {} Response:", "↳".cyan());
                // Pretty-print JSON with indentation for readability
                if let Ok(formatted) = serde_json::to_string_pretty(resp) {
                    for line in formatted.lines() {
                        println!("        {}", line.dimmed());
                    }
                }
            }
        }

        // Store result if requested
        if let Some(var_name) = &step.store_result {
            if let Some(ref resp) = response {
                self.variables.insert(var_name.clone(), resp.clone());
            }
        }

        // Run assertions
        let assertion_results = if let Some(ref resp) = response {
            self.run_assertions(&step.assertions, resp).await
        } else {
            vec![]
        };

        let success = assertion_results.iter().all(|a| a.passed);

        if self.verbose {
            if success {
                println!(
                    "{} ({:.2}s)",
                    "PASSED".green(),
                    start.elapsed().as_secs_f64()
                );
            } else {
                println!("{} ({:.2}s)", "FAILED".red(), start.elapsed().as_secs_f64());
                for assertion in &assertion_results {
                    if !assertion.passed {
                        println!(
                            "      {} Assertion failed: {}",
                            "✗".red(),
                            assertion.message.as_ref().unwrap_or(&assertion.assertion)
                        );
                        // Show actual vs expected for easier debugging
                        if let Some(ref expected) = assertion.expected_value {
                            println!(
                                "        {} {}",
                                "Expected:".yellow(),
                                format_value_compact(expected)
                            );
                        }
                        if let Some(ref actual) = assertion.actual_value {
                            println!(
                                "        {} {}",
                                "Actual:".yellow(),
                                format_value_compact(actual)
                            );
                        } else {
                            println!("        {} null (path not found)", "Actual:".yellow());
                        }
                    }
                }
            }
        }

        Ok(StepResult {
            step_name: step.name.clone(),
            success,
            duration: start.elapsed(),
            response,
            assertion_results,
            error: None,
        })
    }

    /// Execute an operation and return the response
    async fn execute_operation(&mut self, operation: &Operation) -> Result<Value> {
        // Substitute variables in the operation
        let operation = self.substitute_variables_in_operation(operation)?;

        match operation {
            Operation::ToolCall { tool, arguments } => {
                // Call the tool directly to get raw response for assertions
                match self.tester.transport_type {
                    crate::tester::TransportType::Http
                    | crate::tester::TransportType::JsonRpcHttp => {
                        // For both HTTP and JSON-RPC transports, call the tool and extract content
                        let call_result = self.tester.call_tool_raw(&tool, arguments).await;

                        match call_result {
                            Ok(result) => {
                                // Extract the text content from the response
                                let content_text = result
                                    .content
                                    .into_iter()
                                    .filter_map(|c| match c {
                                        pmcp::types::Content::Text { text } => Some(text),
                                        _ => None,
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n");

                                // Check if the content indicates an error
                                if content_text.starts_with("Error:") || result.is_error {
                                    Ok(json!({
                                        "success": false,
                                        "result": content_text,
                                        "parsed": null,
                                        "error": content_text
                                    }))
                                } else {
                                    // Try to parse the content as JSON for easier assertions
                                    // This allows using paths like "parsed.result" instead of string matching
                                    let parsed: Option<Value> =
                                        serde_json::from_str(&content_text).ok();

                                    Ok(json!({
                                        "success": true,
                                        "result": content_text,
                                        "parsed": parsed,
                                        "error": null
                                    }))
                                }
                            },
                            Err(e) => Ok(json!({
                                "success": false,
                                "result": null,
                                "parsed": null,
                                "error": e.to_string()
                            })),
                        }
                    },
                    crate::tester::TransportType::Stdio => {
                        // Fall back to test_tool for stdio transport
                        let result = self.tester.test_tool(&tool, arguments).await?;
                        // Try to parse the details as JSON if it's a string
                        let parsed: Option<Value> = result
                            .details
                            .as_ref()
                            .and_then(|d| serde_json::from_str(d).ok());

                        Ok(json!({
                            "success": result.status == crate::report::TestStatus::Passed,
                            "result": result.details,
                            "parsed": parsed,
                            "error": result.error
                        }))
                    },
                }
            },

            Operation::ListTools => match self.tester.list_tools().await {
                Ok(tools) => Ok(json!({
                    "success": true,
                    "tools": tools.tools,
                    "error": null
                })),
                Err(e) => Ok(json!({
                    "success": false,
                    "tools": [],
                    "error": e.to_string()
                })),
            },

            Operation::ListResources => match self.tester.list_resources().await {
                Ok(resources) => Ok(json!({
                    "success": true,
                    "resources": resources.resources,
                    "error": null
                })),
                Err(e) => Ok(json!({
                    "success": false,
                    "resources": [],
                    "error": e.to_string()
                })),
            },

            Operation::ReadResource { uri } => match self.tester.read_resource(&uri).await {
                Ok(result) => Ok(json!({
                    "success": true,
                    "contents": result.contents,
                    "error": null
                })),
                Err(e) => Ok(json!({
                    "success": false,
                    "contents": [],
                    "error": e.to_string()
                })),
            },

            Operation::ListPrompts => match self.tester.list_prompts().await {
                Ok(prompts) => Ok(json!({
                    "success": true,
                    "prompts": prompts.prompts,
                    "error": null
                })),
                Err(e) => Ok(json!({
                    "success": false,
                    "prompts": [],
                    "error": e.to_string()
                })),
            },

            Operation::GetPrompt { name, arguments } => {
                match self.tester.get_prompt(&name, arguments).await {
                    Ok(result) => Ok(json!({
                        "success": true,
                        "messages": result.messages,
                        "description": result.description,
                        "error": null
                    })),
                    Err(e) => Ok(json!({
                        "success": false,
                        "messages": [],
                        "description": null,
                        "error": e.to_string()
                    })),
                }
            },

            Operation::Custom { method, params } => {
                self.tester.send_custom_request(&method, params).await
            },

            Operation::Wait { seconds } => {
                sleep(Duration::from_secs_f64(seconds)).await;
                Ok(json!({ "waited": seconds }))
            },

            Operation::SetVariable { name, value } => {
                self.variables.insert(name.clone(), value.clone());
                Ok(json!({ "variable_set": name }))
            },
        }
    }

    /// Substitute variables in operation parameters
    fn substitute_variables_in_operation(&self, operation: &Operation) -> Result<Operation> {
        match operation {
            Operation::ToolCall { tool, arguments } => Ok(Operation::ToolCall {
                tool: self.substitute_string(tool)?,
                arguments: self.substitute_value(arguments)?,
            }),
            Operation::ReadResource { uri } => Ok(Operation::ReadResource {
                uri: self.substitute_string(uri)?,
            }),
            Operation::GetPrompt { name, arguments } => Ok(Operation::GetPrompt {
                name: self.substitute_string(name)?,
                arguments: self.substitute_value(arguments)?,
            }),
            Operation::Custom { method, params } => Ok(Operation::Custom {
                method: self.substitute_string(method)?,
                params: self.substitute_value(params)?,
            }),
            Operation::SetVariable { name, value } => Ok(Operation::SetVariable {
                name: name.clone(),
                value: self.substitute_value(value)?,
            }),
            other => Ok(other.clone()),
        }
    }

    /// Substitute variables in a string value
    fn substitute_string(&self, s: &str) -> Result<String> {
        let mut result = s.to_string();
        let var_regex = Regex::new(r"\$\{([^}]+)\}").unwrap();

        for cap in var_regex.captures_iter(s) {
            let var_name = &cap[1];
            if let Some(value) = self.variables.get(var_name) {
                let value_str = match value {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                result = result.replace(&cap[0], &value_str);
            }
        }

        Ok(result)
    }

    /// Substitute variables in a JSON value
    fn substitute_value(&self, value: &Value) -> Result<Value> {
        match value {
            Value::String(s) => Ok(Value::String(self.substitute_string(s)?)),
            Value::Object(map) => {
                let mut new_map = serde_json::Map::new();
                for (k, v) in map {
                    new_map.insert(k.clone(), self.substitute_value(v)?);
                }
                Ok(Value::Object(new_map))
            },
            Value::Array(arr) => {
                let mut new_arr = Vec::new();
                for v in arr {
                    new_arr.push(self.substitute_value(v)?);
                }
                Ok(Value::Array(new_arr))
            },
            other => Ok(other.clone()),
        }
    }

    /// Run assertions against a response
    async fn run_assertions(
        &self,
        assertions: &[Assertion],
        response: &Value,
    ) -> Vec<AssertionResult> {
        let mut results = Vec::new();

        for assertion in assertions {
            let result = self.evaluate_assertion(assertion, response);
            results.push(result);
        }

        results
    }

    /// Evaluate a single assertion
    fn evaluate_assertion(&self, assertion: &Assertion, response: &Value) -> AssertionResult {
        match assertion {
            Assertion::Equals {
                path,
                value,
                ignore_case,
            } => {
                let actual = self.get_value_at_path(response, path);
                let passed = if *ignore_case {
                    // Case-insensitive comparison for strings
                    match (&actual, value) {
                        (Some(Value::String(a)), Value::String(b)) => {
                            a.to_lowercase() == b.to_lowercase()
                        },
                        _ => actual.as_ref() == Some(&value),
                    }
                } else {
                    actual.as_ref() == Some(&value)
                };

                AssertionResult {
                    assertion: format!("Equals: {} == {:?}", path, value),
                    passed,
                    actual_value: actual.cloned(),
                    expected_value: Some(value.clone()),
                    message: if !passed {
                        Some(format!("Expected {} to equal {:?}", path, value))
                    } else {
                        None
                    },
                }
            },

            Assertion::Contains {
                path,
                value,
                ignore_case,
            } => {
                let actual = self.get_value_at_path(response, path);
                let passed = match actual {
                    Some(Value::String(s)) => {
                        if *ignore_case {
                            s.to_lowercase().contains(&value.to_lowercase())
                        } else {
                            s.contains(value)
                        }
                    },
                    Some(Value::Array(arr)) => arr.iter().any(|v| {
                        if let Value::String(s) = v {
                            if *ignore_case {
                                s.to_lowercase() == value.to_lowercase()
                            } else {
                                s == value
                            }
                        } else {
                            false
                        }
                    }),
                    _ => false,
                };

                AssertionResult {
                    assertion: format!("Contains: {} contains '{}'", path, value),
                    passed,
                    actual_value: actual.cloned(),
                    expected_value: Some(Value::String(value.clone())),
                    message: if !passed {
                        Some(format!("Expected {} to contain '{}'", path, value))
                    } else {
                        None
                    },
                }
            },

            Assertion::Matches { path, pattern } => {
                let regex = match Regex::new(pattern) {
                    Ok(r) => r,
                    Err(e) => {
                        return AssertionResult {
                            assertion: format!("Matches: {} ~ /{}/", path, pattern),
                            passed: false,
                            actual_value: None,
                            expected_value: None,
                            message: Some(format!("Invalid regex pattern: {}", e)),
                        };
                    },
                };

                let actual = self.get_value_at_path(response, path);
                let passed = match actual {
                    Some(Value::String(s)) => regex.is_match(s),
                    _ => false,
                };

                AssertionResult {
                    assertion: format!("Matches: {} ~ /{}/", path, pattern),
                    passed,
                    actual_value: actual.cloned(),
                    expected_value: Some(Value::String(pattern.clone())),
                    message: if !passed {
                        Some(format!("Expected {} to match pattern /{}/", path, pattern))
                    } else {
                        None
                    },
                }
            },

            Assertion::Exists { path } => {
                let actual = self.get_value_at_path(response, path);
                let passed = actual.is_some() && actual != Some(&Value::Null);

                AssertionResult {
                    assertion: format!("Exists: {}", path),
                    passed,
                    actual_value: actual.cloned(),
                    expected_value: None,
                    message: if !passed {
                        Some(format!("Expected {} to exist", path))
                    } else {
                        None
                    },
                }
            },

            Assertion::NotExists { path } => {
                let actual = self.get_value_at_path(response, path);
                let passed = actual.is_none() || actual == Some(&Value::Null);

                AssertionResult {
                    assertion: format!("NotExists: {}", path),
                    passed,
                    actual_value: actual.cloned(),
                    expected_value: None,
                    message: if !passed {
                        Some(format!("Expected {} to not exist", path))
                    } else {
                        None
                    },
                }
            },

            Assertion::Success => {
                let has_error = response
                    .get("error")
                    .and_then(|e| if e.is_null() { None } else { Some(e) })
                    .is_some();
                AssertionResult {
                    assertion: "Success".to_string(),
                    passed: !has_error,
                    actual_value: response.get("error").cloned(),
                    expected_value: None,
                    message: if has_error {
                        Some("Expected successful response without error".to_string())
                    } else {
                        None
                    },
                }
            },

            Assertion::Failure => {
                let has_error = response
                    .get("error")
                    .and_then(|e| if e.is_null() { None } else { Some(e) })
                    .is_some();
                AssertionResult {
                    assertion: "Failure".to_string(),
                    passed: has_error,
                    actual_value: response.get("error").cloned(),
                    expected_value: None,
                    message: if !has_error {
                        Some("Expected failure response with error".to_string())
                    } else {
                        None
                    },
                }
            },

            Assertion::ArrayLength { path, comparison } => {
                let actual = self.get_value_at_path(response, path);
                let length = match actual {
                    Some(Value::Array(arr)) => Some(arr.len() as f64),
                    _ => None,
                };

                let passed = if let Some(len) = length {
                    self.evaluate_comparison(len, comparison)
                } else {
                    false
                };

                AssertionResult {
                    assertion: format!("ArrayLength: {} {:?}", path, comparison),
                    passed,
                    actual_value: length.map(|l| json!(l)),
                    expected_value: None,
                    message: if !passed {
                        Some(format!("Array length assertion failed for {}", path))
                    } else {
                        None
                    },
                }
            },

            Assertion::Numeric { path, comparison } => {
                let actual = self.get_value_at_path(response, path);
                let number = match actual {
                    Some(Value::Number(n)) => n.as_f64(),
                    _ => None,
                };

                let passed = if let Some(num) = number {
                    self.evaluate_comparison(num, comparison)
                } else {
                    false
                };

                AssertionResult {
                    assertion: format!("Numeric: {} {:?}", path, comparison),
                    passed,
                    actual_value: actual.cloned(),
                    expected_value: None,
                    message: if !passed {
                        Some(format!("Numeric assertion failed for {}", path))
                    } else {
                        None
                    },
                }
            },

            Assertion::JsonPath {
                expression,
                expected,
            } => {
                // Note: Full JSONPath support would require an external crate
                // For now, we'll treat it as a simple path
                let actual = self.get_value_at_path(response, expression);
                let passed = match expected {
                    Some(exp) => actual.as_ref() == Some(&exp),
                    None => actual.is_some(),
                };

                AssertionResult {
                    assertion: format!("JsonPath: {}", expression),
                    passed,
                    actual_value: actual.cloned(),
                    expected_value: expected.clone(),
                    message: if !passed {
                        Some(format!("JSONPath assertion failed for {}", expression))
                    } else {
                        None
                    },
                }
            },
        }
    }

    /// Get value at a path in the JSON response
    fn get_value_at_path<'b>(&self, value: &'b Value, path: &str) -> Option<&'b Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = value;

        for part in parts {
            // Handle array indices like "items[0]"
            if let Some(bracket_pos) = part.find('[') {
                let field = &part[..bracket_pos];
                let index_str = &part[bracket_pos + 1..part.len() - 1];

                current = current.get(field)?;
                if let Ok(index) = index_str.parse::<usize>() {
                    current = current.get(index)?;
                } else {
                    return None;
                }
            } else {
                current = current.get(part)?;
            }
        }

        Some(current)
    }

    /// Evaluate a numeric comparison
    fn evaluate_comparison(&self, value: f64, comparison: &Comparison) -> bool {
        match comparison {
            Comparison::Equals(v) => (value - v).abs() < f64::EPSILON,
            Comparison::NotEquals(v) => (value - v).abs() >= f64::EPSILON,
            Comparison::GreaterThan(v) => value > *v,
            Comparison::GreaterThanOrEqual(v) => value >= *v,
            Comparison::LessThan(v) => value < *v,
            Comparison::LessThanOrEqual(v) => value <= *v,
            Comparison::Between { min, max } => value >= *min && value <= *max,
        }
    }

    /// Print a summary of the scenario execution
    fn print_summary(&self, result: &ScenarioResult) {
        println!("\n{}", "─".repeat(60));
        println!("{}", "Scenario Summary:".bold());
        println!("  Name: {}", result.scenario_name);
        println!(
            "  Status: {}",
            if result.success {
                "PASSED".green().bold()
            } else {
                "FAILED".red().bold()
            }
        );
        println!("  Duration: {:.2}s", result.duration.as_secs_f64());
        println!("  Steps: {}/{}", result.steps_completed, result.steps_total);

        if let Some(error) = &result.error {
            println!("  Error: {}", error.red());
        }

        println!("{}", "─".repeat(60));
    }
}

/// Format a JSON value compactly for display in error messages.
/// Truncates long strings and arrays for readability.
fn format_value_compact(value: &Value) -> String {
    match value {
        Value::String(s) => {
            if s.len() > 100 {
                format!("\"{}...\" ({} chars)", &s[..100], s.len())
            } else {
                format!("{:?}", s)
            }
        },
        Value::Array(arr) => {
            if arr.len() > 5 {
                format!("[{} items]", arr.len())
            } else {
                format!("{}", value)
            }
        },
        Value::Object(obj) => {
            let keys: Vec<&str> = obj.keys().take(5).map(|s| s.as_str()).collect();
            if obj.len() > 5 {
                format!("{{{}... ({} keys)}}", keys.join(", "), obj.len())
            } else {
                format!("{}", value)
            }
        },
        _ => format!("{}", value),
    }
}
