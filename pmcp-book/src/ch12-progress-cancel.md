# Progress Reporting and Cancellation

Long-running operations require two critical capabilities: **progress tracking** so users know what's happening, and **cancellation** so users can stop operations that are taking too long or are no longer needed.

This chapter covers the PMCP SDK's comprehensive support for both features, following the MCP protocol specifications for progress notifications and request cancellation.

## Overview

### Why Progress Matters

When a tool processes large datasets, downloads files, or performs complex calculations, users need feedback:

- **Visibility**: "Is it still working or stuck?"
- **Time estimation**: "How long until it's done?"
- **Responsiveness**: "Should I wait or cancel?"

Without progress updates, long operations feel like black boxes.

### Why Cancellation Matters

Users should be able to interrupt operations that:

- Are taking longer than expected
- Were started by mistake
- Are no longer needed (user changed their mind)
- Are consuming too many resources

Proper cancellation prevents wasted work and improves user experience.

## Progress Reporting

The PMCP SDK provides a trait-based progress reporting system with automatic rate limiting and validation.

### The ProgressReporter Trait

```rust
use async_trait::async_trait;
use pmcp::error::Result;

#[async_trait]
pub trait ProgressReporter: Send + Sync {
    /// Report progress with optional total and message
    async fn report_progress(
        &self,
        progress: f64,
        total: Option<f64>,
        message: Option<String>,
    ) -> Result<()>;

    /// Report percentage progress (0-100)
    async fn report_percent(&self, percent: f64, message: Option<String>) -> Result<()> {
        self.report_progress(percent, Some(100.0), message).await
    }

    /// Report count-based progress (e.g., "5 of 10 items processed")
    async fn report_count(
        &self,
        current: usize,
        total: usize,
        message: Option<String>,
    ) -> Result<()> {
        self.report_progress(current as f64, Some(total as f64), message).await
    }
}
```

### ServerProgressReporter

The SDK provides a production-ready implementation with several key features:

**Features**:
- ✅ **Rate limiting** - Max 10 notifications/second by default (configurable)
- ✅ **Float validation** - Rejects NaN, infinity, negative values
- ✅ **Epsilon comparisons** - Handles floating-point precision issues
- ✅ **Non-increasing progress handling** - Silently ignores backwards progress (no-op)
- ✅ **Final notification bypass** - Last update always sent, bypassing rate limits
- ✅ **Thread-safe** - Clone and share across tasks

**Validation Rules**:
1. Progress must be finite and non-negative
2. Total (if provided) must be finite and non-negative
3. Progress cannot exceed total (with epsilon tolerance)
4. Progress should increase (non-increasing updates are no-ops)

## Request Metadata and Progress Tokens

The MCP protocol uses the `_meta` field to pass request-level metadata, including progress tokens.

### RequestMeta Structure

```rust
use pmcp::types::{RequestMeta, ProgressToken};

pub struct RequestMeta {
    /// Progress token for out-of-band progress notifications
    pub progress_token: Option<ProgressToken>,
}

pub enum ProgressToken {
    String(String),
    Number(i64),
}
```

### Sending Requests with Progress Tokens

Clients include progress tokens in request metadata:

```rust
use pmcp::types::{CallToolRequest, RequestMeta, ProgressToken};
use serde_json::json;

let request = CallToolRequest {
    name: "process_data".to_string(),
    arguments: json!({ "dataset": "large.csv" }),
    _meta: Some(RequestMeta {
        progress_token: Some(ProgressToken::String("task-123".to_string())),
    }),
};
```

The server automatically extracts the progress token and creates a `ServerProgressReporter` that sends notifications with the matching token.

## Using Progress in Tools

The SDK makes progress reporting simple through `RequestHandlerExtra`.

### Basic Progress Reporting

```rust
use async_trait::async_trait;
use pmcp::error::Result;
use pmcp::server::cancellation::RequestHandlerExtra;
use pmcp::server::ToolHandler;
use serde_json::Value;

struct DataProcessor;

#[async_trait]
impl ToolHandler for DataProcessor {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        let total_items = 100;

        for i in 0..total_items {
            // Process item
            process_item(i).await?;

            // Report progress (no-op if no reporter attached)
            extra.report_count(
                i + 1,
                total_items,
                Some(format!("Processed item {}", i + 1))
            ).await?;
        }

        Ok(json!({"processed": total_items}))
    }
}
```

### Progress Helper Methods

`RequestHandlerExtra` provides three convenience methods:

