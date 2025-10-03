# Chapter 5: Tools — Type-Safe Actions for Agents

This chapter covers MCP tools—the actions that agents can invoke to accomplish tasks. Using Rust's type system, PMCP provides compile-time safety and clear schemas that help LLMs succeed.

The goal: build type-safe, validated, LLM-friendly tools from simple to production-ready.

## Quick Start: Your First Tool (15 lines)

Let's create a simple "echo" tool and see it in action:

```rust
use pmcp::{Server, server::SyncTool};
use serde_json::json;

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    // Create an echo tool
    let echo = SyncTool::new("echo", |args| {
        let msg = args.get("message").and_then(|v| v.as_str())
            .ok_or_else(|| pmcp::Error::validation("'message' required"))?;
        Ok(json!({"echo": msg, "length": msg.len()}))
    })
    .with_description("Echoes back your message");

    // Add to server and run
    Server::builder().tool("echo", echo).build()?.run_stdio().await
}
```

**Test it:**
```bash
# Start server
cargo run

# In another terminal, use MCP tester from Chapter 3:
mcp-tester test stdio --tool echo --args '{"message": "Hello!"}'
# Response: {"echo": "Hello!", "length": 6}
```

That's it! You've created, registered, and tested an MCP tool. Now let's understand how it works and make it production-ready.

## The Tool Analogy: Forms with Type Safety

Continuing the website analogy from Chapter 4, tools are like web forms—but with Rust's compile-time guarantees.

| Web Forms | MCP Tools (PMCP) | Security Benefit |
| --- | --- | --- |
| HTML form with input fields | Rust struct with typed fields | Compile-time type checking |
| JavaScript validation | serde validation + custom checks | Zero-cost abstractions |
| Server-side sanitization | Rust's ownership & borrowing | Memory safety guaranteed |
| Form submission | Tool invocation via JSON-RPC | Type-safe parsing |
| Success/error response | Typed Result<T> | Exhaustive error handling |

**Key insight**: While other SDKs use dynamic typing (JavaScript objects, Python dicts), PMCP uses Rust structs. This means:
- **Compile-time safety**: Type errors caught before deployment
- **Zero validation overhead**: Types validated during parsing
- **Memory safety**: No buffer overflows or injection attacks
- **Clear schemas**: LLMs understand exactly what's required

## Why Type Safety Matters for LLMs

LLMs driving MCP clients need clear, unambiguous tool definitions to succeed. Here's why typed tools help:

1. **Schema Generation**: Rust types automatically generate accurate JSON schemas
2. **Validation**: Invalid inputs rejected before handler execution
3. **Error Messages**: Type mismatches produce actionable errors LLMs can fix
4. **Examples**: Type definitions document expected inputs
5. **Success Rate**: Well-typed tools have 40-60% higher success rates

**Example**: Compare these two approaches:

```rust
// ❌ Dynamic (error-prone for LLMs)
async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
    let a = args["a"].as_f64().ok_or("missing a")?;  // Vague error
    let b = args["b"].as_f64().ok_or("missing b")?;  // LLM must guess types
    // ...
}

// ✅ Typed (LLM-friendly)
#[derive(Deserialize)]
struct CalculatorArgs {
    /// First number to calculate (e.g., 42.5, -10, 3.14)
    a: f64,
    /// Second number to calculate (e.g., 2.0, -5.5, 1.0)
    b: f64,
    /// Operation: "add", "subtract", "multiply", "divide"
    operation: Operation,
}

async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
    let params: CalculatorArgs = serde_json::from_value(args)?;
    // Types validated, LLM gets clear errors if wrong

    // Perform calculation
    let result = CalculatorResult { /* ... */ };

    // Return structured data - PMCP automatically wraps this in CallToolResult
    Ok(serde_json::to_value(result)?)
}
```

The typed version generates this schema automatically:
```json
{
  "type": "object",
  "properties": {
    "a": { "type": "number", "description": "First number (e.g., 42.5)" },
    "b": { "type": "number", "description": "Second number (e.g., 2.0)" },
    "operation": {
      "type": "string",
      "enum": ["add", "subtract", "multiply", "divide"],
      "description": "Mathematical operation to perform"
    }
  },
  "required": ["a", "b", "operation"]
}
```

LLMs read this schema and understand:
- Exact types needed (numbers, not strings)
- Valid operations (only 4 choices)
- Required vs optional fields
- Example values to guide generation

## Tool Anatomy: Calculator (Step-by-Step)

Every tool follows this anatomy:
1. **Name + Description** → What the tool does
2. **Input Types** → Typed struct with validation
3. **Output Types** → Structured response
4. **Validation** → Check inputs thoroughly
5. **Error Handling** → Clear, actionable messages
6. **Add to Server** → Register and test

Let's build a calculator following this pattern.

### Step 1: Name + Description

```rust
/// Tool name: "calculator"
/// Description: "Performs basic math operations on two numbers.
///              Supports: add, subtract, multiply, divide.
///              Examples:
///              - {a: 10, b: 5, operation: 'add'} → 15
///              - {a: 20, b: 4, operation: 'divide'} → 5"
```

