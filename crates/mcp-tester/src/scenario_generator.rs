use crate::scenario::{Assertion, Operation, TestScenario, TestStep};
use crate::tester::ServerTester;
use anyhow::Result;
use pmcp::types::{PromptInfo, ResourceInfo, ToolInfo};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;

pub struct ScenarioGenerator {
    server_url: String,
    all_tools: bool,
    with_resources: bool,
    with_prompts: bool,
}

impl ScenarioGenerator {
    pub fn new(
        server_url: String,
        all_tools: bool,
        with_resources: bool,
        with_prompts: bool,
    ) -> Self {
        Self {
            server_url,
            all_tools,
            with_resources,
            with_prompts,
        }
    }

    /// Generate a scenario file from server discovery
    pub async fn generate(&self, tester: &mut ServerTester, output_path: &str) -> Result<()> {
        // Initialize the server
        println!("🔍 Discovering server capabilities...");

        // Initialize to get server info
        let init_result = tester.test_initialize().await;
        if init_result.status != crate::report::TestStatus::Passed {
            anyhow::bail!("Failed to initialize server: {:?}", init_result.error);
        }

        // Get tools
        let tools = if self.all_tools || self.with_resources || self.with_prompts {
            println!("📋 Listing tools...");
            let tools_result = tester.test_tools_list().await;
            if tools_result.status == crate::report::TestStatus::Passed {
                tester.get_tools().cloned()
            } else {
                None
            }
        } else {
            None
        };

        // Get resources if requested
        let resources = if self.with_resources {
            println!("📦 Listing resources...");
            match tester.list_resources().await {
                Ok(result) => Some(result.resources),
                Err(_) => None,
            }
        } else {
            None
        };

        // Get prompts if requested
        let prompts = if self.with_prompts {
            println!("💬 Listing prompts...");
            match tester.list_prompts().await {
                Ok(result) => Some(result.prompts),
                Err(_) => None,
            }
        } else {
            None
        };

        // Generate scenario
        let scenario = self.create_scenario(
            tester.get_server_name(),
            tools.as_ref(),
            resources.as_ref(),
            prompts.as_ref(),
        );

        // Write to file
        let yaml = serde_yaml::to_string(&scenario)?;
        fs::write(output_path, yaml)?;

        println!("✅ Generated scenario file: {}", output_path);
        println!("📝 Please edit the file to:");
        println!("   - Replace placeholder values with actual test data");
        println!("   - Add assertions for expected results");
        println!("   - Customize the test flow as needed");

        Ok(())
    }

    fn create_scenario(
        &self,
        server_name: Option<String>,
        tools: Option<&Vec<ToolInfo>>,
        resources: Option<&Vec<ResourceInfo>>,
        prompts: Option<&Vec<PromptInfo>>,
    ) -> TestScenario {
        let mut steps = Vec::new();
        let mut variables = HashMap::new();

        // Add some common variables
        variables.insert("test_id".to_string(), json!("test_123"));
        variables.insert("test_value".to_string(), json!("sample_value"));

        // Add initialization step
        steps.push(TestStep {
            name: "List available capabilities".to_string(),
            operation: Operation::ListTools,
            timeout: None,
            continue_on_failure: false,
            store_result: Some("available_tools".to_string()),
            assertions: vec![
                Assertion::Success,
                Assertion::Exists {
                    path: "tools".to_string(),
                },
            ],
        });

        // Add tool steps
        if let Some(tools_list) = tools {
            let tools_to_include = if self.all_tools {
                tools_list.clone()
            } else {
                tools_list.iter().take(5).cloned().collect()
            };

            for tool in tools_to_include {
                steps.push(self.create_tool_step(&tool));
            }
        }

        // Add resource steps
        if self.with_resources {
            steps.push(TestStep {
                name: "List available resources".to_string(),
                operation: Operation::ListResources,
                timeout: None,
                continue_on_failure: false,
                store_result: Some("available_resources".to_string()),
                assertions: vec![
                    Assertion::Success,
                    Assertion::Exists {
                        path: "resources".to_string(),
                    },
                ],
            });

            if let Some(resources_list) = resources {
                if let Some(first_resource) = resources_list.first() {
                    steps.push(TestStep {
                        name: format!("Read resource: {}", first_resource.name),
                        operation: Operation::ReadResource {
                            uri: first_resource.uri.clone(),
                        },
                        timeout: None,
                        continue_on_failure: true,
                        store_result: Some("resource_content".to_string()),
                        assertions: vec![
                            Assertion::Success,
                            Assertion::Exists {
                                path: "contents".to_string(),
                            },
                        ],
                    });
                }
            }
        }

        // Add prompt steps
        if self.with_prompts {
            steps.push(TestStep {
                name: "List available prompts".to_string(),
                operation: Operation::ListPrompts,
                timeout: None,
                continue_on_failure: false,
                store_result: Some("available_prompts".to_string()),
                assertions: vec![
                    Assertion::Success,
                    Assertion::Exists {
                        path: "prompts".to_string(),
                    },
                ],
            });

            if let Some(prompts_list) = prompts {
                if let Some(first_prompt) = prompts_list.first() {
                    let mut args = HashMap::new();

                    // Generate placeholder arguments based on the prompt's argument schema
                    if let Some(prompt_args) = &first_prompt.arguments {
                        for arg in prompt_args {
                            args.insert(
                                arg.name.clone(),
                                json!(format!("TODO: Replace with actual {}", arg.name)),
                            );
                        }
                    }

                    steps.push(TestStep {
                        name: format!("Get prompt: {}", first_prompt.name),
                        operation: Operation::GetPrompt {
                            name: first_prompt.name.clone(),
                            arguments: if args.is_empty() {
                                json!({})
                            } else {
                                json!(args)
                            },
                        },
                        timeout: None,
                        continue_on_failure: true,
                        store_result: Some("prompt_result".to_string()),
                        assertions: vec![
                            Assertion::Success,
                            Assertion::Exists {
                                path: "messages".to_string(),
                            },
                        ],
                    });
                }
            }
        }

        TestScenario {
            name: format!(
                "{} Test Scenario",
                server_name.unwrap_or_else(|| "MCP Server".to_string())
            ),
            description: Some(format!(
                "Automated test scenario for {} server. Please customize values and assertions.",
                self.server_url
            )),
            timeout: 60,
            stop_on_failure: false,
            variables,
            setup: vec![],
            steps,
            cleanup: vec![],
        }
    }