```rust
// 1. Generic progress (any scale)
extra.report_progress(current, Some(total), Some(message)).await?;

// 2. Percentage (0-100 scale)
extra.report_percent(75.0, Some("75% complete")).await?;

// 3. Count-based (items processed)
extra.report_count(75, 100, Some("75 of 100 items")).await?;
```

**Important**: All methods return `Ok(())` if no progress reporter is attached, so you can **always** call them unconditionally. You don't need to check if a reporter exists - the SDK handles it for you automatically.

## Request Cancellation

The SDK uses `tokio_util::sync::CancellationToken` for async-safe cancellation.

### Checking for Cancellation

```rust
use async_trait::async_trait;
use pmcp::error::{Error, Result};
use pmcp::server::cancellation::RequestHandlerExtra;
use pmcp::server::ToolHandler;
use serde_json::{json, Value};

struct LongRunningTool;

#[async_trait]
impl ToolHandler for LongRunningTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        for i in 0..1000 {
            // Check for cancellation
            if extra.is_cancelled() {
                return Err(Error::internal("Operation cancelled by client"));
            }

            // Do work
            process_chunk(i).await?;
        }

        Ok(json!({"status": "completed"}))
    }
}
```

### Async Cancellation Waiting

For more sophisticated patterns, you can await cancellation:

```rust
use tokio::select;

async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> Result<Value> {
    select! {
        result = perform_long_operation() => {
            // Operation completed
            Ok(json!({"result": result?}))
        }
        _ = extra.cancelled() => {
            // Cancellation received
            Err(Error::internal("Operation cancelled"))
        }
    }
}
```

## Complete Example: Countdown Tool

Let's walk through a complete example that demonstrates both progress and cancellation.

### Tool Implementation

```rust
use async_trait::async_trait;
use pmcp::error::Result;
use pmcp::server::cancellation::RequestHandlerExtra;
use pmcp::server::ToolHandler;
use serde_json::{json, Value};
use std::time::Duration;

struct CountdownTool;

#[async_trait]
impl ToolHandler for CountdownTool {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        // Extract starting number
        let start = args.get("from")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        // Count down from start to 0
        for i in (0..=start).rev() {
            // Check for cancellation
            if extra.is_cancelled() {
                return Err(pmcp::error::Error::internal(
                    "Countdown cancelled by client"
                ));
            }

            // Report progress (counting DOWN, so progress goes UP)
            let current = start - i;
            let message = if i == 0 {
                "Countdown complete! 🎉".to_string()
            } else {
                format!("Counting down: {}", i)
            };

            extra.report_count(current, start, Some(message)).await?;

            // Sleep between counts (except at the end)
            if i > 0 {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }

        Ok(json!({
            "result": "Countdown completed successfully",
            "from": start,
        }))
    }
}
```

### Server Setup

```rust
use pmcp::server::Server;

let server = Server::builder()
    .name("countdown-server")
    .version("1.0.0")
    .tool("countdown", CountdownTool)
    .build()?;
```

### Client Request with Progress Token

```rust
use pmcp::types::{CallToolRequest, RequestMeta, ProgressToken};

let request = CallToolRequest {
    name: "countdown".to_string(),
    arguments: json!({ "from": 5 }),
    _meta: Some(RequestMeta {
        progress_token: Some(ProgressToken::String("countdown-1".to_string())),
    }),
};
```

### Expected Output

```
INFO Starting countdown from 5
INFO Countdown: 5 (progress: 0/5)
INFO Countdown: 4 (progress: 1/5)
INFO Countdown: 3 (progress: 2/5)
INFO Countdown: 2 (progress: 3/5)
INFO Countdown: 1 (progress: 4/5)
INFO Countdown: 0 (progress: 5/5)

✅ Countdown completed!
```

Run the full example:
```bash
cargo run --example 11_progress_countdown
```

## End-to-End Flow

Understanding the complete flow helps debug issues and implement custom solutions.

### Progress Notification Flow

1. **Client sends request** with `_meta.progressToken`
   ```json
   {
     "method": "tools/call",
     "params": {
       "name": "process_data",
       "arguments": {"file": "data.csv"},
       "_meta": {
         "progressToken": "task-123"
       }
     }
   }
   ```

2. **Server extracts token** from request metadata
   ```rust
   let progress_token = req._meta
       .as_ref()
       .and_then(|meta| meta.progress_token.as_ref());
   ```

3. **Server creates reporter** and attaches to `RequestHandlerExtra`
   ```rust
   let reporter = ServerProgressReporter::new(
       token.clone(),
       notification_sender,
   );

   let extra = RequestHandlerExtra::new(request_id, cancellation_token)
       .with_progress_reporter(Some(Arc::new(reporter)));
   ```

