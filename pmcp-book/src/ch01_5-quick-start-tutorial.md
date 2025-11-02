# Chapter 1.5: Quick Start Tutorial

Before we dive into the technical details of building MCP servers, let's experience what it's like to build and use one! This hands-on tutorial will give you a feel for the development workflow and show you what's possible with MCP.

> **Note**: If you haven't installed `cargo-pmcp` yet, see [Chapter 1: Installation & Setup](ch01-installation.md).

## What You'll Experience

In the next 30 minutes, you'll progress through three phases of MCP server development:

1. **Phase 1 (10 min)**: Build a simple calculator server with one tool
2. **Phase 2 (10 min)**: Upgrade it to a complete calculator with validation and prompts
3. **Phase 3 (10 min)**: Add a database explorer with workflows

By the end, you'll have:
- Built and tested multiple MCP servers
- Connected them to Claude Code
- Experienced tools, prompts, workflows, and resources
- Seen how servers can be upgraded and composed

## Prerequisites

- `cargo-pmcp` installed (see Chapter 1)
- Claude Code or another MCP client
- 30 minutes of time
- A curious mindset!

## Phase 1: Your First MCP Server

### Create Your Workspace

Let's start by creating a new MCP workspace:

```bash
cargo pmcp new my-mcp-journey
cd my-mcp-journey
```

**What just happened?**

`cargo-pmcp` created a Cargo workspace with a `server-common` crate. This crate provides:
- HTTP server bootstrap (~80 LOC)
- Production logging
- Port configuration
- Error handling

All your servers will share this common infrastructure.

### Add a Simple Calculator

Now, let's add your first server:

```bash
cargo pmcp add server calculator --template calculator
```

```
Adding MCP server
─────────────────────────

  ✓ Created mcp-calculator-core (calculator template)
  ✓ Created calculator-server
  ✓ Created test scenarios
  ✓ Updated workspace members
  ✓ Assigned port 3000

✓ Server 'calculator' added successfully!
```

**Behind the scenes:**

- Created `mcp-calculator-core` with your server's business logic
- Created `calculator-server` with HTTP binary (6 lines!)
- Assigned port 3000 automatically
- Saved configuration to `.pmcp-config.toml`

### Start the Server

```bash
cargo pmcp dev --server calculator
```

You'll see:

```
Starting development server
────────────────────────────────────

Step 1: Building server
  ✓ Server built successfully

Step 2: Starting server
  → Server URL: http://0.0.0.0:3000

─────────────────────────────────────
Server is starting...
Press Ctrl+C to stop
─────────────────────────────────────

2025-01-15T10:30:00.123Z  INFO server_common: Starting MCP HTTP server port=3000
2025-01-15T10:30:00.456Z  INFO server_common: Server started on 0.0.0.0:3000
```

Your server is running! Keep this terminal open.

### Connect to Claude Code

Open a **new terminal** (keep the server running) and connect it to Claude Code:

```bash
cargo pmcp connect --server calculator --client claude-code
```

This updates your Claude Code configuration to include your MCP server.

### Test It!

Open Claude Code and try:

> "Add 5 and 3"

Claude will call your `add` tool and respond with `8`.

Try a few more:
- "What's 42 plus 17?"
- "Calculate the sum of 123 and 456"
- "Add 0.5 and 0.25"

**What's happening under the hood:**

1. Claude sends an MCP request: `{ "method": "tools/call", "params": { "name": "add", "arguments": { "a": 5, "b": 3 } } }`
2. Your server receives the JSON-RPC request
3. The `add` tool executes: `AddInput { a: 5.0, b: 3.0 }` → `AddResult { result: 8.0 }`
4. Server responds with JSON
5. Claude formats the response naturally

### Peek at the Code

While the server is running, open `crates/mcp-calculator-core/src/lib.rs` in your editor:

```rust
// Tool inputs are validated automatically
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddInput {
    /// First number
    #[schemars(description = "The first number to add")]
    pub a: f64,

    /// Second number
    #[schemars(description = "The second number to add")]
    pub b: f64,
}

// Tool handler - simple async function
async fn add_tool(input: AddInput, _extra: RequestHandlerExtra) -> Result<AddResult> {
    Ok(AddResult {
        result: input.a + input.b,
    })
}

// Server registration - type-safe with automatic schema generation
Server::builder()
    .name("calculator")
    .tool("add", TypedTool::new("add", |input, extra| {
        Box::pin(add_tool(input, extra))
    }))
    .build()
```

