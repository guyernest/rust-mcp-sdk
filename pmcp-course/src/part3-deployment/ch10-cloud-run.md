# Google Cloud Run Deployment

Google Cloud Run provides a fully managed container runtime that combines the simplicity of serverless with the flexibility of containers. For MCP servers, this means you get standard Docker deployments with automatic scaling, making it an excellent choice when you need more control than Lambda offers but don't want to manage infrastructure.

## Learning Objectives

By the end of this chapter, you will:
- Deploy MCP servers to Cloud Run using containers
- Configure auto-scaling for optimal cost and performance
- Integrate with Cloud SQL and other GCP services
- Implement proper secrets management
- Set up monitoring and alerting
- Understand when to choose Cloud Run over Lambda or Workers

## Why Cloud Run for MCP?

### The Container Advantage

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Deployment Model Comparison                       │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  AWS Lambda              Cloudflare Workers        Cloud Run        │
│  ───────────            ─────────────────         ──────────        │
│  ZIP Package            WASM Binary               Docker Image      │
│  Custom Runtime         V8 Isolate                Full Linux        │
│  15min timeout          30s-15min timeout         60min timeout     │
│  10GB memory max        128MB memory              32GB memory       │
│  /tmp filesystem        No filesystem             Full filesystem   │
│  AWS-specific           CF-specific               Portable          │
│                                                                     │
│  Best for:              Best for:                 Best for:         │
│  Event-driven           Edge/global               Complex workloads │
│  Quick operations       Low latency               Long operations   │
│  AWS ecosystem          Simple compute            GCP ecosystem     │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### When to Choose Cloud Run

Cloud Run excels for MCP servers that need:

| Requirement | Why Cloud Run |
|-------------|---------------|
| Long-running operations | Up to 60 minute timeout (vs 15min Lambda) |
| Large memory workloads | Up to 32GB RAM (vs 10GB Lambda) |
| Complex dependencies | Full Docker environment |
| GPU access | Cloud Run supports GPUs |
| File system access | Writable filesystem (in-memory) |
| Portability | Standard containers run anywhere |
| GCP ecosystem | Native integration with GCP services |

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Cloud Run MCP Architecture                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│                         ┌──────────────┐                            │
│    Claude Desktop ─────▶│   Cloud Run  │                            │
│    Claude.ai      ─────▶│   Service    │                            │
│    Custom Client  ─────▶│              │                            │
│                         └──────┬───────┘                            │
│                                │                                    │
│         ┌──────────────────────┼──────────────────────┐            │
│         │                      │                      │            │
│         ▼                      ▼                      ▼            │
│  ┌─────────────┐      ┌─────────────┐      ┌─────────────┐        │
│  │  Cloud SQL  │      │   Secret    │      │   Cloud     │        │
│  │  (Postgres) │      │   Manager   │      │   Storage   │        │
│  └─────────────┘      └─────────────┘      └─────────────┘        │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                     VPC Network                              │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │   │
│  │  │  Internal   │  │   Private   │  │   Cloud     │         │   │
│  │  │  Services   │  │   APIs      │  │   NAT       │         │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘         │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Request Flow

1. **Client connects** via HTTPS to Cloud Run URL
2. **Load balancer** routes to available instance
3. **Container** handles MCP request
4. **Service mesh** connects to Cloud SQL, Secret Manager
5. **Response** returns through the same path

## Project Setup

### Prerequisites

```bash
# Install Google Cloud CLI
brew install google-cloud-sdk  # macOS
# Or download from https://cloud.google.com/sdk/docs/install

# Authenticate
gcloud auth login
gcloud auth configure-docker

# Set project
gcloud config set project YOUR_PROJECT_ID

# Enable required APIs
gcloud services enable \
  run.googleapis.com \
  cloudbuild.googleapis.com \
  secretmanager.googleapis.com \
  sqladmin.googleapis.com \
  artifactregistry.googleapis.com
```

### Create MCP Server Project

```bash
# Using cargo-pmcp
cargo pmcp new my-mcp-server --template cloud-run

# Or manually create project structure
mkdir my-mcp-server && cd my-mcp-server
cargo init
```

### Cargo.toml Configuration

```toml
[package]
name = "my-mcp-server"
version = "0.1.0"
edition = "2021"

[dependencies]
# MCP SDK
pmcp-sdk = { version = "0.1", features = ["http"] }

# Async runtime
tokio = { version = "1", features = ["full"] }

# Web framework
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Database
sqlx = { version = "0.7", features = ["runtime-tokio", "postgres", "tls-rustls"] }

# Observability
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Configuration
config = "0.14"

# Error handling
anyhow = "1.0"
thiserror = "1.0"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

## Docker Configuration

### Multi-Stage Dockerfile

Create an optimized multi-stage Dockerfile:

```dockerfile
# Stage 1: Build environment
FROM rust:1.75-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifests first for dependency caching
COPY Cargo.toml Cargo.lock ./

