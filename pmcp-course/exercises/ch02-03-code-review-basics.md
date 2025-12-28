# Exercise: Code Review - Common MCP Server Mistakes

::: exercise
id: ch02-03-code-review-basics
type: code_review
difficulty: beginner
time: 20 minutes
prerequisites: ch02-01-hello-mcp, ch02-02-calculator
:::

You've been asked to review a colleague's MCP server code before it goes
to production. The server is supposed to process user messages and return
responses, but something isn't quite right.

This exercise develops a crucial skill: **code review**. When working with
AI assistants, you'll often need to review generated code for issues. Even
when you write code yourself, a critical eye catches bugs before users do.

Your task: Find at least 5 issues in this code, categorize them by severity,
and suggest fixes.

::: objectives
thinking:
  - How to systematically review code for issues
  - Distinguishing bugs from style issues from security concerns
  - Why error handling patterns matter
doing:
  - Identify bugs, security issues, and anti-patterns
  - Categorize issues by severity
  - Propose concrete fixes
:::

::: discussion
- What's your usual approach when reviewing code?
- What categories of issues should you look for?
- How do you prioritize fixes?
:::

## Code to Review

Review this code and identify at least 5 issues:

```rust
//! Message Processor MCP Server
//!
//! Processes user messages and returns responses.

use pmcp::{Server, ServerCapabilities, ToolCapabilities};
use pmcp::server::TypedTool;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use std::sync::Mutex;

// Global message counter
static MESSAGE_COUNT: Mutex<i32> = Mutex::new(0);

#[derive(Deserialize, JsonSchema)]
struct ProcessMessageInput {
    message: String,
    user_id: String,
}

#[derive(Serialize)]
struct MessageResponse {
    processed: String,
    message_number: i32,
}

fn process_message(input: ProcessMessageInput) -> MessageResponse {
    // Increment counter
    let count = MESSAGE_COUNT.lock().unwrap();
    *count += 1;

    // Process the message
    let processed = if input.message.len() > 0 {
        format!("User {} said: {}", input.user_id, input.message)
    } else {
        "Empty message received".to_string()
    };

    // Log for debugging
    println!("Processing message #{}: {}", count, input.message);

    MessageResponse {
        processed,
        message_number: *count,
    }
}

#[tokio::main]
async fn main() {
    let server = Server::builder()
        .name("message-processor")
        .version("0.1")
        .capabilities(ServerCapabilities {
            tools: Some(ToolCapabilities::default()),
            ..Default::default()
        })
        .tool("process", TypedTool::new("process", |input: ProcessMessageInput| {
            Box::pin(async {
                let response = process_message(input);
                Ok(serde_json::to_value(response).unwrap())
            })
        }))
        .build()
        .unwrap();

    // Server is ready but never started...
}
```

## Review Categories

Categorize issues by:
- **Bugs / Correctness** - Will cause incorrect behavior
- **Error Handling** - Missing or improper error handling
- **Concurrency Issues** - Problems with shared state
- **Security Concerns** - Potential vulnerabilities
- **Code Quality** - Style and best practices

## Severity Guide

- **Critical**: Will cause crashes, data loss, or security vulnerabilities
- **High**: Will cause incorrect behavior or poor user experience
- **Medium**: Could cause issues under certain conditions
- **Low**: Style issues, missing best practices

::: hint level=1 title="Where to look"
Focus on these areas:
1. How is the mutex being used?
2. What happens with all those `.unwrap()` calls?
3. Does the server actually run?
4. What gets logged?
:::

::: hint level=2 title="Critical issues"
The most critical issues:
- The mutex lock usage has a problem with mutable access
- The server is built but never started with a transport
- Multiple `.unwrap()` calls can panic
:::

::: hint level=3 title="Full list"
Issues to find:
1. **Critical**: Mutex borrow issue - needs `mut` for `*count += 1`
2. **High**: `.lock().unwrap()` panics if mutex poisoned
3. **High**: Server never starts (no transport)
4. **High**: Multiple `.unwrap()` calls can panic
5. **Medium**: Global mutable state hurts testing/scaling
6. **Medium**: Raw user input logged (security)
7. **Low**: `.len() > 0` should be `!.is_empty()`
8. **Low**: Version "0.1" should be "0.1.0" for semver
9. **Low**: main() should return Result
:::

::: solution
### Issues Found

**1. Critical - Mutex Borrow Issue**
```rust
let count = MESSAGE_COUNT.lock().unwrap();
*count += 1;  // Error: count is not mutable!
```
Fix: `let mut count = MESSAGE_COUNT.lock().unwrap();`

**2. High - Panic on Poisoned Mutex**
```rust
.lock().unwrap()  // Panics if another thread panicked
```
Fix: Handle PoisonError or use `lock().unwrap_or_else(|e| e.into_inner())`

**3. High - Server Never Starts**
```rust
// Server is ready but never started...
```
Fix: Add `server.run_stdio().await?;` or HTTP transport

**4. High - Unwrap on Serialization**
```rust
Ok(serde_json::to_value(response).unwrap())
```
Fix: Use `?` operator: `Ok(serde_json::to_value(response)?)`

**5. Medium - Global Mutable State**
```rust
static MESSAGE_COUNT: Mutex<i32> = Mutex::new(0);
```
Fix: Use per-request or per-connection state, or Arc<Mutex<>> passed to handlers

**6. Medium - Logging User Input**
```rust
println!("Processing message #{}: {}", count, input.message);
```
Fix: Use structured logging (tracing), sanitize/truncate input

**7. Low - Non-idiomatic Empty Check**
```rust
if input.message.len() > 0
```
Fix: `if !input.message.is_empty()`

**8. Low - Semver Version Format**
```rust
.version("0.1")
```
Fix: `.version("0.1.0")`

**9. Low - main() Return Type**
```rust
async fn main() {
```
Fix: `async fn main() -> Result<(), Box<dyn std::error::Error>>`
:::

::: reflection
- What's your process for reviewing unfamiliar code?
- How do you prioritize which issues to fix first?
- How would you give feedback to the author without being discouraging?
- What tools could help catch some of these issues automatically?
:::