**Key takeaways:**

- Tools are just Rust functions with typed inputs/outputs
- `TypedTool` generates JSON schemas automatically from your types
- The MCP protocol handles serialization, validation, and errors
- You write business logic, PMCP handles the protocol

### Phase 1 Complete! ✓

You've just:
- Created an MCP workspace
- Built a server with one tool
- Connected it to Claude Code
- Made your first MCP tool calls

Press Ctrl+C in the server terminal to stop it.

## Phase 2: Level Up with Advanced Features

Now let's see what a more complete server looks like. Instead of starting from scratch, we'll **upgrade** your existing calculator.

### Upgrade Your Server

```bash
cargo pmcp add server calculator --template complete-calculator --replace
```

You'll see a confirmation prompt:

```
⚠ Server 'calculator' already exists:
  Current template: calculator
  Current port:     3000
  New template:     complete-calculator

⚠ This will delete the existing server crates. Continue? [y/N]:
```

Type `y` and press Enter.

```
  ✓ Removed crates/mcp-calculator-core
  ✓ Removed crates/calculator-server

  ✓ Created mcp-calculator-core (complete-calculator template)
  ✓ Created calculator-server
  ✓ Created test scenarios
  ✓ Updated workspace members
  ✓ Assigned port 3000

✓ Server 'calculator' added successfully!
```

**What happened:**

- Old calculator crates were removed
- New crates with more tools were generated
- **Port 3000 was preserved** (Claude Code still works!)
- Configuration updated seamlessly

### Restart and Test

```bash
cargo pmcp dev --server calculator
```

The server starts on the same port (3000). Your Claude Code connection automatically works with the upgraded server!

Try these in Claude Code:

> "Multiply 7 by 8"

> "Divide 100 by 4"

> "What's 2 to the power of 10?"

> "Calculate the square root of 144"

**New tools available:**
- `add`, `subtract`, `multiply` - Basic arithmetic
- `divide` - With zero-division validation
- `power` - Exponentiation
- `sqrt` - Square root

### Test Error Handling

Try this:

> "Divide 10 by 0"

Claude responds with something like: *"I can't divide by zero. Division by zero is mathematically undefined."*

**Why?** Your server's `divide` tool includes validation:

```rust
async fn divide_tool(input: DivideInput, _extra: RequestHandlerExtra) -> Result<DivideResult> {
    if input.b == 0.0 {
        return Err(Error::validation("Cannot divide by zero"));
    }
    Ok(DivideResult {
        result: input.a / input.b,
    })
}
```

MCP's error handling lets Claude understand what went wrong and respond appropriately.

### Try a Prompt (Workflow)

In Claude Code, use the `/prompts` command:

```
/quadratic a: 1 b: -5 c: 6
```

Claude shows a step-by-step solution for the quadratic equation **x² - 5x + 6 = 0**:

```
This equation has two real roots.
Discriminant Δ = b² - 4ac = 1

Using the quadratic formula:
x = (-b ± √Δ) / 2a

Solution 1: x₁ = (5 + 1) / 2 = 3
Solution 2: x₂ = (5 - 1) / 2 = 2

Verification:
  x = 3: (3)² - 5(3) + 6 = 0 ✓
  x = 2: (2)² - 5(2) + 6 = 0 ✓
```

**What's different from tools?**

Prompts are **multi-step workflows** that:
- Guide Claude through a structured process
- Can call multiple tools in sequence
- Provide context and instructions
- Return rich, formatted responses

### Explore Resources

Ask Claude:

> "Show me the quadratic formula guide"

Claude accesses the `quadratic-formula` resource and explains the mathematical theory with examples.

**Resources** are like documentation that Claude can read:
- Static content (markdown, JSON, etc.)
- Provides context for the AI
- Educational material
- API documentation
- Configuration data

### Phase 2 Complete! ✓

You've experienced:
- Server upgrades with `--replace`
- Multiple tools in one server
- Input validation and error handling
- Prompts (multi-step workflows)
- Resources (static knowledge)

Stop the server (Ctrl+C) and move to the final phase.

## Phase 3: Real-World Application (Database)

Let's build something more substantial—a database explorer that demonstrates advanced MCP features.

### Add a Second Server

