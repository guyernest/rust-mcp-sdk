# Prompting for MCP Tools

Effective prompts lead to better code faster. This chapter covers strategies for communicating tool requirements to AI assistants, from simple requests to complex multi-tool servers.

## The Anatomy of a Good Prompt

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Effective MCP Tool Prompt                            │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │ 1. CONTEXT                                                      │    │
│  │    "Create an MCP server for [domain]..."                       │    │
│  │    Sets the problem space and technology                        │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │ 2. CAPABILITY                                                   │    │
│  │    "...with tools that [action] and [action]..."                │    │
│  │    Describes what the server should do                          │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │ 3. CONSTRAINTS                                                  │    │
│  │    "...limit [X], require [Y], return [Z]..."                   │    │
│  │    Sets boundaries and requirements                             │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │ 4. EXAMPLES (Optional)                                          │    │
│  │    "For example, when given [input], return [output]"           │    │
│  │    Clarifies expected behavior                                  │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## Prompt Levels

### Level 1: Simple Tool Request

For single, straightforward tools:

```
Create a tool that converts temperatures between Celsius and Fahrenheit.
Input: temperature and source unit.
Output: converted temperature in both units.
```

**What AI generates**:
- Input type with temperature (f64) and unit (enum)
- Output type with both conversions
- Validation for reasonable temperature range
- Unit tests

### Level 2: Detailed Tool Request

For tools with specific requirements:

```
Create a `search_logs` tool that queries application logs.

Input:
- query: String (required) - search pattern (regex supported)
- start_time: DateTime (optional) - earliest log timestamp
- end_time: DateTime (optional) - latest log timestamp
- level: enum [DEBUG, INFO, WARN, ERROR] (optional) - filter by level
- limit: u32 (optional, default 100, max 1000) - result count

Output:
- matches: array of log entries with timestamp, level, message
- total_count: total matches (may exceed limit)
- truncated: boolean indicating if results were limited

Error cases:
- Invalid regex pattern → validation error with pattern location
- Invalid time range (end before start) → validation error
- No logs found → empty result (not an error)

Use chrono for timestamps. Return newest logs first.
```

**What AI generates**:
- Complete input/output types with all fields
- Regex validation with helpful error messages
- Time range validation
- Pagination handling
- Comprehensive test coverage

### Level 3: Server Architecture

For complete server design:

```
Create a CI/CD MCP server for GitHub Actions.

Tools:
1. list_workflows - Get all workflows for a repo
   - Input: owner, repo
   - Output: workflow id, name, state, path

2. get_workflow_runs - Get recent runs for a workflow
   - Input: owner, repo, workflow_id, status filter (optional)
   - Output: run id, status, conclusion, started_at, duration

3. trigger_workflow - Start a workflow run
   - Input: owner, repo, workflow_id, ref (branch), inputs (map)
   - Output: run id, url
   - IMPORTANT: Require confirmation in description

4. cancel_run - Cancel an in-progress run
   - Input: owner, repo, run_id
   - Output: success boolean

Architecture:
- Use octocrab crate for GitHub API
- Token from GITHUB_TOKEN env var
- Rate limiting: implement retry with backoff
- All times in UTC

Security:
- trigger_workflow must log the action
- No workflow deletion capabilities
```

## Specifying Input Types

### Required vs Optional Fields

```
Create a `send_notification` tool:

Required inputs:
- recipient: String (email or phone)
- message: String (1-1000 chars)

Optional inputs:
- subject: String (for email only)
- priority: enum [LOW, NORMAL, HIGH] (default: NORMAL)
- schedule_at: DateTime (send later, must be future)
```

AI understands:
- `Option<T>` for optional fields
- Default values via `unwrap_or`
- Conditional validation (subject only for email)

### Enums and Constraints

