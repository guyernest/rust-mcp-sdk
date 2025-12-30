# Setting Up Claude Code

Claude Code is a command-line AI assistant that integrates deeply with your development workflow. This chapter covers installing and configuring Claude Code for MCP server development with the mcp-developer agent.

## Installation

### Install Claude Code

```bash
# macOS
brew install claude-code

# Or via npm
npm install -g @anthropic-ai/claude-code

# Verify installation
claude --version
```

### Install Prerequisites

```bash
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update

# cargo-pmcp toolkit
cargo install cargo-pmcp

# Verify
cargo pmcp --version
rustc --version
```

## Installing the MCP Developer Agent

The mcp-developer agent teaches Claude Code how to build MCP servers using cargo-pmcp and pmcp SDK best practices.

### User-Level Installation (Recommended)

Available across all your projects:

```bash
# Create agents directory
mkdir -p ~/.claude/agents

# Download the agent
curl -fsSL https://raw.githubusercontent.com/paiml/rust-mcp-sdk/main/ai-agents/claude-code/mcp-developer.md \
  -o ~/.claude/agents/mcp-developer.md
```

### Project-Level Installation

Available only in the current project:

```bash
# Create project agents directory
mkdir -p .claude/agents

# Download the agent
curl -fsSL https://raw.githubusercontent.com/paiml/rust-mcp-sdk/main/ai-agents/claude-code/mcp-developer.md \
  -o .claude/agents/mcp-developer.md
```

### Verify Installation

```bash
# Start Claude Code
claude

# List agents
/agents

# Should show:
# - mcp-developer: Expert MCP server developer using pmcp Rust SDK
```

## What the Agent Provides

### Knowledge Encoded

The mcp-developer agent knows:

1. **cargo-pmcp workflow** - Never create files manually
2. **pmcp SDK patterns** - TypedTool, resources, workflows
3. **Type-safe tool design** - JsonSchema, validation, error handling
4. **Testing workflow** - mcp-tester scenarios, unit tests
5. **Quality standards** - Toyota Way principles

### Automatic Behaviors

When you mention MCP servers, the agent:

- Uses `cargo pmcp new` to scaffold workspaces
- Uses `cargo pmcp add server` to add servers
- Implements tools following type-safe patterns
- Generates tests using mcp-tester
- Validates with clippy and cargo test

## First Project Walkthrough

### Step 1: Start Claude Code

```bash
# In a new directory
mkdir mcp-projects && cd mcp-projects
claude
```

### Step 2: Request an MCP Server

```
You: Create a GitHub MCP server that can fetch repository information

Claude Code: I'll create a GitHub MCP server using cargo-pmcp.

First, let me scaffold the workspace:

$ cargo pmcp new github-mcp-workspace
$ cd github-mcp-workspace

Now adding the server:

$ cargo pmcp add server github --template minimal

This creates:
- crates/mcp-github-core/ - Library with tools
- crates/github-server/ - Binary executable
- scenarios/github/ - Test directory

Now I'll implement the tools...
```

### Step 3: Watch the Implementation

Claude Code will:

1. **Create input/output types** with JsonSchema

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetRepoInput {
    #[schemars(description = "Repository owner (user or org)")]
    pub owner: String,

    #[schemars(description = "Repository name")]
    pub repo: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetRepoOutput {
    pub full_name: String,
    pub description: Option<String>,
    pub stars: u64,
    pub forks: u64,
    pub language: Option<String>,
}
```

2. **Implement handlers** with proper error handling

```rust
async fn get_repo_handler(
    input: GetRepoInput,
    extra: RequestHandlerExtra,
) -> Result<GetRepoOutput> {
    // Validation
    if input.owner.is_empty() || input.repo.is_empty() {
        return Err(Error::validation("Owner and repo required"));
    }

    // Get token from metadata
    let token = extra.metadata
        .get("github_token")
        .ok_or_else(|| Error::internal("GitHub token not configured"))?;

    // API call with error handling
    let client = reqwest::Client::new();
    let response = client
        .get(&format!("https://api.github.com/repos/{}/{}", input.owner, input.repo))
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "mcp-github-server")
        .send()
        .await
        .context("Failed to connect to GitHub")?;

    // Handle HTTP errors
    if response.status() == 404 {
        return Err(Error::validation(format!(
            "Repository {}/{} not found", input.owner, input.repo
        )));
    }

    if !response.status().is_success() {
        return Err(Error::internal(format!(
            "GitHub API error: {}", response.status()
        )));
    }

    // Parse response
    let repo: GitHubRepo = response
        .json()
        .await
        .context("Failed to parse GitHub response")?;

    Ok(GetRepoOutput {
        full_name: repo.full_name,
        description: repo.description,
        stars: repo.stargazers_count,
        forks: repo.forks_count,
        language: repo.language,
    })
}
```

3. **Register tools** in lib.rs

```rust
pub fn build_github_server() -> Result<McpServer> {
    McpServer::builder("github-server", "1.0.0")
        .tool("get-repo", tools::get_repo::build_tool())
        .tool("list-repos", tools::list_repos::build_tool())
        .build()
}
```

### Step 4: Test and Validate

```bash
# Claude Code runs automatically:
$ cargo fmt --check
$ cargo clippy -- -D warnings
$ cargo test