```bash
cargo pmcp add server explorer --template sqlite-explorer
```

Notice what happened:

```
  ✓ Assigned port 3001
```

Your workspace now has **two servers**:
- `calculator` (port 3000)
- `explorer` (port 3001)

No port conflicts! `cargo-pmcp` automatically assigned the next available port.

### Download Sample Database

The SQLite Explorer needs a database. Let's use the Chinook database (a sample music store):

```bash
cd crates/mcp-explorer-core
curl -L https://github.com/lerocha/chinook-database/raw/master/ChinookDatabase/DataSources/Chinook_Sqlite.sqlite -o chinook.db
cd ../..
```

This database contains:
- 11 tables (customers, tracks, albums, artists, etc.)
- 3,500+ music tracks
- Real-world relationships and data

### Start the Explorer

```bash
cargo pmcp dev --server explorer
```

Server starts on port 3001.

### Connect to Claude Code

In a new terminal:

```bash
cargo pmcp connect --server explorer --client claude-code
```

Your Claude Code now has **two MCP servers** configured:
- Calculator (port 3000) - Off
- Explorer (port 3001) - Running

### Explore the Database

In Claude Code, try:

> "Show me all tables in the database"

Claude calls `list_tables` and shows you the schema.

> "Get sample rows from the Track table"

Claude calls `get_sample_rows` with `table: "Track"` and displays a preview.

> "List the top 10 customers by total purchases"

Claude writes a SQL query, calls `execute_query`, and shows the results!

**Tools available:**
- `execute_query` - Run SELECT queries (read-only, validated)
- `list_tables` - Show tables with row counts
- `get_sample_rows` - Preview table data

### Advanced: Workflow with Template Substitution

Use the workflow prompts:

```
/monthly_sales_report month: 3 year: 2021
```

This runs a workflow that:
1. Takes your arguments (`month: 3`, `year: 2021`)
2. Substitutes them into a SQL template: `WHERE CAST(strftime("%m", InvoiceDate) AS INTEGER) = {month}`
3. Executes the query
4. Returns formatted results

**Template substitution** allows workflows to use user input safely in SQL queries, API calls, etc.

### Advanced: Multi-Step Workflow with Bindings

```
/analyze_customer customer_id: 5
```

This executes a **3-step workflow**:

1. **Step 1**: Query customer info → binds as `customer_info`
2. **Step 2**: Query purchase history → binds as `purchase_history`
3. **Step 3**: Calculate lifetime value → binds as `lifetime_metrics`

Each step can reference previous steps' outputs. The server orchestrates the entire flow!

Try the most advanced workflow:

```
/customers_who_bought_top_tracks limit: 10
```

This demonstrates **client-side orchestration**:

1. Server executes: "Get top 10 tracks by sales"
2. Result binds as `top_tracks`
3. **Claude receives the binding** and continues
4. Claude extracts track IDs from results
5. Claude generates SQL: "Find customers who bought these tracks"
6. Claude calls `execute_query` with generated SQL

The workflow combines **server-side** (SQL execution) and **client-side** (data analysis, query generation) orchestration!

### Run Both Servers Simultaneously

Want to see both servers at once?

Terminal 1:
```bash
cargo pmcp dev --server calculator
```

Terminal 2:
```bash
cargo pmcp dev --server explorer
```

Now Claude Code has access to **both** calculator and database tools simultaneously!

Try:

> "Calculate the square root of the number of tracks in the database"

Claude:
1. Calls `explorer.list_tables` to get track count
2. Extracts the number
3. Calls `calculator.sqrt` with that number
4. Returns the result

**This is the power of composable MCP servers!**

### Phase 3 Complete! ✓

You've experienced:
- Multiple servers in one workspace
- Automatic port management (3000, 3001)
- Database operations with safety
- Workflow template substitution
- Multi-step workflows with bindings
- Client-side orchestration
- Server composition

## Phase 4: Automated Testing (10 min)

You've built two MCP servers manually and tested them with Claude Code. But production servers need **automated tests**. Let's see how `cargo-pmcp` makes testing effortless.

### Test the Calculator

Stop any running servers (Ctrl+C), then let's test the calculator:

```bash
cargo pmcp test --server calculator --generate-scenarios
```

**What happens:**

