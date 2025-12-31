::: exercise
id: ch02-02-calculator
difficulty: beginner
time: 25 minutes
prerequisites: ch02-01-hello-mcp
:::

Now that you've created your first MCP server, let's build something more
useful: a calculator. But this isn't just about math - it's about learning
how to handle different operations, validate inputs, and return meaningful
errors.

Think about it: when an AI asks your calculator to divide by zero, what
should happen? When someone passes "abc" instead of a number, how do you
respond helpfully?

Production MCP servers must handle edge cases gracefully. This exercise
teaches you how.

::: objectives
thinking:
  - Why input validation matters for AI interactions
  - How to design tools that handle edge cases
  - The difference between expected errors and bugs
doing:
  - Create a tool that handles multiple operations
  - Implement input validation with helpful error messages
  - Use Rust's Result type for error handling
:::

::: discussion
- If you were an AI trying to use a calculator, what operations would you expect?
- What should happen if someone tries to divide by zero?
- How can error messages help an AI correct its request?
- Should a calculator tool accept 'two plus three' or just '2 + 3'?
:::

::: starter file="src/main.rs"
```rust
//! Calculator MCP Server
//!
//! A calculator that demonstrates input validation and error handling.
//! Supports: add, subtract, multiply, divide

use pmcp::{Server, ServerCapabilities, ToolCapabilities};
use pmcp::server::TypedTool;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use anyhow::{Result, anyhow};

/// Supported mathematical operations
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum Operation {
    Add,
    Subtract,
    Multiply,
    Divide,
}

/// Input for the calculate tool
#[derive(Deserialize, JsonSchema)]
struct CalculateInput {
    /// First operand
    a: f64,
    /// Second operand
    b: f64,
    /// The operation to perform
    operation: Operation,
}

/// Result of a calculation
#[derive(Serialize)]
struct CalculateResult {
    result: f64,
    expression: String,
}

/// Error response for invalid calculations
#[derive(Serialize)]
struct CalculateError {
    error: String,
    suggestion: String,
}

/// Perform the calculation
///
/// TODO: Implement this function
/// - Handle all four operations
/// - Return an error for division by zero
/// - Return an error for operations that produce NaN or Infinity
fn calculate(input: &CalculateInput) -> Result<CalculateResult> {
    // TODO: Match on the operation and perform the calculation
    //
    // Hints:
    // 1. Use a match expression on input.operation
    // 2. For division, check if b is zero BEFORE dividing
    // 3. After calculation, check if result.is_nan() or result.is_infinite()
    // 4. Create a human-readable expression like "5 + 3 = 8"

    todo!("Implement the calculate function")
}

#[tokio::main]
async fn main() -> Result<()> {
    let server = Server::builder()
        .name("calculator")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(ToolCapabilities::default()),
            ..Default::default()
        })
        .tool("calculate", TypedTool::new("calculate", |input: CalculateInput| {
            Box::pin(async move {
                // TODO: Call calculate() and handle both success and error cases
                //
                // On success: return CalculateResult as JSON
                // On error: return CalculateError as JSON with a helpful message

                todo!("Handle the calculation result")
            })
        }))
        .build()?;

    println!("Calculator server ready!");
    Ok(())
}
```
:::

::: hint level=1 title="Start with the match"
Use pattern matching to handle each operation:
```rust
fn calculate(input: &CalculateInput) -> Result<CalculateResult> {
    let (result, op_symbol) = match input.operation {
        Operation::Add => (input.a + input.b, "+"),
        // Add other operations...
    };
    
    // Build the result
}
```
:::

::: hint level=2 title="Handle division safely"
Check for division by zero before computing:
```rust
Operation::Divide => {
    if input.b == 0.0 {
        return Err(anyhow!("Cannot divide by zero"));
    }
    (input.a / input.b, "/")
}
```
:::

::: hint level=3 title="Complete calculate function"
```rust
fn calculate(input: &CalculateInput) -> Result<CalculateResult> {
    let (result, op_symbol) = match input.operation {
        Operation::Add => (input.a + input.b, "+"),
        Operation::Subtract => (input.a - input.b, "-"),
        Operation::Multiply => (input.a * input.b, "*"),
        Operation::Divide => {
            if input.b == 0.0 {
                return Err(anyhow!("Cannot divide by zero"));
            }
            (input.a / input.b, "/")
        }
    };

    if result.is_nan() || result.is_infinite() {
        return Err(anyhow!("Invalid result"));
    }

    Ok(CalculateResult {
        result,
        expression: format!("{} {} {} = {}", input.a, op_symbol, input.b, result),
    })
}
```
:::

