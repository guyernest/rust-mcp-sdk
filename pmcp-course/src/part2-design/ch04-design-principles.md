# Beyond Tool Sprawl

You've built your first MCP servers. They work. Tools respond, resources load, tests pass. But working code isn't the same as well-designed code—especially in the MCP ecosystem.

This chapter challenges a dangerous assumption: that converting an existing API to MCP tools is sufficient. It's not. MCP operates in a fundamentally different environment than traditional APIs, and understanding this difference is critical to building servers that actually succeed in production.

## The MCP Environment Is Not What You Think

When you build a REST API, you control:
- Which endpoints exist
- How clients authenticate
- The order operations are called
- Error handling and retries
- Rate limiting and quotas

When you build an MCP server, you control almost none of this.

### You Don't Control Other Servers

Your MCP server isn't alone. The MCP client (Claude Desktop, Cursor, ChatGPT, or a custom application) may have multiple servers connected simultaneously:

```
┌─────────────────────────────────────────────────────────────┐
│                      MCP Client                             │
│                   (Claude Desktop)                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │ Your Server  │  │ Filesystem   │  │ GitHub       │       │
│  │ (db-explorer)│  │ Server       │  │ Server       │       │
│  │              │  │              │  │              │       │
│  │ • query_db   │  │ • read_file  │  │ • get_issues │       │
│  │ • list_tables│  │ • write_file │  │ • create_pr  │       │
│  │ • get_schema │  │ • list_dir   │  │ • get_commits│       │
│  └──────────────┘  └──────────────┘  └──────────────┘       │
│                                                             │
│  The AI sees ALL tools from ALL servers simultaneously      │
└─────────────────────────────────────────────────────────────┘
```

If your `db-explorer` server has a tool called `list`, and another server also has `list`, you've created ambiguity. The AI must choose between them based on descriptions alone. Poor naming, vague descriptions, or overlapping functionality leads to unpredictable behavior.

### You Don't Control the Client

The MCP client—typically an AI model—decides:

- **Which tools to call**: Based on the user's request and tool descriptions
- **In what order**: The AI determines the sequence of operations
- **With what parameters**: The AI constructs the arguments
- **How many times**: The AI may retry, iterate, or abandon

You cannot force the AI to call your tools in a specific order. You cannot prevent it from calling tools you didn't intend for a particular workflow. You cannot guarantee it will use the "right" tool for a task.

```
User: "Show me the sales data"

AI's internal reasoning (you don't see this):
- Found 3 potential tools: query_db, get_report, fetch_data
- query_db description mentions "SQL queries"
- get_report description mentions "sales reports"
- fetch_data description is vague: "fetches data"
- Choosing: get_report (best match for "sales")

What if get_report is from a DIFFERENT server than you expected?
```

### The User Has Some Control (But Not You)

Modern MCP clients like Claude Desktop and ChatGPT provide users with control mechanisms:

**Server Selection**: Users can enable/disable MCP servers per conversation:
- "Use only the database server for this task"
- "Don't use the GitHub server right now"

**Prompt Templates**: Users can invoke pre-defined prompts that guide the AI:
- `/analyze-schema` - A prompt that structures how schema analysis should proceed
- `/generate-report` - A prompt that defines report generation workflow

But notice: the *user* has this control, not you as the developer. Your job is to design servers that work well regardless of what other servers are connected, and to provide prompts that give users meaningful control over workflows.

## What You Actually Control

As an MCP server developer, your influence is limited to three things:

### 1. Tool Design

How you name, describe, and structure your tools determines whether the AI will use them correctly:

```rust
// Poor design: vague, overlapping with common names
Tool::new("get")
    .description("Gets data")

// Better design: specific, clear purpose
Tool::new("query_sales_database")
    .description("Execute read-only SQL queries against the sales PostgreSQL database. Returns results as JSON. Use for retrieving sales records, customer data, and transaction history.")
```

### 2. Resource Design

How you expose data as resources affects discoverability and appropriate usage:

```rust
// Resources are for stable, addressable data
Resource::new("sales://schema/customers")
    .description("Customer table schema including all columns and constraints")
    .mime_type("application/json")
```

### 3. Prompt Design

Prompts are your most powerful tool for guiding complex workflows:

```rust
// Prompts give users control over multi-step operations
Prompt::new("analyze-sales-trend")
    .description("Analyze sales trends over a specified period")
    .arguments(vec![
        PromptArgument::new("period").description("Time period: daily, weekly, monthly"),
        PromptArgument::new("metric").description("Metric to analyze: revenue, units, customers"),
    ])
```

## The Design Imperative

This chapter covers three critical design principles:

1. **Avoid Anti-Patterns**: Why "50 confusing tools" fails and what to do instead
2. **Design for Cohesion**: How to create tool sets that work together naturally
3. **Single Responsibility**: Why each tool should do one thing well

These principles aren't academic—they determine whether your MCP server will be reliably selected and correctly used by AI clients in a multi-server environment.

Let's start by examining what goes wrong when these principles are ignored.
