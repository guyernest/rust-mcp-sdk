//! WASM-compatible typed tools example
//!
//! This example demonstrates how to use typed tools with automatic schema generation
//! in WASM environments (Browser, Cloudflare Workers, WASI).
//!
//! To run this example:
//! ```bash
//! cargo build --example 35_wasm_typed_tools --target wasm32-wasi --features "schema-generation"
//! wasmtime target/wasm32-wasi/debug/examples/35_wasm_typed_tools.wasm
//! ```

// Main function for non-WASM builds (for examples checking)
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("This example is designed to run in WASM environments only.");
    eprintln!("To build for WASM, use: cargo build --target wasm32-unknown-unknown --example 35_wasm_typed_tools");
}

#[cfg(target_arch = "wasm32")]
mod wasm_example {

    use pmcp::server::error_codes::{ValidationError, ValidationErrorCode};
    use pmcp::server::wasm_server::WasmMcpServer;
    use pmcp::server::wasm_typed_tool::{validation, SimpleWasmTool, WasmTypedTool};
    use pmcp::types::{Request, RequestId};
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    /// Arguments for the greeting tool
    #[derive(Debug, Deserialize, Serialize, JsonSchema)]
    struct GreetingArgs {
        /// The name of the person to greet
        name: String,
        /// Language for the greeting (en, es, fr, de)
        #[serde(default = "default_language")]
        language: String,
        /// Include timestamp in greeting
        #[serde(default)]
        include_time: bool,
    }

    fn default_language() -> String {
        "en".to_string()
    }

    /// Arguments for the text processing tool
    #[derive(Debug, Deserialize, Serialize, JsonSchema)]
    struct TextProcessArgs {
        /// The text to process
        text: String,
        /// Operation to perform
        operation: TextOperation,
        /// Options for the operation
        #[serde(default)]
        options: ProcessOptions,
    }

    #[derive(Debug, Deserialize, Serialize, JsonSchema)]
    #[serde(rename_all = "lowercase")]
    enum TextOperation {
        Uppercase,
        Lowercase,
        Reverse,
        WordCount,
        CharCount,
    }

    #[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
    struct ProcessOptions {
        /// Trim whitespace before processing
        #[serde(default)]
        trim: bool,
        /// Remove punctuation
        #[serde(default)]
        remove_punctuation: bool,
    }

    /// Arguments for the validation demo tool
    #[derive(Debug, Deserialize, Serialize, JsonSchema)]
    struct ValidationDemoArgs {
        /// Email address to validate
        email: String,
        /// URL to validate
        url: String,
        /// Age to validate (must be 18-120)
        age: u32,
        /// Username (3-20 characters)
        username: String,
    }

    /// Response from validation demo
    #[derive(Debug, Serialize)]
    struct ValidationDemoResult {
        valid: bool,
        email_valid: bool,
        url_valid: bool,
        age_valid: bool,
        username_valid: bool,
        message: String,
    }

    fn main() {
        // In a real WASM environment, this would be the entry point
        // For this example, we'll demonstrate the server creation
        let server = create_typed_wasm_server();

        // In a real deployment, you would handle requests
        // For example, in Cloudflare Workers:
        // ```
        // #[wasm_bindgen]
        // pub async fn fetch(request: Request, _env: Env, _ctx: Context) -> Result<Response> {
        //     let server = create_typed_wasm_server();
        //     // Parse request, call server.handle_request(), return response
        // }
        // ```

        println!("WASM server with typed tools created successfully");
    }