# Create dummy main.rs for dependency compilation
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies only (cached layer)
RUN cargo build --release && rm -rf src

# Copy actual source code
COPY src ./src

# Build the application
RUN touch src/main.rs && cargo build --release

# Stage 2: Runtime environment
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash appuser

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/my-mcp-server .

# Set ownership
RUN chown -R appuser:appuser /app

# Switch to non-root user
USER appuser

# Cloud Run expects PORT environment variable
ENV PORT=8080
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Run the binary
CMD ["./my-mcp-server"]
```

### Docker Ignore

```
# .dockerignore
target/
.git/
.gitignore
.env
*.md
Dockerfile
.dockerignore
tests/
examples/
benches/
```

### Local Docker Testing

```bash
# Build locally
docker build -t my-mcp-server:local .

# Run locally
docker run -p 8080:8080 \
  -e DATABASE_URL="postgres://..." \
  my-mcp-server:local

# Test the server
curl http://localhost:8080/health
curl -X POST http://localhost:8080/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}'
```

## MCP Server Implementation

### Main Entry Point

```rust
// src/main.rs
use axum::{
    routing::{get, post},
    Router,
    Json,
    http::StatusCode,
    extract::State,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod mcp;
mod tools;
mod error;

use config::Config;
use mcp::McpServer;

#[derive(Clone)]
struct AppState {
    mcp_server: Arc<McpServer>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing (Cloud Run captures stdout)
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    // Load configuration
    let config = Config::from_env()?;

    // Initialize MCP server
    let mcp_server = Arc::new(McpServer::new(&config).await?);

    let state = AppState { mcp_server };

    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/mcp", post(handle_mcp))
        .layer(CorsLayer::permissive())
        .with_state(state);

    // Get port from environment (Cloud Run sets PORT)
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()?;

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Starting MCP server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> StatusCode {
    StatusCode::OK
}

async fn handle_mcp(
    State(state): State<AppState>,
    Json(request): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.mcp_server.handle_request(request).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => {
            tracing::error!("MCP error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
```

### Configuration Management

```rust
// src/config.rs
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub allowed_origins: Vec<String>,
    pub max_query_rows: usize,
    pub request_timeout_secs: u64,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        // Cloud Run injects secrets as environment variables
        let database_url = std::env::var("DATABASE_URL")
            .map_err(|_| anyhow::anyhow!("DATABASE_URL not set"))?;

        let allowed_origins = std::env::var("ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "*".to_string())
            .split(',')
            .map(String::from)
            .collect();

        let max_query_rows = std::env::var("MAX_QUERY_ROWS")
            .unwrap_or_else(|_| "1000".to_string())
            .parse()?;

        let request_timeout_secs = std::env::var("REQUEST_TIMEOUT_SECS")
            .unwrap_or_else(|_| "30".to_string())
            .parse()?;

        Ok(Self {
            database_url,
            allowed_origins,
            max_query_rows,
            request_timeout_secs,
        })
    }
}
```

### MCP Server Core

```rust
// src/mcp.rs
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::config::Config;
use crate::tools;
use crate::error::McpError;

pub struct McpServer {
    pool: PgPool,
    config: Config,
}

impl McpServer {
    pub async fn new(config: &Config) -> anyhow::Result<Self> {
        let pool = PgPool::connect(&config.database_url).await?;

        // Run migrations if needed
        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Self {
            pool,
            config: config.clone(),
        })
    }

    pub async fn handle_request(&self, request: Value) -> Result<Value, McpError> {
        let method = request["method"]
            .as_str()
            .ok_or_else(|| McpError::InvalidRequest("Missing method".into()))?;

        let id = &request["id"];
        let params = &request["params"];

        let result = match method {
            "initialize" => self.handle_initialize(params),
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tool_call(params).await,
            "resources/list" => self.handle_resources_list(),
            "resources/read" => self.handle_resource_read(params).await,
            _ => Err(McpError::MethodNotFound(method.to_string())),
        }?;

        Ok(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result
        }))
    }

    fn handle_initialize(&self, _params: &Value) -> Result<Value, McpError> {
        Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {},
                "resources": {}
            },
            "serverInfo": {
                "name": "cloud-run-mcp-server",
                "version": env!("CARGO_PKG_VERSION")
            }
        }))
    }

    fn handle_tools_list(&self) -> Result<Value, McpError> {
        Ok(json!({
            "tools": tools::list_tools()
        }))
    }

    async fn handle_tool_call(&self, params: &Value) -> Result<Value, McpError> {
        let tool_name = params["name"]
            .as_str()
            .ok_or_else(|| McpError::InvalidRequest("Missing tool name".into()))?;

        let arguments = &params["arguments"];

        tools::call_tool(tool_name, arguments, &self.pool, &self.config).await
    }

    fn handle_resources_list(&self) -> Result<Value, McpError> {
        Ok(json!({
            "resources": [
                {
                    "uri": "db://tables",
                    "name": "Database Tables",
                    "description": "List of available database tables",
                    "mimeType": "application/json"
                }
            ]
        }))
    }

    async fn handle_resource_read(&self, params: &Value) -> Result<Value, McpError> {
        let uri = params["uri"]
            .as_str()
            .ok_or_else(|| McpError::InvalidRequest("Missing uri".into()))?;

        match uri {
            "db://tables" => {
                let tables: Vec<(String,)> = sqlx::query_as(
                    "SELECT table_name FROM information_schema.tables
                     WHERE table_schema = 'public'"
                )
                .fetch_all(&self.pool)
                .await
                .map_err(|e| McpError::DatabaseError(e.to_string()))?;

                Ok(json!({
                    "contents": [{
                        "uri": uri,
                        "mimeType": "application/json",
                        "text": serde_json::to_string_pretty(&tables)?
                    }]
                }))
            }
            _ => Err(McpError::ResourceNotFound(uri.to_string())),
        }
    }
}
```

## Deployment

### Using cargo-pmcp

The simplest deployment method:

```bash
# Deploy to Cloud Run
cargo pmcp deploy cloud-run \
  --project my-gcp-project \
  --region us-central1 \
  --service my-mcp-server