::: solution
```rust
use pmcp::{Server, ServerCapabilities, ToolCapabilities};
use pmcp::server::TypedTool;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use anyhow::{Result, anyhow};

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum Operation {
    Add,
    Subtract,
    Multiply,
    Divide,
}

#[derive(Deserialize, JsonSchema)]
struct CalculateInput {
    a: f64,
    b: f64,
    operation: Operation,
}

#[derive(Serialize)]
struct CalculateResult {
    result: f64,
    expression: String,
}

fn calculate(input: &CalculateInput) -> Result<CalculateResult> {
    let (result, op_symbol) = match input.operation {
        Operation::Add => (input.a + input.b, "+"),
        Operation::Subtract => (input.a - input.b, "-"),
        Operation::Multiply => (input.a * input.b, "*"),
        Operation::Divide => {
            if input.b == 0.0 {
                return Err(anyhow!(
                    "Cannot divide by zero. Please provide a non-zero divisor."
                ));
            }
            (input.a / input.b, "/")
        }
    };

    if result.is_nan() || result.is_infinite() {
        return Err(anyhow!(
            "Calculation produced an invalid result (NaN or Infinity)"
        ));
    }

    Ok(CalculateResult {
        result,
        expression: format!("{} {} {} = {}", input.a, op_symbol, input.b, result),
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let server = Server::builder()
        .name("calculator")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(ToolCapabilities::default()),
            ..Default::default()
        })
        .tool("calculate", TypedTool::new("calculate", |input: CalculateInput| {
            Box::pin(async move {
                match calculate(&input) {
                    Ok(result) => Ok(serde_json::to_value(result)?),
                    Err(e) => Ok(serde_json::json!({
                        "error": e.to_string(),
                        "suggestion": "Check your inputs and try again"
                    })),
                }
            })
        }))
        .build()?;

    println!("Calculator server ready!");
    Ok(())
}
```

### Explanation

This solution demonstrates several important patterns:

**1. Enum for Operations**
Using an enum instead of a string for operations:
- Compile-time validation of operation types
- Pattern matching ensures all cases are handled
- `#[serde(rename_all = "lowercase")]` allows JSON like `"add"` instead of `"Add"`

**2. Separation of Concerns**
The `calculate()` function is separate from the tool handler:
- Easier to test (pure function, no async)
- Cleaner error handling
- Reusable logic

**3. Defensive Error Handling**
- Check for division by zero BEFORE computing
- Check for NaN/Infinity AFTER computing
- Return helpful error messages that guide the AI

**4. Human-Readable Output**
- The `expression` field shows the full calculation
- Helps debugging and transparency
- AI can show this to users

**5. Error Response Pattern**
Instead of returning a tool error (which might retry), we return a
structured error response. This lets the AI understand what went wrong
and explain it to the user.
:::

::: tests mode=local
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_addition() {
        let input = CalculateInput {
            a: 5.0,
            b: 3.0,
            operation: Operation::Add,
        };
        let result = calculate(&input).unwrap();
        assert_eq!(result.result, 8.0);
    }

    #[test]
    fn test_division_by_zero() {
        let input = CalculateInput {
            a: 10.0,
            b: 0.0,
            operation: Operation::Divide,
        };
        assert!(calculate(&input).is_err());
    }

    #[test]
    fn test_expression_format() {
        let input = CalculateInput {
            a: 10.0,
            b: 5.0,
            operation: Operation::Multiply,
        };
        let result = calculate(&input).unwrap();
        assert!(result.expression.contains("10 * 5 = 50"));
    }
}
```
:::

::: reflection
- Why do we check for division by zero before computing, not after?
- What's the advantage of returning a structured error vs failing the tool call?
- How would you add a 'power' operation to this calculator?
- What might go wrong with floating-point math that integers wouldn't have?
:::

## Related Examples

For more patterns and variations, explore these SDK examples:

- **[02_server_basic.rs](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/02_server_basic.rs)** - Calculator implemented with `ToolHandler` trait
- **[32_typed_tools.rs](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/32_typed_tools.rs)** - Calculator with enum operations and automatic schema
- **[12_error_handling.rs](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/12_error_handling.rs)** - Comprehensive error handling patterns

Run locally with:
```bash
cargo run --example 02_server_basic
cargo run --example 12_error_handling
```
