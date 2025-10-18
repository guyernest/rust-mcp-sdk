# Progress & Cancellation Improvements Design

## Executive Summary

The PMCP SDK has basic types for progress and cancellation but lacks practical integration that makes it easy for developers to use these features. This document outlines the gaps and proposes Rust-idiomatic solutions.

## Current State Assessment

### ✅ What Exists

1. **Core Types** (`src/types/protocol.rs`):
   - `ProgressNotification` struct
   - `ProgressToken` enum (String | Number)
   - `CancelledNotification` struct
   - Notification enums include progress/cancelled variants

2. **Cancellation Infrastructure** (`src/server/cancellation.rs`):
   - `CancellationManager` for tracking request cancellation tokens
   - `RequestHandlerExtra` includes `cancellation_token: CancellationToken`
   - Tools can check `extra.is_cancelled()` and `extra.cancelled().await`

3. **Basic Examples**:
   - `examples/10_progress_notifications.rs` - Type demonstration only
   - `examples/11_request_cancellation.rs` - Type demonstration only

### ❌ Critical Gaps

#### 1. **Missing `total` Field in `ProgressNotification`**

**Protocol Spec**: "The receiver MAY send progress notifications containing... an optional 'total' value"

**Current Implementation**:
```rust
pub struct ProgressNotification {
    pub progress_token: ProgressToken,
    pub progress: f64,  // ✅ Present
    pub message: Option<String>,  // ✅ Present
    // ❌ MISSING: total field!
}
```

**Problem**: Cannot send progress as "5 of 10 items processed" - only percentage/absolute values.

#### 2. **No Progress Token in `RequestHandlerExtra`**

**Current**: Tools cannot access the `progressToken` from the request metadata.

```rust
pub struct RequestHandlerExtra {
    pub cancellation_token: CancellationToken,  // ✅ Has cancellation
    pub request_id: String,
    pub session_id: Option<String>,
    pub auth_info: Option<AuthInfo>,
    pub auth_context: Option<AuthContext>,
    pub metadata: HashMap<String, String>,
    // ❌ MISSING: progress_token field!
}
```

**Impact**: Tools have no way to know if the client wants progress updates or what token to use.

#### 3. **No Progress Reporter Abstraction**

**Problem**: Tools must manually construct notifications and somehow send them through the server.

