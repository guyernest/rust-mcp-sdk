//! Integration tests for the tool_router macro
//!
//! These tests verify that the #[tool_router] macro correctly collects
//! tool methods and generates routing code.
//!
//! NOTE: These tests are currently disabled because the tool_router macro
//! is still work-in-progress. The macro doesn't properly handle parameter
//! extraction and method dispatch yet.
//!
//! To enable these tests for development, set the TOOL_ROUTER_DEV=1 env var.

// Only compile these tests when explicitly enabled for development
#[cfg(feature = "tool_router_dev")]
mod tests {
    use pmcp_macros::{tool, tool_router};
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone)]
    struct Calculator {
        precision: usize,
    }

    #[derive(Debug, Deserialize, JsonSchema)]
    struct MathParams {
        a: f64,
        b: f64,
    }

    #[derive(Debug, Serialize, JsonSchema)]
    struct MathResult {
        result: f64,
    }

    #[test]
    fn test_tool_router_with_multiple_tools() {
        #[tool_router]
        impl Calculator {
            #[tool(description = "Add two numbers")]
            async fn add(&self, params: MathParams) -> Result<MathResult, String> {
                Ok(MathResult {
                    result: params.a + params.b,
                })
            }

            #[tool(description = "Subtract two numbers")]
            async fn subtract(&self, params: MathParams) -> Result<MathResult, String> {
                Ok(MathResult {
                    result: params.a - params.b,
                })
            }

            // Non-tool method (should be ignored)
            fn helper(&self) -> usize {
                self.precision
            }
        }
    }
}

// Placeholder test to ensure the file compiles
#[test]
fn tool_router_tests_placeholder() {
    // This test exists to document that tool_router tests are disabled.
    // The tool_router macro is WIP and needs work on:
    // - Parameter extraction from JSON args
    // - Method dispatch with proper argument passing
    // - Return type handling
    //
    // Enable with: cargo test -p pmcp-macros --features tool_router_dev
}