4. **Tool reports progress** using helper methods
   ```rust
   extra.report_count(50, 100, Some("Halfway done")).await?;
   ```

5. **Reporter sends notification** through notification channel
   ```json
   {
     "method": "notifications/progress",
     "params": {
       "progressToken": "task-123",
       "progress": 50,
       "total": 100,
       "message": "Halfway done"
     }
   }
   ```

6. **Client receives notifications** and updates UI

### Cancellation Flow

1. **Client sends cancellation** notification
   ```json
   {
     "method": "notifications/cancelled",
     "params": {
       "requestId": "123",
       "reason": "User cancelled operation"
     }
   }
   ```

2. **Server's CancellationManager** cancels the token
   ```rust
   cancellation_manager.cancel_request(request_id, reason).await?;
   ```

3. **Tool checks cancellation** in its loop
   ```rust
   if extra.is_cancelled() {
       return Err(Error::internal("Cancelled"));
   }
   ```

4. **Tool returns early** with cancellation error

## Best Practices

### 1. Always Report Final Progress

The final notification bypasses rate limiting and confirms completion:

```rust
// Good: Report 100% at the end
for i in 0..100 {
    process_item(i).await?;
    extra.report_count(i + 1, 100, None).await?;
}
// Last call (100/100) always sends notification

// Bad: Skip final progress
for i in 0..100 {
    process_item(i).await?;
    if i < 99 {  // ❌ Skips final update
        extra.report_count(i + 1, 100, None).await?;
    }
}
```

### 2. Check Cancellation Regularly

Check at least once per second of work:

```rust
// Good: Check in loop
for item in large_dataset {
    if extra.is_cancelled() {
        return Err(Error::internal("Cancelled"));
    }
    process(item).await?;
    extra.report_progress(...).await?;
}

// Bad: Never check
for item in large_dataset {
    process(item).await?;  // ❌ Can't be cancelled
}
```

### 3. Provide Meaningful Progress Messages

```rust
// Good: Descriptive messages
extra.report_count(
    processed,
    total,
    Some(format!("Processed {} of {} files", processed, total))
).await?;

// Acceptable: No message (progress bar is enough)
extra.report_count(processed, total, None).await?;

// Bad: Useless message
extra.report_count(processed, total, Some("Working...".to_string())).await?;
```

### 4. Handle Progress Errors Gracefully

Progress reporting failures shouldn't crash your tool:

```rust
// Good: Log and continue
if let Err(e) = extra.report_progress(current, total, msg).await {
    tracing::warn!("Failed to report progress: {}", e);
    // Continue processing
}

// Also good: Propagate if progress is critical
extra.report_progress(current, total, msg).await?;
```

### 5. Use Appropriate Progress Scales

Choose the right method for your use case:

```rust
// Count-based (items, files, records)
extra.report_count(processed_files, total_files, msg).await?;

// Percentage (0-100)
extra.report_percent(completion_percentage, msg).await?;

// Custom scale (bytes, seconds, etc.)
extra.report_progress(bytes_downloaded, Some(total_bytes), msg).await?;
```

### 6. Don't Report Progress Too Frequently

The rate limiter protects against flooding, but be considerate:

```rust
// Good: Report on significant milestones
for i in 0..10000 {
    process(i).await?;
    if i % 100 == 0 {  // Every 100 items
        extra.report_count(i, 10000, None).await?;
    }
}

// Bad: Report on every iteration (will be rate-limited)
for i in 0..10000 {
    process(i).await?;
    extra.report_count(i, 10000, None).await?;  // Too frequent!
}
```

The default rate limit (100ms) means you can report ~10 times per second without throttling.

## Advanced Patterns

### Progress with Nested Operations

When operations have sub-tasks, scale progress appropriately:

```rust
async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> Result<Value> {
    let tasks = vec!["download", "process", "upload"];
    let total_steps = tasks.len();

    for (i, task) in tasks.iter().enumerate() {
        match *task {
            "download" => {
                // Sub-task progress: 0-33%
                download_with_progress(&extra, 0.0, 33.0).await?;
            }
            "process" => {
                // Sub-task progress: 33-66%
                process_with_progress(&extra, 33.0, 66.0).await?;
            }
            "upload" => {
                // Sub-task progress: 66-100%
                upload_with_progress(&extra, 66.0, 100.0).await?;
            }
            _ => {}
        }

        // Report overall progress
        extra.report_count(i + 1, total_steps, Some(format!("Completed {}", task))).await?;
    }

    Ok(json!({"status": "all tasks completed"}))
}
```

### Cancellation with Cleanup

Always clean up resources on cancellation:

```rust
async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> Result<Value> {
    let temp_file = create_temp_file().await?;

    let result = tokio::select! {
        result = process_file(&temp_file) => result,
        _ = extra.cancelled() => {
            // Cleanup on cancellation
            cleanup_temp_file(&temp_file).await?;
            return Err(Error::internal("Operation cancelled"));
        }
    };

    // Normal cleanup
    cleanup_temp_file(&temp_file).await?;
    result
}
```

### Custom Progress Reporters

Implement `ProgressReporter` for custom behavior:

```rust
use pmcp::server::progress::ProgressReporter;

struct LoggingProgressReporter;

#[async_trait]
impl ProgressReporter for LoggingProgressReporter {
    async fn report_progress(
        &self,
        progress: f64,
        total: Option<f64>,
        message: Option<String>,
    ) -> Result<()> {
        let percentage = total.map(|t| (progress / t) * 100.0);
        tracing::info!(
            progress = progress,
            total = ?total,
            percentage = ?percentage,
            message = ?message,
            "Progress update"
        );
        Ok(())
    }
}
```

### NoopProgressReporter (Advanced)

A no-op implementation that discards all progress reports. Most developers won't need this because `RequestHandlerExtra` already handles missing reporters gracefully.

**When you might need it**:

1. **Testing code that takes ProgressReporter directly**:
```rust
async fn process_with_reporter(reporter: Arc<dyn ProgressReporter>) {
    reporter.report_progress(50.0, Some(100.0), None).await.unwrap();
}

#[test]
async fn test_processing() {
    let reporter = Arc::new(NoopProgressReporter);
    process_with_reporter(reporter).await; // No notifications sent
}
```

2. **Manual context construction** without a real reporter.

**Note**: If you're using `RequestHandlerExtra`, you don't need this! The helper methods already return `Ok(())` when no reporter is attached.

## Testing Progress and Cancellation

### Testing Progress Reporting

```rust
#[tokio::test]
async fn test_progress_reporting() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    let reporter = ServerProgressReporter::with_rate_limit(
        ProgressToken::String("test".to_string()),
        Arc::new(move |_| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        }),
        Duration::ZERO, // No rate limiting for tests
    );

    // Report progress
    reporter.report_count(1, 10, Some("Starting".to_string())).await.unwrap();
    reporter.report_count(5, 10, Some("Halfway".to_string())).await.unwrap();
    reporter.report_count(10, 10, Some("Done".to_string())).await.unwrap();

    // Verify all notifications sent
    assert_eq!(counter.load(Ordering::SeqCst), 3);
}
```

### Testing Cancellation

```rust
#[tokio::test]
async fn test_cancellation() {
    let token = CancellationToken::new();
    let extra = RequestHandlerExtra::new("test".to_string(), token.clone());

    // Not cancelled initially
    assert!(!extra.is_cancelled());

    // Cancel the token
    token.cancel();

    // Now cancelled
    assert!(extra.is_cancelled());
}
```

## Troubleshooting

### Progress Not Appearing

**Symptom**: No progress notifications received by client

**Checks**:
1. Client sent `_meta.progressToken` in request?
2. Server has `notification_tx` channel configured?
3. Progress values are valid (finite, non-negative)?
4. Rate limiting not too aggressive? (check interval)

### Cancellation Not Working

**Symptom**: Tool continues running after cancellation

**Checks**:
1. Tool calls `extra.is_cancelled()` regularly?
2. Tool doesn't have blocking operations preventing cancellation checks?
3. CancellationManager received the cancellation notification?
4. Tool returns error on cancellation?

### Rate Limiting Too Aggressive

**Symptom**: Some progress updates missing

**Solution**: Customize rate limit interval:
```rust
let reporter = ServerProgressReporter::with_rate_limit(
    token,
    notification_sender,
    Duration::from_millis(50), // 20 notifications/second
);
```

## Summary

The PMCP SDK provides production-ready progress reporting and cancellation:

**Progress Features**:
- ✅ Trait-based abstraction
- ✅ Automatic rate limiting (configurable)
- ✅ Float validation and epsilon comparisons
- ✅ Multiple convenience methods
- ✅ Thread-safe and clone-able

**Cancellation Features**:
- ✅ Async-safe tokens
- ✅ Easy integration with `RequestHandlerExtra`
- ✅ Support for cleanup on cancellation
- ✅ Works with `tokio::select!` for advanced patterns

**Best Practices**:
- Always report final progress (bypasses rate limits)
- Check cancellation regularly (at least once per second)
- Provide meaningful progress messages
- Choose appropriate progress scales
- Clean up resources on cancellation

See `examples/11_progress_countdown.rs` for a complete working example.