# With additional options
cargo pmcp deploy cloud-run \
  --project my-gcp-project \
  --region us-central1 \
  --service my-mcp-server \
  --memory 1Gi \
  --cpu 2 \
  --min-instances 1 \
  --max-instances 10 \
  --concurrency 80 \
  --timeout 300
```

### Manual Deployment

```bash
# Build and push to Artifact Registry
gcloud builds submit --tag gcr.io/PROJECT_ID/my-mcp-server

# Or use Artifact Registry (recommended)
gcloud artifacts repositories create mcp-servers \
  --repository-format=docker \
  --location=us-central1

docker tag my-mcp-server:local \
  us-central1-docker.pkg.dev/PROJECT_ID/mcp-servers/my-mcp-server:v1

docker push us-central1-docker.pkg.dev/PROJECT_ID/mcp-servers/my-mcp-server:v1

# Deploy to Cloud Run
gcloud run deploy my-mcp-server \
  --image us-central1-docker.pkg.dev/PROJECT_ID/mcp-servers/my-mcp-server:v1 \
  --platform managed \
  --region us-central1 \
  --allow-unauthenticated \
  --memory 1Gi \
  --cpu 2 \
  --min-instances 1 \
  --max-instances 10 \
  --concurrency 80 \
  --timeout 300 \
  --set-env-vars "RUST_LOG=info"
```

### Cloud Run Service Configuration

Create a `service.yaml` for declarative deployments:

```yaml
# service.yaml
apiVersion: serving.knative.dev/v1
kind: Service
metadata:
  name: my-mcp-server
  annotations:
    run.googleapis.com/ingress: all
spec:
  template:
    metadata:
      annotations:
        # Scaling configuration
        autoscaling.knative.dev/minScale: "1"
        autoscaling.knative.dev/maxScale: "10"
        # CPU allocation
        run.googleapis.com/cpu-throttling: "false"
        # VPC connector for private resources
        run.googleapis.com/vpc-access-connector: projects/PROJECT/locations/REGION/connectors/CONNECTOR
        run.googleapis.com/vpc-access-egress: private-ranges-only
    spec:
      containerConcurrency: 80
      timeoutSeconds: 300
      containers:
        - image: us-central1-docker.pkg.dev/PROJECT/mcp-servers/my-mcp-server:v1
          ports:
            - containerPort: 8080
          resources:
            limits:
              memory: 1Gi
              cpu: "2"
          env:
            - name: RUST_LOG
              value: info
            - name: DATABASE_URL
              valueFrom:
                secretKeyRef:
                  name: database-url
                  key: latest
          startupProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 0
            timeoutSeconds: 3
            periodSeconds: 3
            failureThreshold: 10
          livenessProbe:
            httpGet:
              path: /health
              port: 8080
            periodSeconds: 30
