---
inclusion: always
---

# MCP Project Structure

## Workspace Layout (cargo-pmcp Standard)

The cargo-pmcp toolkit generates a Cargo workspace structure that follows best practices from 6 production servers:

```
my-mcp-workspace/
├── Cargo.toml                    # Workspace manifest
├── Cargo.lock                    # Dependency lock file
├── .gitignore                    # Git ignore patterns
├── README.md                     # Workspace documentation
│
├── crates/                       # All crates in workspace
│   ├── server-common/            # Shared transport code
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs            # Module exports
│   │       └── http.rs           # HTTP transport helpers
│   │
│   ├── mcp-myserver-core/        # Server implementation (library)
│   │   ├── Cargo.toml
│   │   ├── README.md
│   │   └── src/
│   │       ├── lib.rs            # Server struct + builder
│   │       ├── tools/            # Tool implementations
│   │       │   ├── mod.rs        # Tool module exports
│   │       │   ├── calculate.rs  # Example tool
│   │       │   └── convert.rs    # Another tool
│   │       ├── resources/        # Resource implementations
│   │       │   ├── mod.rs        # Resource module exports
│   │       │   └── data.rs       # Example resource
│   │       ├── prompts/          # Prompt templates
│   │       │   ├── mod.rs        # Prompt module exports
│   │       │   └── review.rs     # Example prompt
│   │       └── workflows/        # Workflow definitions (NEW)
│   │           ├── mod.rs        # Workflow module exports
│   │           └── pipeline.rs   # Example workflow
│   │
│   └── myserver-server/          # Binary entry point
│       ├── Cargo.toml
│       └── src/
│           └── main.rs           # Transport selection + startup
│
├── scenarios/                    # mcp-tester test scenarios
│   └── myserver/
│       ├── basic.yaml            # Basic functionality tests
│       ├── error_handling.yaml   # Error case tests
│       └── generated.yaml        # Auto-generated scenarios
│
├── docs/                         # Documentation (optional)
│   ├── API.md
│   └── DEVELOPMENT.md
│
└── examples/                     # Usage examples (optional)
    └── simple_usage.rs
```

## Why This Structure?

### Workspace Benefits
- **Code Sharing**: `server-common` shared across all servers
- **Parallel Builds**: Cargo builds crates in parallel
- **Unified Dependencies**: Single `Cargo.lock` for consistency
- **Easy Testing**: `cargo test` runs all tests

### Core Library (`mcp-{name}-core`)
- **Reusability**: Can be used as dependency in other projects
- **Testing**: Unit tests alongside implementation
- **Documentation**: `cargo doc` generates API docs
- **Independence**: No transport coupling

### Binary Crate (`{name}-server`)
- **Deployment**: Single binary for production
- **Transport Selection**: Choose stdio/HTTP/WebSocket
- **Configuration**: Environment-specific settings
- **Minimal Code**: Just glue between core and transport

### server-common Crate
- **DRY Principle**: HTTP setup shared across servers
- **Consistency**: All servers use same transport patterns
- **Maintenance**: Update once, benefits all servers

## Naming Conventions

### Crate Names

| Type | Pattern | Example | Rationale |
|------|---------|---------|-----------|
| Workspace | `{name}-mcp-workspace` | `weather-mcp-workspace` | Clear purpose |
| Core library | `mcp-{name}-core` | `mcp-weather-core` | MCP prefix for discovery |
| Binary | `{name}-server` | `weather-server` | Deployable name |
| Shared | `server-common` | `server-common` | Consistent across workspaces |

### Code Names

| Element | Convention | Example | Notes |
|---------|-----------|---------|-------|
| Tool names (code) | `snake_case` | `get_forecast` | Rust convention |
| Tool names (MCP) | `kebab-case` | `get-forecast` | MCP protocol standard |
| Types | `PascalCase` | `WeatherInput` | Rust convention |
| Functions | `snake_case` | `fetch_weather_data` | Rust convention |
| Resources URIs | `{server}://{category}/{item}` | `weather://forecast/london` | MCP standard |

### File Names

| Type | Pattern | Example |
|------|---------|---------|
| Tool modules | `{tool_name}.rs` | `get_forecast.rs` |
| Resource modules | `{resource_type}.rs` | `forecast_data.rs` |
| Prompt modules | `{prompt_name}.rs` | `weather_alert.rs` |
| Workflow modules | `{workflow_name}.rs` | `daily_summary.rs` |
| Tests | `{module}_test.rs` or inline | `weather_test.rs` |