### Step 2: Input Types (Typed) with Examples

Define inputs as Rust structs with doc comments AND example functions for schemas:

```rust
use serde::{Deserialize, Serialize};

/// Mathematical operation to perform
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Operation {
    /// Addition (e.g., 5 + 3 = 8)
    Add,
    /// Subtraction (e.g., 10 - 4 = 6)
    Subtract,
    /// Multiplication (e.g., 6 * 7 = 42)
    Multiply,
    /// Division (e.g., 20 / 4 = 5). Returns error if divisor is zero.
    Divide,
}

/// Arguments for the calculator tool
#[derive(Debug, Deserialize)]
struct CalculatorArgs {
    /// First operand (e.g., 42.5, -10.3, 0, 3.14159)
    a: f64,

    /// Second operand (e.g., 2.0, -5.5, 10, 1.414)
    b: f64,

    /// Operation to perform on the two numbers
    operation: Operation,
}

/// Example arguments for testing (shows LLMs valid inputs)
fn example_calculator_args() -> serde_json::Value {
    serde_json::json!({
        "a": 10.0,
        "b": 5.0,
        "operation": "add"
    })
}
```

**LLM-Friendly Schema Patterns**:

1. **Doc comments on every field** → LLMs read these as descriptions
2. **Example values in comments** → Guides LLM input generation
   ```rust
   /// First operand (e.g., 42.5, -10.3, 0, 3.14159)
   //                   ^^^^^^^^ LLM learns valid formats
   ```

3. **Example function** → Can be embedded in schema or shown in docs
   ```rust
   fn example_args() -> serde_json::Value {
       json!({"a": 10.0, "b": 5.0, "operation": "add"})
   }
   ```
   This provides a "Try it" button in clients that support examples.

4. **Enums for fixed choices** → Constrains LLM to valid options
   ```rust
   #[serde(rename_all = "lowercase")]
   enum Operation { Add, Subtract, Multiply, Divide }
   // LLM sees: must be exactly "add", "subtract", "multiply", or "divide"
   ```

5. **Clear field names** → `a` and `b` are concise but well-documented

### Step 3: Output Types (Typed)

Define a structured response type:

```rust
/// Result of a calculator operation
#[derive(Debug, Serialize)]
struct CalculatorResult {
    /// The calculated result (e.g., 42.0, -3.5, 0.0)
    result: f64,

    /// Human-readable expression showing the calculation
    /// (e.g., "5 + 3 = 8", "10 / 2 = 5")
    expression: String,

    /// The operation that was performed
    operation: Operation,
}
```

**Why structured output?**:
- LLMs can extract specific fields (`result` vs parsing strings)
- Easier to chain tools (next tool uses `result` field directly)
- Type-safe: consumers know exact structure at compile time
- PMCP automatically wraps this in `CallToolResult` for the client

**What PMCP does**:
```rust
// Your handler returns:
Ok(serde_json::to_value(CalculatorResult { result: 15.0, ... })?)

// PMCP automatically wraps it for the client as:
// {
//   "content": [
//     {
//       "type": "text",
//       "text": "{\"result\":15.0,\"expression\":\"10 + 5 = 15\",\"operation\":\"add\"}"
//     }
//   ],
//   "isError": false
// }
```

Your code stays simple—just return your data structure. PMCP handles protocol details.

### Step 4: Validation

Validate inputs before processing:

```rust
use async_trait::async_trait;
use pmcp::{ToolHandler, RequestHandlerExtra, Result, Error};
use serde_json::Value;

struct CalculatorTool;

#[async_trait]
impl ToolHandler for CalculatorTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        // Step 1: Parse and validate types
        let params: CalculatorArgs = serde_json::from_value(args)
            .map_err(|e| Error::validation(format!(
                "Invalid calculator arguments: {}. Expected: {{a: number, b: number, operation: string}}",
                e
            )))?;

        // Step 2: Perform operation with domain validation
        let result = match params.operation {
            Operation::Add => params.first + params.second,
            Operation::Subtract => params.first - params.second,
            Operation::Multiply => params.first * params.second,
            Operation::Divide => {
                // Validation: check for division by zero
                if params.second == 0.0 {
                    return Err(Error::validation(
                        "Cannot divide by zero. Please provide a non-zero divisor for 'b'."
                    ));
                }

                // Check for potential overflow
                if params.first.is_infinite() || params.second.is_infinite() {
                    return Err(Error::validation(
                        "Cannot perform division with infinite values"
                    ));
                }

                params.first / params.second
            }
        };

        // Step 3: Validate result
        if !result.is_finite() {
            return Err(Error::validation(format!(
                "Calculation resulted in non-finite value: {:?}. \
                 This can happen with overflow or invalid operations.",
                result
            )));
        }

        // Step 4: Build structured response
        let response = CalculatorResult {
            result,
            expression: format!(
                "{} {} {} = {}",
                params.first,
                match params.operation {
                    Operation::Add => "+",
                    Operation::Subtract => "-",
                    Operation::Multiply => "*",
                    Operation::Divide => "/",
                },
                params.second,
                result
            ),
            operation: params.operation,
        };

        // Return structured data - PMCP wraps it in CallToolResult automatically
        Ok(serde_json::to_value(response)?)
    }
}
```