```

Deploy with:

```bash
gcloud run services replace service.yaml --region us-central1
```

## Secrets Management

### Creating Secrets

```bash
# Create a secret
echo -n "postgres://user:pass@host:5432/db" | \
  gcloud secrets create database-url --data-file=-

# Grant Cloud Run access
gcloud secrets add-iam-policy-binding database-url \
  --member="serviceAccount:PROJECT_NUMBER-compute@developer.gserviceaccount.com" \
  --role="roles/secretmanager.secretAccessor"
```

### Mounting Secrets

```bash
# As environment variable
gcloud run deploy my-mcp-server \
  --set-secrets="DATABASE_URL=database-url:latest"

# As file (for certificates, etc.)
gcloud run deploy my-mcp-server \
  --set-secrets="/secrets/db-cert=db-certificate:latest"
```

### Accessing Secrets in Code

```rust
// Secrets are injected as environment variables
let database_url = std::env::var("DATABASE_URL")?;

// Or read from mounted file
let cert = std::fs::read_to_string("/secrets/db-cert")?;
```

## Cloud SQL Integration

### Setting Up Cloud SQL

```bash
# Create Cloud SQL instance
gcloud sql instances create mcp-database \
  --database-version=POSTGRES_15 \
  --tier=db-f1-micro \
  --region=us-central1 \
  --root-password=YOUR_PASSWORD

# Create database
gcloud sql databases create mcp_db --instance=mcp-database

# Create user
gcloud sql users create mcp_user \
  --instance=mcp-database \
  --password=USER_PASSWORD
```

### VPC Connector for Private IP

```bash
# Create VPC connector
gcloud compute networks vpc-access connectors create mcp-connector \
  --region us-central1 \
  --network default \
  --range 10.8.0.0/28

# Deploy with VPC connector
gcloud run deploy my-mcp-server \
  --vpc-connector mcp-connector \
  --vpc-egress private-ranges-only
```

### Connection String

```bash
# Private IP (via VPC connector)
DATABASE_URL=postgres://mcp_user:PASSWORD@PRIVATE_IP:5432/mcp_db

# Or Cloud SQL Auth Proxy (in sidecar)
DATABASE_URL=postgres://mcp_user:PASSWORD@localhost:5432/mcp_db
```

### Cloud SQL Auth Proxy Sidecar

```yaml
# service.yaml with Cloud SQL proxy
spec:
  template:
    metadata:
      annotations:
        run.googleapis.com/cloudsql-instances: PROJECT:REGION:mcp-database
    spec:
      containers:
        - image: us-central1-docker.pkg.dev/PROJECT/mcp-servers/my-mcp-server:v1
          env:
            - name: DATABASE_URL
              value: postgres://mcp_user:PASSWORD@localhost:5432/mcp_db
```

## Monitoring and Observability

### Structured Logging

Cloud Run automatically captures stdout/stderr. Use structured JSON logging:

```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn init_logging() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_target(true)
                .with_file(true)
                .with_line_number(true)
        )
        .init();
}

// Usage
tracing::info!(
    tool = tool_name,
    duration_ms = elapsed.as_millis(),
    "Tool execution completed"
);
```

### Cloud Monitoring Metrics

```bash
# View metrics
gcloud run services describe my-mcp-server --format="value(status.url)"

# Custom metrics via OpenTelemetry
# Add to Cargo.toml:
# opentelemetry = "0.21"
# opentelemetry-gcp = "0.10"
```

```rust
use opentelemetry::metrics::{Counter, Histogram};
use once_cell::sync::Lazy;

static TOOL_CALLS: Lazy<Counter<u64>> = Lazy::new(|| {
    let meter = opentelemetry::global::meter("mcp-server");
    meter.u64_counter("mcp.tool.calls").init()
});

static TOOL_LATENCY: Lazy<Histogram<f64>> = Lazy::new(|| {
    let meter = opentelemetry::global::meter("mcp-server");
    meter.f64_histogram("mcp.tool.latency").init()
});

// Record metrics
TOOL_CALLS.add(1, &[KeyValue::new("tool", tool_name)]);
TOOL_LATENCY.record(elapsed.as_secs_f64(), &[KeyValue::new("tool", tool_name)]);
```

### Alerting

```bash
# Create alert policy for high error rate
gcloud alpha monitoring policies create \
  --policy-from-file=alert-policy.yaml