```
Create a `convert_document` tool:

Input format (enum): PDF, DOCX, HTML, MARKDOWN
Output format (enum): PDF, DOCX, HTML, MARKDOWN, TXT

Constraint: Cannot convert to same format (validation error)
Constraint: PDF output only from DOCX, HTML, MARKDOWN
```

AI generates proper validation:

```rust
if input.source_format == input.target_format {
    return Err(Error::validation("Cannot convert to same format"));
}

if matches!(input.target_format, OutputFormat::Pdf) {
    if !matches!(input.source_format, SourceFormat::Docx | SourceFormat::Html | SourceFormat::Markdown) {
        return Err(Error::validation("PDF output requires DOCX, HTML, or Markdown input"));
    }
}
```

### Complex Nested Types

```
Create a `create_order` tool:

Input:
- customer_id: String
- items: array of:
  - product_id: String
  - quantity: u32 (min 1, max 100)
  - options: optional map of String → String
- shipping_address:
  - street: String
  - city: String
  - state: String (2 letters for US)
  - zip: String
  - country: String (ISO 3166-1 alpha-2)
- payment_method: enum [CREDIT_CARD, PAYPAL, INVOICE]
```

AI generates proper nested types with validation.

## Specifying Output Types

### Simple Output

```
Return temperature in both Celsius and Fahrenheit
```

### Structured Output

```
Output for get_user tool:
- id: String (UUID)
- email: String
- created_at: DateTime
- profile:
  - display_name: String
  - avatar_url: Option<String>
  - bio: Option<String>
- settings:
  - theme: enum [LIGHT, DARK, SYSTEM]
  - notifications: boolean
```

### Pagination Output

```
Output for list_items tool:
- items: array of Item objects
- pagination:
  - total: u64 (total matching items)
  - page: u32 (current page, 1-indexed)
  - per_page: u32
  - has_next: boolean
  - next_cursor: Option<String>
```

## Error Handling Guidance

### Explicit Error Cases

```
Handle these error cases for the database query tool:

1. Empty query → Error::validation("Query cannot be empty")
2. Query too long (>10000 chars) → Error::validation with limit info
3. Query timeout (>30s) → Error::internal("Query exceeded timeout")
4. Connection failure → Error::internal with retry suggestion
5. Permission denied → Error::validation("Insufficient permissions for table X")
6. Invalid SQL syntax → Error::validation with position of error

For all errors, include:
- What went wrong
- Why it matters
- How to fix it (if possible)
```

### Error Context

```
Use .context() for all fallible operations:

Good: .context("Failed to connect to database at {url}")?
Good: .context("Query returned invalid JSON for field 'created_at'")?

Bad: .context("error")?  // Too vague
Bad: ? alone  // No context
```

## Iterating on Generated Code

### Refinement Prompts

After initial generation:

```
The get_weather tool works but:
1. Add caching for 5 minutes (same city returns cached result)
2. Support multiple cities in one call (batch lookup)
3. Add unit tests for cache expiration
```

### Bug Fix Prompts

When something doesn't work:

```
The search_users tool has an issue:
- Input: { "query": "john", "limit": 10 }
- Expected: Users with "john" in name or email
- Actual: Returns all users

Fix the handler to actually filter by the query parameter.
```

### Performance Prompts

For optimization:

```
The list_transactions tool is slow for large accounts.

Requirements:
1. Add cursor-based pagination instead of offset
2. Limit results to 100 per call max
3. Add index hint for created_at field
4. Return only id, amount, timestamp (not full transaction)
```

## Domain-Specific Patterns

### Database Tools

```
Create a PostgreSQL MCP server with these patterns:

1. Read-only by default: Only SELECT queries allowed
2. Query timeout: 30 second max
3. Row limit: 1000 rows max (with truncation indicator)
4. Schema filtering: Only show tables matching pattern
5. Sensitive columns: Hide columns named *password*, *secret*, *token*

Use sqlx with connection pooling.
```

### API Integration Tools

