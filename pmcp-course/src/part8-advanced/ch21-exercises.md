# Chapter 21 Exercises

These exercises build your fluency with MCP Tasks. Each one targets a specific skill from the chapter.

## Exercise 1: Add TaskSupport::Optional to an Existing Tool

**Difficulty:** Introductory (15 min)

Take the calculator tool from Chapter 2 and add task support to it. This exercise is intentionally simple -- the goal is to practice the mechanical steps of wiring up task support, not to build something that genuinely needs it.

**Steps:**

1. Start with a basic `TypedTool` that adds two numbers
2. Add `.with_execution(ToolExecution::new().with_task_support(TaskSupport::Optional))` to the tool
3. Add `.task_store(Arc::new(InMemoryTaskStore::new()))` to the server builder
4. Build and run the server
5. Use `mcp-tester` to verify the tool's `execution.taskSupport` field appears in `tools/list`

**Verify your solution:**

```bash
# Run the server, then in another terminal:
mcp-tester stdio ./target/debug/your-server

# In the mcp-tester output, look for:
# tools/list response:
#   - name: "add"
#     execution:
#       taskSupport: "optional"
```

**Questions to answer:**
- What happens if you add `.with_execution()` to a tool but forget `.task_store()` on the builder? Does the server start? Does the tool still appear in `tools/list`?
- What changes in the `initialize` response capabilities when you add `.task_store()`?

---

## Exercise 2: Implement a Dual-Path Handler with is_task_request()

**Difficulty:** Intermediate (30 min)

Build a `generate_report` tool that generates a CSV report from hardcoded data. The tool should support both synchronous and task-based execution.

**Requirements:**

1. Define a `GenerateReportArgs` struct with fields:
   - `report_type`: String (e.g., "sales", "inventory")
   - `row_count`: u32 (number of rows to generate)
2. When `extra.is_task_request()` is `true`:
   - Create a task in the store
   - Spawn a background `tokio::spawn` that sleeps for `row_count * 10` milliseconds (simulating work)
   - Update the task to `Completed` when done
   - Return a `CreateTaskResult` JSON
3. When `extra.is_task_request()` is `false`:
   - Generate the report inline (same sleep for simulation)
   - Return a `CallToolResult` with the CSV as text content
4. Register the tool with `TaskSupport::Optional`

**Skeleton:**

```rust
#[derive(Deserialize, JsonSchema)]
struct GenerateReportArgs {
    report_type: String,
    row_count: u32,
}

fn build_report_tool(store: Arc<dyn TaskStore>) -> /* ... */ {
    TypedTool::new("generate_report", move |args: GenerateReportArgs, extra| {
        let store = store.clone();
        Box::pin(async move {
            if extra.is_task_request() {
                // TODO: Create task, spawn background work, return CreateTaskResult
                todo!()
            } else {
                // TODO: Generate report inline, return CallToolResult
                todo!()
            }
        })
    })
    .with_description("Generate a CSV report")
    .with_execution(ToolExecution::new().with_task_support(TaskSupport::Optional))
}
```

**Test your implementation:**

Write two unit tests:

```rust
#[tokio::test]
async fn test_sync_path_returns_content() {
    // Call the handler with extra.is_task_request() == false
    // Assert the response contains "content" with CSV text
}

#[tokio::test]
async fn test_task_path_returns_create_task_result() {
    // Call the handler with extra.with_task_request(Some(json!({})))
    // Assert the response contains "task" with "taskId" and "status": "working"
}
```

---

## Exercise 3: Build a get_task_result Fallback Tool

**Difficulty:** Intermediate (30 min)

Extend your solution from Exercise 2 by adding a `get_task_result` tool that any client can use to check on and retrieve task results.

**Requirements:**

1. Accept a `task_id` string argument
2. Call `store.get()` to retrieve the task
3. Return different content based on task status:
   - `Working`: "Task {id} is still running. Check again in {poll_interval/1000} seconds."
   - `Completed`: "Task {id} completed. Result: {status_message}"
   - `Failed`: "Task {id} failed: {status_message}" with `isError: true`
   - `Cancelled`: "Task {id} was cancelled." with `isError: true`
   - `InputRequired`: "Task {id} requires input." (for future use)
