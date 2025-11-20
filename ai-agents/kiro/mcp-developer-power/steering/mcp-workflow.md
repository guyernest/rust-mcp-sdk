---
inclusion: always
---

# MCP Development Workflow - The cargo-pmcp Way

## CRITICAL: Never Create Files Manually

**IMPORTANT**: When building MCP servers, you must **ALWAYS** use `cargo pmcp` commands to scaffold the workspace and server structure. **DO NOT** create Cargo.toml, lib.rs, main.rs, or directory structures manually.

### Why This Matters

cargo-pmcp encodes best practices from 6 production servers. Manual file creation:
- ❌ Misses proven patterns and conventions
- ❌ Creates inconsistent structure across servers
- ❌ Requires manual setup of transport, testing, and quality gates
- ❌ Wastes time on boilerplate instead of business logic

cargo-pmcp scaffolding:
- ✅ Generates complete, working server structure in seconds
- ✅ Includes server-common, core library, and binary crates
- ✅ Sets up HTTP transport with hot-reload
- ✅ Creates test scenario directories
- ✅ Configures workspace dependencies correctly
- ✅ Ready for immediate development

## The Standard Workflow (ALWAYS Follow This)

### Step 1: Create Workspace (One-Time Setup)

**Command**:
```bash
cargo pmcp new <workspace-name>
cd <workspace-name>
```

**What This Does**:
- Creates workspace root with Cargo.toml
- Defines workspace members array
- Sets up workspace-level dependencies (pmcp, tokio, serde, etc.)
- Creates .gitignore
- Initializes server-common crate with HTTP transport helpers
- Creates scenarios/ directory for testing

**Output Structure**:
```
<workspace-name>/
├── Cargo.toml                 # Workspace manifest (GENERATED)
├── Cargo.lock                 # Dependency lock file
├── .gitignore                 # Git patterns (GENERATED)
├── README.md                  # Workspace docs (GENERATED)
├── crates/
│   └── server-common/         # HTTP transport helpers (GENERATED)
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           └── http.rs
└── scenarios/                 # Test scenarios directory
```

**YOU MUST USE THIS COMMAND** - Do not create these files manually!

### Step 2: Add MCP Server to Workspace

**Command**:
```bash
cargo pmcp add server <server-name> --template <template>
```

**Template Options**:
- `minimal` - Basic server structure, no example tools (for custom servers)
- `calculator` - Simple arithmetic example (for learning)
- `complete_calculator` - Full-featured calculator with tests
- `sqlite_explorer` - Database browser example (resource-heavy)

**What This Does**:
- Creates `mcp-<server-name>-core/` library crate
- Creates `<server-name>-server/` binary crate
- Updates workspace Cargo.toml to include new crates
- Generates scaffolding based on template
- Creates `scenarios/<server-name>/` test directory
- Sets up module structure (tools/, resources/, prompts/, workflows/)

**Output Structure** (example: `cargo pmcp add server calculator --template calculator`):
```
crates/
├── server-common/             # Already exists
├── mcp-calculator-core/       # NEW - GENERATED
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs             # Server builder
│       ├── tools/             # Tool implementations
│       │   ├── mod.rs
│       │   └── add.rs         # Example tool (from template)
│       ├── resources/         # Resource implementations (if --resources)
│       ├── prompts/           # Prompt templates (if --prompts)
│       └── workflows/         # Workflow definitions (if --workflows)
└── calculator-server/         # NEW - GENERATED
    ├── Cargo.toml
    └── src/
        └── main.rs            # Binary entry point with transport

scenarios/
└── calculator/                # NEW - GENERATED
    └── README.md
```

**YOU MUST USE THIS COMMAND** - Do not create these crates manually!

### Step 3: Customize Tools (This is Where You Write Code)

**Now** you can edit the generated files to implement your business logic.

**Only Modify**:
- Tool handlers in `crates/mcp-<name>-core/src/tools/*.rs`
- Resource handlers in `crates/mcp-<name>-core/src/resources/*.rs`
- Workflow definitions in `crates/mcp-<name>-core/src/workflows/*.rs`
- Server builder in `crates/mcp-<name>-core/src/lib.rs` (to register tools)

**Never Modify**:
- Workspace Cargo.toml (unless adding new dependencies)
- server-common crate (shared across all servers)
- Directory structure
- Binary main.rs (unless changing transport configuration)

### Step 4: Start Development Server

**Command**:
```bash
cargo pmcp dev --server <server-name>
```

**What This Does**:
- Builds the server
- Starts HTTP transport on http://0.0.0.0:3000
- Enables hot-reload (rebuild on code changes)
- Shows live logs