    fn create_tool_step(&self, tool: &ToolInfo) -> TestStep {
        let arguments = self.generate_arguments_from_schema(&tool.input_schema);

        TestStep {
            name: format!(
                "Test tool: {} {}",
                tool.name,
                if tool.description.is_some() {
                    format!("({})", tool.description.as_ref().unwrap())
                } else {
                    "".to_string()
                }
            ),
            operation: Operation::ToolCall {
                tool: tool.name.clone(),
                arguments,
            },
            timeout: Some(30),
            continue_on_failure: true,
            store_result: Some(format!("{}_result", tool.name.replace('-', "_"))),
            assertions: vec![
                Assertion::Success,
                // Add more assertions based on expected output
            ],
        }
    }

    fn generate_arguments_from_schema(&self, schema: &Value) -> Value {
        if let Some(obj) = schema.as_object() {
            // Check if it's an object schema with properties
            if obj.get("type") == Some(&json!("object")) {
                if let Some(properties) = obj.get("properties").and_then(|p| p.as_object()) {
                    let mut args = serde_json::Map::new();

                    for (key, prop_schema) in properties {
                        args.insert(key.clone(), self.generate_value_for_type(key, prop_schema));
                    }

                    return json!(args);
                }
            }
        }

        // Return empty object if no schema or unrecognized format
        json!({})
    }

    fn generate_value_for_type(&self, field_name: &str, schema: &Value) -> Value {
        if let Some(obj) = schema.as_object() {
            // Check for enum values
            if let Some(enum_values) = obj.get("enum").and_then(|e| e.as_array()) {
                if !enum_values.is_empty() {
                    return enum_values[0].clone();
                }
            }

            // Check for examples
            if let Some(example) = obj.get("example") {
                return example.clone();
            }

            // Generate based on type
            if let Some(type_val) = obj.get("type").and_then(|t| t.as_str()) {
                match type_val {
                    "string" => {
                        // Check for specific formats
                        if let Some(format) = obj.get("format").and_then(|f| f.as_str()) {
                            match format {
                                "uri" | "url" => json!("https://example.com"),
                                "email" => json!("test@example.com"),
                                "date" => json!("2024-01-01"),
                                "date-time" => json!("2024-01-01T00:00:00Z"),
                                "uuid" => json!("550e8400-e29b-41d4-a716-446655440000"),
                                _ => json!(format!("TODO: {} (format: {})", field_name, format)),
                            }
                        } else {
                            // Check for description hints
                            if let Some(desc) = obj.get("description").and_then(|d| d.as_str()) {
                                if desc.to_lowercase().contains("path") {
                                    json!("/path/to/file")
                                } else if desc.to_lowercase().contains("name") {
                                    json!("example_name")
                                } else if desc.to_lowercase().contains("id") {
                                    json!("test_id_123")
                                } else {
                                    json!(format!("TODO: {}", field_name))
                                }
                            } else {
                                json!(format!("TODO: {}", field_name))
                            }
                        }
                    },
                    "number" | "integer" => {
                        if let Some(min) = obj.get("minimum").and_then(|m| m.as_i64()) {
                            json!(min)
                        } else if let Some(default) = obj.get("default").and_then(|d| d.as_i64()) {
                            json!(default)
                        } else {
                            json!(0)
                        }
                    },
                    "boolean" => json!(false),
                    "array" => {
                        if let Some(items_schema) = obj.get("items") {
                            json!([self.generate_value_for_type(
                                &format!("{}_item", field_name),
                                items_schema
                            )])
                        } else {
                            json!([])
                        }
                    },
                    "object" => {
                        if let Some(properties) = obj.get("properties").and_then(|p| p.as_object())
                        {
                            let mut nested = serde_json::Map::new();
                            for (key, prop_schema) in properties {
                                nested.insert(
                                    key.clone(),
                                    self.generate_value_for_type(key, prop_schema),
                                );
                            }
                            json!(nested)
                        } else {
                            json!({})
                        }
                    },
                    _ => json!(format!("TODO: {} (type: {})", field_name, type_val)),
                }
            } else {
                json!(format!("TODO: {}", field_name))
            }
        } else {
            json!(format!("TODO: {}", field_name))
        }
    }
}
