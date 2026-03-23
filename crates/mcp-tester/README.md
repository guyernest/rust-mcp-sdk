# MCP Tester

The Swiss Army knife for testing MCP servers. Validate protocol conformance, test tools, generate scenarios, and diagnose connection issues — all from a single binary.

```
$ mcp-tester test http://localhost:3000

  MCP Server Test Report
  ══════════════════════════════════════════════════════
  Server: my-server v1.0.0 | Protocol: 2025-11-25

  Core
    ✓ Initialize          Server responded with valid capabilities
    ✓ Protocol Version    Protocol version: 2025-11-25
    ✓ Server Info         my-server v1.0.0
    ✓ Capabilities        tools, resources, prompts

  Tools
    ✓ List                Found 5 tools
    ✓ Schema              All 5 tool schemas valid

  Resources
    ✓ List                Found 2 resources

  Summary: 7 passed, 0 failed, 0 warnings in 1.23s
```

## Install

### Option 1: Cargo (recommended for Rust developers)

```bash
# Standalone binary
cargo install mcp-tester

# Or as part of the full PMCP toolkit
cargo install cargo-pmcp
```

### Option 2: Shell script (no Rust required)

Linux and macOS — downloads the pre-built binary for your platform:

```bash
curl -fsSL https://raw.githubusercontent.com/paiml/rust-mcp-sdk/main/install/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/paiml/rust-mcp-sdk/main/install/install.ps1 | iex
```

Pre-built binaries are available for Linux x86_64/ARM64, macOS Intel/Apple Silicon, and Windows x86_64.

### Option 3: Via MCP (use from any MCP client)

The PMCP server at `https://pmcp-server.us-east.true-mcp.com/mcp` exposes testing tools you can call directly from Claude Desktop, ChatGPT, or any MCP client — no local install needed.

## Quick Start: Check

The fastest way to validate an MCP server — one command, pass/fail answer:

```bash
# Test a local server
mcp-tester quick http://localhost:3000

# Test a remote server
mcp-tester quick https://my-server.example.com/mcp

# Test with OAuth
mcp-tester quick https://api.example.com/mcp \
  --oauth-issuer "https://auth.example.com" \
  --oauth-client-id "my-client-id"

# Via cargo-pmcp (auto-discovers server in your workspace)
cargo pmcp test check http://localhost:3000
```

## Protocol Conformance

Validate any MCP server against the protocol spec (2025-11-25). Tests 5 domains: Core, Tools, Resources, Prompts, Tasks. Each domain reports independently — a server with no resources still passes.

```bash
# Full conformance check
mcp-tester conformance http://localhost:3000

# Strict mode (warnings → failures)
mcp-tester conformance http://localhost:3000 --strict

# Test specific domains only
mcp-tester conformance http://localhost:3000 --domain core,tools

# Via cargo-pmcp
cargo pmcp test conformance http://localhost:3000
```

Output includes a per-domain CI summary line:

```
Conformance: Core=PASS Tools=PASS Resources=SKIP Prompts=PASS Tasks=SKIP
```

## Generate Test Scenarios

Auto-generate test scenarios from your server's capabilities. The generator discovers all tools, analyzes their JSON schemas, and creates YAML scenario files with smart placeholder values:

```bash
# Generate from a running server
mcp-tester generate-scenario http://localhost:3000 -o tests/my-server.yaml \
  --all-tools --with-resources --with-prompts

# Via cargo-pmcp
cargo pmcp test generate --server my-server --port 3000
```

This produces editable YAML like:

```yaml
name: my-server Test Scenario
timeout: 60
steps:
  - name: Test tool search
    operation:
      type: tool_call
      tool: search
      arguments:
        query: "TODO: query"    # ← fill in real test data
    assertions:
      - type: success
      - type: exists
        path: results
```

## Run Test Scenarios

Execute generated or hand-written scenarios against your server:

```bash
# Run a single scenario
mcp-tester scenario http://localhost:3000 tests/my-server.yaml --detailed

# Run all scenarios in a directory
cargo pmcp test run --server my-server --scenarios tests/
```

## All Commands

| Command | Description |
|---------|-------------|
| `test` | Full test suite — protocol, tools, resources, prompts |
| `quick` | Fast connectivity and protocol check |
| `conformance` | MCP protocol conformance validation (19 scenarios across 5 domains) |
| `tools` | Discover tools and validate schemas |
| `resources` | Test resource discovery and reading |
| `prompts` | Validate prompt templates and arguments |
| `apps` | Validate MCP App metadata (standard, ChatGPT, Claude Desktop modes) |
| `generate-scenario` | Auto-generate test scenarios from server capabilities |
| `scenario` | Run YAML/JSON test scenarios |
| `diagnose` | Layer-by-layer connection diagnostics |
| `compare` | Compare two servers side-by-side |
| `health` | Health check endpoint |

## Key Features

- **Multi-transport**: HTTP, HTTPS, WebSocket, stdio — auto-detected or forced with `--transport`
- **OAuth 2.0**: Interactive browser-based PKCE flow with token caching (`--oauth-*` flags)
- **Schema validation**: Warns about missing properties, empty schemas, incomplete metadata
- **MCP App validation**: Checks `_meta`, `ui.resourceUri`, resource cross-refs, ChatGPT keys
- **CI/CD ready**: `--format json` for machine-readable output, deterministic exit codes
- **Multiple output formats**: `pretty` (default), `json`, `minimal`, `verbose`

## CI/CD Integration

```yaml
# GitHub Actions
- name: Test MCP Server
  run: |
    curl -fsSL https://raw.githubusercontent.com/paiml/rust-mcp-sdk/main/install/install.sh | sh
    mcp-tester test ${{ env.SERVER_URL }} --format json > results.json
```

```bash
# Any CI — exit code tells you pass/fail
mcp-tester test "$SERVER_URL" --format minimal
```

## Documentation

- [Scenario Format Reference](SCENARIO_FORMAT.md) — YAML/JSON scenario structure, operations, and assertions
- [cargo-pmcp README](../../cargo-pmcp/README.md) — Full PMCP toolkit including test, preview, and deploy commands
- [PMCP SDK](../../README.md) — The Rust MCP SDK that powers mcp-tester

## License

MIT — See [LICENSE](../../LICENSE) in the repository root.