**Example Output**:
```
Building calculator-server...
   Compiling mcp-calculator-core v1.0.0
   Compiling calculator-server v1.0.0
    Finished dev [unoptimized + debuginfo] target(s) in 2.34s

MCP server running on http://0.0.0.0:3000

Capabilities:
  - tools: add, subtract, multiply, divide
  - resources: None
  - prompts: None

Logs:
[INFO] Server ready to accept connections
```

**Leave this running** while you develop and test.

### Step 5: Generate Test Scenarios

**Command** (in another terminal):
```bash
cargo pmcp test --server <server-name> --generate-scenarios
```

**What This Does**:
- Connects to running dev server
- Calls `tools/list` to discover capabilities
- Generates smart test scenarios with realistic values
- Creates `scenarios/<server-name>/generated.yaml`
- Includes success assertions and error cases

**Example Generated Scenario**:
```yaml
name: "Calculator Test Scenarios"
description: "Auto-generated tests for calculator server"
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
      - type: contains
        path: "content.0.text"
        value: "357"
```

### Step 6: Run Automated Tests

**Command**:
```bash
cargo pmcp test --server <server-name>
```

**Prerequisites**:
- Dev server must be running (Step 4)
- Test scenarios must exist (Step 5 or manual creation)

**What This Does**:
- Connects to http://0.0.0.0:3000
- Runs all scenarios in `scenarios/<server-name>/`
- Validates assertions
- Reports pass/fail for each step

**Example Output**:
```
Running scenarios for calculator server...

Scenario: Calculator Test Scenarios
  ✓ Test addition (123 + 234 = 357)
  ✓ Test subtraction (100 - 42 = 58)
  ✓ Test multiplication (12 * 13 = 156)
  ✓ Test division (100 / 4 = 25)
  ✗ Test divide by zero (Expected error)

Results: 4 passed, 1 failed
```

### Step 7: Validate Quality Gates

**Commands**:
```bash
# Format check
cargo fmt --check

# Linting
cargo clippy -- -D warnings

# Unit tests
cargo test

# All quality gates
make quality-gate  # If Makefile exists
```

**What This Validates**:
- Code formatting (100% cargo fmt compliant)
- Zero clippy warnings
- All unit tests pass
- Coverage ≥80% (if coverage tools configured)

### Step 8: Build for Production

**Command**:
```bash
cargo build --release
```

**Output**:
```
target/release/<server-name>-server
```

**Configure MCP Client** (e.g., Kiro, Claude Code):
```json
{
  "mcpServers": {
    "calculator": {
      "command": "/path/to/calculator-server",
      "args": []
    }
  }
}
```

## Complete Example: Building a Weather Server

### Incorrect Approach ❌ (What Kiro Was Doing)

```
User: "Create a weather MCP server"

Kiro: Creating files manually...
  1. Creating workspace Cargo.toml
  2. Creating server-common/
  3. Creating mcp-weather-core/
  4. Writing lib.rs from scratch
  5. Creating main.rs from scratch
  ...
```

**Problems**:
- Manual file creation is error-prone
- Misses cargo-pmcp conventions
- No hot-reload dev server setup
- No test scaffolding
- Wastes time on boilerplate

### Correct Approach ✅ (Use cargo-pmcp)

```
User: "Create a weather MCP server"

Kiro: Following cargo-pmcp workflow...

Step 1: Create workspace
$ cargo pmcp new weather-mcp-workspace
$ cd weather-mcp-workspace

Step 2: Add weather server
$ cargo pmcp add server weather --template minimal

Step 3: Now I'll implement the weather tools
[Edits crates/mcp-weather-core/src/tools/get_forecast.rs]
[Edits crates/mcp-weather-core/src/lib.rs to register tool]

Step 4: Start dev server
$ cargo pmcp dev --server weather

Step 5: Generate test scenarios
[In another terminal]
$ cargo pmcp test --server weather --generate-scenarios

Step 6: Run tests
$ cargo pmcp test --server weather

All tests passing! Server ready.
```

**Benefits**:
- Complete structure in seconds
- Follows proven patterns
- Hot-reload enabled
- Tests scaffolded
- Ready for production

## Workflow Decision Tree