```
Testing MCP server
─────────────────────────────────────

Step 1: Building server
  ✓ Server built successfully

Step 2: Generating test scenarios
  → Starting server on port 3000...
  → Generating scenarios...
  ✓ Scenarios generated at scenarios/calculator/generated.yaml

Step 3: Running tests
  → Starting server on port 3000...
  → Running mcp-tester...

  Testing: scenarios/calculator/generated.yaml

Step 1: List available capabilities
  ✓ PASSED

Step 2: Test tool: add (123 + 234 = 357)
  ✓ PASSED - Result: 357

Step 3: Test tool: multiply (12 × 3 = 36)
  ✓ PASSED - Result: 36

Step 4: Test tool: divide (100 ÷ 4 = 25)
  ✓ PASSED - Result: 25

═════════════════════════════════════
✓ All tests passed!
═════════════════════════════════════
```

### What Just Happened?

The `--generate-scenarios` flag told cargo-pmcp to:

1. **Discover** all tools from the running server
2. **Generate smart test cases** with meaningful values:
   - `add(123, 234)` expects `357`
   - `multiply(12, 3)` expects `36`
   - `divide(100, 4)` expects `25`
3. **Create assertions** to verify the results
4. **Run the tests** automatically

### Inspect the Generated Scenarios

Open `scenarios/calculator/generated.yaml`:

```yaml
name: calculator Test Scenario
description: Automated test scenario for http://0.0.0.0:3000 server
timeout: 60
stop_on_failure: false

variables:
  test_id: "test_123"
  test_value: "sample_value"

steps:
  - name: List available capabilities
    operation:
      type: list_tools
    assertions:
      - type: success
      - type: exists
        path: tools

  - name: Test tool: add (123 + 234 = 357)
    operation:
      type: tool_call
      tool: add
      arguments:
        a: 123
        b: 234
    timeout: 30
    continue_on_failure: true
    store_result: add_result
    assertions:
      - type: success
      - type: equals
        path: result
        value: 357

  - name: Test tool: multiply (12 × 3 = 36)
    operation:
      type: tool_call
      tool: multiply
      arguments:
        a: 12
        b: 3
    timeout: 30
    continue_on_failure: true
    store_result: multiply_result
    assertions:
      - type: success
      - type: equals
        path: result
        value: 36
```

**Notice:**
- Smart test values: `123 + 234 = 357` (not just `1 + 1 = 2`)
- Correct expected results calculated automatically
- Assertions verify both success and correctness
- Human-readable YAML format

### Customize and Re-Run

You can edit the scenario file to add more tests:

```yaml
  - name: Test division by zero (should fail)
    operation:
      type: tool_call
      tool: divide
      arguments:
        a: 10
        b: 0
    assertions:
      - type: failure  # We expect this to fail!
```

Then run tests again (without regenerating):

```bash
cargo pmcp test --server calculator
```

### Test the Database Explorer

Now let's test the SQLite explorer:

```bash
cargo pmcp test --server explorer --generate-scenarios
```

The test generator will:
- Discover the `execute_query`, `list_tables`, and `get_sample_rows` tools
- Generate safe test queries
- Verify the database operations work correctly

### Detailed Test Output

Want to see every request/response?

```bash
cargo pmcp test --server calculator --detailed
```

This shows:
- Full JSON-RPC requests
- Server responses
- Assertion evaluation details
- Timing information

### Integration with CI/CD

These tests are perfect for CI/CD pipelines:

```yaml
# .github/workflows/test.yml
name: Test MCP Servers

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable

      - name: Test Calculator
        run: cargo pmcp test --server calculator

      - name: Test Explorer
        run: cargo pmcp test --server explorer
```

### Why Automated Testing Matters

**Before automated tests:**
- ❌ Manual testing with Claude Code every time
- ❌ Easy to miss edge cases
- ❌ No regression protection
- ❌ Can't validate in CI/CD

**With automated tests:**
- ✅ One command tests everything
- ✅ Smart test generation with correct values
- ✅ Regression protection on every commit
- ✅ CI/CD ready
- ✅ Confidence in changes

### Test Scenario Format

The YAML format supports:

**Operations:**
- `list_tools` - Discover capabilities
- `tool_call` - Call a tool with arguments
- `list_resources` - List available resources
- `read_resource` - Read resource content
- `list_prompts` - List available prompts
- `get_prompt` - Get a prompt template

