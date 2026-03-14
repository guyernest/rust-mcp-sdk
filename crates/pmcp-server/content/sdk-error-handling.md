# Error Handling

PMCP uses a structured error type with variants for common failure modes.
All public APIs return `pmcp::Result<T>` (alias for `Result<T, pmcp::Error>`).

## Error Variants

```rust
use pmcp::Error;

// Validation errors (bad input)
Err(Error::validation("Email must contain @"))

// Resource not found
Err(Error::not_found("myapp://unknown-resource"))

// Internal errors (unexpected failures)
Err(Error::internal("Database connection lost"))

// Protocol errors (JSON-RPC level)
Err(Error::protocol(ErrorCode::INVALID_PARAMS, "Missing required field"))

// Authentication errors
Err(Error::authentication("Token expired"))

// Timeout
Err(Error::timeout(30_000))
```

## Result Patterns

### Tool Handlers

Return `pmcp::Result<CallToolResult>`:

```rust
async fn call(&self, input: Self::Input, _extra: RequestHandlerExtra)
    -> pmcp::Result<pmcp::CallToolResult>
{
    let result = do_work(&input).map_err(|e| Error::internal(e.to_string()))?;
    Ok(pmcp::CallToolResult::text(result))
}
```

### Resource Handlers

Return `pmcp::Result<ReadResourceResult>`:

```rust
async fn read(&self, uri: &str, _extra: RequestHandlerExtra)
    -> pmcp::Result<ReadResourceResult>
{
    match uri {
        "myapp://data" => Ok(ReadResourceResult::new(vec![content])),
        _ => Err(Error::not_found(uri)),
    }
}
```

## Error Propagation

Use the `?` operator with `anyhow::Error` conversion:

```rust
let data = std::fs::read_to_string(path)?;  // io::Error -> Error::Other
let parsed: Config = serde_json::from_str(&data)?;  // serde -> Error::Serialization
```

## Error Codes

Standard JSON-RPC error codes are available via `ErrorCode`:

| Code    | Constant             | Usage                    |
|---------|----------------------|--------------------------|
| -32700  | PARSE_ERROR          | Malformed JSON           |
| -32600  | INVALID_REQUEST      | Invalid JSON-RPC request |
| -32601  | METHOD_NOT_FOUND     | Unknown method           |
| -32602  | INVALID_PARAMS       | Bad parameters           |
| -32603  | INTERNAL_ERROR       | Server error             |

## Best Practices

- Use `Error::validation()` for user-correctable input errors
- Use `Error::not_found()` for missing resources (returns appropriate MCP error)
- Use `Error::internal()` for unexpected failures (logs stack trace)
- Avoid exposing internal details in error messages sent to clients