```
┌─────────────────────────────────────┐
│  New MCP Server Project?            │
└────────────┬────────────────────────┘
             │
             ▼
    ┌────────────────────┐
    │ Workspace exists?  │
    └────┬───────────┬───┘
         No          Yes
         │           │
         ▼           │
    cargo pmcp new   │
         │           │
         └─────┬─────┘
               │
               ▼
    ┌──────────────────────────┐
    │  cargo pmcp add server   │
    │  --template <type>       │
    └──────────┬───────────────┘
               │
               ▼
    ┌──────────────────────────┐
    │  Edit generated files:   │
    │  - tools/*.rs            │
    │  - lib.rs (register)     │
    └──────────┬───────────────┘
               │
               ▼
    ┌──────────────────────────┐
    │  cargo pmcp dev          │
    │  --server <name>         │
    └──────────┬───────────────┘
               │
               ▼
    ┌──────────────────────────┐
    │  cargo pmcp test         │
    │  --generate-scenarios    │
    └──────────┬───────────────┘
               │
               ▼
    ┌──────────────────────────┐
    │  cargo pmcp test         │
    │  --server <name>         │
    └──────────┬───────────────┘
               │
               ▼
    ┌──────────────────────────┐
    │  Quality gates           │
    │  (fmt, clippy, test)     │
    └──────────┬───────────────┘
               │
               ▼
         Production!
```

## When to Use Each Template

### `minimal` Template
**Use When**: Building custom server with unique tools
**Includes**:
- Empty tools/ directory
- Empty resources/ directory
- Server builder skeleton
- Transport setup

**Example**:
```bash
cargo pmcp add server github --template minimal
# Then implement: get_repo, create_issue, list_prs tools
```

### `calculator` Template
**Use When**: Learning MCP or simple arithmetic server
**Includes**:
- Single `add` tool example
- Basic input/output types
- Minimal test

**Example**:
```bash
cargo pmcp add server calc --template calculator
# Good starting point for understanding MCP
```

### `complete_calculator` Template
**Use When**: Full-featured reference implementation
**Includes**:
- 5 arithmetic tools (add, subtract, multiply, divide, power)
- Input validation with validator crate
- Comprehensive tests
- Prompts example (quadratic solver)
- Resources example (math formulas)

**Example**:
```bash
cargo pmcp add server advanced-calc --template complete_calculator
# Best for seeing all MCP capabilities
```

### `sqlite_explorer` Template
**Use When**: Building database browser or resource-heavy server
**Includes**:
- Dynamic resource discovery
- Database connection pooling
- Query execution tools
- Schema introspection

**Example**:
```bash
cargo pmcp add server db-explorer --template sqlite_explorer
# Then customize for PostgreSQL, MySQL, etc.
```

## Adding Components to Existing Server

### Add a Tool

**Command**:
```bash
cargo pmcp add tool <tool-name> --server <server-name>
```

**What This Does**:
- Creates `crates/mcp-<server>-core/src/tools/<tool-name>.rs`
- Generates tool scaffolding (input type, output type, handler)
- Updates `tools/mod.rs` to export the tool
- Prints next steps for registration

**Example**:
```bash
cargo pmcp add tool subtract --server calculator

Created: crates/mcp-calculator-core/src/tools/subtract.rs

Next steps:
1. Edit subtract.rs to implement logic
2. Register in lib.rs:
   .tool("subtract", tools::subtract::build_tool())
```

### Add a Workflow

**Command**:
```bash
cargo pmcp add workflow <workflow-name> --server <server-name>
```

**What This Does**:
- Creates `crates/mcp-<server>-core/src/workflows/<workflow-name>.yaml`
- Generates workflow template with steps
- Updates `workflows/mod.rs`

**Example**:
```bash
cargo pmcp add workflow data-pipeline --server analytics

Created: crates/mcp-analytics-core/src/workflows/data_pipeline.yaml
```

## Testing Workflow in Detail

### Interactive Testing with Kiro

Since Kiro can connect to MCP servers directly:

```bash
# Terminal 1: Start server
$ cargo pmcp dev --server myserver

# Kiro: Add to .kiro/settings.json
{
  "mcpServers": {
    "myserver-dev": {
      "command": "node",
      "args": ["/path/to/http-sse-mcp-proxy.js"],
      "env": {
        "MCP_SERVER_URL": "http://0.0.0.0:3000"
      }
    }
  }
}

# Kiro: Now test tools interactively
[Calls tools via MCP protocol]
[Validates responses]
[Reports issues]
```

This enables:
- Real-time testing during development
- Immediate feedback on tool behavior
- Validation of error handling
- Interactive debugging

## Common Mistakes to Avoid

### ❌ Mistake 1: Creating Files Manually

```bash
# WRONG
mkdir -p crates/mcp-myserver-core/src
touch crates/mcp-myserver-core/Cargo.toml
# ... manual file creation
```

```bash
# CORRECT
cargo pmcp add server myserver --template minimal
```

### ❌ Mistake 2: Modifying Generated Structure

```bash
# WRONG - Don't reorganize directories
mv crates/mcp-myserver-core/src/tools crates/mcp-myserver-core/src/handlers
```

```bash
# CORRECT - Follow cargo-pmcp conventions
# Edit within existing structure
```