# Start dev server
$ cargo pmcp dev --server github

# Generate test scenarios
$ cargo pmcp test --server github --generate-scenarios
```

## Configuration Options

### Project-Level Settings

Create `.claude/settings.json`:

```json
{
  "agents": {
    "mcp-developer": {
      "autoInvoke": true,
      "keywords": ["mcp", "server", "tool", "pmcp"]
    }
  },
  "rust": {
    "formatOnSave": true,
    "clippyOnSave": true
  }
}
```

### Environment Variables

```bash
# .env file for MCP development
RUST_LOG=debug
GITHUB_TOKEN=ghp_your_token
WEATHER_API_KEY=your_key

# cargo-pmcp settings
CARGO_PMCP_TEMPLATE_DIR=~/.cargo-pmcp/templates
```

## Working with the Agent

### Effective Requests

**Good**:
```
Create an MCP server that queries a PostgreSQL database with
list_tables and execute_query tools. Include pagination.
```

**Better**:
```
Create a PostgreSQL MCP server with:
1. list_tables - returns table names and row counts
2. describe_table - returns column info for a table
3. execute_query - runs SELECT queries with 100 row limit

Use the sqlx crate. Database URL from DATABASE_URL env var.
```

### Iterating on Generated Code

```
You: The get_repo tool works but I want to also return the last commit

Claude Code: I'll update the GetRepoOutput and handler to include
the latest commit information.

[Modifies types to add last_commit field]
[Updates API call to fetch commit data]
[Adds tests for new functionality]

$ cargo test
All tests passing.
```

### Handling Errors

```
You: cargo build is failing with a lifetime error

Claude Code: Let me look at the error...

error[E0597]: `response` does not live long enough

I see the issue. The response is being borrowed after it's dropped.
Let me fix this by cloning the data before the response goes out of scope.

[Applies fix]

$ cargo build
Build successful.
```

## Connecting Your MCP Server to Claude Code

After building your server, connect it as an MCP server for Claude Code itself:

### Configure in Claude Code

```json
// ~/.claude/mcp_servers.json
{
  "github": {
    "command": "/path/to/github-mcp-workspace/target/release/github-server",
    "args": [],
    "env": {
      "GITHUB_TOKEN": "${env:GITHUB_TOKEN}"
    }
  }
}
```

### Verify Connection

```bash
claude

# Claude Code now has access to your GitHub tools
You: Use the GitHub MCP server to get info about rust-lang/rust

Claude Code: I'll use the get-repo tool from the GitHub server.

[Calls get-repo with owner="rust-lang", repo="rust"]

The rust-lang/rust repository has:
- 95,000+ stars
- Language: Rust
- Description: Empowering everyone to build reliable software
```

## Updating the Agent

Keep the agent current:

```bash
# Check current version
head -5 ~/.claude/agents/mcp-developer.md

# Update to latest
curl -fsSL https://raw.githubusercontent.com/paiml/rust-mcp-sdk/main/ai-agents/claude-code/mcp-developer.md \
  -o ~/.claude/agents/mcp-developer.md
```

## Troubleshooting

### Agent Not Found

```bash
# Verify file exists
ls -la ~/.claude/agents/mcp-developer.md

# Check file has correct frontmatter
head -10 ~/.claude/agents/mcp-developer.md

# Should show:
# ---
# name: mcp-developer
# description: Expert MCP server developer...
# ---
```

### Agent Not Invoked

Try explicit invocation:

```
Use the mcp-developer agent to create a calculator server
```

Or mention keywords: "MCP server", "build", "create", "scaffold", "pmcp"

### cargo-pmcp Not Found

```bash
# Reinstall
cargo install cargo-pmcp --force

# Verify
which cargo-pmcp
cargo pmcp --version
```

## Summary

Setting up Claude Code for MCP development:

1. **Install Claude Code** and prerequisites (Rust, cargo-pmcp)
2. **Install mcp-developer agent** to `~/.claude/agents/`
3. **Verify with `/agents`** command
4. **Start building** - describe what you want
5. **Iterate** - let AI fix errors, add features
6. **Connect** your MCP server back to Claude Code

The agent handles the cargo-pmcp workflow, letting you focus on what you want to build rather than how to build it.

---

*Continue to [Alternative AI Assistants](./ch15-03-alternatives.md) →*