    /// Create a WASM server with typed tools
    pub fn create_typed_wasm_server() -> WasmMcpServer {
        WasmMcpServer::builder()
        .name("wasm-typed-tools-example")
        .version("1.0.0")
        // Add a typed greeting tool
        .tool_typed("greeting", |args: GreetingArgs| {
            // Validate language
            let valid_languages = ["en", "es", "fr", "de"];
            if !valid_languages.contains(&args.language.as_str()) {
                return Err(ValidationError::new(ValidationErrorCode::NotAllowed, "language")
                    .expected(format!("One of: {:?}", valid_languages))
                    .to_error());
            }

            // Generate greeting based on language
            let greeting = match args.language.as_str() {
                "en" => format!("Hello, {}!", args.name),
                "es" => format!("¡Hola, {}!", args.name),
                "fr" => format!("Bonjour, {} !", args.name),
                "de" => format!("Hallo, {}!", args.name),
                _ => format!("Hello, {}!", args.name),
            };

            let mut result = json!({
                "greeting": greeting,
                "language": args.language,
            });

            if args.include_time {
                // In WASM, we might not have access to system time
                // This is a placeholder
                result["timestamp"] = json!("2024-01-01T00:00:00Z");
            }

            Ok(result)
        })
        // Add a text processing tool
        .tool_typed("process_text", |args: TextProcessArgs| {
            let mut text = args.text;

            // Apply options
            if args.options.trim {
                text = text.trim().to_string();
            }
            if args.options.remove_punctuation {
                text = text.chars()
                    .filter(|c| !c.is_ascii_punctuation())
                    .collect();
            }

            // Perform operation
            let result = match args.operation {
                TextOperation::Uppercase => json!({
                    "result": text.to_uppercase(),
                    "operation": "uppercase"
                }),
                TextOperation::Lowercase => json!({
                    "result": text.to_lowercase(),
                    "operation": "lowercase"
                }),
                TextOperation::Reverse => json!({
                    "result": text.chars().rev().collect::<String>(),
                    "operation": "reverse"
                }),
                TextOperation::WordCount => json!({
                    "count": text.split_whitespace().count(),
                    "operation": "word_count"
                }),
                TextOperation::CharCount => json!({
                    "count": text.chars().count(),
                    "operation": "char_count"
                }),
            };

            Ok(result)
        })
        // Add a validation demonstration tool
        .tool_typed("validate_input", |args: ValidationDemoArgs| {
            let mut result = ValidationDemoResult {
                valid: true,
                email_valid: true,
                url_valid: true,
                age_valid: true,
                username_valid: true,
                message: String::new(),
            };

            // Validate email
            if let Err(e) = validation::validate_email("email", &args.email) {
                result.email_valid = false;
                result.valid = false;
                result.message.push_str(&format!("Email invalid: {}. ", e));
            }

            // Validate URL
            if let Err(e) = validation::validate_url("url", &args.url) {
                result.url_valid = false;
                result.valid = false;
                result.message.push_str(&format!("URL invalid: {}. ", e));
            }

            // Validate age range
            if let Err(e) = validation::validate_range("age", args.age, 18, 120) {
                result.age_valid = false;
                result.valid = false;
                result.message.push_str(&format!("Age invalid: {}. ", e));
            }

            // Validate username length
            if let Err(e) = validation::validate_length("username", &args.username, Some(3), Some(20)) {
                result.username_valid = false;
                result.valid = false;
                result.message.push_str(&format!("Username invalid: {}. ", e));
            }

            if result.valid {
                result.message = "All inputs are valid!".to_string();
            }

            serde_json::to_value(result)
                .map_err(|e| pmcp::Error::Internal(format!("Serialization error: {}", e)))
        })
        // Add a simple calculator tool
        .tool_typed_simple("calculator", |args: CalculatorArgs| {
            let result = match args.operation.as_str() {
                "add" => args.a + args.b,
                "subtract" => args.a - args.b,
                "multiply" => args.a * args.b,
                "divide" => {
                    if args.b == 0.0 {
                        return Err(ValidationError::new(ValidationErrorCode::OutOfRange, "b")
                            .message("Cannot divide by zero")
                            .to_error());
                    }
                    args.a / args.b
                }
                _ => {
                    return Err(ValidationError::new(ValidationErrorCode::NotAllowed, "operation")
                        .expected("add, subtract, multiply, or divide")
                        .to_error());
                }
            };

            Ok(CalculatorArgs {
                a: result,
                b: 0.0,
                operation: "result".to_string(),
            })
        })
        .build()
    }

    #[derive(Debug, Deserialize, Serialize, JsonSchema)]
    struct CalculatorArgs {
        /// First number
        a: f64,
        /// Second number
        b: f64,
        /// Operation: add, subtract, multiply, divide
        operation: String,
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use pmcp::server::wasm_typed_tool::WasmTool;

        #[test]
        fn test_greeting_tool() {
            let tool = WasmTypedTool::new("greeting", |args: GreetingArgs| {
                Ok(json!({
                    "greeting": format!("Hello, {}!", args.name),
                    "language": args.language
                }))
            });

            let args = json!({
                "name": "Alice",
                "language": "en"
            });

            let result = tool.execute(args).unwrap();
            assert_eq!(result["greeting"], "Hello, Alice!");
        }

        #[test]
        fn test_validation() {
            let args = ValidationDemoArgs {
                email: "user@example.com".to_string(),
                url: "https://example.com".to_string(),
                age: 25,
                username: "alice123".to_string(),
            };

            // All validations should pass
            assert!(validation::validate_email("email", &args.email).is_ok());
            assert!(validation::validate_url("url", &args.url).is_ok());
            assert!(validation::validate_range("age", &args.age, &18, &120).is_ok());
            assert!(
                validation::validate_length("username", &args.username, Some(3), Some(20)).is_ok()
            );
        }
    }
} // End of wasm_example module