### ❌ Mistake 3: Skipping Dev Server

```bash
# WRONG - Testing without dev server
cargo build
./target/debug/myserver-server  # stdio mode
# Try to test... doesn't work well
```

```bash
# CORRECT - Use dev server for testing
cargo pmcp dev --server myserver
# Server available on HTTP for easy testing
```

### ❌ Mistake 4: Not Using Test Generation

```bash
# WRONG - Writing test scenarios manually from scratch
vim scenarios/myserver/manual.yaml
# ... writing YAML by hand
```

```bash
# CORRECT - Generate then customize
cargo pmcp test --server myserver --generate-scenarios
vim scenarios/myserver/generated.yaml  # Customize as needed
```

## Environment Variables for Development

### For cargo pmcp dev

```bash
# Default port is 3000, change with:
cargo pmcp dev --server myserver --port 8080

# Or use environment:
export PORT=8080
cargo pmcp dev --server myserver
```

### For Server Runtime

Create `.env` file in workspace root:

```bash
# API Keys
WEATHER_API_KEY=your_key_here
GITHUB_TOKEN=ghp_your_token

# Database
DATABASE_URL=sqlite://data.db

# Logging
RUST_LOG=info,myserver=debug

# Transport (for production)
TRANSPORT=stdio  # or http
PORT=3000        # for HTTP
```

## Quality Gate Integration

### Pre-commit Hook (Recommended)

Create `.git/hooks/pre-commit`:

```bash
#!/bin/bash
set -e

echo "Running quality gates..."

# Format check
cargo fmt --check || {
    echo "❌ Format check failed. Run: cargo fmt"
    exit 1
}

# Clippy
cargo clippy -- -D warnings || {
    echo "❌ Clippy failed. Fix warnings."
    exit 1
}

# Tests
cargo test || {
    echo "❌ Tests failed."
    exit 1
}

echo "✅ All quality gates passed!"
```

### Continuous Integration

In your CI pipeline:

```yaml
# .github/workflows/quality.yml
- name: Quality Gates
  run: |
    cargo fmt --check
    cargo clippy -- -D warnings
    cargo test
    cargo pmcp test --server myserver  # If server running
```

## Production Deployment

### Building for Production

```bash
# Release build (optimized)
cargo build --release

# Binary location
ls target/release/*-server

# Size check (should be <10MB for simple servers)
ls -lh target/release/*-server
```

### Configuring for stdio Transport

Production servers typically use stdio transport for MCP clients:

```rust
// In main.rs - auto-generated, rarely needs changes
#[tokio::main]
async fn main() -> Result<()> {
    let server = build_myserver()?;

    // Check TRANSPORT env var
    match std::env::var("TRANSPORT").as_deref() {
        Ok("http") => {
            // Development mode
            run_http_server(server, "0.0.0.0:3000").await
        }
        _ => {
            // Production mode (default)
            let transport = StdioTransport::new();
            transport.run(server).await
        }
    }
}
```

### MCP Client Configuration

For Kiro or Claude Code:

```json
{
  "mcpServers": {
    "myserver": {
      "command": "/path/to/myserver-server",
      "args": [],
      "env": {
        "WEATHER_API_KEY": "${env:WEATHER_API_KEY}",
        "RUST_LOG": "error"
      }
    }
  }
}
```

## Summary: The cargo-pmcp Advantage

| Aspect | Manual Creation ❌ | cargo-pmcp ✅ |
|--------|-------------------|--------------|
| **Setup Time** | 30-60 minutes | 30 seconds |
| **Structure** | Inconsistent | Proven patterns |
| **Hot-reload** | Manual setup | Built-in |
| **Testing** | Manual scaffolding | Auto-generated |
| **Quality** | Hit or miss | Toyota Way enforced |
| **Maintenance** | High effort | Update once, all servers benefit |
| **Learning Curve** | Steep | Guided by templates |

## Critical Reminders for Kiro

1. **NEVER create Cargo.toml manually** - Use `cargo pmcp new` for workspaces
2. **NEVER create server directories manually** - Use `cargo pmcp add server`
3. **ALWAYS start with a template** - Don't build from scratch
4. **ALWAYS use dev server** - `cargo pmcp dev` enables hot-reload
5. **ALWAYS generate scenarios first** - `cargo pmcp test --generate-scenarios`
6. **ONLY edit tool implementations** - Don't modify scaffolding structure

Following these rules ensures:
- Faster development (minutes vs hours)
- Consistent quality (Toyota Way standards)
- Easy testing (built-in dev server)
- Production-ready output (proven patterns)

---

**Remember**: cargo-pmcp is the **only correct way** to scaffold MCP servers. Manual file creation is a violation of the workflow and should be avoided at all costs.
