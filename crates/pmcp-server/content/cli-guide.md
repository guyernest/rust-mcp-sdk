# cargo-pmcp CLI Guide

`cargo pmcp` is the developer CLI for PMCP SDK projects. It provides
scaffolding, testing, previewing, and deployment commands.

## Commands

### init / scaffold

Create a new PMCP workspace or add components:

```bash
cargo pmcp init my-server          # New workspace with server template
cargo pmcp scaffold tool greet     # Add a tool to existing server
cargo pmcp scaffold resource docs  # Add a resource handler
```

### test

Run protocol compliance and validation tests:

```bash
cargo pmcp test check <url>        # Protocol compliance against a running server
cargo pmcp test run <url>          # Run test scenarios
cargo pmcp test generate <url>     # Auto-generate test scenarios from server listing
cargo pmcp test apps <url>         # Validate MCP Apps metadata and widgets
```

### preview

Launch an interactive browser preview for MCP Apps:

```bash
cargo pmcp preview <url>           # Open widget preview for a server
cargo pmcp preview <url> --chatgpt # Preview in ChatGPT compatibility mode
```

### schema

Export tool schemas:

```bash
cargo pmcp schema export <url>     # Export schemas as JSON
cargo pmcp schema diff <url>       # Compare schemas between versions
```

### validate

Validate server configuration and manifests:

```bash
cargo pmcp validate                # Validate current project
cargo pmcp validate --strict       # Strict mode with additional checks
```

### deploy

Deploy to hosted infrastructure:

```bash
cargo pmcp deploy                  # Deploy current server
cargo pmcp deploy --server <name>  # Deploy to named server
```

### secret

Manage deployment secrets:

```bash
cargo pmcp secret set KEY=VALUE    # Set a secret
cargo pmcp secret list             # List configured secrets
```

### connect

Connect to a remote MCP server for inspection:

```bash
cargo pmcp connect <url>           # Interactive connection to server
```

## Global Flags

```
--verbose, -v    Enable verbose output
--quiet, -q      Suppress non-essential output
--no-color       Disable colored output
--format <fmt>   Output format: text (default), json, yaml
```

## Environment Variables

| Variable         | Purpose                          |
|------------------|----------------------------------|
| PMCP_QUIET       | Suppress output (same as --quiet)|
| NO_COLOR         | Disable colors (same as --no-color)|
| PMCP_SERVER_URL  | Default server URL               |
| PMCP_API_KEY     | API key for authenticated servers|