## Core Library Structure (lib.rs)

### Standard Layout

```rust
//! MCP Weather Server
//!
//! Provides weather data access via MCP protocol.
//!
//! ## Features
//! - Current weather conditions
//! - 5-day forecasts
//! - Weather alerts
//!
//! ## Example
//! ```no_run
//! use mcp_weather_core::WeatherServer;
//!
//! let server = WeatherServer::builder()
//!     .with_api_key("your-key")
//!     .build()?;
//! ```

// Re-export pmcp types for convenience
pub use pmcp::{Result, Server, TypedTool};

// Module declarations
pub mod tools;
pub mod resources;
pub mod prompts;
pub mod workflows;

// Internal modules
mod types;
mod client;  // If wrapping external API
mod error;   // If custom error types needed

// Main server struct
use pmcp::types::{ServerCapabilities, ToolCapabilities};

/// Weather MCP server
pub struct WeatherServer {
    api_key: String,
    // ... other fields
}

/// Builder for WeatherServer
pub struct WeatherServerBuilder {
    api_key: Option<String>,
    // ... other fields
}

impl WeatherServer {
    /// Create a new builder
    pub fn builder() -> WeatherServerBuilder {
        WeatherServerBuilder::default()
    }

    /// Build the MCP server
    pub fn build(self) -> Result<Server> {
        Server::builder()
            .name("weather")
            .version(env!("CARGO_PKG_VERSION"))
            .capabilities(ServerCapabilities {
                tools: Some(ToolCapabilities {
                    list_changed: Some(true)
                }),
                resources: Some(Default::default()),
                ..Default::default()
            })
            // Register tools
            .tool(
                "get-current-weather",
                tools::current::build_tool(self.api_key.clone())
            )
            .tool(
                "get-forecast",
                tools::forecast::build_tool(self.api_key.clone())
            )
            // Register resources
            .resource_handler(|uri| resources::handle_resource(uri, self.api_key.clone()))
            .build()
    }
}

impl WeatherServerBuilder {
    /// Set the API key
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Build the server
    pub fn build(self) -> Result<WeatherServer> {
        let api_key = self.api_key
            .ok_or_else(|| pmcp::Error::validation("API key is required"))?;

        Ok(WeatherServer { api_key })
    }
}

