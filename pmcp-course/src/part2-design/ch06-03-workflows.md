# Designing Multi-Step Workflows

Complex tasks require multiple steps. Designing workflows that AI can reliably execute requires understanding how AI clients reason through multi-step processes—and where they commonly fail.

## The Multi-Step Challenge

Consider a "customer analysis" workflow:

```
User: "Analyze our top customer's recent behavior"

Required steps:
1. Identify the top customer by revenue
2. Get their order history
3. Get their support ticket history
4. Get their engagement metrics
5. Synthesize findings into actionable insights
```

Without guidance, an AI might:
- Call tools in the wrong order
- Miss intermediate steps
- Forget to pass IDs between steps
- Get confused about what data to use where

## Workflow Design Patterns

### Pattern 1: The Guided Prompt

Design prompts that explicitly guide multi-step execution:

```rust
Prompt::new("customer-analysis")
    .description("Comprehensive customer analysis workflow")
    .arguments(vec![
        PromptArgument::new("customer_id")
            .description("Customer ID, or 'top' for highest revenue customer")
    ])
    .messages(vec![
        PromptMessage::user(
            "Perform a comprehensive analysis of {{customer_id}}:\n\n\
            **Step 1: Identify Customer**\n\
            {{#if customer_id == 'top'}}\n\
            - Use `sales_top_customers` with limit=1 to find the top customer\n\
            - Note the customer_id for subsequent steps\n\
            {{else}}\n\
            - Use customer_id: {{customer_id}}\n\
            {{/if}}\n\n\
            **Step 2: Gather Data** (can run in parallel)\n\
            - Use `order_history` with the customer_id\n\
            - Use `support_tickets` with the customer_id\n\
            - Use `engagement_metrics` with the customer_id\n\n\
            **Step 3: Analyze Patterns**\n\
            - Calculate order frequency trend (increasing/decreasing?)\n\
            - Identify support ticket patterns (recurring issues?)\n\
            - Correlate engagement with purchase behavior\n\n\
            **Step 4: Generate Report**\n\
            Format findings as:\n\
            ```\n\
            ## Customer Analysis: [Name]\n\
            ### Key Metrics\n\
            - Total Revenue: $X\n\
            - Order Frequency: X orders/month\n\
            - Support Health: Good/At-Risk/Poor\n\n\
            ### Trends\n\
            1. [Trend 1]\n\
            2. [Trend 2]\n\n\
            ### Recommendations\n\
            1. [Action 1]\n\
            2. [Action 2]\n\
            ```"
        )
    ])
```

### Pattern 2: The Orchestration Tool

Create a tool that coordinates multiple operations:

```rust
Tool::new("customer_analysis")
    .description(
        "Comprehensive customer analysis. \
        Automatically gathers order history, support tickets, and engagement metrics, \
        then synthesizes findings. Returns structured analysis report."
    )
    .input_schema(json!({
        "type": "object",
        "required": ["customer_id"],
        "properties": {
            "customer_id": {
                "type": "string",
                "description": "Customer ID to analyze"
            },
            "include_recommendations": {
                "type": "boolean",
                "default": true
            }
        }
    }))
    .output_schema(json!({
        "type": "object",
        "properties": {
            "customer": { /* customer info */ },
            "orders": { /* order analysis */ },
            "support": { /* support analysis */ },
            "engagement": { /* engagement analysis */ },
            "recommendations": { /* actionable items */ }
        }
    }))
```

The orchestration happens inside the tool—the AI just calls once.

### Pattern 3: The State Machine

For complex workflows with branching, define explicit states:

```rust
Tool::new("workflow_execute")
    .description("Execute a defined workflow step by step")
    .input_schema(json!({
        "type": "object",
        "properties": {
            "workflow": {
                "type": "string",
                "enum": ["customer_onboarding", "order_fulfillment", "issue_escalation"]
            },
            "state": {
                "type": "string",
                "description": "Current workflow state (omit to start)"
            },
            "input": {
                "type": "object",
                "description": "Input for current state"
            }
        }
    }))
    .output_schema(json!({
        "type": "object",
        "properties": {
            "state": { "type": "string" },
            "completed": { "type": "boolean" },
            "next_actions": {
                "type": "array",
                "items": { "type": "string" }
            },
            "result": { "type": "object" }
        }
    }))
```

The AI receives explicit guidance on what to do next.

## Workflow Building Blocks

### Passing IDs Between Steps

Design tools to clearly pass identifiers:

```rust
// Step 1: Returns IDs
Tool::new("find_candidates")
    .output_schema(json!({
        "properties": {
            "candidates": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "string",
                            "description": "Use with process_candidate tool"
                        }
                    }
                }
            }
        }
    }))

// Step 2: Accepts IDs from Step 1
Tool::new("process_candidate")
    .input_schema(json!({
        "properties": {
            "candidate_id": {
                "type": "string",
                "description": "ID from find_candidates output"
            }
        }
    }))
```

Explicit documentation helps the AI connect the steps.

### Parallel vs Sequential

Indicate when steps can run in parallel:

```rust
Prompt::new("data-gathering")
    .messages(vec![
        PromptMessage::user(
            "Gather customer data:\n\n\
            **These steps can run in parallel:**\n\
            - order_history(customer_id)\n\
            - support_tickets(customer_id)\n\
            - engagement_metrics(customer_id)\n\n\
            **This step requires all above to complete:**\n\
            - Synthesize findings into report"
        )
    ])
```

### Checkpoints and Rollback

For critical workflows, design checkpoint support:

```rust
Tool::new("workflow_checkpoint")
    .description("Save workflow state for potential rollback")
    .input_schema(json!({
        "properties": {
            "workflow_id": { "type": "string" },
            "checkpoint_name": { "type": "string" },
            "state": { "type": "object" }
        }
    }))

Tool::new("workflow_rollback")
    .description("Rollback workflow to a checkpoint")
    .input_schema(json!({
        "properties": {
            "workflow_id": { "type": "string" },
            "checkpoint_name": { "type": "string" }
        }
    }))
```

## Error Handling in Workflows

### Explicit Error Paths

Design for failure scenarios:

```rust
Prompt::new("safe-data-migration")
    .messages(vec![
        PromptMessage::user(
            "Migrate customer data to new format:\n\n\
            **Step 1: Validate Source**\n\
            - Use validate_source to check data integrity\n\
            - If validation fails: STOP and report issues\n\n\
            **Step 2: Create Backup**\n\
            - Use create_backup before any modifications\n\
            - Record backup_id for potential rollback\n\n\
            **Step 3: Transform Data**\n\
            - Use transform_records in batches of 100\n\
            - If any batch fails: rollback using backup_id\n\n\
            **Step 4: Verify Results**\n\
            - Use validate_target to verify migration\n\
            - If validation fails: rollback using backup_id\n\n\
            **On Success:**\n\
            - Report migration statistics\n\
            - Keep backup for 24 hours"
        )
    ])
```

### Retry Guidance

Tell the AI how to handle failures:

```rust
Tool::new("external_api_call")
    .description(
        "Call external API. \
        If rate limited (error code RATE_LIMITED), wait retry_after_seconds and retry. \
        If timeout (error code TIMEOUT), retry up to 3 times with exponential backoff. \
        If permission denied (error code PERMISSION_DENIED), do not retry."
    )
```

## Workflow Visibility

### Progress Reporting

Design for visibility into long workflows:

```rust
// Progress update tool
Tool::new("workflow_progress")
    .description("Report workflow progress to user")
    .input_schema(json!({
        "properties": {
            "workflow": { "type": "string" },
            "step": { "type": "string" },
            "progress_percent": { "type": "integer" },
            "message": { "type": "string" }
        }
    }))

// Prompt includes progress reporting
Prompt::new("long-running-analysis")
    .messages(vec![
        PromptMessage::user(
            "Analyze all customers:\n\n\
            For each batch of 10 customers:\n\
            1. Process the batch\n\
            2. Call workflow_progress with completion percentage\n\
            3. Continue to next batch\n\n\
            Report progress every 10% completion."
        )
    ])
```

### Audit Trail

For compliance, design audit capability:

```rust
Tool::new("workflow_log")
    .description("Log workflow action for audit trail")
    .input_schema(json!({
        "properties": {
            "workflow_id": { "type": "string" },
            "action": { "type": "string" },
            "actor": { "type": "string" },
            "details": { "type": "object" },
            "timestamp": { "type": "string", "format": "date-time" }
        }
    }))
```

## Workflow Templates

### The CRUD Workflow

Standard create-read-update-delete pattern:

```rust
// List → Select → View/Edit → Confirm
Prompt::new("entity-management")
    .arguments(vec![
        PromptArgument::new("entity_type")
            .description("Type: customer, order, product")
    ])
    .messages(vec![
        PromptMessage::user(
            "Manage {{entity_type}} entities:\n\n\
            1. Ask what the user wants to do (list, view, create, edit, delete)\n\
            2. For list: use {{entity_type}}_list with any filters\n\
            3. For view: use {{entity_type}}_get with ID\n\
            4. For create: collect required fields, then use {{entity_type}}_create\n\
            5. For edit: get current state, show changes, confirm, then use {{entity_type}}_update\n\
            6. For delete: confirm explicitly, then use {{entity_type}}_delete"
        )
    ])
```

### The Investigation Workflow

Drill-down analysis pattern:

```rust
Prompt::new("investigate-anomaly")
    .messages(vec![
        PromptMessage::user(
            "Investigate the reported anomaly:\n\n\
            **Phase 1: Scope**\n\
            - Identify affected time range\n\
            - Identify affected entities\n\
            - Quantify the impact\n\n\
            **Phase 2: Correlate**\n\
            - Check for system events in the time range\n\
            - Check for related anomalies\n\
            - Identify potential causes\n\n\
            **Phase 3: Diagnose**\n\
            - For each potential cause:\n\
              - Gather supporting evidence\n\
              - Gather contradicting evidence\n\
            - Rank causes by likelihood\n\n\
            **Phase 4: Recommend**\n\
            - Immediate actions to mitigate\n\
            - Long-term fixes to prevent recurrence\n\
            - Monitoring to detect future occurrences"
        )
    ])
```

### The Approval Workflow

Human-in-the-loop pattern:

```rust
Prompt::new("require-approval")
    .messages(vec![
        PromptMessage::user(
            "This operation requires approval:\n\n\
            **Before Proceeding:**\n\
            1. Summarize what will happen\n\
            2. List all affected entities\n\
            3. Describe the impact (reversible/irreversible)\n\
            4. Ask for explicit confirmation: 'yes' to proceed\n\n\
            **On Confirmation:**\n\
            - Log the approval with timestamp\n\
            - Execute the operation\n\
            - Report results\n\n\
            **On Denial:**\n\
            - Log the denial\n\
            - Suggest alternative approaches if applicable"
        )
    ])
```

## Summary

Effective multi-step workflows require:

| Design Element | Purpose |
|----------------|---------|
| **Explicit steps** | AI knows the sequence |
| **Clear ID passing** | AI connects outputs to inputs |
| **Parallel indicators** | AI optimizes execution |
| **Error handling** | AI knows what to do on failure |
| **Progress reporting** | Users see what's happening |
| **Approval points** | Humans stay in control |

The key insight: AI clients don't automatically know how to orchestrate multi-step processes. Your design must guide them through the workflow explicitly, handling both the happy path and failure scenarios.