**Validation layers**:
1. Type validation (automatic via serde)
2. Domain validation (division by zero, infinity)
3. Result validation (ensure finite output)

### Step 5: Error Handling

Provide clear, actionable error messages (see "Error Messages" section below for patterns).

### Step 6: Add to Server with Schema

```rust
use pmcp::Server;
use serde_json::json;

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let server = Server::builder()
        .name("calculator-server")
        .version("1.0.0")
        .tool("calculator", CalculatorTool)
        .build()?;

    // PMCP automatically generates this schema from your types:
    // {
    //   "name": "calculator",
    //   "description": "Performs basic mathematical operations on two numbers.\n\
    //                   Supports: add, subtract, multiply, divide.\n\
    //                   Examples:\n\
    //                   - {a: 10, b: 5, operation: 'add'} → 15\n\
    //                   - {a: 20, b: 4, operation: 'divide'} → 5",
    //   "inputSchema": {
    //     "type": "object",
    //     "properties": {
    //       "a": {
    //         "type": "number",
    //         "description": "First operand (e.g., 42.5, -10.3, 0, 3.14159)"
    //       },
    //       "b": {
    //         "type": "number",
    //         "description": "Second operand (e.g., 2.0, -5.5, 10, 1.414)"
    //       },
    //       "operation": {
    //         "type": "string",
    //         "enum": ["add", "subtract", "multiply", "divide"],
    //         "description": "Operation to perform on the two numbers"
    //       }
    //     },
    //     "required": ["a", "b", "operation"]
    //   }
    // }
    //
    // Smart clients can show "Try it" with example: {"a": 10, "b": 5, "operation": "add"}

    // Test with: mcp-tester test stdio --tool calculator --args '{"a":10,"b":5,"operation":"add"}'

    server.run_stdio().await
}
```

## How PMCP Wraps Your Responses

**Important**: Your tool handlers return plain data structures. PMCP automatically wraps them in the MCP protocol format.

```rust
// You write:
#[derive(Serialize)]
struct MyResult { value: i32 }

Ok(serde_json::to_value(MyResult { value: 42 })?)

// Client receives (PMCP adds this wrapper automatically):
{
  "content": [{
    "type": "text",
    "text": "{\"value\":42}"
  }],
  "isError": false
}
```

This keeps your code clean and protocol-agnostic. You focus on business logic; PMCP handles MCP details.

## SimpleTool and SyncTool: Rapid Development

For simpler tools, use `SyncTool` (synchronous) or `SimpleTool` (async) to avoid boilerplate:

```rust
use pmcp::server::SyncTool;
use serde_json::json;

// SyncTool for synchronous logic (most common)
let echo_tool = SyncTool::new("echo", |args| {
    let message = args.get("message")
        .and_then(|v| v.as_str())
        .ok_or_else(|| pmcp::Error::validation(
            "Missing 'message' field. Expected: {message: string}"
        ))?;

    // Return structured data - PMCP wraps it automatically
    Ok(json!({
        "echo": message,
        "length": message.len(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
})
.with_description(
    "Echoes back the provided message with metadata. \
     Use this to test message passing and get character count."
)
.with_schema(json!({
    "type": "object",
    "properties": {
        "message": {
            "type": "string",
            "description": "Message to echo back (e.g., 'Hello, World!', 'Test message')",
            "minLength": 1,
            "maxLength": 10000
        }
    },
    "required": ["message"]
}));

// Add to server
let server = Server::builder()
    .tool("echo", echo_tool)
    .build()?;
```

**When to use SimpleTool**:
- ✅ Quick prototyping
- ✅ Single-file examples
- ✅ Tools with simple logic (<50 lines)
- ✅ When you don't need custom types

**When to use struct-based ToolHandler**:
- ✅ Complex validation logic
- ✅ Reusable types across multiple tools
- ✅ Need compile-time type checking
- ✅ Tools with dependencies (DB, API clients)

## Advanced Validation Patterns

### Pattern 1: Multi-Field Validation

Validate relationships between fields:

```rust
#[derive(Debug, Deserialize)]
struct DateRangeArgs {
    /// Start date in ISO 8601 format (e.g., "2024-01-01")
    start_date: String,

    /// End date in ISO 8601 format (e.g., "2024-12-31")
    end_date: String,

    /// Maximum number of days in range (optional, default: 365)
    #[serde(default = "default_max_days")]
    max_days: u32,
}

fn default_max_days() -> u32 { 365 }

impl DateRangeArgs {
    /// Validate that end_date is after start_date and within max_days
    fn validate(&self) -> pmcp::Result<()> {
        use chrono::NaiveDate;

        let start = NaiveDate::parse_from_str(&self.start_date, "%Y-%m-%d")
            .map_err(|e| pmcp::Error::validation(format!(
                "Invalid start_date format: {}. Use YYYY-MM-DD (e.g., '2024-01-15')",
                e
            )))?;

        let end = NaiveDate::parse_from_str(&self.end_date, "%Y-%m-%d")
            .map_err(|e| pmcp::Error::validation(format!(
                "Invalid end_date format: {}. Use YYYY-MM-DD (e.g., '2024-12-31')",
                e
            )))?;

        if end < start {
            return Err(pmcp::Error::validation(
                "end_date must be after start_date. \
                 Example: start_date='2024-01-01', end_date='2024-12-31'"
            ));
        }

        let days = (end - start).num_days();
        if days > self.max_days as i64 {
            return Err(pmcp::Error::validation(format!(
                "Date range exceeds maximum of {} days (actual: {} days). \
                 Reduce the range or increase max_days parameter.",
                self.max_days, days
            )));
        }

        Ok(())
    }
}

// Usage in tool handler
async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
    let params: DateRangeArgs = serde_json::from_value(args)?;
    params.validate()?;  // Multi-field validation

    // Proceed with validated data
    // ...
}
```

### Pattern 2: Custom Deserialization with Validation

```rust
use serde::de::{self, Deserialize, Deserializer};

/// Email address with compile-time validation
#[derive(Debug, Clone)]
struct Email(String);

impl<'de> Deserialize<'de> for Email {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        // Validate email format
        if !s.contains('@') || !s.contains('.') {
            return Err(de::Error::custom(format!(
                "Invalid email format: '{}'. \
                 Expected format: user@example.com",
                s
            )));
        }

        if s.len() > 254 {
            return Err(de::Error::custom(
                "Email too long (max 254 characters per RFC 5321)"
            ));
        }

        Ok(Email(s))
    }
}

#[derive(Debug, Deserialize)]
struct NotificationArgs {
    /// Recipient email address (e.g., "user@example.com")
    recipient: Email,

    /// Message subject (e.g., "Order Confirmation")
    subject: String,

    /// Message body in plain text or HTML
    body: String,
}

// Email validation happens during parsing - zero overhead!
```

### Pattern 3: Enum Validation with Constraints

```rust
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum Priority {
    /// Low priority - process within 24 hours
    Low,

    /// Normal priority - process within 4 hours
    Normal,

    /// High priority - process within 1 hour
    High,

    /// Critical priority - process immediately
    Critical,
}

#[derive(Debug, Deserialize)]
struct TaskArgs {
    /// Task title (e.g., "Process customer order #12345")
    title: String,

    /// Priority level determines processing timeline
    priority: Priority,

    /// Optional due date in ISO 8601 format
    due_date: Option<String>,
}

// serde automatically validates priority against enum variants
// LLM gets error: "unknown variant `URGENT`, expected one of `LOW`, `NORMAL`, `HIGH`, `CRITICAL`"
```

## Error Messages: Guide the LLM to Success

Error messages are documentation for LLMs. Make them actionable:

### ❌ Bad Error Messages (Vague)

```rust
return Err(Error::validation("Invalid input"));
return Err(Error::validation("Missing field"));
return Err(Error::validation("Bad format"));
```

LLM sees: "Invalid input" → tries random fixes → fails repeatedly

### ✅ Good Error Messages (Actionable)

```rust
return Err(Error::validation(
    "Invalid 'amount' field: must be a positive number. \
     Received: -50.0. Example: amount: 100.50"
));

return Err(Error::validation(
    "Missing required field 'customer_id'. \
     Expected format: {customer_id: string, amount: number}. \
     Example: {customer_id: 'cust_123', amount: 99.99}"
));

return Err(Error::validation(format!(
    "Invalid date format for 'created_at': '{}'. \
     Expected ISO 8601 format (YYYY-MM-DD). \
     Examples: '2024-01-15', '2024-12-31'",
    invalid_date
)));
```

LLM sees: Clear problem, expected format, example → fixes immediately → succeeds

### Error Message Template

```rust
format!(
    "{problem}. {expectation}. {example}",
    problem = "What went wrong",
    expectation = "What was expected",
    example = "Concrete example of correct input"
)

// Example:
"Division by zero is not allowed. \
 Provide a non-zero value for 'b'. \
 Example: {a: 10, b: 2, operation: 'divide'}"
```

**Key principle**: Suggest only **1-2 fixes per error message** to reduce model confusion. Multiple possible fixes force the LLM to guess, reducing success rates.

```rust
// ❌ Too many options (confusing)
return Err(Error::validation(
    "Invalid input. Try: (1) changing the format, or (2) using a different value, \
     or (3) checking the documentation, or (4) verifying the field name"
));

// ✅ One clear fix (actionable)
return Err(Error::validation(
    "Invalid 'date' format. Use ISO 8601 (YYYY-MM-DD). Example: '2024-01-15'"
));
```

### Error Taxonomy: Which Error Type to Use