```

```yaml
# alert-policy.yaml
displayName: "MCP Server High Error Rate"
conditions:
  - displayName: "Error rate > 1%"
    conditionThreshold:
      filter: >
        resource.type="cloud_run_revision"
        AND resource.labels.service_name="my-mcp-server"
        AND metric.type="run.googleapis.com/request_count"
        AND metric.labels.response_code_class="5xx"
      comparison: COMPARISON_GT
      thresholdValue: 0.01
      duration: 300s
      aggregations:
        - alignmentPeriod: 60s
          perSeriesAligner: ALIGN_RATE
notificationChannels:
  - projects/PROJECT/notificationChannels/CHANNEL_ID
```

## CI/CD with Cloud Build

### cloudbuild.yaml

```yaml
# cloudbuild.yaml
steps:
  # Run tests
  - name: 'rust:1.75'
    entrypoint: 'cargo'
    args: ['test']

  # Build Docker image
  - name: 'gcr.io/cloud-builders/docker'
    args:
      - 'build'
      - '-t'
      - 'us-central1-docker.pkg.dev/$PROJECT_ID/mcp-servers/my-mcp-server:$COMMIT_SHA'
      - '-t'
      - 'us-central1-docker.pkg.dev/$PROJECT_ID/mcp-servers/my-mcp-server:latest'
      - '.'

  # Push to Artifact Registry
  - name: 'gcr.io/cloud-builders/docker'
    args:
      - 'push'
      - '--all-tags'
      - 'us-central1-docker.pkg.dev/$PROJECT_ID/mcp-servers/my-mcp-server'

  # Deploy to Cloud Run
  - name: 'gcr.io/google.com/cloudsdktool/cloud-sdk'
    entrypoint: 'gcloud'
    args:
      - 'run'
      - 'deploy'
      - 'my-mcp-server'
      - '--image'
      - 'us-central1-docker.pkg.dev/$PROJECT_ID/mcp-servers/my-mcp-server:$COMMIT_SHA'
      - '--region'
      - 'us-central1'
      - '--platform'
      - 'managed'

images:
  - 'us-central1-docker.pkg.dev/$PROJECT_ID/mcp-servers/my-mcp-server:$COMMIT_SHA'
  - 'us-central1-docker.pkg.dev/$PROJECT_ID/mcp-servers/my-mcp-server:latest'

options:
  logging: CLOUD_LOGGING_ONLY
```

### Trigger Setup

```bash
# Create trigger for main branch
gcloud builds triggers create github \
  --repo-name=my-mcp-server \
  --repo-owner=myorg \
  --branch-pattern="^main$" \
  --build-config=cloudbuild.yaml
```

## Connecting Clients

### Service URL

After deployment, get your service URL:

```bash
gcloud run services describe my-mcp-server \
  --region us-central1 \
  --format="value(status.url)"

# Example: https://my-mcp-server-abc123-uc.a.run.app
```

### Claude Desktop Configuration

```json
{
  "mcpServers": {
    "cloud-run-server": {
      "url": "https://my-mcp-server-abc123-uc.a.run.app/mcp",
      "transport": "http"
    }
  }
}
```

### Authentication (Optional)

For authenticated endpoints:

```bash
# Require authentication
gcloud run deploy my-mcp-server --no-allow-unauthenticated

# Get identity token
TOKEN=$(gcloud auth print-identity-token)

# Use with curl
curl -H "Authorization: Bearer $TOKEN" \
  https://my-mcp-server-abc123-uc.a.run.app/mcp
```

For service-to-service authentication:

```json
{
  "mcpServers": {
    "cloud-run-server": {
      "url": "https://my-mcp-server-abc123-uc.a.run.app/mcp",
      "transport": "http",
      "headers": {
        "Authorization": "Bearer ${GOOGLE_ID_TOKEN}"
      }
    }
  }
}
```

## Summary

Google Cloud Run provides a powerful platform for MCP servers when you need:

- **Container flexibility** - Full Docker environment with any dependencies
- **Long-running operations** - Up to 60 minute timeouts
- **Large memory workloads** - Up to 32GB RAM
- **GCP ecosystem integration** - Native Cloud SQL, Secret Manager, etc.
- **Portability** - Standard containers run anywhere

Key deployment steps:
1. Create optimized multi-stage Dockerfile
2. Configure secrets and database connections
3. Deploy with appropriate scaling settings
4. Set up monitoring and alerting
5. Configure CI/CD for automated deployments

In the next lesson, we'll explore container optimization patterns and advanced scaling configurations.
