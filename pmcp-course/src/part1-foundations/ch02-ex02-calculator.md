# Exercise: The Calculator Tool

::: exercise
id: ch02-02-calculator
difficulty: beginner
time: 25 minutes
prerequisites: [ch02-01-hello-mcp]
:::

Build a calculator MCP server that handles multiple operations with proper error handling. This isn't just about math - it's about learning how to validate inputs and return meaningful errors.

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
:::

## Your Task

Implement a `calculate` function that:
1. Takes two numbers and an operation (add, subtract, multiply, divide)
2. Returns `Ok(result)` for valid operations
3. Returns `Err(message)` for invalid operations (like division by zero)

::: starter file="src/main.rs" language=rust
```rust
//! Calculator MCP Server

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
    // TODO: Implement calculation with error handling
    // 1. Match on operation
    // 2. Check for division by zero
    // 3. Check for NaN/Infinity results
    todo!()
}

#[tokio::main]
async fn main() -> Result<()> {
    // TODO: Build server with calculate tool
    todo!()
}
```
:::

::: hint level=1 title="Getting Started with Match"
```rust
match input.operation {
    Operation::Add => (input.a + input.b, "+"),
    Operation::Subtract => (input.a - input.b, "-"),
    // ...
}
```
:::

::: hint level=2 title="Handling Division by Zero"
```rust
Operation::Divide => {
    if input.b == 0.0 {
        return Err(anyhow!("Cannot divide by zero"));
    }
    (input.a / input.b, "/")
}
```
:::

::: solution
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
        return Err(anyhow!("Calculation produced invalid result"));
    }

    Ok(CalculateResult {
        result,
        expression: format!("{} {} {} = {}", input.a, op_symbol, input.b, result),
    })
}
```

### Explanation

- **Pattern matching** on the `Operation` enum handles all cases
- **Early return** on division by zero prevents the error
- **Result validation** catches NaN/Infinity from edge cases
- **Structured output** includes both the result and a human-readable expression
:::

::: tests mode=local
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_addition() {
        let input = CalculateInput { a: 5.0, b: 3.0, operation: Operation::Add };
        let result = calculate(&input).unwrap();
        assert_eq!(result.result, 8.0);
    }

    #[test]
    fn test_division_by_zero() {
        let input = CalculateInput { a: 10.0, b: 0.0, operation: Operation::Divide };
        assert!(calculate(&input).is_err());
    }
}
```
:::

::: reflection
- Why do we check for division by zero before computing, not after?
- What's the advantage of returning a structured error vs failing the tool call?
- How would you add a 'power' operation to this calculator?
:::