**Assertions:**
- `success` - Response has no error
- `failure` - Response has an error
- `equals` - Field equals exact value
- `contains` - Field contains substring
- `exists` - Field exists
- `matches` - Field matches regex
- `numeric` - Numeric comparisons (>, <, between)
- `array_length` - Array has specific length

**Advanced Features:**
- `variables` - Store and reuse values
- `setup`/`cleanup` - Pre/post test steps
- `store_result` - Save results for later steps
- `continue_on_failure` - Don't stop on failure

### Phase 4 Complete! ✓

You've experienced:
- Automated test generation
- Smart test value generation (123+234=357)
- Protocol compliance testing
- Scenario-based testing
- YAML test format
- CI/CD integration
- Regression protection

## What You've Learned

In 40 minutes, you've progressed from zero to production-ready:

| Concept | Phase 1 | Phase 2 | Phase 3 | Phase 4 |
|---------|---------|---------|---------|---------|
| **Tools** | 1 | 6 | 3 | Same |
| **Validation** | None | Yes | SQL safety | Same |
| **Prompts** | No | 1 | 3 | Same |
| **Resources** | No | 1 | 2 | Same |
| **Workflows** | No | Simple | Multi-step + bindings | Same |
| **Servers** | 1 | 1 | 2 | Same |
| **Testing** | Manual | Manual | Manual | **Automated!** |
| **CI/CD Ready** | No | No | No | **Yes!** |

### Key Concepts Experienced

1. **Tools** - Functions Claude can call (your business logic)
2. **Prompts** - Multi-step workflows that guide the AI
3. **Resources** - Static knowledge the AI can access
4. **Workflows** - Orchestrated sequences with data bindings
5. **Validation** - Type-safe inputs with automatic error handling
6. **Composition** - Multiple servers working together
7. **Automated Testing** - Smart test generation with meaningful values (123+234=357)
8. **CI/CD Integration** - Production-ready testing infrastructure

### What Makes PMCP Different

Through this tutorial, you experienced:

✅ **Type Safety**: Rust types → JSON schemas automatically
✅ **Zero Boilerplate**: `server-common` handles HTTP/logging/ports
✅ **Developer Experience**: `cargo-pmcp` CLI makes it effortless
✅ **Progressive Complexity**: Start simple, add features incrementally
✅ **Production Ready**: Middleware, auth, monitoring built-in
✅ **Composition**: Multiple servers, automatic port management
✅ **Smart Testing**: Automated test generation with meaningful values (123+234=357)
✅ **CI/CD Ready**: One command tests everything, perfect for automation

## Next Steps

Now that you've experienced MCP development, you're ready to learn the technical details:

### Continue the Book

- **[Chapter 2: Your First MCP Server](ch02-first-server.md)** - Build a server from scratch, understand every line
- **[Chapter 5: Tools & Tool Handlers](ch05-tools.md)** - Deep dive into tool patterns
- **[Chapter 7: Prompts & Templates](ch07-prompts.md)** - Master workflow orchestration
- **[Chapter 13: Building Production Servers](ch13-production.md)** - Take servers to production

### Explore the Examples

```bash
cd /Users/guy/Development/mcp/sdk/rust-mcp-sdk/examples
```

Check out:
- `weather-server` - Real-world API integration
- `file-explorer` - Filesystem operations with security
- `calculator-advanced` - Complete math server with units

### Customize Your Servers

Go back to your workspace and experiment:

```bash
cd my-mcp-journey
```

Try:
- Add a new tool to the calculator
- Create a custom workflow for the database
- Build your own server from scratch: `cargo pmcp add server my-server --template minimal`

### Join the Community

- Read the [full TUTORIAL.md](../../TUTORIAL.md) for more details
- Check [GitHub discussions](https://github.com/paiml/rust-mcp-sdk/discussions)
- Contribute your own examples!

## Reflection

You've just experienced what makes MCP powerful:

> **MCP turns your Rust functions into AI-accessible tools with type safety, validation, and protocol handling—all automatically.**

The protocol is simple, but the possibilities are endless:
- Database operations
- API integrations
- File system access
- Code execution
- Image processing
- Machine learning
- IoT control
- And much more...

Now you're ready to dive deeper and build production MCP servers!

---

**Next Chapter**: [Chapter 2: Your First MCP Server](ch02-first-server.md) - Learn the technical details of everything you just experienced.
