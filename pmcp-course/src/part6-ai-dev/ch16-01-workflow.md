# The Development Workflow

This chapter walks through the complete cargo-pmcp workflow for AI-assisted MCP server development. Following this workflow ensures consistent, high-quality results.

## The Standard Workflow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                  cargo-pmcp Development Workflow                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌───────────────────┐                                                  │
│  │ 1. Scaffold       │ cargo pmcp new <workspace>                       │
│  │    Workspace      │ cargo pmcp add server <name> --template minimal  │
│  └─────────┬─────────┘                                                  │
│            │                                                            │
│            ▼                                                            │
│  ┌───────────────────┐                                                  │
│  │ 2. Implement      │ Edit crates/mcp-<name>-core/src/tools/*.rs       │
│  │    Tools          │ Register tools in lib.rs                         │
│  └─────────┬─────────┘                                                  │
│            │                                                            │
│            ▼                                                            │
│  ┌───────────────────┐                                                  │
│  │ 3. Development    │ cargo pmcp dev --server <name>                   │
│  │    Server         │ Hot-reload on http://0.0.0.0:3000                │
│  └─────────┬─────────┘                                                  │
│            │                                                            │
│            ▼                                                            │
│  ┌───────────────────┐                                                  │
│  │ 4. Generate       │ cargo pmcp test --server <name>                  │
│  │    Tests          │     --generate-scenarios                         │
│  └─────────┬─────────┘                                                  │
│            │                                                            │
│            ▼                                                            │
│  ┌───────────────────┐                                                  │
│  │ 5. Run Tests      │ cargo pmcp test --server <name>                  │
│  │                   │ cargo test                                       │
│  └─────────┬─────────┘                                                  │
│            │                                                            │
│            ▼                                                            │
│  ┌───────────────────┐                                                  │
│  │ 6. Quality        │ cargo fmt --check                                │
│  │    Gates          │ cargo clippy -- -D warnings                      │
│  └─────────┬─────────┘                                                  │
│            │                                                            │
│            ▼                                                            │
│  ┌───────────────────┐                                                  │
│  │ 7. Production     │ cargo build --release                            │
│  │    Build          │ Configure MCP client                             │
│  └───────────────────┘                                                  │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## Step 1: Scaffold Workspace

### Create New Workspace

**Request to AI**:
```
Create a new MCP workspace called "weather-mcp-workspace"
```

**AI executes**:
```bash
cargo pmcp new weather-mcp-workspace
cd weather-mcp-workspace
```

**Generated structure**:
```
weather-mcp-workspace/
├── Cargo.toml              # Workspace manifest
├── Cargo.lock
├── .gitignore
├── README.md
├── crates/
│   └── server-common/      # Shared transport helpers
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           └── http.rs
└── scenarios/              # Test scenarios directory
```

### Add Server to Workspace

**Request to AI**:
```
Add a weather server using the minimal template
```

**AI executes**:
```bash
cargo pmcp add server weather --template minimal
```

**Extended structure**:
```
crates/
├── server-common/
├── mcp-weather-core/       # NEW - Library crate
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs          # Server builder
│       ├── tools/          # Tool implementations
│       │   └── mod.rs
│       ├── resources/      # Resource handlers
│       └── workflows/      # Workflow definitions
└── weather-server/         # NEW - Binary crate
    ├── Cargo.toml
    └── src/
        └── main.rs         # Entry point

scenarios/
└── weather/               # NEW - Test scenarios
```

### Template Options

| Template | Use Case | Includes |
|----------|----------|----------|
| `minimal` | Custom servers | Empty structure |
| `calculator` | Learning MCP | Single add tool |
| `complete_calculator` | Reference implementation | 5 tools, tests, prompts |
| `sqlite_explorer` | Database servers | DB connection, queries |

**Rule**: Always use templates. Never create this structure manually.

## Step 2: Implement Tools

### Tool File Structure

Each tool lives in its own file:

```
crates/mcp-weather-core/src/tools/
├── mod.rs              # Module exports
├── get_current.rs      # Current weather tool
└── get_forecast.rs     # Forecast tool
```

### Adding a Tool

**Request to AI**:
```
Add a get_current_weather tool that takes a city name and
returns temperature (Celsius), humidity, and conditions
```

**AI creates** `tools/get_current.rs`:

```rust
use pmcp::{Error, RequestHandlerExtra, Result, TypedTool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Input for current weather lookup
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct GetCurrentInput {
    /// City name (e.g., "London", "New York")
    #[schemars(description = "City name to get weather for")]
    pub city: String,
}

/// Current weather data
#[derive(Debug, Serialize, JsonSchema)]
pub struct GetCurrentOutput {
    /// City name
    pub city: String,
    /// Temperature in Celsius
    pub temperature_celsius: f64,
    /// Relative humidity percentage
    pub humidity: u8,
    /// Weather conditions (e.g., "Sunny", "Cloudy")
    pub conditions: String,
}

async fn handler(
    input: GetCurrentInput,
    _extra: RequestHandlerExtra,
) -> Result<GetCurrentOutput> {
    // Validate input
    if input.city.trim().is_empty() {
        return Err(Error::validation("City name cannot be empty"));
    }

    // In production, call weather API here
    // For now, return mock data
    Ok(GetCurrentOutput {
        city: input.city,
        temperature_celsius: 18.5,
        humidity: 65,
        conditions: "Partly Cloudy".to_string(),
    })
}

pub fn build_tool() -> TypedTool<GetCurrentInput, GetCurrentOutput> {
    TypedTool::new("get-current-weather", |input, extra| {
        Box::pin(handler(input, extra))
    })
    .with_description("Get current weather conditions for a city")
}
```

### Register in lib.rs

**AI updates** `lib.rs`:

```rust
pub mod tools;

use pmcp::{McpServer, Result};

pub fn build_weather_server() -> Result<McpServer> {
    McpServer::builder("weather-server", "1.0.0")
        .tool("get-current-weather", tools::get_current::build_tool())
        .build()
}
```

### Update mod.rs

**AI updates** `tools/mod.rs`:

```rust
pub mod get_current;
```

## Step 3: Development Server

### Start Hot-Reload Server

**Request to AI**:
```
Start the development server for weather
```

**AI executes**:
```bash
cargo pmcp dev --server weather
```

**Output**:
```
Building weather-server...
   Compiling mcp-weather-core v1.0.0
   Compiling weather-server v1.0.0
    Finished dev [unoptimized + debuginfo] target(s) in 2.34s

MCP server running on http://0.0.0.0:3000

Capabilities:
  - tools: get-current-weather

Watching for changes...
[INFO] Server ready to accept connections
```

### Iterating with Hot-Reload

When you request changes:

```
Add validation for city name length (max 100 characters)
```

AI edits the tool, hot-reload automatically rebuilds:

```
[INFO] File changed: crates/mcp-weather-core/src/tools/get_current.rs
[INFO] Rebuilding...
   Compiling mcp-weather-core v1.0.0
    Finished dev [unoptimized + debuginfo] target(s) in 0.89s
[INFO] Server restarted
```

### Custom Port

```bash
cargo pmcp dev --server weather --port 8080
```

## Step 4: Generate Test Scenarios

### Auto-Generate from Server

**Request to AI**:
```
Generate test scenarios for the weather server
```

**AI executes** (in another terminal):
```bash
cargo pmcp test --server weather --generate-scenarios
```

**Generated** `scenarios/weather/generated.yaml`:

```yaml
name: "Weather Server Tests"
description: "Auto-generated tests for weather server"
timeout: 60
stop_on_failure: false

steps:
  - name: "Test get-current-weather with valid city"
    operation:
      type: tool_call
      tool: "get-current-weather"
      arguments:
        city: "London"
    assertions:
      - type: success
      - type: field_exists
        path: "content.0.text"

  - name: "Test get-current-weather with empty city"
    operation:
      type: tool_call
      tool: "get-current-weather"
      arguments:
        city: ""
    assertions:
      - type: error
      - type: contains
        path: "error.message"
        value: "cannot be empty"
```

### Manual Scenario Customization

Edit generated scenarios to add edge cases:

```yaml
  - name: "Test Unicode city name"
    operation:
      type: tool_call
      tool: "get-current-weather"
      arguments:
        city: "東京"  # Tokyo in Japanese
    assertions:
      - type: success

  - name: "Test very long city name"
    operation:
      type: tool_call
      tool: "get-current-weather"
      arguments:
        city: "A very long city name that exceeds the maximum..."
    assertions:
      - type: error
```

## Step 5: Run Tests

### Integration Tests

```bash
# Dev server must be running
cargo pmcp test --server weather
```

**Output**:
```
Running scenarios for weather server...

Scenario: Weather Server Tests
  ✓ Test get-current-weather with valid city (15ms)
  ✓ Test get-current-weather with empty city (8ms)
  ✓ Test Unicode city name (12ms)
  ✓ Test very long city name (7ms)

Results: 4 passed, 0 failed
```

### Unit Tests

**Request to AI**:
```
Add unit tests for the get_current handler
```

**AI adds** to `tools/get_current.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_valid_city() {
        let input = GetCurrentInput {
            city: "London".to_string(),
        };
        let result = handler(input, RequestHandlerExtra::default()).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.city, "London");
    }

    #[tokio::test]
    async fn test_empty_city() {
        let input = GetCurrentInput {
            city: "".to_string(),
        };
        let result = handler(input, RequestHandlerExtra::default()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_whitespace_only_city() {
        let input = GetCurrentInput {
            city: "   ".to_string(),
        };
        let result = handler(input, RequestHandlerExtra::default()).await;
        assert!(result.is_err());
    }
}
```

**Run unit tests**:
```bash
cargo test
```

## Step 6: Quality Gates

### Format Check

```bash
cargo fmt --check
```

If formatting issues exist:
```bash
cargo fmt  # Auto-fix
```

### Clippy Linting

```bash
cargo clippy -- -D warnings
```

**Common issues AI fixes**:

```rust
// Before: Clippy warning about redundant clone
let city = input.city.clone();

// After: AI fix
let city = input.city;
```

### All Quality Gates

**Request to AI**:
```
Run all quality gates and fix any issues
```

**AI executes**:
```bash
cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

If any fail, AI iterates until all pass.

### Makefile Integration

For projects with Makefile:

```bash
make quality-gate
```

## Step 7: Production Build

### Release Build

```bash
cargo build --release
```

**Binary location**:
```
target/release/weather-server
```

### Configure MCP Client

**Claude Code** (`~/.claude/mcp_servers.json`):
```json
{
  "weather": {
    "command": "/path/to/weather-mcp-workspace/target/release/weather-server",
    "args": [],
    "env": {
      "WEATHER_API_KEY": "${env:WEATHER_API_KEY}"
    }
  }
}
```

**Kiro** (`.kiro/settings.json`):
```json
{
  "mcpServers": {
    "weather": {
      "command": "/path/to/weather-server",
      "args": []
    }
  }
}
```

## Complete Session Example

### Initial Request

```
Create an MCP server for managing Kubernetes pods.
Include tools to list pods, get pod details, and view logs.
```

### AI Workflow

```bash
# Step 1: Scaffold
$ cargo pmcp new k8s-mcp-workspace
$ cd k8s-mcp-workspace
$ cargo pmcp add server k8s --template minimal

# Step 2: Implement (AI edits files)
# Creates: list_pods.rs, get_pod.rs, get_logs.rs

# Step 3: Dev server
$ cargo pmcp dev --server k8s

# Step 4: Generate tests (in another terminal)
$ cargo pmcp test --server k8s --generate-scenarios

# Step 5: Run tests
$ cargo pmcp test --server k8s
$ cargo test

# Step 6: Quality gates
$ cargo fmt --check
$ cargo clippy -- -D warnings

# Step 7: Build
$ cargo build --release
```

### Iteration Cycle

When issues arise:

```
User: The list_pods tool should filter by namespace

AI: I'll update the input type to accept an optional namespace parameter.
[Edits list_pods.rs]

$ cargo build  # Check compilation
$ cargo test   # Verify behavior
$ cargo clippy -- -D warnings  # Quality check

All gates passing. The tool now accepts an optional 'namespace' parameter.
```

## Workflow Decision Tree

```
Start
  │
  ├─ New project?
  │     │
  │     Yes → cargo pmcp new <workspace>
  │             cargo pmcp add server <name> --template minimal
  │
  ├─ Add server to existing workspace?
  │     │
  │     Yes → cargo pmcp add server <name> --template <template>
  │
  ├─ Add tool to existing server?
  │     │
  │     Yes → cargo pmcp add tool <tool> --server <server>
  │           (or manually create in tools/)
  │
  ├─ Test changes?
  │     │
  │     Yes → cargo pmcp dev --server <name>  (terminal 1)
  │           cargo pmcp test --server <name>  (terminal 2)
  │
  └─ Ready for production?
        │
        Yes → cargo fmt --check
              cargo clippy -- -D warnings
              cargo test
              cargo build --release
```

## Summary

The cargo-pmcp workflow:

1. **Scaffold** - Never create files manually
2. **Implement** - Focus on tool logic, not boilerplate
3. **Dev Server** - Hot-reload for fast iteration
4. **Test Generation** - Smart scenarios from schema
5. **Test Execution** - Integration + unit tests
6. **Quality Gates** - Format, lint, test
7. **Production** - Release build and client config

Following this workflow with AI assistance transforms MCP server development from hours of setup to minutes of implementation.

---

*Continue to [Prompting for MCP Tools](./ch16-02-prompting.md) →*
