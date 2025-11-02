# MCP Server Development Tutorial

Learn MCP server development through progressive discovery. This tutorial takes you from basic concepts to advanced features in three phases.

## Prerequisites

- Rust installed (1.70 or later)
- Claude Code or another MCP client
- 30 minutes

## Learning Path Overview

| Phase | Template | What You'll Learn | Duration |
|-------|----------|-------------------|----------|
| 1 | Simple Calculator | Basic MCP: tools, requests, responses | 10 min |
| 2 | Complete Calculator | Advanced: validation, errors, resources, prompts | 10 min |
| 3 | SQLite Explorer | Expert: workflows, step bindings, databases | 10 min |

---

## Phase 1: Your First MCP Server (Simple Calculator)

**Goal**: Understand basic MCP concepts with a single `add` tool.

### Step 1: Create Workspace

```bash
cargo pmcp new my-mcp-learning
cd my-mcp-learning
```

This creates a workspace with `server-common` (shared HTTP bootstrap).

### Step 2: Add Simple Calculator Server

```bash
cargo pmcp add server calculator --template calculator
```

**What happened:**
- Created `mcp-calculator-core` (business logic)
- Created `calculator-server` (HTTP binary)
- Assigned port 3000 automatically
- Saved configuration to `.pmcp-config.toml`

### Step 3: Start the Server

```bash
cargo pmcp dev --server calculator
```

**What to observe:**
- Server builds and starts on port 3000
- Shows "Server URL: http://0.0.0.0:3000"
- Logs show MCP server is ready

### Step 4: Connect to Claude Code

In a **new terminal** (keep server running):

```bash
cargo pmcp connect --server calculator --client claude-code
```

This configures Claude Code to connect to your server.

### Step 5: Test the `add` Tool

In Claude Code, try:
- "Add 5 and 3"
- "What's 42 plus 17?"
- "Calculate the sum of 123 and 456"

**What's happening:**
1. Claude Code sends MCP request to your server
2. Server receives JSON-RPC request for `add` tool
3. Tool executes: `{ a: 5, b: 3 }` → `{ result: 8 }`
4. Claude Code receives response and answers

### Step 6: Explore the Code

Open `crates/mcp-calculator-core/src/lib.rs` and examine:

```rust
// Tool Input - Auto-generates JSON schema
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddInput {
    pub a: f64,
    pub b: f64,
}

// Tool Handler - Async function
async fn add_tool(input: AddInput, _extra: RequestHandlerExtra) -> Result<AddResult> {
    Ok(AddResult {
        result: input.a + input.b,
    })
}

// Server Registration
Server::builder()
    .tool("add", TypedTool::new("add", |input, extra| {
        Box::pin(add_tool(input, extra))
    }))
    .build()
```

**Key Concepts:**
- **Tools** = Functions Claude can call
- **TypedTool** = Type-safe tool with automatic JSON schema
- **Input/Output** = Defined by Rust types (validated automatically)

### Checkpoint: What You've Learned ✓

- [x] MCP servers provide tools to AI agents
- [x] Tools have typed inputs and outputs
- [x] `cargo-pmcp` handles HTTP bootstrap
- [x] Servers run on specific ports (3000 by default)

Stop the server (Ctrl+C) and proceed to Phase 2.

---

## Phase 2: Level Up (Complete Calculator)

**Goal**: Learn advanced patterns—multiple tools, validation, resources, prompts.

### Step 1: Upgrade Your Server

```bash
cargo pmcp add server calculator --template complete-calculator --replace
```

**What happened:**
- Prompted for confirmation (shows current vs new template)
- Deleted old `mcp-calculator-core` and `calculator-server`
- Generated new crates with complete template
- **Kept port 3000** (no config change needed in Claude Code!)

### Step 2: Restart the Server

```bash
cargo pmcp dev --server calculator
```

Server starts on **same port** (3000)—Claude Code reconnects automatically!

### Step 3: Test Multiple Tools

In Claude Code, try:
- "Multiply 7 by 8"
- "Divide 100 by 4"
- "What's 2 to the power of 10?"
- "Calculate the square root of 144"

**New capabilities:**
- `add`, `subtract`, `multiply` (basic arithmetic)
- `divide` (with zero-division validation!)
- `power` (exponentiation)
- `sqrt` (square root)

### Step 4: Test Error Handling

Try: "Divide 10 by 0"

Observe Claude's response—the server returned a validation error!

```rust
// In divide_tool():
if input.b == 0.0 {
    return Err(Error::validation("Cannot divide by zero"));
}
```

### Step 5: Try a Prompt (Workflow)

In Claude Code, use the `/prompts` command:
```
/quadratic a: 1 b: -5 c: 6
```

**What's happening:**
1. Prompt orchestrates multiple tools
2. Calculates discriminant (b² - 4ac)
3. Determines if real roots exist
4. Calculates roots using quadratic formula
5. Returns step-by-step solution

### Step 6: Explore Resources

Ask Claude: "Show me the quadratic formula guide"

**Resources provide:**
- Static knowledge/documentation
- Context for the AI
- Educational material

### Checkpoint: What You've Learned ✓

- [x] Servers can have multiple tools
- [x] Validation returns errors to the client
- [x] Prompts orchestrate multi-step workflows
- [x] Resources provide static knowledge
- [x] `--replace` upgrades servers in-place

Stop the server and proceed to Phase 3.

---

## Phase 3: Advanced Features (SQLite Explorer)

**Goal**: Master workflows with step bindings, resources, and real databases.

### Step 1: Add Database Server

```bash
cargo pmcp add server explorer --template sqlite-explorer
```

