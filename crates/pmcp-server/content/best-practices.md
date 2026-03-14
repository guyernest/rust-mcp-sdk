# Best Practices

Guidelines for building robust MCP servers with PMCP SDK.

## Tool Design

- **Single responsibility**: Each tool does one thing well
- **Descriptive names**: Use verb-noun format (`search_logs`, `create_user`)
- **Input validation**: Validate early in `call()` with `Error::validation()`
- **Structured output**: Use `TypedToolWithOutput` for machine-readable results
- **Error messages**: Return user-actionable error descriptions

```rust
// Good: clear, specific tool
struct SearchLogs;  // Searches application logs by query

// Avoid: vague, overloaded tool
struct DoStuff;     // Does many unrelated things
```

## Resource Organization

- **Stable URIs**: Don't change URIs between versions
- **Consistent naming**: Use `scheme://category/item` pattern
- **MIME types**: Always set `mime_type` for proper client rendering
- **Descriptions**: Every resource needs a human-readable description
- **Embedded content**: Use `include_str!` for static docs (no runtime I/O)

## Error Handling

- Return `Error::validation()` for bad user input (client can retry with fixes)
- Return `Error::not_found()` for missing resources
- Return `Error::internal()` for unexpected server failures
- Never expose stack traces or internal paths to clients
- Log detailed errors server-side with `tracing::error!`

## Testing

### Unit tests
```bash
cargo test -p my-server
```

### Protocol compliance
```bash
cargo pmcp test check http://localhost:8080
```

### MCP Apps validation
```bash
cargo pmcp test apps http://localhost:8080
```

### Test scenarios
```bash
cargo pmcp test generate http://localhost:8080 > scenarios.json
cargo pmcp test run http://localhost:8080 --scenarios scenarios.json
```

## Performance

- **Async by default**: Use `TypedTool` (async) unless CPU-bound
- **Embedded content**: `include_str!` avoids file I/O at runtime
- **Connection pooling**: Reuse HTTP clients across tool calls
- **Pagination**: Use cursors for large resource lists (> 50 items)

## Server Configuration

```rust
let server = pmcp::Server::builder()
    .name("my-server")
    .version(env!("CARGO_PKG_VERSION"))
    .tool_typed(MyTool)
    .prompt(MyPrompt)
    .resource_handler("myapp://", MyResources)
    .build()?;
```

## Deployment Checklist

1. All tools have descriptions and input schemas
2. Resources return proper MIME types
3. Prompts have metadata with argument descriptions
4. Error handling covers all edge cases
5. `cargo pmcp test check` passes
6. `cargo pmcp test apps` passes (if using MCP Apps)
