# Orchestration Patterns

Orchestration enables complex workflows that span multiple domains. When a task requires coordination across HR, Finance, and Engineering (like employee onboarding), orchestration servers tie everything together.

## When to Use Orchestration

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Orchestration vs Direct Calls                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Direct AI-to-Tools (without orchestration):                            │
│  ═══════════════════════════════════════════                            │
│                                                                         │
│  AI Client                                                              │
│      │                                                                  │
│      ├─▶ HR Server: create_employee() ─────────────────▶ Step 1        │
│      │                                                                  │
│      ├─▶ Finance Server: create_payroll_account() ─────▶ Step 2        │
│      │                                                                  │
│      ├─▶ Engineering Server: create_github_access() ───▶ Step 3        │
│      │                                                                  │
│      └─▶ IT Server: provision_laptop() ────────────────▶ Step 4        │
│                                                                         │
│  Problems:                                                              │
│  • AI must know correct order                                           │
│  • No rollback if step 3 fails                                          │
│  • Multiple round trips (slow)                                          │
│  • AI might skip steps or call in wrong order                           │
│                                                                         │
│  ─────────────────────────────────────────────────────────────────────  │
│                                                                         │
│  With Orchestration:                                                    │
│  ════════════════════                                                   │
│                                                                         │
│  AI Client                                                              │
│      │                                                                  │
│      └─▶ Orchestration Server: onboard_employee()                       │
│              │                                                          │
│              ├─▶ HR Server: create_employee()                           │
│              │       │                                                  │
│              │       └─▶ Store employee_id for later steps              │
│              │                                                          │
│              ├─▶ Finance Server: create_payroll_account()               │
│              │       │                                                  │
│              │       └─▶ Uses employee_id from step 1                   │
│              │                                                          │
│              ├─▶ Engineering Server: create_github_access()             │
│              │                                                          │
│              └─▶ IT Server: provision_laptop()                          │
│                                                                         │
│  Benefits:                                                              │
│  ✓ Single tool call for AI                                              │
│  ✓ Guaranteed execution order                                           │
│  ✓ Data flows between steps automatically                               │
│  ✓ Single round trip                                                    │
│  ✓ Deterministic, testable                                              │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## PMCP Workflow System

PMCP provides a workflow system for building multi-step orchestrations with automatic data binding between steps.

### Basic Workflow Structure

```rust
use pmcp::server::workflow::{
    dsl::{constant, field, from_step, prompt_arg},
    InternalPromptMessage, SequentialWorkflow, ToolHandle, WorkflowStep,
};
use serde_json::json;

/// Create an employee onboarding workflow
fn create_onboarding_workflow() -> SequentialWorkflow {
    SequentialWorkflow::new(
        "onboard_employee",
        "Complete employee onboarding across all systems",
    )
    // Define required inputs
    .argument("employee_name", "Full name of the employee", true)
    .argument("department", "Department to join", true)
    .argument("role", "Job role/title", true)
    .argument("manager_id", "ID of the reporting manager", true)
    .argument("start_date", "Start date (YYYY-MM-DD)", true)

    // Add system instructions for the AI
    .instruction(InternalPromptMessage::system(
        "Execute employee onboarding workflow. All steps are mandatory."
    ))

    // Step 1: Create employee record in HR system
    .step(
        WorkflowStep::new("create_employee", ToolHandle::new("hr_create_employee"))
            .arg("name", prompt_arg("employee_name"))
            .arg("department", prompt_arg("department"))
            .arg("role", prompt_arg("role"))
            .arg("manager_id", prompt_arg("manager_id"))
            .arg("start_date", prompt_arg("start_date"))
            .bind("employee_record")  // Store output for later steps
    )

    // Step 2: Create payroll account using employee_id from step 1
    .step(
        WorkflowStep::new("setup_payroll", ToolHandle::new("finance_create_payroll"))
            .arg("employee_id", field("employee_record", "employee_id"))  // Extract from step 1
            .arg("department", prompt_arg("department"))
            .bind("payroll_record")
    )

    // Step 3: Create GitHub access
    .step(
        WorkflowStep::new("github_access", ToolHandle::new("eng_create_github_user"))
            .arg("employee_id", field("employee_record", "employee_id"))
            .arg("email", field("employee_record", "email"))
            .arg("team", prompt_arg("department"))
            .bind("github_record")
    )

    // Step 4: Provision laptop
    .step(
        WorkflowStep::new("provision_laptop", ToolHandle::new("it_provision_laptop"))
            .arg("employee_id", field("employee_record", "employee_id"))
            .arg("department", prompt_arg("department"))
            .arg("start_date", prompt_arg("start_date"))
            .bind("laptop_record")
    )

    // Step 5: Send welcome email with all account info
    .step(
        WorkflowStep::new("send_welcome", ToolHandle::new("comms_send_email"))
            .arg("to", field("employee_record", "email"))
            .arg("template", constant(json!("welcome_employee")))
            .arg("employee_name", prompt_arg("employee_name"))
            .arg("github_username", field("github_record", "username"))
            .arg("laptop_tracking", field("laptop_record", "tracking_number"))
            .bind("email_result")
    )
}
```

