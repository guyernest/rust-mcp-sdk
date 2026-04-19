# JSON-RPC 2.0 Compatibility Analysis

## Issue #38 Resolution

This document addresses [issue #38](https://github.com/paiml/rust-mcp-sdk/issues/38) regarding incompatibility with Claude Code and standard MCP clients due to non-standard JSON-RPC message format.

## Executive Summary

**The compatibility issue was real in pmcp versions prior to 1.4.0** but has been completely resolved:

- **pmcp < 1.4.0**: ❌ Incompatible with Claude Code (used custom message format)
- **pmcp ≥ 1.4.0**: ✅ Fully compatible with Claude Code (standard JSON-RPC 2.0)

Current versions of the Rust MCP SDK (1.4.0+) are fully compatible with JSON-RPC 2.0 and work correctly with Claude Code and TypeScript SDK clients.

## Historical Context

### The Problem (pmcp < 1.4.0)

Earlier versions of pmcp used a custom `TransportMessage` format that was incompatible with standard MCP clients:

```json
// What old pmcp expected (custom format)
{
  "id": 1,
  "request": {
    "Client": {
      "Initialize": {
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {"name": "claude-code", "version": "1.0.0"}
      }
    }
  }
}
```

This caused:
- Connection timeouts with Claude Code
- "Invalid JSON" parse errors
- Complete inability to use Rust-based MCP servers with standard clients

### The Fix (pmcp ≥ 1.4.0)

Version 1.4.0 introduced a JSON-RPC 2.0 compatibility layer that automatically converts between the internal typed representation and standard JSON-RPC format:

```json
// What pmcp 1.4.0+ produces/accepts (standard JSON-RPC 2.0)
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2024-11-05",
    "capabilities": {},
    "clientInfo": {"name": "claude-code", "version": "1.0.0"}
  }
}
```

## Current Implementation

### 1. Serialization Format ✅

The Rust SDK correctly serializes messages to standard JSON-RPC 2.0 format:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2025-06-18",
    "capabilities": {},
    "clientInfo": {
      "name": "test-client",
      "version": "1.0.0"
    }
  }
}
```

### 2. Deserialization Compatibility ✅

The SDK successfully parses standard JSON-RPC 2.0 messages from TypeScript SDK/Claude Code:

- ✅ Request messages with `method` and `params`
- ✅ Response messages with `result` or `error`
- ✅ Notification messages (no `id` field)

### 3. Test Evidence

Added comprehensive test suite in `tests/json_rpc_compatibility.rs` that verifies:

1. **Serialization to JSON-RPC 2.0**: Messages are serialized in standard format
2. **TypeScript SDK compatibility**: Can parse messages from TypeScript SDK
3. **Roundtrip integrity**: Messages survive serialization/deserialization
4. **All message types**: Requests, responses, notifications, errors

All tests pass successfully:

```
test result: ok. 8 passed; 0 failed; 0 ignored
```

### 4. Architecture

The SDK uses an internal abstraction (`TransportMessage`) that provides type safety while maintaining JSON-RPC 2.0 compatibility on the wire:

```rust
// Internal representation (type-safe)
TransportMessage::Request { id, request }
    ↓
// Wire format (JSON-RPC 2.0)
{"jsonrpc": "2.0", "id": 1, "method": "...", "params": {...}}
```

## Common Misconceptions

The issue may have arisen from:

1. **Debug output confusion**: Seeing the internal `TransportMessage` debug format instead of the actual JSON output
2. **Documentation gap**: The internal architecture wasn't clearly documented
3. **Testing approach**: Not testing the actual serialized output

## How to Verify

Run the compatibility tests:

```bash
cargo test --test json_rpc_compatibility
```

Or test manually:

```rust
use pmcp::shared::{TransportMessage, StdioTransport};

// Create any MCP message
let msg = /* ... */;

// Serialize to JSON-RPC 2.0
let json = StdioTransport::serialize_message(&msg)?;
println!("{}", String::from_utf8(json)?);
// Output: {"jsonrpc":"2.0","id":1,"method":"...","params":{...}}
```

## Conclusion

The issue reported in #38 was **valid for pmcp versions prior to 1.4.0**. The incompatibility has been **completely resolved** in current versions.

### Version Summary

| Version | Claude Code Support | Status |
|---------|-------------------|---------|
| pmcp < 1.4.0 | ❌ Incompatible | Used custom message format |
| pmcp ≥ 1.4.0 | ✅ Compatible | Standard JSON-RPC 2.0 |

### Current State (pmcp 1.4.0+)

The Rust MCP SDK now:
- ✅ Implements standard JSON-RPC 2.0 format
- ✅ Is fully compatible with Claude Code
- ✅ Is fully compatible with TypeScript SDK
- ✅ Can parse and generate standard MCP messages
- ✅ Works seamlessly with all standard MCP clients

### Action Required

If you're experiencing connection issues with Claude Code:
1. Check your pmcp version: `cargo tree | grep pmcp`
2. If < 1.4.0, upgrade: `cargo update -p pmcp`
3. See the [Migration Guide](archive/MIGRATION_GUIDE.md) for detailed steps

No code changes are needed beyond upgrading the dependency version.