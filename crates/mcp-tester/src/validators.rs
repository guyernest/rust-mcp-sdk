use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub info: HashMap<String, String>,
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            info: HashMap::new(),
        }
    }
}

impl ValidationResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_error(&mut self, error: impl Into<String>) {
        self.valid = false;
        self.errors.push(error.into());
    }

    pub fn add_warning(&mut self, warning: impl Into<String>) {
        self.warnings.push(warning.into());
    }

    pub fn add_info(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.info.insert(key.into(), value.into());
    }
}

#[derive(Default)]
pub struct Validator {
    strict_mode: bool,
}

impl Validator {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    pub fn strict() -> Self {
        Self { strict_mode: true }
    }

    pub fn validate_protocol_version(&self, version: &str) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Check for supported protocol versions
        let supported_versions = vec!["2024-11-05", "2025-03-26", "2025-06-18"];

        if !supported_versions.contains(&version) {
            if self.strict_mode {
                result.add_error(format!(
                    "Unsupported protocol version: {}. Supported: {:?}",
                    version, supported_versions
                ));
            } else {
                result.add_warning(format!(
                    "Unknown protocol version: {}. Known versions: {:?}",
                    version, supported_versions
                ));
            }
        }

        result.add_info("protocol_version", version);
        result
    }

    #[allow(dead_code)]
    pub fn validate_initialize_response(&self, response: &Value) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Check required fields
        if !response.is_object() {
            result.add_error("Response must be an object");
            return result;
        }

        let obj = response.as_object().unwrap();

        // Check protocol version
        if !obj.contains_key("protocolVersion") {
            result.add_error("Missing required field: protocolVersion");
        } else if let Some(version) = obj.get("protocolVersion") {
            if let Some(version_str) = version.as_str() {
                let version_result = self.validate_protocol_version(version_str);
                result.errors.extend(version_result.errors);
                result.warnings.extend(version_result.warnings);
            }
        }

        // Check capabilities
        if !obj.contains_key("capabilities") {
            result.add_error("Missing required field: capabilities");
        } else if let Some(capabilities) = obj.get("capabilities") {
            if !capabilities.is_object() {
                result.add_error("capabilities must be an object");
            }
        }

        // Check server info
        if !obj.contains_key("serverInfo") {
            result.add_error("Missing required field: serverInfo");
        } else if let Some(info) = obj.get("serverInfo") {
            let info_result = self.validate_server_info(info);
            result.errors.extend(info_result.errors);
            result.warnings.extend(info_result.warnings);
        }

        result
    }

    #[allow(dead_code)]
    pub fn validate_server_info(&self, info: &Value) -> ValidationResult {
        let mut result = ValidationResult::new();

        if !info.is_object() {
            result.add_error("serverInfo must be an object");
            return result;
        }

        let obj = info.as_object().unwrap();

        // Required fields
        if !obj.contains_key("name") {
            result.add_error("serverInfo missing required field: name");
        } else if let Some(name) = obj.get("name") {
            if !name.is_string() {
                result.add_error("serverInfo.name must be a string");
            }
        }

        if !obj.contains_key("version") {
            result.add_error("serverInfo missing required field: version");
        } else if let Some(version) = obj.get("version") {
            if !version.is_string() {
                result.add_error("serverInfo.version must be a string");
            }
        }

        result
    }

    #[allow(dead_code)]
    pub fn validate_tools_list_response(&self, response: &Value) -> ValidationResult {
        let mut result = ValidationResult::new();

        if !response.is_object() {
            result.add_error("Response must be an object");
            return result;
        }

        let obj = response.as_object().unwrap();

        // Check tools array
        if !obj.contains_key("tools") {
            result.add_error("Missing required field: tools");
        } else if let Some(tools) = obj.get("tools") {
            if !tools.is_array() {
                result.add_error("tools must be an array");
            } else {
                let tools_array = tools.as_array().unwrap();
                for (i, tool) in tools_array.iter().enumerate() {
                    let tool_result = self.validate_tool_definition(tool);
                    if !tool_result.errors.is_empty() {
                        result.add_error(format!(
                            "Tool[{}] validation failed: {}",
                            i,
                            tool_result.errors.join(", ")
                        ));
                    }
                    result.warnings.extend(
                        tool_result
                            .warnings
                            .iter()
                            .map(|w| format!("Tool[{}]: {}", i, w)),
                    );
                }
                result.add_info("tool_count", tools_array.len().to_string());
            }
        }

        result
    }

    #[allow(dead_code)]
    pub fn validate_tool_definition(&self, tool: &Value) -> ValidationResult {
        let mut result = ValidationResult::new();

        if !tool.is_object() {
            result.add_error("Tool must be an object");
            return result;
        }

        let obj = tool.as_object().unwrap();

        // Required: name
        if !obj.contains_key("name") {
            result.add_error("Tool missing required field: name");
        } else if let Some(name) = obj.get("name") {
            if !name.is_string() {
                result.add_error("Tool name must be a string");
            } else {
                let name_str = name.as_str().unwrap();
                // Validate naming convention
                if !name_str
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
                {
                    result.add_warning(format!(
                        "Tool name '{}' contains non-standard characters",
                        name_str
                    ));
                }
            }
        }

        // Optional but recommended: description
        if !obj.contains_key("description") {
            result.add_warning("Tool missing recommended field: description");
        }

        // Optional: inputSchema
        if let Some(schema) = obj.get("inputSchema") {
            if !schema.is_object() && !schema.is_null() {
                result.add_error("Tool inputSchema must be an object or null");
            }
        }

        result
    }

    #[allow(dead_code)]
    pub fn validate_tool_call_response(&self, response: &Value) -> ValidationResult {
        let mut result = ValidationResult::new();

        if !response.is_object() {
            result.add_error("Response must be an object");
            return result;
        }

        let obj = response.as_object().unwrap();

        // Should have either content or error
        let has_content = obj.contains_key("content");
        let has_error = obj.contains_key("error");

        if !has_content && !has_error {
            result.add_error("Response must have either 'content' or 'error'");
        }

        if has_content && has_error {
            result.add_warning("Response should not have both 'content' and 'error'");
        }

        // Validate content structure
        if let Some(content) = obj.get("content") {
            if !content.is_array() {
                result.add_error("content must be an array");
            } else {
                let content_array = content.as_array().unwrap();
                for (i, item) in content_array.iter().enumerate() {
                    let item_result = self.validate_content_item(item);
                    if !item_result.errors.is_empty() {
                        result.add_error(format!(
                            "Content[{}] validation failed: {}",
                            i,
                            item_result.errors.join(", ")
                        ));
                    }
                }
            }
        }

        result
    }

    #[allow(dead_code)]
    pub fn validate_content_item(&self, item: &Value) -> ValidationResult {
        let mut result = ValidationResult::new();

        if !item.is_object() {
            result.add_error("Content item must be an object");
            return result;
        }

        let obj = item.as_object().unwrap();

        // Required: type
        if !obj.contains_key("type") {
            result.add_error("Content item missing required field: type");
        } else if let Some(item_type) = obj.get("type") {
            if let Some(type_str) = item_type.as_str() {
                let valid_types = vec!["text", "image", "resource"];
                if !valid_types.contains(&type_str) {
                    result.add_error(format!(
                        "Invalid content type '{}'. Valid types: {:?}",
                        type_str, valid_types
                    ));
                }

                // Type-specific validation
                match type_str {
                    "text" => {
                        if !obj.contains_key("text") {
                            result.add_error("Text content item missing 'text' field");
                        }
                    },
                    "image" => {
                        if !obj.contains_key("data") && !obj.contains_key("url") {
                            result.add_error("Image content item must have 'data' or 'url'");
                        }
                    },
                    "resource" => {
                        if !obj.contains_key("uri") {
                            result.add_error("Resource content item missing 'uri' field");
                        }
                    },
                    _ => {},
                }
            }
        }

        result
    }

    #[allow(dead_code)]
    pub fn validate_json_rpc_response(&self, response: &Value) -> ValidationResult {
        let mut result = ValidationResult::new();

        if !response.is_object() {
            result.add_error("JSON-RPC response must be an object");
            return result;
        }

        let obj = response.as_object().unwrap();

        // Required: jsonrpc version
        if !obj.contains_key("jsonrpc") {
            result.add_error("Missing required field: jsonrpc");
        } else if let Some(version) = obj.get("jsonrpc") {
            if version != "2.0" {
                result.add_error(format!(
                    "Invalid JSON-RPC version: {}. Expected: 2.0",
                    version
                ));
            }
        }

        // Must have either result or error, not both
        let has_result = obj.contains_key("result");
        let has_error = obj.contains_key("error");

        if !has_result && !has_error {
            result.add_error("JSON-RPC response must have either 'result' or 'error'");
        }

        if has_result && has_error {
            result.add_error("JSON-RPC response cannot have both 'result' and 'error'");
        }

        // Validate error structure if present
        if let Some(error) = obj.get("error") {
            let error_result = self.validate_json_rpc_error(error);
            result.errors.extend(error_result.errors);
            result.warnings.extend(error_result.warnings);
        }

        // Should have id (null or value)
        if !obj.contains_key("id") {
            result.add_warning("JSON-RPC response missing 'id' field");
        }

        result
    }

    #[allow(dead_code)]
    pub fn validate_json_rpc_error(&self, error: &Value) -> ValidationResult {
        let mut result = ValidationResult::new();

        if !error.is_object() {
            result.add_error("JSON-RPC error must be an object");
            return result;
        }

        let obj = error.as_object().unwrap();

        // Required fields
        if !obj.contains_key("code") {
            result.add_error("JSON-RPC error missing required field: code");
        } else if let Some(code) = obj.get("code") {
            if !code.is_i64() {
                result.add_error("JSON-RPC error code must be an integer");
            } else {
                let code_num = code.as_i64().unwrap();
                // Standard error codes
                let standard_codes = [
                    (-32700, "Parse error"),
                    (-32600, "Invalid Request"),
                    (-32601, "Method not found"),
                    (-32602, "Invalid params"),
                    (-32603, "Internal error"),
                ];

                let is_standard = standard_codes.iter().any(|(c, _)| *c == code_num);

                if !(is_standard || (-32099..=-32000).contains(&code_num)) {
                    result.add_info("error_code", code_num.to_string());
                    if !(-32768..=-32000).contains(&code_num) {
                        result.add_warning(format!("Non-standard error code: {}", code_num));
                    }
                }
            }
        }

        if !obj.contains_key("message") {
            result.add_error("JSON-RPC error missing required field: message");
        } else if let Some(message) = obj.get("message") {
            if !message.is_string() {
                result.add_error("JSON-RPC error message must be a string");
            }
        }

        result
    }
}