### DSL Helpers

The workflow DSL provides helpers for binding data between steps:

| Helper | Purpose | Example |
|--------|---------|---------|
| `prompt_arg("name")` | Reference workflow input argument | `arg("email", prompt_arg("employee_email"))` |
| `from_step("binding")` | Reference entire output of a step | `arg("data", from_step("employee_record"))` |
| `field("binding", "field")` | Extract specific field from step output | `arg("id", field("employee_record", "employee_id"))` |
| `constant(value)` | Provide a constant value | `arg("template", constant(json!("welcome")))` |

### Server-Side Execution

Workflows execute **server-side**, not client-side. When a client calls `prompts/get`, the server:

1. Receives the request with workflow name and arguments
2. Executes each step sequentially
3. Passes data between steps via bindings
4. Returns a conversation trace showing all tool calls and results

```rust
use pmcp::{Result, Server};

/// Create orchestration server with workflows
fn create_orchestration_server() -> Result<Server> {
    Server::builder()
        .name("orchestration-server")
        .version("1.0.0")
        // Register the tools that workflows use
        .tool_typed("hr_create_employee", hr_create_employee_handler)
        .tool_typed("finance_create_payroll", finance_create_payroll_handler)
        .tool_typed("eng_create_github_user", eng_create_github_handler)
        .tool_typed("it_provision_laptop", it_provision_laptop_handler)
        .tool_typed("comms_send_email", comms_send_email_handler)
        // Register workflows as prompts
        .prompt_workflow(create_onboarding_workflow())?
        .prompt_workflow(create_offboarding_workflow())?
        .build()
}
```

### Execution Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Workflow Execution Flow                              │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Client Request:                                                        │
│  ═══════════════                                                        │
│  {                                                                      │
│    "method": "prompts/get",                                             │
│    "params": {                                                          │
│      "name": "onboard_employee",                                        │
│      "arguments": {                                                     │
│        "employee_name": "Alice Smith",                                  │
│        "department": "engineering",                                     │
│        "role": "Software Engineer",                                     │
│        "manager_id": "mgr-123",                                         │
│        "start_date": "2024-02-01"                                       │
│      }                                                                  │
│    }                                                                    │
│  }                                                                      │
│                                                                         │
│  ─────────────────────────────────────────────────────────────────────  │
│                                                                         │
│  Server-Side Execution:                                                 │
│  ══════════════════════                                                 │
│                                                                         │
│  Step 1: hr_create_employee                                             │
│          Input: {name: "Alice Smith", department: "engineering", ...}   │
│          Output: {employee_id: "emp-456", email: "alice@company.com"}   │
│          → Stored as "employee_record"                                  │
│                                                                         │
│  Step 2: finance_create_payroll                                         │
│          Input: {employee_id: "emp-456", department: "engineering"}     │
│          Output: {payroll_id: "pay-789", status: "active"}              │
│          → Stored as "payroll_record"                                   │
│                                                                         │
│  Step 3: eng_create_github_user                                         │
│          Input: {employee_id: "emp-456", email: "alice@company.com"}    │
│          Output: {username: "asmith", access_level: "developer"}        │
│          → Stored as "github_record"                                    │
│                                                                         │
│  Step 4: it_provision_laptop                                            │
│          Input: {employee_id: "emp-456", start_date: "2024-02-01"}      │
│          Output: {tracking_number: "FX123456", eta: "2024-01-30"}       │
│          → Stored as "laptop_record"                                    │
│                                                                         │
│  Step 5: comms_send_email                                               │
│          Input: {to: "alice@company.com", github: "asmith", ...}        │
│          Output: {sent: true, message_id: "msg-abc"}                    │
│          → Stored as "email_result"                                     │
│                                                                         │
│  ─────────────────────────────────────────────────────────────────────  │
│                                                                         │
│  Server Response (conversation trace):                                  │
│  ═══════════════════════════════════                                    │
│                                                                         │
│  [                                                                      │
│    {role: "user", content: "Onboard Alice Smith to engineering..."},    │
│    {role: "assistant", content: "Executing 5-step onboarding..."},      │
│    {role: "assistant", content: "Calling hr_create_employee..."},       │
│    {role: "user", content: "Tool result: {employee_id: 'emp-456'...}"}, │
│    ... (more messages for each step)                                    │
│  ]                                                                      │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## Real-World Workflow Example