4. Handle the `NotFound` case with a clear error message
5. Register with `TaskSupport::Forbidden`

**Test your implementation:**

Write integration tests that exercise the full flow:

```rust
#[tokio::test]
async fn test_full_task_flow() {
    let store = Arc::new(InMemoryTaskStore::new());

    // 1. Call generate_report with task mode -> get task_id
    // 2. Call get_task_result with that task_id -> "still running"
    // 3. Wait for background work to complete
    // 4. Call get_task_result again -> completed result
}

#[tokio::test]
async fn test_get_task_result_not_found() {
    let store = Arc::new(InMemoryTaskStore::new());

    // Call get_task_result with a nonexistent task_id
    // Assert the error message is clear and actionable
}
```

---

## Exercise 4: Design Exercise -- Required, Optional, or Forbidden?

**Difficulty:** Design thinking (20 min)

For each tool below, decide which `TaskSupport` level is appropriate. Write your reasoning -- there is no single correct answer for some of these, but you should be able to justify your choice.

| # | Tool | Description | Your Choice | Your Reasoning |
|---|------|-------------|-------------|----------------|
| 1 | `get_user` | Look up a user by ID from a database | | |
| 2 | `generate_pdf_report` | Generate a 50-page compliance PDF from live data | | |
| 3 | `deploy_service` | Deploy a Docker container to production (2-5 min) | | |
| 4 | `validate_sql` | Parse and validate a SQL query (no execution) | | |
| 5 | `run_etl_pipeline` | Run a data pipeline that processes 1M rows (10-30 min) | | |
| 6 | `search_documents` | Full-text search across an index (50ms-5s) | | |
| 7 | `train_model` | Fine-tune an ML model (30 min - 4 hours) | | |
| 8 | `resize_image` | Resize an uploaded image (1-10 seconds) | | |

**Guiding questions for each:**

- What is the expected duration range?
- Is the result needed immediately for downstream tool calls?
- Would a sync timeout produce a confusing user experience?
- Does the operation have meaningful intermediate states worth polling?
- Could a Lambda deployment support this synchronously?

**Reference answers** (collapse after you have written yours):

<details>
<summary>Click to reveal reference answers</summary>

1. **get_user** -- `Forbidden`. Database lookups are fast. Task overhead adds latency for no benefit.

2. **generate_pdf_report** -- `Optional`. Usually takes 10-30 seconds. Might complete synchronously on fast data, but task mode is safer for large reports.

3. **deploy_service** -- `Required`. 2-5 minutes always exceeds typical timeouts. Sync path would always fail. Task mode is the only viable path.

4. **validate_sql** -- `Forbidden`. Pure parsing, no I/O, sub-millisecond. Adding task support would be misleading.

5. **run_etl_pipeline** -- `Required`. 10-30 minutes far exceeds any timeout. Task mode is mandatory.

6. **search_documents** -- `Forbidden`. Fast enough that task overhead is not justified. If your index is unusually slow, `Optional` could be defensible.

7. **train_model** -- `Required`. Hours of compute. No sync path is feasible.

8. **resize_image** -- `Optional` or `Forbidden`. 1-10 seconds is borderline. `Optional` if you want to be safe for large images. `Forbidden` if your images are always small.

</details>

## Prerequisites

Before starting these exercises, ensure you have:
- Completed Chapter 21 sections on lifecycle and capability negotiation
- A working Rust development environment with `pmcp` in your dependencies
- `mcp-tester` installed (`cargo install mcp-tester` or via `cargo-pmcp`)

## Next Steps

After completing these exercises, continue to:
- [Appendix A: cargo pmcp Reference](../appendix/cargo-pmcp-reference.md) -- CLI tooling reference
- [Appendix B: Template Gallery](../appendix/template-gallery.md) -- Production-ready templates including task-enabled servers