```
Create a Stripe MCP server following these patterns:

1. API key from STRIPE_API_KEY env var
2. Rate limiting: Respect Stripe's rate limits with backoff
3. Pagination: Use Stripe's cursor pagination
4. Idempotency: Add idempotency_key for mutations
5. Webhooks: NOT included (separate concern)

Tools:
- list_customers, get_customer, create_customer
- list_charges, get_charge, create_charge
- list_subscriptions, get_subscription
```

### File System Tools

```
Create a safe file system MCP server:

Security constraints:
1. Sandbox to specified root directory
2. No path traversal (reject ../.. patterns)
3. No symlink following outside sandbox
4. Max file size: 10MB for read/write
5. No execution of files

Tools:
- list_files: dir contents with type, size, modified
- read_file: contents as text (detect encoding)
- write_file: create/overwrite with content
- delete_file: remove single file (not directories)
```

## Anti-Patterns in Prompting

### Too Vague

**Bad**:
```
Make a tool that does stuff with data
```

**Good**:
```
Create a tool that parses CSV files and returns rows as JSON
```

### Too Prescriptive

**Bad**:
```
Create a struct named DataInput with field data of type Vec<u8>.
Then create a function named process_data that takes DataInput
and returns Result<DataOutput, Error>. The function should first
check if data.len() > 0...
```

**Good**:
```
Create a data processing tool that accepts binary data,
validates it's not empty, and returns the parsed result.
```

Let AI choose implementation details.

### Missing Error Cases

**Bad**:
```
Create a tool that divides two numbers
```

**Good**:
```
Create a division tool:
- Input: numerator and denominator (both f64)
- Output: result
- Error: Division by zero should return validation error
- Edge cases: Handle infinity and NaN appropriately
```

### Ambiguous Requirements

**Bad**:
```
Create a search tool with good performance
```

**Good**:
```
Create a search tool that:
- Returns results in <100ms for queries under 10 chars
- Supports up to 10,000 items in the search index
- Uses case-insensitive matching
- Returns max 50 results, sorted by relevance
```

## Prompt Templates

### New Tool Template

```
Create a `[tool-name]` tool for [purpose].

Input:
- [field]: [type] ([required/optional]) - [description]
- ...

Output:
- [field]: [type] - [description]
- ...

Error cases:
- [condition] → [error type with message]
- ...

[Additional constraints or requirements]
```

### Tool Modification Template

```
Update the `[tool-name]` tool:

Current behavior: [what it does now]
Desired behavior: [what it should do]

Changes needed:
1. [Specific change]
2. [Specific change]

Preserve: [what should stay the same]
```

### Bug Fix Template

```
Fix issue in `[tool-name]`:

Steps to reproduce:
1. [Action]
2. [Action]

Expected: [result]
Actual: [result]

Additional context: [relevant details]
```

### Server Design Template

```
Create a [domain] MCP server.

Purpose: [what problem it solves]

Tools (list with brief descriptions):
1. [tool_name] - [purpose]
2. [tool_name] - [purpose]

Technical requirements:
- [Dependency/library to use]
- [Configuration approach]
- [Security consideration]

Quality requirements:
- [Coverage, testing, etc.]
```

## Summary

Effective prompting for MCP tools:

| Aspect | Approach |
|--------|----------|
| **Context** | Set domain and technology |
| **Capability** | Describe what, not how |
| **Constraints** | Set clear boundaries |
| **Error cases** | Enumerate explicitly |
| **Output** | Specify structure clearly |
| **Iteration** | Refine with focused requests |

The key is being specific enough that AI understands intent, while leaving implementation flexibility. Focus on:
- What the tool should accomplish
- What inputs it needs
- What outputs it produces
- What errors it handles
- What constraints apply

Let AI handle the Rust implementation details - it knows TypedTool patterns, JsonSchema derives, and error handling conventions.

---

*Continue to [Quality Assurance with AI](./ch16-03-qa.md) →*
