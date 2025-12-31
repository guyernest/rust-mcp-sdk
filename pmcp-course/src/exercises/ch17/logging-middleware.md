::: exercise
id: ch17-01-logging-middleware
difficulty: intermediate
time: 45 minutes
:::

Build production-ready logging middleware that provides visibility into your
MCP server's behavior. Transform "printf debugging" into structured observability.

::: objectives
thinking:
  - Why structured logging beats text-based logging
  - The importance of request correlation with IDs
  - How to balance verbosity vs storage costs
doing:
  - Add tracing subscriber with structured output
  - Create spans with tool name, user ID, request ID
  - Record request duration and status
  - Redact sensitive data before logging
:::

::: discussion
- What's the first thing you check when investigating a production bug?
- How do you currently know if your MCP server is working correctly?
- What information would help debug a slow request?
:::

## Step 1: Add Dependencies

In `Cargo.toml`:

```toml
[dependencies]
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
uuid = { version = "1", features = ["v4"] }
```

## Step 2: Initialize Tracing

```rust
use tracing_subscriber::{fmt, EnvFilter};

fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .json()  // Structured output for log aggregators
        .init();
}
```

## Step 3: Build Logging Middleware

```rust
use tracing::{info, warn, instrument, Span};
use std::time::Instant;
use uuid::Uuid;

pub struct LoggingMiddleware {
    redact_patterns: Vec<&'static str>,
}

impl LoggingMiddleware {
    pub fn new() -> Self {
        Self {
            redact_patterns: vec!["token", "password", "secret", "key", "authorization"],
        }
    }

    fn should_redact(&self, key: &str) -> bool {
        let lower = key.to_lowercase();
        self.redact_patterns.iter().any(|p| lower.contains(p))
    }
}

#[async_trait]
impl AdvancedMiddleware for LoggingMiddleware {
    async fn on_request(
        &self,
        request: &Request,
        context: &mut Context,
    ) -> Result<()> {
        let request_id = Uuid::new_v4();
        let start = Instant::now();

        context.set("request_id", request_id);
        context.set("start_time", start);

        info!(
            request_id = %request_id,
            method = %request.method(),
            tool = request.tool_name().unwrap_or("unknown"),
            "Request started"
        );

        Ok(())
    }

    async fn on_response(
        &self,
        response: &Response,
        context: &Context,
    ) -> Result<()> {
        let request_id: Uuid = context.get("request_id")?;
        let start: Instant = context.get("start_time")?;
        let duration = start.elapsed();

        info!(
            request_id = %request_id,
            duration_ms = duration.as_millis() as u64,
            status = "success",
            "Request completed"
        );

        Ok(())
    }

    async fn on_error(
        &self,
        error: &Error,
        context: &Context,
    ) -> Result<()> {
        let request_id: Uuid = context.get("request_id")?;
        let start: Instant = context.get("start_time")?;

        warn!(
            request_id = %request_id,
            duration_ms = start.elapsed().as_millis() as u64,
            error = %error,
            status = "error",
            "Request failed"
        );

        Ok(())
    }
}
```

## Step 4: Use #[instrument] for Tools

```rust
#[instrument(
    skip(context),
    fields(request_id = %context.get::<Uuid>("request_id").unwrap())
)]
pub async fn my_tool(
    input: MyInput,
    context: &ToolContext,
) -> Result<Output> {
    info!("Processing tool request");
    // ... implementation
}
```

## Step 5: Add to Server

```rust
let server = ServerBuilder::new("observable-server", "1.0.0")
    .middleware(LoggingMiddleware::new())
    .with_tool(tools::MyTool)
    .build()?;
```

## Step 6: Configure Log Levels

```bash
# Development - verbose
RUST_LOG=debug cargo run

# Production - info and above
RUST_LOG=info cargo run

# Specific modules
RUST_LOG=my_server=debug,hyper=warn cargo run
```

::: hints
level_1: "Always include request_id in every log line for correlation."
level_2: "Use structured fields (key=value) instead of string interpolation for searchable logs."
level_3: "Set up log shipping to CloudWatch, Datadog, or another aggregator for production."
:::

## Success Criteria

- [ ] All requests logged with structured context
- [ ] Request duration captured for every call
- [ ] Sensitive data redacted before logging
- [ ] Request ID present in all related logs
- [ ] Log level appropriately configured for production
- [ ] Can find all logs for a specific request using request_id

---

*Next: [Metrics Collection](./metrics-collection.md) for quantitative observability.*