impl Default for WeatherServerBuilder {
    fn default() -> Self {
        Self {
            api_key: None,
        }
    }
}
```

## Tool Module Organization

### Pattern 1: Simple Tool (Single File)

**File**: `src/tools/calculate.rs`

```rust
use pmcp::{Result, TypedTool, RequestHandlerExtra};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ============================================================================
// TYPE DEFINITIONS
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct CalculateInput {
    #[schemars(description = "Operation to perform: add, subtract, multiply, divide")]
    pub operation: String,

    #[schemars(description = "First operand")]
    pub a: f64,

    #[schemars(description = "Second operand")]
    pub b: f64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CalculateOutput {
    pub result: f64,
    pub description: String,
}

// ============================================================================
// HANDLER
// ============================================================================

async fn handler(input: CalculateInput, _extra: RequestHandlerExtra) -> Result<CalculateOutput> {
    let result = match input.operation.as_str() {
        "add" => input.a + input.b,
        "subtract" => input.a - input.b,
        "multiply" => input.a * input.b,
        "divide" => {
            if input.b == 0.0 {
                return Err(pmcp::Error::validation("Cannot divide by zero"));
            }
            input.a / input.b
        }
        _ => return Err(pmcp::Error::validation(
            format!("Unknown operation: {}", input.operation)
        ))
    };

    Ok(CalculateOutput {
        result,
        description: format!("{} {} {} = {}", input.a, input.operation, input.b, result),
    })
}

// ============================================================================
// BUILDER
// ============================================================================

pub fn build_tool() -> TypedTool<CalculateInput, CalculateOutput> {
    TypedTool::new("calculate", |input, extra| {
        Box::pin(handler(input, extra))
    })
    .with_description("Perform arithmetic operations")
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_addition() {
        let input = CalculateInput {
            operation: "add".to_string(),
            a: 5.0,
            b: 3.0,
        };

        let result = handler(input, RequestHandlerExtra::default())
            .await
            .unwrap();

        assert_eq!(result.result, 8.0);
    }

    #[tokio::test]
    async fn test_divide_by_zero() {
        let input = CalculateInput {
            operation: "divide".to_string(),
            a: 10.0,
            b: 0.0,
        };

        let result = handler(input, RequestHandlerExtra::default()).await;
        assert!(result.is_err());
    }
}
```

### Pattern 2: Complex Tool (Directory)

**Structure**:
```
src/tools/weather/
├── mod.rs          # Public interface
├── types.rs        # Input/Output types
├── handler.rs      # Business logic
├── client.rs       # External API client
└── tests.rs        # Tests
```

**File**: `src/tools/weather/mod.rs`
```rust
mod types;
mod handler;
mod client;

pub use types::{WeatherInput, WeatherOutput};
pub use handler::build_tool;

#[cfg(test)]
mod tests;
```

### Tool Module Exports (tools/mod.rs)

```rust
//! MCP tools for weather server

pub mod calculate;
pub mod weather;

// Re-export builders for convenience
pub use calculate::build_tool as build_calculate_tool;
pub use weather::build_tool as build_weather_tool;
```

## Resource Module Organization

### Simple Resource Handler

**File**: `src/resources/mod.rs`

```rust
use pmcp::{Result, Error};

pub async fn handle_resource(uri: &str) -> Result<String> {
    match uri {
        uri if uri.starts_with("weather://current/") => {
            let city = uri.strip_prefix("weather://current/").unwrap();
            get_current_weather(city).await
        }
        uri if uri.starts_with("weather://forecast/") => {
            let city = uri.strip_prefix("weather://forecast/").unwrap();
            get_forecast(city).await
        }
        _ => Err(Error::validation(format!("Unknown resource: {}", uri)))
    }
}

async fn get_current_weather(city: &str) -> Result<String> {
    // Implementation
    Ok(format!("Current weather for {}", city))
}

async fn get_forecast(city: &str) -> Result<String> {
    // Implementation
    Ok(format!("Forecast for {}", city))
}
```

### Resource with State

**File**: `src/resources/database.rs`

```rust
use pmcp::{Result, Error};
use sqlx::SqlitePool;

pub struct DatabaseResources {
    pool: SqlitePool,
}

impl DatabaseResources {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn handle(&self, uri: &str) -> Result<String> {
        match uri {
            uri if uri.starts_with("db://tables/") => {
                let table = uri.strip_prefix("db://tables/").unwrap();
                self.get_table_schema(table).await
            }
            uri if uri.starts_with("db://query/") => {
                let query_id = uri.strip_prefix("db://query/").unwrap();
                self.execute_saved_query(query_id).await
            }
            _ => Err(Error::validation(format!("Unknown resource: {}", uri)))
        }
    }

    async fn get_table_schema(&self, table: &str) -> Result<String> {
        // Use self.pool for database access
        Ok(format!("Schema for table: {}", table))
    }

    async fn execute_saved_query(&self, query_id: &str) -> Result<String> {
        // Use self.pool for database access
        Ok(format!("Results for query: {}", query_id))
    }
}
```

## Binary Entry Point (main.rs)

### Standard Pattern

```rust
use anyhow::Result;
use mcp_weather_core::WeatherServer;
use server_common::http::run_http_server;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into())
        )
        .init();

    // Build server
    let api_key = std::env::var("WEATHER_API_KEY")
        .expect("WEATHER_API_KEY environment variable required");

    let weather_server = WeatherServer::builder()
        .with_api_key(api_key)
        .build()?;

    let mcp_server = weather_server.build()?;

    // Select transport based on environment
    let transport = std::env::var("TRANSPORT")
        .unwrap_or_else(|_| "http".to_string());

    match transport.as_str() {
        "stdio" => {
            use pmcp::transport::StdioTransport;
            tracing::info!("Starting server with stdio transport");
            let transport = StdioTransport::new();
            transport.run(mcp_server).await?;
        }
        "http" => {
            let port = std::env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse::<u16>()?;

            tracing::info!("Starting server with HTTP transport on port {}", port);
            run_http_server(mcp_server, &format!("0.0.0.0:{}", port)).await?;
        }
        _ => {
            eprintln!("Unknown transport: {}. Use 'stdio' or 'http'", transport);
            std::process::exit(1);
        }
    }

    Ok(())
}
```

## Cargo.toml Patterns

### Workspace Cargo.toml

```toml
[workspace]
members = [
    "crates/server-common",
    "crates/mcp-weather-core",
    "crates/weather-server",
]
resolver = "2"

[workspace.package]
version = "1.0.0"
edition = "2021"
rust-version = "1.70"
license = "MIT"
authors = ["Your Name <email@example.com>"]
repository = "https://github.com/yourusername/weather-mcp-workspace"

