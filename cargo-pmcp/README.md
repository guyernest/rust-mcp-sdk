# cargo-pmcp

Production-grade MCP server development toolkit.

## Overview

`cargo-pmcp` is a comprehensive scaffolding and testing tool for building Model Context Protocol (MCP) servers using the PMCP SDK. It streamlines the entire development workflow from project creation to automated testing.

## Features

- **Project Scaffolding**: Create new MCP server workspaces with best practices built-in
- **Server Management**: Add multiple MCP servers to a single workspace
- **Development Mode**: Hot-reload MCP servers with HTTP transport for rapid development
- **Automated Testing**: Generate and run comprehensive test scenarios for your MCP servers
- **Smart Test Generation**: Automatically creates meaningful test cases with realistic values

## Installation

```bash
cargo install cargo-pmcp
```

## Quick Start

### 1. Create a New Workspace

```bash
cargo pmcp new my-mcp-workspace
cd my-mcp-workspace
```

### 2. Add an MCP Server

```bash
cargo pmcp add calculator --tools --resources
```

This creates:
- A new MCP server with example tools and resources
- A `scenarios/calculator/` directory for test scenarios
- Template code ready for customization

### 3. Develop Your Server

```bash
cargo pmcp dev --server calculator
```

This starts the server with hot-reload enabled on `http://0.0.0.0:3000`.

### 4. Test Your Server

Generate test scenarios:

```bash
# In another terminal, with server running
cargo pmcp test --server calculator --generate-scenarios
```

Run tests:

```bash
cargo pmcp test --server calculator
```

## Commands

### `new <name>`

Create a new MCP server workspace.

**Options:**
- `--description` - Optional workspace description

**Example:**
```bash
cargo pmcp new my-workspace --description "My MCP servers"
```

### `add <name>`

Add a new MCP server to the current workspace.

**Options:**
- `--tools` - Include example tool implementations
- `--resources` - Include example resource implementations
- `--prompts` - Include example prompt implementations

**Example:**
```bash
cargo pmcp add calculator --tools --resources
```

### `dev --server <name>`

Start an MCP server in development mode with HTTP transport.

**Options:**
- `--server` - Name of the server to run
- `--port` - Port to listen on (default: 3000)

**Example:**
```bash
cargo pmcp dev --server calculator --port 8080
```

### `test --server <name>`

Test an MCP server using scenario-based testing.

**Prerequisites:**
- Server must be running in another terminal (use `cargo pmcp dev --server <name>`)

**Options:**
- `--server` - Name of the server to test
- `--port` - Port the server is running on (default: 3000)
- `--generate-scenarios` - Generate test scenarios from server schema
- `--detailed` - Show detailed test output

**Example:**
```bash
# Terminal 1: Start server
cargo pmcp dev --server calculator

# Terminal 2: Generate and run tests
cargo pmcp test --server calculator --generate-scenarios
cargo pmcp test --server calculator --detailed
```

## Test Scenarios

Test scenarios are YAML files that define test steps and assertions for your MCP server.

### Generating Scenarios

The `--generate-scenarios` flag discovers your server's capabilities and generates smart test cases:

```bash
cargo pmcp test --server calculator --generate-scenarios
```

This creates `scenarios/calculator/generated.yaml` with:
- Smart test values (e.g., `add(123, 234) = 357`)
- Realistic assertions
- Tool, resource, and prompt test coverage

### Scenario Format

```yaml
name: "Calculator Test Scenario"
description: "Test calculator operations"
timeout: 60
stop_on_failure: false

steps:
  - name: "Test addition"
    operation:
      type: tool_call
      tool: "add"
      arguments:
        a: 123
        b: 234
    assertions:
      - type: success
      - type: equals
        path: "result"
        value: 357
```

### MCP Response Format

MCP tool responses are wrapped in a `content` array. The actual result is in `content[0].text`:

```json
{
  "result": {
    "content": [{
      "type": "text",
      "text": "{\"result\":357.0,\"operation\":\"123 + 234 = 357\"}"
    }]
  }
}
```

To assert on nested values, use JSON path notation or adjust the generated scenarios.

## Workflow

The typical development workflow:

1. **Create workspace**: `cargo pmcp new my-workspace`
2. **Add server**: `cargo pmcp add myserver --tools`
3. **Implement features**: Edit code in `crates/myserver/`
4. **Start dev server**: `cargo pmcp dev --server myserver`
5. **Generate tests**: `cargo pmcp test --server myserver --generate-scenarios`
6. **Customize tests**: Edit `scenarios/myserver/generated.yaml`
7. **Run tests**: `cargo pmcp test --server myserver`
8. **Iterate**: Make changes and repeat from step 4

## Architecture

- **Workspace**: Top-level Cargo workspace containing multiple MCP servers
- **Server**: Individual MCP server crate with its own capabilities
- **Scenarios**: YAML test definitions for each server
- **Templates**: Code generation templates for consistent server structure

## Requirements

- Rust 1.70 or later
- Cargo

## License

MIT

## Contributing

See the main PMCP SDK repository for contributing guidelines.
