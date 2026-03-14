# Typed Tools

PMCP provides typed tool abstractions that handle JSON schema generation,
input validation, and output serialization automatically.

## TypedTool (async)

```rust
use pmcp::server::TypedTool;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct AddInput {
    a: f64,
    b: f64,
}

struct AddTool;

#[async_trait::async_trait]
impl TypedTool for AddTool {
    type Input = AddInput;

    fn metadata(&self) -> pmcp::types::ToolInfo {
        pmcp::types::ToolInfo::new("add", "Add two numbers")
    }

    async fn call(
        &self,
        input: Self::Input,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<pmcp::CallToolResult> {
        let sum = input.a + input.b;
        Ok(pmcp::CallToolResult::text(sum.to_string()))
    }
}
```

Register with the server builder:

```rust
server_builder.tool_typed(AddTool);
```

## TypedSyncTool (synchronous)

For CPU-bound or simple tools that don't need async:

```rust
use pmcp::server::TypedSyncTool;

struct UpperTool;

impl TypedSyncTool for UpperTool {
    type Input = TextInput;

    fn metadata(&self) -> pmcp::types::ToolInfo {
        pmcp::types::ToolInfo::new("upper", "Convert text to uppercase")
    }

    fn call_sync(
        &self,
        input: Self::Input,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<pmcp::CallToolResult> {
        Ok(pmcp::CallToolResult::text(input.text.to_uppercase()))
    }
}
```

## TypedToolWithOutput (structured output)

For tools that return structured JSON via `outputSchema`:

```rust
use pmcp::server::TypedToolWithOutput;

#[derive(Debug, Serialize)]
struct CalcResult {
    value: f64,
    unit: String,
}

struct CalcTool;

#[async_trait::async_trait]
impl TypedToolWithOutput for CalcTool {
    type Input = CalcInput;
    type Output = CalcResult;

    fn metadata(&self) -> pmcp::types::ToolInfo {
        pmcp::types::ToolInfo::new("calc", "Calculate with units")
    }

    async fn call(
        &self,
        input: Self::Input,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<Self::Output> {
        Ok(CalcResult { value: input.value * 2.0, unit: input.unit })
    }
}
```

## MCP Apps Integration

Add widget UI to any tool with `.with_ui()`:

```rust
server_builder.tool_typed(
    MyTool.with_ui(pmcp::types::UIMimeType::Html, "widget://my-widget")
);
```

## Input Validation

Input structs use serde for deserialization. Add validation in `call()`:

```rust
if input.value < 0.0 {
    return Err(pmcp::Error::validation("Value must be non-negative"));
}
```

The JSON schema is auto-generated from the `Input` type's `Deserialize` impl.