**Current Workaround** (doesn't exist):
```rust
// Tools have NO WAY to send progress notifications!
// They would need direct access to ServerCore's notification sender
```

#### 4. **No Progress Token Tracking**

**Protocol Requirements**:
- "Progress notifications MUST only reference tokens that were provided in an active request"
- "Both parties SHOULD implement rate limiting to prevent flooding"

**Current**: No validation, no tracking, no rate limiting.

#### 5. **No Notification Sender in `RequestHandlerExtra`**

**Problem**: Even if tools had the progress token, they have no way to send notifications.

**Missing**: A callback/channel to send progress notifications back to the client.

## Proposed Solutions

### Solution 1: Add `total` Field to `ProgressNotification`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressNotification {
    /// Progress token from the original request
    pub progress_token: ProgressToken,

    /// Current progress value (must increase with each notification)
    pub progress: f64,

    /// Optional total value for the operation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<f64>,

    /// Optional human-readable progress message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}
```

**Benefit**: Matches protocol spec, enables "5 of 10" progress reporting.

### Solution 2: Add `ProgressReporter` to `RequestHandlerExtra`

```rust
/// Trait for sending progress notifications during tool execution
#[async_trait]
pub trait ProgressReporter: Send + Sync {
    /// Send a progress notification
    async fn report_progress(&self, progress: f64, total: Option<f64>, message: Option<String>) -> Result<()>;
}

/// Progress reporter that wraps a notification sender
pub struct ServerProgressReporter {
    progress_token: ProgressToken,
    notification_sender: Arc<dyn Fn(Notification) + Send + Sync>,
    last_progress: Arc<Mutex<f64>>,  // Track for validation
    last_sent: Arc<Mutex<Instant>>,  // Rate limiting
}

impl ServerProgressReporter {
    /// Report progress with optional total and message
    pub async fn report(&self, progress: f64, total: Option<f64>, message: Option<String>) -> Result<()> {
        // Validate progress increases
        let mut last = self.last_progress.lock().unwrap();
        if progress <= *last {
            return Err(Error::validation("Progress must increase"));
        }
        *last = progress;

        // Rate limiting (max 10 notifications per second)
        let mut last_sent = self.last_sent.lock().unwrap();
        let now = Instant::now();
        if now.duration_since(*last_sent) < Duration::from_millis(100) {
            // Skip this notification (rate limit)
            return Ok(());
        }
        *last_sent = now;

        // Send notification
        let notification = Notification::Server(ServerNotification::Progress(
            ProgressNotification {
                progress_token: self.progress_token.clone(),
                progress,
                total,
                message,
            }
        ));

        (self.notification_sender)(notification);
        Ok(())
    }

    /// Helper: Report with percentage (0-100)
    pub async fn report_percent(&self, percent: f64, message: Option<String>) -> Result<()> {
        self.report(percent, Some(100.0), message).await
    }

    /// Helper: Report with count (current of total)
    pub async fn report_count(&self, current: usize, total: usize, message: Option<String>) -> Result<()> {
        self.report(current as f64, Some(total as f64), message).await
    }
}
```

### Solution 3: Extend `RequestHandlerExtra` with Progress Support

```rust
pub struct RequestHandlerExtra {
    // Existing fields
    pub cancellation_token: CancellationToken,
    pub request_id: String,
    pub session_id: Option<String>,
    pub auth_info: Option<AuthInfo>,
    pub auth_context: Option<AuthContext>,
    pub metadata: HashMap<String, String>,

    // NEW: Progress reporting
    progress_reporter: Option<Arc<dyn ProgressReporter>>,
}

impl RequestHandlerExtra {
    /// Check if client requested progress updates
    pub fn has_progress_reporter(&self) -> bool {
        self.progress_reporter.is_some()
    }

    /// Get progress reporter if available
    pub fn progress_reporter(&self) -> Option<&Arc<dyn ProgressReporter>> {
        self.progress_reporter.as_ref()
    }

    /// Report progress (convenience method)
    pub async fn report_progress(&self, progress: f64, total: Option<f64>, message: Option<String>) -> Result<()> {
        if let Some(reporter) = &self.progress_reporter {
            reporter.report_progress(progress, total, message).await
        } else {
            Ok(()) // Silently ignore if no progress requested
        }
    }

    /// Report percentage progress (0-100)
    pub async fn report_percent(&self, percent: f64, message: impl Into<Option<String>>) -> Result<()> {
        self.report_progress(percent, Some(100.0), message.into()).await
    }

    /// Report count-based progress
    pub async fn report_count(&self, current: usize, total: usize, message: impl Into<Option<String>>) -> Result<()> {
        self.report_progress(current as f64, Some(total as f64), message.into()).await
    }
}
```

### Solution 4: Update `ServerCore` to Create Progress Reporters

```rust
impl ServerCore {
    async fn handle_request(&self, id: RequestId, request: Request, auth_context: Option<AuthContext>) -> JSONRPCResponse {
        // Extract progress token from request metadata
        let progress_token = extract_progress_token(&request);

        // Create progress reporter if token provided
        let progress_reporter = progress_token.map(|token| {
            Arc::new(ServerProgressReporter::new(
                token,
                self.notification_sender.clone(),
            )) as Arc<dyn ProgressReporter>
        });

        // Create RequestHandlerExtra with progress reporter
        let extra = RequestHandlerExtra::new(request_id, cancellation_token)
            .with_progress_reporter(progress_reporter)
            .with_auth_context(auth_context);

        // ... rest of request handling
    }
}
```

## Implementation Plan

### Phase 1: Core Types & Infrastructure
1. Add `total` field to `ProgressNotification`
2. Create `ProgressReporter` trait
3. Implement `ServerProgressReporter` with rate limiting and validation
4. Add `progress_reporter` field to `RequestHandlerExtra`
5. Add helper methods to `RequestHandlerExtra`

### Phase 2: Server Integration
1. Update `ServerCore` to extract `progressToken` from request metadata
2. Create `ProgressReporter` instances for requests with progress tokens
3. Pass progress reporter through to tool handlers via `RequestHandlerExtra`

### Phase 3: Examples & Documentation
1. Create realistic example: Long-running file processing tool with progress
2. Create realistic example: Database query with cancellation
3. Update book chapter with comprehensive guide
4. Add API documentation

### Phase 4: Testing
1. Unit tests for `ProgressReporter` rate limiting
2. Unit tests for progress validation (must increase)
3. Integration tests for full progress flow
4. Cancellation integration tests

## Rust Best Practices

1. **Trait-Based Design**: `ProgressReporter` trait allows flexibility and testing
2. **Builder Pattern**: `RequestHandlerExtra::with_progress_reporter()`
3. **Zero-Cost Abstraction**: `Option<Arc<dyn ProgressReporter>>` - no overhead if unused
4. **Error Handling**: Validation errors returned as `Result<()>`
5. **Rate Limiting**: Built-in protection against notification flooding
6. **Type Safety**: `ProgressToken` enum prevents invalid token types

## Breaking Changes

**None** - all changes are additive:
- New `total` field in `ProgressNotification` is optional
- New `progress_reporter` field in `RequestHandlerExtra` is optional
- Existing code continues to work without modification

## Example Usage (After Implementation)

```rust
#[async_trait]
impl ToolHandler for ProcessFilesTool {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        let files: Vec<String> = serde_json::from_value(args)?;
        let total_files = files.len();

        for (idx, file) in files.iter().enumerate() {
            // Check for cancellation
            if extra.is_cancelled() {
                return Err(Error::cancelled("Processing cancelled by user"));
            }

            // Report progress
            extra.report_count(
                idx + 1,
                total_files,
                Some(format!("Processing {}", file))
            ).await?;

            // Do actual work
            process_file(file).await?;
        }

        Ok(json!({"processed": total_files}))
    }
}
```

## Success Metrics

1. **Developer Experience**: Tools can report progress in 1-3 lines of code
2. **Protocol Compliance**: Full support for MCP progress/cancellation spec
3. **Safety**: Built-in rate limiting and validation
4. **Performance**: Zero overhead when progress not requested
5. **Documentation**: Comprehensive examples and book chapter

## Next Steps

1. Get approval on this design
2. Implement Phase 1 (core types)
3. Share with MCP server team for early feedback
4. Iterate based on feedback
5. Complete implementation and documentation