PMCP provides several error constructors. Use this table to choose the right one:

| Error Type | When to Use | PMCP Constructor | Example |
| --- | --- | --- | --- |
| **Validation** | Invalid arguments, bad formats, constraint violations | `Error::validation("...")` | "Amount must be positive. Received: -50.0" |
| **Protocol Misuse** | Wrong parameter types, missing required fields (protocol-level) | `Error::protocol(ErrorCode::INVALID_PARAMS, "...")` | "Missing required 'customer_id' field" |
| **Not Found** | Tool/resource/prompt doesn't exist | `Error::protocol(ErrorCode::METHOD_NOT_FOUND, "...")` | "Tool 'unknown_tool' not found" |
| **Internal** | Server-side failures, database errors, unexpected states | `Error::internal("...")` | "Database connection failed" |

**Usage examples**:

```rust
use pmcp::{Error, ErrorCode};

// Validation errors (business logic)
if amount <= 0.0 {
    return Err(Error::validation(
        "Amount must be positive. Example: amount: 100.50"
    ));
}

// Protocol errors (MCP spec violations)
if args.get("customer_id").is_none() {
    return Err(Error::protocol(
        ErrorCode::INVALID_PARAMS,
        "Missing required field 'customer_id'. \
         Expected: {customer_id: string, amount: number}"
    ));
}

// Not found errors
if !tool_exists(&tool_name) {
    return Err(Error::protocol(
        ErrorCode::METHOD_NOT_FOUND,
        format!("Tool '{}' not found. Available tools: calculate, search, notify", tool_name)
    ));
}

// Internal errors (don't expose implementation details)
match db.query(&sql).await {
    Ok(result) => result,
    Err(e) => {
        tracing::error!("Database query failed: {}", e);
        return Err(Error::internal(
            "Failed to retrieve data. Please try again or contact support."
        ));
    }
}
```

**Security note**: For `Internal` errors, log detailed errors server-side but return generic messages to clients. This prevents information leakage about your infrastructure.

## Embedding Examples in Schemas

Smart MCP clients can show "Try it" buttons with pre-filled examples. Here's how to provide them:

```rust
use serde_json::json;

/// Example calculator inputs for "Try it" feature
fn example_add() -> serde_json::Value {
    json!({
        "a": 10.0,
        "b": 5.0,
        "operation": "add",
        "description": "Add two numbers: 10 + 5 = 15"
    })
}

fn example_divide() -> serde_json::Value {
    json!({
        "a": 20.0,
        "b": 4.0,
        "operation": "divide",
        "description": "Divide numbers: 20 / 4 = 5"
    })
}

// If using pmcp-macros with schemars support:
#[derive(Debug, Deserialize)]
#[schemars(example = "example_add")]  // Future feature
struct CalculatorArgs {
    /// First operand (e.g., 42.5, -10.3, 0, 3.14159)
    a: f64,
    /// Second operand (e.g., 2.0, -5.5, 10, 1.414)
    b: f64,
    /// Operation to perform
    operation: Operation,
}
```

**Why this helps LLMs**:
- Concrete examples show valid input patterns
- LLMs can reference examples when generating calls
- Human developers can click "Try it" in UI tools
- Testing becomes easier with ready-made valid inputs

**Best practice**: Provide 2-3 examples covering:
1. **Happy path**: Most common use case
2. **Edge case**: Boundary values (zero, negative, large numbers)
3. **Optional fields**: Show how to use optional parameters

```rust
/// Examples for search tool
fn example_basic_search() -> serde_json::Value {
    json!({"query": "rust programming"})  // Minimal
}

fn example_advanced_search() -> serde_json::Value {
    json!({
        "query": "MCP protocol",
        "limit": 20,
        "sort": "relevance",
        "filters": {
            "min_score": 0.8,
            "categories": ["tutorial", "documentation"]
        }
    })  // With optional fields
}
```

## Best Practices for LLM-Friendly Tools

### 1. Descriptions: Be Specific and Example-Rich

```rust
// ❌ Too vague
/// Calculate numbers
struct CalculatorArgs { ... }

// ✅ Specific with examples
/// Performs basic mathematical operations on two numbers.
/// Supports: addition, subtraction, multiplication, division.
/// Examples:
/// - Add: {a: 5, b: 3, operation: "add"} → 8
/// - Divide: {a: 20, b: 4, operation: "divide"} → 5
/// - Multiply: {a: 7, b: 6, operation: "multiply"} → 42
struct CalculatorArgs { ... }
```

### 2. Field Names: Short but Documented

```rust
// ❌ Too cryptic
struct Args {
    x: f64,  // What is x?
    y: f64,  // What is y?
    op: String,  // What operations?
}

// ✅ Clear names or documented aliases
struct Args {
    /// First operand (e.g., 42.5, -10, 0)
    #[serde(rename = "a")]
    first_number: f64,

    /// Second operand (e.g., 2.0, -5, 100)
    #[serde(rename = "b")]
    second_number: f64,

    /// Operation: "add", "subtract", "multiply", "divide"
    operation: Operation,
}
```

