# Exercise: Code Review Challenge

::: exercise
id: ch02-03-code-review-basics
difficulty: beginner
time: 20 minutes
prerequisites: [ch02-01-hello-mcp, ch02-02-calculator]
:::

You've been asked to review a colleague's MCP server code before it goes to production. Find at least **5 issues**, categorize them by severity, and suggest fixes.

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
- What's the first thing you look for when reviewing code?
- How do you distinguish between critical bugs and style preferences?
- What automated tools can help catch issues before review?
:::

## Code to Review

**Severity Guide:**
- **Critical**: Crashes, data loss, or security vulnerabilities
- **High**: Incorrect behavior or poor user experience
- **Medium**: Issues under certain conditions
- **Low**: Style issues, missing best practices

```rust
//! Message Processor MCP Server

use pmcp::{Server, ServerCapabilities, ToolCapabilities};
use pmcp::server::TypedTool;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use std::sync::Mutex;

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
    let count = MESSAGE_COUNT.lock().unwrap();
    *count += 1;

    let processed = if input.message.len() > 0 {
        format!("User {} said: {}", input.user_id, input.message)
    } else {
        "Empty message received".to_string()
    };

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

::: hint level=1 title="Look for Error Handling"
Count how many `.unwrap()` calls there are. What happens if any of them fail?
:::

::: hint level=2 title="Check the Mutex Usage"
Look at line 27. The lock is taken, but can you modify what's inside? What happens to the lock when the function ends?
:::

::: hint level=3 title="Key Issues"
1. **Mutex deadlock** - Lock isn't released properly, and `*count += 1` on an immutable borrow won't compile
2. **Multiple unwrap()** - Can panic on poisoned mutex, failed serialization, or failed build
3. **Server never runs** - Built but no transport started
4. **Global mutable state** - Makes testing difficult
5. **Unsanitized logging** - User input logged directly
6. **Style issues** - `.len() > 0` vs `.is_empty()`, version "0.1" vs "0.1.0"
:::

::: solution
```rust
use pmcp::{Server, ServerCapabilities, ToolCapabilities};
use pmcp::server::TypedTool;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use std::sync::atomic::{AtomicI32, Ordering};
use anyhow::Result;

static MESSAGE_COUNT: AtomicI32 = AtomicI32::new(0);

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

fn process_message(input: &ProcessMessageInput) -> MessageResponse {
    let count = MESSAGE_COUNT.fetch_add(1, Ordering::SeqCst) + 1;

    let processed = if !input.message.is_empty() {
        format!("User {} said: {}", input.user_id, input.message)
    } else {
        "Empty message received".to_string()
    };

    let log_msg: String = input.message.chars().take(50).collect();
    tracing::info!(message_number = count, "Processing: {}...", log_msg);

    MessageResponse { processed, message_number: count }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().init();

    let server = Server::builder()
        .name("message-processor")
        .version("0.1.0")
        .capabilities(ServerCapabilities {
            tools: Some(ToolCapabilities::default()),
            ..Default::default()
        })
        .tool("process", TypedTool::new("process", |input: ProcessMessageInput| {
            Box::pin(async move {
                let response = process_message(&input);
                Ok(serde_json::to_value(response)?)
            })
        }))
        .build()?;

    server_common::serve_http(server, "0.0.0.0:3000").await
}
```

### Fixes Applied

1. **AtomicI32** instead of Mutex for simple counter
2. **`?` operator** instead of `.unwrap()`
3. **`main() -> Result<()>`** to enable error propagation
4. **Server actually runs** with transport
5. **Truncated logging** to prevent log injection
6. **Idiomatic Rust** - `!is_empty()`, "0.1.0"
:::

::: reflection
- What's your process for reviewing unfamiliar code?
- How do you prioritize which issues to fix first?
- What tools could help catch some of these issues automatically?
:::