[workspace.dependencies]
# MCP dependencies
pmcp = "1.8"
pmcp-macros = "0.2"

# Async runtime
tokio = { version = "1", features = ["full"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
schemars = "0.8"

# Error handling
anyhow = "1"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# HTTP client (for API integration)
reqwest = { version = "0.11", features = ["json"] }

[profile.release]
lto = true
codegen-units = 1
strip = true  # Remove debug symbols
```

### Core Library Cargo.toml

```toml
[package]
name = "mcp-weather-core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true

description = "Weather data MCP server core library"
keywords = ["mcp", "weather", "api"]
categories = ["api-bindings"]

[dependencies]
pmcp.workspace = true
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
schemars.workspace = true
anyhow.workspace = true
tracing.workspace = true
reqwest.workspace = true

[dev-dependencies]
tokio = { workspace = true, features = ["test-util"] }
```

### Binary Cargo.toml

```toml
[package]
name = "weather-server"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true

description = "Weather MCP server binary"

[[bin]]
name = "weather-server"
path = "src/main.rs"

[dependencies]
mcp-weather-core = { path = "../mcp-weather-core" }
server-common = { path = "../server-common" }
pmcp.workspace = true
tokio.workspace = true
anyhow.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
```

## Environment Configuration

### .env File (Development)

```bash
# API Keys
WEATHER_API_KEY=your_api_key_here

# Transport
TRANSPORT=http
PORT=3000

# Logging
RUST_LOG=info,weather_server=debug

# Database (if applicable)
DATABASE_URL=sqlite://weather.db
```

### Production Configuration

```bash
# Use stdio transport for MCP clients
TRANSPORT=stdio

# Minimal logging
RUST_LOG=error,weather_server=warn

# Production database
DATABASE_URL=postgresql://user:pass@host/db
```

## Documentation Structure

### README.md (Workspace)

```markdown
# Weather MCP Workspace

Production-grade MCP server for weather data access.

## Features
- Current weather conditions
- 5-day forecasts
- Weather alerts

## Quick Start

\`\`\`bash
# Install
cargo build --release

# Set API key
export WEATHER_API_KEY=your_key

# Run with HTTP (development)
cargo run --bin weather-server

# Run with stdio (production)
TRANSPORT=stdio cargo run --bin weather-server
\`\`\`

## Testing

\`\`\`bash
# Unit tests
cargo test

# Integration tests
cargo pmcp test --server weather

# Generate scenarios
cargo pmcp test --server weather --generate-scenarios
\`\`\`

## Project Structure

See [STRUCTURE.md](docs/STRUCTURE.md) for detailed architecture.

## License

MIT
```

### README.md (Core Library)

```markdown
# mcp-weather-core

Core library for Weather MCP server.

## Usage

\`\`\`rust
use mcp_weather_core::WeatherServer;

let server = WeatherServer::builder()
    .with_api_key("your_key")
    .build()?;

let mcp_server = server.build()?;
\`\`\`

## API Documentation

See [docs.rs/mcp-weather-core](https://docs.rs/mcp-weather-core)
```

## Testing Organization

### Unit Tests (Inline)

```rust
// In tool file
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_successful_calculation() {
        // Test implementation
    }

    #[tokio::test]
    async fn test_error_handling() {
        // Test implementation
    }
}
```

### Integration Tests (scenarios/)

```
scenarios/
└── weather/
    ├── basic.yaml              # Happy path scenarios
    ├── error_handling.yaml     # Error cases
    ├── edge_cases.yaml         # Boundary conditions
    └── generated.yaml          # Auto-generated from schema
```

## Common Patterns Summary

### File Organization
- ✅ Cargo workspace with multiple crates
- ✅ Core library separate from binary
- ✅ Shared code in `server-common`
- ✅ Tests alongside implementation

### Naming
- ✅ `mcp-{name}-core` for libraries
- ✅ `{name}-server` for binaries
- ✅ `snake_case` for Rust code
- ✅ `kebab-case` for MCP protocol

### Module Structure
- ✅ `tools/`, `resources/`, `prompts/`, `workflows/` directories
- ✅ `mod.rs` for public exports
- ✅ Separate files for complex components
- ✅ Tests in `#[cfg(test)]` modules or `tests.rs`

### Configuration
- ✅ Environment variables for runtime config
- ✅ `.env` for development
- ✅ Workspace dependencies for consistency

---

**Next**: Read tool patterns in `mcp-tool-patterns.md` for implementation details.