Here's a complete code review workflow from `examples/53_typed_tools_workflow_integration.rs`:

```rust
use pmcp::server::workflow::dsl::*;
use pmcp::server::workflow::{SequentialWorkflow, ToolHandle, WorkflowStep};
use pmcp::{Result, Server};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// ============================================================================
// Tool Definitions
// ============================================================================

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct AnalyzeCodeInput {
    code: String,
    #[serde(default = "default_language")]
    language: String,
    #[serde(default = "default_depth")]
    depth: u8,
}

fn default_language() -> String { "rust".to_string() }
fn default_depth() -> u8 { 2 }

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct ReviewCodeInput {
    analysis: String,
    focus: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct FormatCodeInput {
    code: String,
    issues: Vec<String>,
}

// Tool implementations
async fn analyze_code(input: AnalyzeCodeInput, _extra: RequestHandlerExtra) -> Result<Value> {
    Ok(json!({
        "language": input.language,
        "depth": input.depth,
        "lines_of_code": input.code.lines().count(),
        "issues_found": 3,
        "complexity_score": 7.5,
        "analysis_summary": format!(
            "Analyzed {} lines of {} code. Found 3 potential issues.",
            input.code.lines().count(),
            input.language
        ),
        "issue_details": [
            "Function has high cyclomatic complexity",
            "Missing error handling",
            "Consider using Result<T> instead of panicking"
        ]
    }))
}

async fn review_code(input: ReviewCodeInput, _extra: RequestHandlerExtra) -> Result<Value> {
    Ok(json!({
        "review_summary": format!("Reviewed with focus on: {}", input.focus.join(", ")),
        "recommendations": [
            "Refactor complex functions into smaller units",
            "Add comprehensive error handling",
            "Improve inline documentation",
            "Add unit tests for edge cases"
        ],
        "priority_issues": input.focus,
        "approval_status": "conditional"
    }))
}

async fn format_code(input: FormatCodeInput, _extra: RequestHandlerExtra) -> Result<Value> {
    let annotations = input.issues
        .iter()
        .enumerate()
        .map(|(i, issue)| format!("// TODO (Issue {}): {}", i + 1, issue))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(json!({
        "formatted_code": format!("{}\n\n{}", annotations, input.code),
        "changes_made": "Added TODO comments for identified issues",
        "issues_annotated": input.issues.len()
    }))
}

// ============================================================================
// Workflow Definition
// ============================================================================

fn create_code_review_workflow() -> SequentialWorkflow {
    SequentialWorkflow::new(
        "code_review_workflow",
        "Comprehensive code review with analysis and formatting",
    )
    .argument("code", "Source code to review", true)
    .argument("language", "Programming language (default: rust)", false)

    // Step 1: Analyze code
    .step(
        WorkflowStep::new("analyze", ToolHandle::new("analyze_code"))
            .arg("code", prompt_arg("code"))
            .arg("language", prompt_arg("language"))
            .arg("depth", constant(json!(2)))
            .bind("analysis_result")
    )

    // Step 2: Review code (uses analysis from step 1)
    .step(
        WorkflowStep::new("review", ToolHandle::new("review_code"))
            .arg("analysis", field("analysis_result", "analysis_summary"))
            .arg("focus", constant(json!(["security", "performance", "maintainability"])))
            .bind("review_result")
    )

    // Step 3: Format code (uses review from step 2)
    .step(
        WorkflowStep::new("format", ToolHandle::new("format_code"))
            .arg("code", prompt_arg("code"))
            .arg("issues", field("review_result", "recommendations"))
            .bind("formatted_result")
    )
}

// ============================================================================
// Server Setup
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    let server = Server::builder()
        .name("code-review-server")
        .version("1.0.0")
        // Register typed tools
        .tool_typed("analyze_code", analyze_code)
        .tool_typed("review_code", review_code)
        .tool_typed("format_code", format_code)
        // Register workflow
        .prompt_workflow(create_code_review_workflow())?
        .build()?;

    println!("Code review server ready!");
    println!("Workflow 'code_review_workflow' executes 3 tools server-side");

    Ok(())
}
```

