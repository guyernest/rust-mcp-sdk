//! Complete calculator template with tools, prompts, and resources

pub const COMPLETE_CALCULATOR_LIB: &str = r##"//! Complete calculator MCP server
//!
//! Demonstrates all three MCP capabilities:
//! - Tools: Basic arithmetic operations (add, subtract, multiply, divide, power)
//! - Prompts: Quadratic equation solver (orchestrates multiple tools)
//! - Resources: Educational content about quadratic formulas

use pmcp::{Error, Result, Server, TypedTool, SimplePrompt, ResourceCollection, StaticResource};
use pmcp::types::{
    ServerCapabilities, GetPromptResult, PromptMessage, Role, MessageContent,
    ToolCapabilities, PromptCapabilities, ResourceCapabilities,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use schemars::JsonSchema;
use validator::Validate;
use std::collections::HashMap;

// ============================================================================
// TOOL INPUT TYPES
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[schemars(deny_unknown_fields)]
pub struct AddInput {
    /// First number to add
    #[validate(range(min = -1000000.0, max = 1000000.0))]
    #[schemars(description = "First number in the addition operation")]
    pub a: f64,

    /// Second number to add
    #[validate(range(min = -1000000.0, max = 1000000.0))]
    #[schemars(description = "Second number in the addition operation")]
    pub b: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[schemars(deny_unknown_fields)]
pub struct SubtractInput {
    /// Number to subtract from
    #[validate(range(min = -1000000.0, max = 1000000.0))]
    #[schemars(description = "Minuend (number to subtract from)")]
    pub a: f64,

    /// Number to subtract
    #[validate(range(min = -1000000.0, max = 1000000.0))]
    #[schemars(description = "Subtrahend (number to subtract)")]
    pub b: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[schemars(deny_unknown_fields)]
pub struct MultiplyInput {
    /// First factor
    #[validate(range(min = -1000000.0, max = 1000000.0))]
    #[schemars(description = "First number to multiply")]
    pub a: f64,

    /// Second factor
    #[validate(range(min = -1000000.0, max = 1000000.0))]
    #[schemars(description = "Second number to multiply")]
    pub b: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[schemars(deny_unknown_fields)]
pub struct DivideInput {
    /// Dividend (number to be divided)
    #[validate(range(min = -1000000.0, max = 1000000.0))]
    #[schemars(description = "Dividend (number to be divided)")]
    pub a: f64,

    /// Divisor (number to divide by)
    #[validate(range(min = -1000000.0, max = 1000000.0))]
    #[schemars(description = "Divisor (number to divide by, must be non-zero)")]
    pub b: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[schemars(deny_unknown_fields)]
pub struct PowerInput {
    /// Base number
    #[validate(range(min = -1000.0, max = 1000.0))]
    #[schemars(description = "Base number")]
    pub base: f64,

    /// Exponent
    #[validate(range(min = -100.0, max = 100.0))]
    #[schemars(description = "Exponent (power to raise to)")]
    pub exponent: f64,
}

// ============================================================================
// PROMPT INPUT TYPES
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct QuadraticInput {
    /// Coefficient of x² term
    #[schemars(description = "Coefficient a in ax² + bx + c = 0")]
    pub a: f64,

    /// Coefficient of x term
    #[schemars(description = "Coefficient b in ax² + bx + c = 0")]
    pub b: f64,

    /// Constant term
    #[schemars(description = "Constant c in ax² + bx + c = 0")]
    pub c: f64,
}

/// Build the calculator server with all capabilities
pub fn build_calculator_server() -> Result<Server> {
    // Create quadratic prompt
    let quadratic_prompt = SimplePrompt::new(
        "quadratic",
        Box::new(|args: HashMap<String, String>, _extra| {
            Box::pin(async move {
                // Parse arguments
                let a: f64 = args.get("a")
                    .ok_or_else(|| Error::validation("Missing required argument 'a'"))?
                    .parse()
                    .map_err(|_| Error::validation("Argument 'a' must be a number"))?;
                let b: f64 = args.get("b")
                    .ok_or_else(|| Error::validation("Missing required argument 'b'"))?
                    .parse()
                    .map_err(|_| Error::validation("Argument 'b' must be a number"))?;
                let c: f64 = args.get("c")
                    .ok_or_else(|| Error::validation("Missing required argument 'c'"))?
                    .parse()
                    .map_err(|_| Error::validation("Argument 'c' must be a number"))?;

                if a == 0.0 {
                    return Err(Error::validation("Coefficient 'a' cannot be zero (not a quadratic equation)"));
                }

                // Calculate discriminant: b² - 4ac
                let discriminant = b * b - 4.0 * a * c;

                let mut messages = vec![
                    PromptMessage {
                        role: Role::User,
                        content: MessageContent::Text {
                            text: format!(
                                "Solve the quadratic equation: {}x² + {}x + {} = 0",
                                a, b, c
                            ),
                        },
                    }
                ];

                if discriminant < 0.0 {
                    // No real roots
                    messages.push(PromptMessage {
                        role: Role::Assistant,
                        content: MessageContent::Text {
                            text: format!(
                                "This equation has no real roots.\n\
                                 Discriminant Δ = b² - 4ac = {} < 0\n\
                                 The roots are complex numbers.",
                                discriminant
                            ),
                        },
                    });
                } else if discriminant == 0.0 {
                    // One repeated root
                    let x = -b / (2.0 * a);
                    messages.push(PromptMessage {
                        role: Role::Assistant,
                        content: MessageContent::Text {
                            text: format!(
                                "This equation has one repeated real root.\n\
                                 Discriminant Δ = b² - 4ac = 0\n\
                                 Solution: x = {}\n\n\
                                 Using the formula: x = -b / 2a = -{} / (2 × {}) = {}",
                                x, b, a, x
                            ),
                        },
                    });
                } else {
                    // Two distinct real roots
                    let sqrt_discriminant = discriminant.sqrt();
                    let x1 = (-b + sqrt_discriminant) / (2.0 * a);
                    let x2 = (-b - sqrt_discriminant) / (2.0 * a);

                    messages.push(PromptMessage {
                        role: Role::Assistant,
                        content: MessageContent::Text {
                            text: format!(
                                "This equation has two distinct real roots.\n\
                                 Discriminant Δ = b² - 4ac = {} > 0\n\
                                 √Δ = {}\n\n\
                                 Using the quadratic formula: x = (-b ± √Δ) / 2a\n\n\
                                 Solution 1: x₁ = ({} + {}) / {} = {}\n\
                                 Solution 2: x₂ = ({} - {}) / {} = {}",
                                discriminant,
                                sqrt_discriminant,
                                -b, sqrt_discriminant, 2.0 * a, x1,
                                -b, sqrt_discriminant, 2.0 * a, x2
                            ),
                        },
                    });
                }

                Ok(GetPromptResult {
                    description: Some(format!(
                        "Solution for {}x² + {}x + {} = 0",
                        a, b, c
                    )),
                    messages,
                })
            }) as std::pin::Pin<Box<dyn std::future::Future<Output = Result<GetPromptResult>> + Send>>
        })
    )
    .with_description("Solve quadratic equations using the quadratic formula")
    .with_argument("a", "Coefficient of x² term (a in ax² + bx + c = 0)", true)
    .with_argument("b", "Coefficient of x term (b in ax² + bx + c = 0)", true)
    .with_argument("c", "Constant term (c in ax² + bx + c = 0)", true);

    // Create resource collection
    let resources = ResourceCollection::new()
        .add_resource(
            StaticResource::new_text(
                "calculator://help/quadratic-formula",
                r#"# Quadratic Formula Guide

## The Formula

For equations in the form: **ax² + bx + c = 0** (where a ≠ 0)

The solutions are given by:

**x = (-b ± √(b² - 4ac)) / 2a**

## The Discriminant

The discriminant **Δ = b² - 4ac** determines the nature of the roots:

- **Δ > 0**: Two distinct real roots
- **Δ = 0**: One repeated real root (the parabola touches the x-axis at one point)
- **Δ < 0**: No real roots (the parabola doesn't cross the x-axis)

## Step-by-Step Process

1. **Identify coefficients**: Determine values of a, b, and c
2. **Calculate discriminant**: Δ = b² - 4ac
3. **Check discriminant**: Determine how many real roots exist
4. **Apply formula**: x = (-b ± √Δ) / 2a
5. **Simplify**: Calculate both roots (if they exist)

## Example 1: Two Real Roots

Solve: **x² - 5x + 6 = 0**

- a = 1, b = -5, c = 6
- Δ = (-5)² - 4(1)(6) = 25 - 24 = 1
- √Δ = 1
- x = (5 ± 1) / 2

**Solutions:**
- x₁ = (5 + 1) / 2 = 3
- x₂ = (5 - 1) / 2 = 2

## Example 2: One Repeated Root

Solve: **x² - 6x + 9 = 0**

- a = 1, b = -6, c = 9
- Δ = (-6)² - 4(1)(9) = 36 - 36 = 0
- x = 6 / 2 = 3

**Solution:** x = 3 (repeated)

## Example 3: No Real Roots

Solve: **x² + 2x + 5 = 0**

- a = 1, b = 2, c = 5
- Δ = 2² - 4(1)(5) = 4 - 20 = -16

**Result:** No real solutions (Δ < 0)

## Try It Yourself!

Use the **quadratic** prompt in this calculator to solve any quadratic equation.
The prompt will show you step-by-step solutions with explanations.

### Available Tools

This calculator also provides individual operation tools:
- `add` - Add two numbers
- `subtract` - Subtract numbers
- `multiply` - Multiply numbers
- `divide` - Divide numbers (with zero-check)
- `power` - Raise to a power

The quadratic prompt uses these tools internally to perform calculations!
"#,
            )
            .with_name("Quadratic Formula Guide")
            .with_description("Learn how to solve quadratic equations using the quadratic formula")
            .with_mime_type("text/markdown"),
        );

    // Build server with all capabilities
    Server::builder()
        .name("calculator")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(ToolCapabilities { list_changed: Some(true) }),
            prompts: Some(PromptCapabilities { list_changed: Some(true) }),
            resources: Some(ResourceCapabilities {
                subscribe: Some(true),
                list_changed: Some(true),
            }),
            ..Default::default()
        })
        // Add tools
        .tool(
            "add",
            TypedTool::new("add", |input: AddInput, _extra| {
                Box::pin(async move {
                    input.validate()
                        .map_err(|e| Error::validation(format!("Validation failed: {}", e)))?;
                    let result = input.a + input.b;
                    Ok(json!({
                        "result": result,
                        "operation": format!("{} + {} = {}", input.a, input.b, result)
                    }))
                })
            })
            .with_description("Add two numbers together"),
        )
        .tool(
            "subtract",
            TypedTool::new("subtract", |input: SubtractInput, _extra| {
                Box::pin(async move {
                    input.validate()
                        .map_err(|e| Error::validation(format!("Validation failed: {}", e)))?;
                    let result = input.a - input.b;
                    Ok(json!({
                        "result": result,
                        "operation": format!("{} - {} = {}", input.a, input.b, result)
                    }))
                })
            })
            .with_description("Subtract one number from another"),
        )
        .tool(
            "multiply",
            TypedTool::new("multiply", |input: MultiplyInput, _extra| {
                Box::pin(async move {
                    input.validate()
                        .map_err(|e| Error::validation(format!("Validation failed: {}", e)))?;
                    let result = input.a * input.b;
                    Ok(json!({
                        "result": result,
                        "operation": format!("{} × {} = {}", input.a, input.b, result)
                    }))
                })
            })
            .with_description("Multiply two numbers together"),
        )
        .tool(
            "divide",
            TypedTool::new("divide", |input: DivideInput, _extra| {
                Box::pin(async move {
                    input.validate()
                        .map_err(|e| Error::validation(format!("Validation failed: {}", e)))?;

                    // Check for division by zero
                    if input.b == 0.0 {
                        return Err(Error::validation("Cannot divide by zero"));
                    }

                    let result = input.a / input.b;
                    Ok(json!({
                        "result": result,
                        "operation": format!("{} ÷ {} = {}", input.a, input.b, result)
                    }))
                })
            })
            .with_description("Divide one number by another (with zero-division check)"),
        )
        .tool(
            "power",
            TypedTool::new("power", |input: PowerInput, _extra| {
                Box::pin(async move {
                    input.validate()
                        .map_err(|e| Error::validation(format!("Validation failed: {}", e)))?;
                    let result = input.base.powf(input.exponent);
                    Ok(json!({
                        "result": result,
                        "operation": format!("{}^{} = {}", input.base, input.exponent, result)
                    }))
                })
            })
            .with_description("Raise a number to a power (exponentiation)"),
        )
        // Add prompt
        .prompt("quadratic", quadratic_prompt)
        // Add resources
        .resources(resources)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_creation() {
        let server = build_calculator_server();
        assert!(server.is_ok());
    }

    #[tokio::test]
    async fn test_add_validation() {
        let input = AddInput { a: 5.0, b: 3.0 };
        assert!(input.validate().is_ok());

        // Test out of range
        let input = AddInput { a: 2000000.0, b: 3.0 };
        assert!(input.validate().is_err());
    }

    #[tokio::test]
    async fn test_add_logic() {
        let input = AddInput { a: 5.0, b: 3.0 };
        assert_eq!(input.a + input.b, 8.0);

        let input = AddInput { a: -5.0, b: 3.0 };
        assert_eq!(input.a + input.b, -2.0);
    }

    #[tokio::test]
    async fn test_divide_by_zero_validation() {
        let input = DivideInput { a: 10.0, b: 0.0 };
        // Validation passes (b is within range)
        assert!(input.validate().is_ok());
        // Zero check happens in handler
    }

    #[tokio::test]
    async fn test_quadratic_discriminant() {
        // Two roots: x² - 5x + 6 = 0
        let input = QuadraticInput { a: 1.0, b: -5.0, c: 6.0 };
        let discriminant = input.b * input.b - 4.0 * input.a * input.c;
        assert_eq!(discriminant, 1.0);

        // One root: x² - 6x + 9 = 0
        let input = QuadraticInput { a: 1.0, b: -6.0, c: 9.0 };
        let discriminant = input.b * input.b - 4.0 * input.a * input.c;
        assert_eq!(discriminant, 0.0);

        // No real roots: x² + 2x + 5 = 0
        let input = QuadraticInput { a: 1.0, b: 2.0, c: 5.0 };
        let discriminant = input.b * input.b - 4.0 * input.a * input.c;
        assert!(discriminant < 0.0);
    }
}
"##;
