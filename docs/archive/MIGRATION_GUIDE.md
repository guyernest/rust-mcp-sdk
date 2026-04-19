# Migration Guide: Fixing Claude Code Compatibility

## ⚠️ Critical Compatibility Issue (Resolved in v1.4.0+)

**If your MCP server cannot connect to Claude Code**, you are likely using an older version of pmcp that has a JSON-RPC compatibility issue.

### The Problem

Versions of pmcp prior to v1.4.0 used a custom message format that was incompatible with Claude Code and other standard MCP clients.

#### What Claude Code expects (JSON-RPC 2.0):
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": { ... }
}
```

#### What old pmcp versions expected:
```json
{
  "id": 1,
  "request": {
    "Client": {
      "Initialize": { ... }
    }
  }
}
```

This caused:
- Connection timeouts with Claude Code
- "Invalid JSON" errors
- Complete inability to use Rust-based MCP servers

### The Solution

**Upgrade to pmcp v1.4.1 or later**, which includes full JSON-RPC 2.0 compatibility.

## Migration Steps

### 1. Update your Cargo.toml

```toml
[dependencies]
pmcp = "1.4.1"  # or latest version
```

### 2. Remove any compatibility workarounds

If you have custom JSON-RPC conversion code, remove it. The SDK now handles this automatically.

#### Before (workaround code):
```rust
// Custom JSON-RPC to TransportMessage conversion
fn convert_jsonrpc_to_transport(json: Value) -> TransportMessage {
    // Complex conversion logic
}
```

#### After (just use the SDK):
```rust
// No conversion needed - SDK handles it internally
server.run_stdio().await?;
```

### 3. Update and rebuild

```bash
cargo update
cargo clean
cargo build --release
```

### 4. Test with Claude Code

```bash
# Add your server to Claude Code
claude mcp add my-server ./target/release/my-mcp-server

# Test the connection
claude mcp test my-server
```

## Common Issues and Solutions

### Issue: "data did not match any variant of untagged enum TransportMessage"

**Cause**: Using pmcp < 1.4.0 with Claude Code  
**Solution**: Upgrade to pmcp 1.4.1+

### Issue: Connection timeout after 30 seconds

**Cause**: Server cannot parse JSON-RPC messages from Claude Code  
**Solution**: Upgrade to pmcp 1.4.1+

### Issue: Server works with custom client but not Claude Code

**Cause**: Custom client using old pmcp message format  
**Solution**: Upgrade both server and client to pmcp 1.4.1+

## Version Compatibility Matrix

| pmcp Version | Claude Code | TypeScript SDK | Python SDK | Notes |
|-------------|-------------|----------------|------------|-------|
| < 1.4.0     | ❌          | ❌             | ❌         | Custom format only |
| 1.4.0       | ✅          | ✅             | ✅         | JSON-RPC 2.0 added |
| 1.4.1+      | ✅          | ✅             | ✅         | Recommended |

## Example: Minimal Claude Code Compatible Server

```rust
use pmcp::{Server, ServerCapabilities, ToolHandler};
use async_trait::async_trait;

struct MyTool;

#[async_trait]
impl ToolHandler for MyTool {
    async fn handle(
        &self, 
        args: serde_json::Value, 
        _extra: pmcp::RequestHandlerExtra
    ) -> Result<serde_json::Value, pmcp::Error> {
        Ok(serde_json::json!({
            "result": "Tool executed successfully"
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = Server::builder()
        .name("my-mcp-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .tool("my_tool", MyTool)
        .build()?;

    // This now works with Claude Code!
    server.run_stdio().await?;
    Ok(())
}
```

## Testing Your Migration

1. **Create a test script** (`test_connection.sh`):
```bash
#!/bin/bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' | \
    timeout 5 ./target/release/my-mcp-server
```

2. **Expected output** (successful connection):
```json
{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{...},"serverInfo":{...}}}
```

## For Library Authors

If you maintain a library that depends on pmcp:

1. Update your dependency to require pmcp 1.4.1+:
```toml
[dependencies]
pmcp = "^1.4.1"
```

2. Add a note in your README about Claude Code compatibility
3. Consider adding a compatibility test

## Need Help?

- Check the [JSON-RPC compatibility tests](tests/json_rpc_compatibility.rs)
- See the [compatibility documentation](docs/JSON_RPC_COMPATIBILITY.md)
- Open an issue if you encounter problems after upgrading

---

**Remember**: The compatibility issue is completely resolved in pmcp 1.4.1+. A simple version upgrade is all you need!