## Workflow Validation

Workflows are validated at registration time:

```rust
fn create_workflow() -> SequentialWorkflow {
    let workflow = SequentialWorkflow::new("my_workflow", "Description")
        .argument("input", "Required input", true)
        .step(
            WorkflowStep::new("step1", ToolHandle::new("tool1"))
                .arg("data", prompt_arg("input"))
                .bind("result1")
        )
        .step(
            WorkflowStep::new("step2", ToolHandle::new("tool2"))
                .arg("prev", field("result1", "output"))  // References step1 output
                .bind("result2")
        );

    // Validate before registering
    workflow.validate().expect("Workflow should be valid");

    workflow
}
```

### Validation Checks

| Check | Error Example |
|-------|---------------|
| **Undefined binding** | `field("nonexistent", "field")` - binding doesn't exist |
| **Missing argument** | `prompt_arg("missing")` - argument not declared |
| **Duplicate binding** | Two steps with same `.bind("name")` |
| **Empty workflow** | No steps defined |

## Error Handling in Workflows

If a step fails, the workflow stops and returns the error:

```rust
// Step that might fail
.step(
    WorkflowStep::new("risky_operation", ToolHandle::new("external_api"))
        .arg("data", from_step("previous_result"))
        .bind("api_result")
        // If external_api fails, workflow stops here
        // Client receives error with context about which step failed
)
```

For advanced error handling, implement retry logic in the tool itself:

```rust
async fn external_api_with_retry(input: ApiInput, _extra: RequestHandlerExtra) -> Result<Value> {
    let mut attempts = 0;
    let max_attempts = 3;

    loop {
        attempts += 1;
        match call_external_api(&input).await {
            Ok(result) => return Ok(result),
            Err(e) if attempts < max_attempts => {
                tracing::warn!(attempt = attempts, error = %e, "Retrying...");
                tokio::time::sleep(Duration::from_secs(2_u64.pow(attempts))).await;
            }
            Err(e) => return Err(e),
        }
    }
}
```

## When NOT to Use Orchestration

Orchestration adds complexity. Avoid it when:

| Scenario | Better Approach |
|----------|-----------------|
| Single tool call | Direct tool call |
| Steps are independent | Parallel direct calls |
| AI needs to make decisions | Let AI orchestrate |
| Dynamic step order | AI-driven workflow |
| User interaction between steps | Multiple client requests |

## Summary

| Concept | Purpose |
|---------|---------|
| **SequentialWorkflow** | Define multi-step workflows |
| **WorkflowStep** | Individual step with tool and arguments |
| **bind()** | Store step output for later steps |
| **prompt_arg()** | Reference workflow input |
| **field()** | Extract field from previous step output |
| **from_step()** | Reference entire step output |
| **Server-side execution** | Single request, deterministic execution |

Orchestration is powerful for complex, multi-domain workflows. Use it when you need guaranteed execution order, data flow between steps, and single-request completion of multi-step processes.

---

*Return to [Server Composition](./ch19-composition.md) | Return to [Course Home](../SUMMARY.md)*