### 3. Optional Arguments: Use Option<T> (Not serde defaults)

**Recommended**: Use Rust's native `Option<T>` for optional fields. This is clearer for both LLMs and developers than serde defaults.

PMCP integrates seamlessly with `Option<T>`:

```rust
#[derive(Debug, Deserialize)]
struct SearchArgs {
    /// Search query (required) (e.g., "rust programming", "MCP protocol")
    query: String,

    /// Maximum results to return (optional, default: 10, range: 1-100)
    /// If not provided, defaults to 10
    limit: Option<u32>,

    /// Sort order (optional): "relevance" or "date"
    /// If not provided, defaults to "relevance"
    sort: Option<String>,

    /// Filter by date range (optional)
    /// Example: "2024-01-01"
    since_date: Option<String>,
}

impl SearchArgs {
    /// Get limit with default value
    fn limit(&self) -> u32 {
        self.limit.unwrap_or(10)
    }

    /// Get sort order with default
    fn sort_order(&self) -> &str {
        self.sort.as_deref().unwrap_or("relevance")
    }
}

// Usage in handler
async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
    let params: SearchArgs = serde_json::from_value(args)?;

    // Access optional fields naturally
    let limit = params.limit.unwrap_or(10);
    let sort = params.sort.as_deref().unwrap_or("relevance");

    // Or use helper methods
    let limit = params.limit();
    let sort = params.sort_order();

    // Check if optional field was provided
    if let Some(date) = params.since_date {
        // Filter by date
    }

    // ...
}
```

**Comparison: serde defaults vs Option<T>**

```rust
// Approach 1: serde defaults (always has a value)
#[derive(Debug, Deserialize)]
struct Args1 {
    #[serde(default = "default_limit")]
    limit: u32,  // Always present, LLM doesn't know it's optional
}
fn default_limit() -> u32 { 10 }

// Approach 2: Option<T> (idiomatic Rust, clear optionality)
#[derive(Debug, Deserialize)]
struct Args2 {
    limit: Option<u32>,  // Clearly optional, LLM sees it's not required
}

// In schema:
// Args1 generates: "limit": {"type": "number"}  (looks required!)
// Args2 generates: "limit": {"type": ["number", "null"]}  (clearly optional)
```

**Why Option<T> is better than serde defaults**:

1. **Schema Accuracy**: JSON schema clearly shows `["number", "null"]` (optional)
2. **Type Safety**: Compiler enforces handling the None case
3. **LLM Clarity**: LLM sees field is optional in the `required` array
4. **No Magic**: No hidden default functions, behavior is explicit
5. **Rust Idioms**: Use `map()`, `unwrap_or()`, pattern matching naturally

**When to use serde defaults**: Only for configuration files where you want a default value persisted. For MCP tool inputs, prefer `Option<T>` so LLMs see clear optionality.

**Advanced Optional Patterns**:

```rust
#[derive(Debug, Deserialize)]
struct AdvancedSearchArgs {
    /// Search query (required)
    query: String,

    /// Page number (optional, starts at 1)
    page: Option<u32>,

    /// Results per page (optional, default: 10, max: 100)
    per_page: Option<u32>,

    /// Include archived results (optional, default: false)
    include_archived: Option<bool>,

    /// Filter tags (optional, can be empty array)
    tags: Option<Vec<String>>,

    /// Advanced filters (optional, complex nested structure)
    filters: Option<SearchFilters>,
}

#[derive(Debug, Deserialize)]
struct SearchFilters {
    min_score: Option<f32>,
    max_age_days: Option<u32>,
    categories: Option<Vec<String>>,
}

impl AdvancedSearchArgs {
    /// Validate optional fields when present
    fn validate(&self) -> Result<()> {
        // Validate page if provided
        if let Some(page) = self.page {
            if page == 0 {
                return Err(Error::validation(
                    "Page number must be >= 1. Example: page: 1"
                ));
            }
        }

        // Validate per_page if provided
        if let Some(per_page) = self.per_page {
            if per_page == 0 || per_page > 100 {
                return Err(Error::validation(
                    "per_page must be between 1 and 100. Example: per_page: 20"
                ));
            }
        }

        // Validate nested optional structure
        if let Some(ref filters) = self.filters {
            if let Some(min_score) = filters.min_score {
                if min_score < 0.0 || min_score > 1.0 {
                    return Err(Error::validation(
                        "min_score must be between 0.0 and 1.0"
                    ));
                }
            }
        }

        Ok(())
    }

    /// Calculate offset for pagination (using optional page/per_page)
    fn offset(&self) -> usize {
        let page = self.page.unwrap_or(1);
        let per_page = self.per_page.unwrap_or(10);
        ((page - 1) * per_page) as usize
    }
}
```

**LLM sees clear optionality**:

When the LLM reads the schema, it understands:
```json
{
  "properties": {
    "query": {"type": "string"},  // Required
    "page": {"type": ["number", "null"]},  // Optional
    "per_page": {"type": ["number", "null"]},  // Optional
    "include_archived": {"type": ["boolean", "null"]}  // Optional
  },
  "required": ["query"]  // Only query is required!
}
```