**What happened:**
- Created explorer server
- **Auto-assigned port 3001** (next available)
- Both servers in workspace:
  - `calculator` (port 3000)
  - `explorer` (port 3001)

### Step 2: Download Sample Database

```bash
cd crates/mcp-explorer-core
curl -L https://github.com/lerocha/chinook-database/raw/master/ChinookDatabase/DataSources/Chinook_Sqlite.sqlite -o chinook.db
cd ../..
```

This downloads the Chinook database (music store with 11 tables, 3500+ tracks).

### Step 3: Start Explorer Server

```bash
cargo pmcp dev --server explorer
```

Server starts on port 3001.

### Step 4: Connect Explorer to Claude Code

In a **new terminal**:

```bash
cargo pmcp connect --server explorer --client claude-code
```

**Now you have TWO servers configured in Claude Code:**
- `calculator` (port 3000) - Off
- `explorer` (port 3001) - Running

### Step 5: Explore the Database

In Claude Code, try:
- "Show me all tables in the database"
- "Get sample rows from the Track table"
- "List customers and their countries"

**Tools available:**
- `execute_query` - Run SELECT queries (read-only)
- `list_tables` - Show all tables with row counts
- `get_sample_rows` - Preview table data

### Step 6: Use Advanced Workflows

Try the workflow prompts:

```
/monthly_sales_report month: 3 year: 2021
```

**What's different from Phase 2:**
- Workflows use SQL template substitution: `WHERE month = {month}`
- Multi-step workflows with bindings: Step 1 output → Step 2 input
- Real database queries (not just calculations)

```
/analyze_customer customer_id: 5
```

This runs a **3-step workflow**:
1. Get customer info
2. Get purchase history
3. Calculate lifetime value

All results are bound and composed automatically!

### Step 7: Multi-Step Workflow

```
/customers_who_bought_top_tracks limit: 10
```

**How it works:**
1. Step 1: Query top 10 tracks by purchase count → binds as `top_tracks`
2. Client (Claude) receives `top_tracks` binding
3. Claude extracts track IDs
4. Claude generates SQL to find customers
5. Claude calls `execute_query` with generated SQL

This demonstrates **client-side orchestration**!

### Step 8: Run Both Servers

You can run both servers simultaneously:

Terminal 1:
```bash
cargo pmcp dev --server calculator
```

Terminal 2:
```bash
cargo pmcp dev --server explorer
```

Both servers run on different ports—Claude Code can use both at once!

### Checkpoint: What You've Learned ✓

- [x] Multiple servers can coexist (different ports)
- [x] Workflows can use template substitution
- [x] Multi-step workflows with bindings
- [x] Resources provide database schemas
- [x] Real-world use case: database operations

---

## Summary: Your MCP Journey

| Concept | Phase 1 | Phase 2 | Phase 3 |
|---------|---------|---------|---------|
| **Tools** | 1 (add) | 6 (arithmetic) | 3 (database ops) |
| **Validation** | None | Division by zero | SQL safety |
| **Prompts** | None | Quadratic solver | 3 workflows |
| **Resources** | None | Formula guide | DB schema |
| **Bindings** | No | No | Yes |
| **Complexity** | ⭐ Simple | ⭐⭐ Intermediate | ⭐⭐⭐ Advanced |

## Next Steps

### Customize Your Servers

1. **Add a new tool** to calculator:
   ```rust
   // In mcp-calculator-core/src/lib.rs
   async fn factorial_tool(input: FactorialInput, _extra: RequestHandlerExtra) -> Result<FactorialResult> {
       // Your implementation
   }
   ```

2. **Create a custom workflow** for explorer:
   ```rust
   fn create_genre_analysis_workflow() -> SequentialWorkflow {
       // Your workflow
   }
   ```

3. **Build your own server** from scratch:
   ```bash
   cargo pmcp add server my-server --template minimal
   ```

### Production Deployment

See the [Deployment Guide](./DEPLOYMENT.md) for:
- Docker containerization
- Environment configuration
- Production logging
- Monitoring and metrics

### Advanced Topics

- **Authentication**: Add auth middleware
- **Rate Limiting**: Protect your APIs
- **Custom Transport**: Beyond HTTP
- **Testing**: Write integration tests

## Troubleshooting

### Port Already in Use

```bash
Error: Port 3000 is already in use
```

**Solution**: Stop the other server or use a different port:
```bash
cargo pmcp dev --server calculator --port 3005
```

### Server Not Showing in Claude Code

1. Check `.pmcp-config.toml` has correct port
2. Restart Claude Code
3. Run connect command again:
   ```bash
   cargo pmcp connect --server explorer --client claude-code
   ```

### Template Substitution Not Working

Make sure you're using the latest version:
```bash
cargo install --path /path/to/rust-mcp-sdk/cargo-pmcp --force
```

## Resources

- [MCP Specification](https://spec.modelcontextprotocol.io/)
- [PMCP SDK Documentation](https://docs.rs/pmcp)
- [Example Servers](./examples/)
- [API Reference](https://docs.rs/pmcp/latest/pmcp/)

## Feedback

Found an issue or have a suggestion?
- [Open an issue](https://github.com/paiml/rust-mcp-sdk/issues)
- [Discussions](https://github.com/paiml/rust-mcp-sdk/discussions)

---

**Congratulations!** 🎉 You've completed the MCP Server Development Tutorial. You now understand:
- Basic MCP architecture (tools, prompts, resources)
- Progressive server development (simple → complete → advanced)
- Multi-server workspaces with port management
- Production patterns with cargo-pmcp

Happy building! 🚀