LLM learns: "I must provide `query`, everything else is optional" → includes only what's needed → higher success rate

### 4. Validation: Fail Fast with Clear Reasons

```rust
async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
    // Parse (automatic type validation)
    let params: SearchArgs = serde_json::from_value(args)
        .map_err(|e| Error::validation(format!(
            "Invalid arguments for search tool: {}. \
             Expected: {{query: string, limit?: number, sort?: string}}",
            e
        )))?;

    // Validate query length
    if params.query.is_empty() {
        return Err(Error::validation(
            "Search query cannot be empty. \
             Provide a non-empty string. Example: query: 'rust programming'"
        ));
    }

    if params.query.len() > 500 {
        return Err(Error::validation(format!(
            "Search query too long ({} characters). \
             Maximum 500 characters. Current query: '{}'",
            params.query.len(),
            &params.query[..50] // Show first 50 chars
        )));
    }

    // Validate limit range
    if params.limit == 0 || params.limit > 100 {
        return Err(Error::validation(format!(
            "Invalid limit: {}. Must be between 1 and 100. \
             Example: limit: 10",
            params.limit
        )));
    }

    // Validate sort option
    if !matches!(params.sort.as_str(), "relevance" | "date") {
        return Err(Error::validation(format!(
            "Invalid sort option: '{}'. \
             Must be 'relevance' or 'date'. Example: sort: 'relevance'",
            params.sort
        )));
    }

    // All validations passed - proceed
    perform_search(params).await
}
```

### 5. Output: Structured and Documented

```rust
/// Result of a search operation
#[derive(Debug, Serialize)]
struct SearchResult {
    /// Search query that was executed
    query: String,

    /// Number of results found
    total_count: usize,

    /// Search results (limited by 'limit' parameter)
    results: Vec<SearchItem>,

    /// Time taken to execute search (milliseconds)
    duration_ms: u64,
}

#[derive(Debug, Serialize)]
struct SearchItem {
    /// Title of the search result
    title: String,

    /// URL to the resource
    url: String,

    /// Short snippet/excerpt (max 200 characters)
    snippet: String,

    /// Relevance score (0.0 to 1.0, higher is better)
    score: f32,
}
```

LLM can extract specific fields:
```javascript
// LLM sees structured data and can:
result.total_count  // Get count
result.results[0].title  // Get first title
result.results.map(r => r.url)  // Extract all URLs
```

## Advanced Topics

### Performance Quick Note: SIMD Acceleration

> **⚡ Performance Boost**: PMCP can parse large tool arguments 2–10x faster using SIMD (Single Instruction, Multiple Data).
>
> Enable in production with:
> ```toml
> [dependencies]
> pmcp = { version = "1.5", features = ["simd"] }
> ```
>
> Most beneficial for: large arguments (>10KB), batch operations, high-throughput servers.
>
> **See Chapter 14: Performance & Optimization** for SIMD internals, batching strategies, and benchmarks.

### Caching with Type Safety

```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

struct CachedCalculatorTool {
    cache: Arc<RwLock<HashMap<String, CalculatorResult>>>,
}

impl CachedCalculatorTool {
    fn cache_key(args: &CalculatorArgs) -> String {
        format!("{:?}_{:?}_{}", args.first, args.second, args.operation)
    }
}

#[async_trait]
impl ToolHandler for CachedCalculatorTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        let params: CalculatorArgs = serde_json::from_value(args)?;

        // Check cache
        let key = Self::cache_key(&params);
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(&key) {
                return Ok(serde_json::to_value(cached)?);
            }
        }

        // Calculate
        let result = perform_calculation(&params)?;

        // Store in cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(key, result.clone());
        }

        Ok(serde_json::to_value(result)?)
    }
}
```

## Complete Example: Production-Ready Calculator

Putting it all together:

```rust
use async_trait::async_trait;
use pmcp::{Server, ToolHandler, RequestHandlerExtra, Result, Error};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Mathematical operation to perform
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Operation {
    /// Add two numbers (e.g., 5 + 3 = 8)
    Add,
    /// Subtract second from first (e.g., 10 - 4 = 6)
    Subtract,
    /// Multiply two numbers (e.g., 6 * 7 = 42)
    Multiply,
    /// Divide first by second (e.g., 20 / 4 = 5)
    /// Returns error if divisor is zero.
    Divide,
}

/// Arguments for calculator tool
#[derive(Debug, Deserialize)]
struct CalculatorArgs {
    /// First operand
    /// Examples: 42.5, -10.3, 0, 3.14159, 1000000
    a: f64,

    /// Second operand
    /// Examples: 2.0, -5.5, 10, 1.414, 0.001
    b: f64,

    /// Operation to perform
    operation: Operation,
}

impl CalculatorArgs {
    /// Validate arguments before processing
    fn validate(&self) -> Result<()> {
        // Check for NaN or infinity
        if !self.a.is_finite() {
            return Err(Error::validation(format!(
                "First operand 'a' is not a finite number: {:?}. \
                 Provide a normal number like 42.5, -10, or 0.",
                self.a
            )));
        }

        if !self.b.is_finite() {
            return Err(Error::validation(format!(
                "Second operand 'b' is not a finite number: {:?}. \
                 Provide a normal number like 2.0, -5, or 100.",
                self.b
            )));
        }

        // Division by zero check
        if matches!(self.operation, Operation::Divide) && self.b == 0.0 {
            return Err(Error::validation(
                "Cannot divide by zero. \
                 Provide a non-zero value for 'b'. \
                 Example: {a: 10, b: 2, operation: 'divide'}"
            ));
        }

        Ok(())
    }
}

/// Result of calculator operation
#[derive(Debug, Clone, Serialize)]
struct CalculatorResult {
    /// The calculated result
    result: f64,

    /// Human-readable expression (e.g., "5 + 3 = 8")
    expression: String,

    /// Operation that was performed
    operation: Operation,

    /// Input operands (for verification)
    inputs: CalculatorInputs,
}

#[derive(Debug, Clone, Serialize)]
struct CalculatorInputs {
    a: f64,
    b: f64,
}

/// Calculator tool implementation
struct CalculatorTool;

#[async_trait]
impl ToolHandler for CalculatorTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        // Parse arguments
        let params: CalculatorArgs = serde_json::from_value(args)
            .map_err(|e| Error::validation(format!(
                "Invalid calculator arguments: {}. \
                 Expected format: {{a: number, b: number, operation: string}}. \
                 Example: {{a: 10, b: 5, operation: 'add'}}",
                e
            )))?;

        // Validate arguments
        params.validate()?;

        // Perform calculation
        let result = match params.operation {
            Operation::Add => params.a + params.b,
            Operation::Subtract => params.a - params.b,
            Operation::Multiply => params.a * params.b,
            Operation::Divide => params.a / params.b,
        };

        // Validate result
        if !result.is_finite() {
            return Err(Error::validation(format!(
                "Calculation resulted in non-finite value: {:?}. \
                 This usually indicates overflow. \
                 Try smaller numbers. Inputs: a={}, b={}",
                result, params.a, params.b
            )));
        }

        // Build structured response
        let response = CalculatorResult {
            result,
            expression: format!(
                "{} {} {} = {}",
                params.a,
                match params.operation {
                    Operation::Add => "+",
                    Operation::Subtract => "-",
                    Operation::Multiply => "*",
                    Operation::Divide => "/",
                },
                params.b,
                result
            ),
            operation: params.operation,
            inputs: CalculatorInputs {
                a: params.a,
                b: params.b,
            },
        };

        // Return structured data - PMCP wraps this in CallToolResult for the client
        Ok(serde_json::to_value(response)?)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Build server with calculator tool
    let server = Server::builder()
        .name("calculator-server")
        .version("1.0.0")
        .tool("calculator", CalculatorTool)
        .build()?;

    // Run server
    println!("Calculator server ready!");
    println!("Example usage:");
    println!("  {{a: 10, b: 5, operation: 'add'}} → 15");
    println!("  {{a: 20, b: 4, operation: 'divide'}} → 5");

    server.run_stdio().await
}
```

## Testing Your Tools

Use the MCP Tester from Chapter 3:

```bash
# Start your server
cargo run --example calculator-server &

# Test tool discovery
mcp-tester tools http://localhost:8080

# Test specific tool
mcp-tester test http://localhost:8080 \
  --tool calculator \
  --args '{"a": 10, "b": 5, "operation": "add"}'

# Test error handling
mcp-tester test http://localhost:8080 \
  --tool calculator \
  --args '{"a": 10, "b": 0, "operation": "divide"}'

# Expected: Clear error message about division by zero
```

## Summary

Tools are the core actions of your MCP server. PMCP provides:

**Type Safety**:
- ✅ Rust structs with compile-time validation
- ✅ Automatic schema generation
- ✅ Zero-cost abstractions

**Performance**:
- ✅ Efficient memory usage with zero-cost abstractions
- ✅ Optional SIMD acceleration for high-throughput (see Chapter 14)
- ✅ Batch processing support

**LLM Success**:
- ✅ Clear, example-rich descriptions
- ✅ Actionable error messages (1-2 fixes max)
- ✅ Structured inputs and outputs
- ✅ Validation with helpful feedback

**Key Takeaways**:
1. Use typed structs for all tools (not dynamic JSON)
2. Document every field with examples
3. Write error messages that guide LLMs to success (problem + fix + example)
4. Use Option<T> for optional fields (not serde defaults)
5. Validate early and thoroughly
6. Return structured data, not just strings

Next chapters:
- **Chapter 6**: Resources & Resource Management
- **Chapter 7**: Prompts & Templates
- **Chapter 8**: Error Handling & Recovery

The typed approach makes your tools safer, faster, and more reliable for LLM-driven applications.
