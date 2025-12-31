# Container-Based Deployment

Building optimized Docker containers for Rust MCP servers requires understanding the unique characteristics of Rust binaries and the Cloud Run execution environment. This lesson covers advanced Dockerfile patterns, image optimization, and container best practices.

## Learning Objectives

By the end of this lesson, you will:
- Create highly optimized multi-stage Dockerfiles for Rust
- Minimize container image size for faster deployments
- Implement proper caching strategies for faster builds
- Configure containers for Cloud Run's execution model
- Handle cross-compilation for different architectures

## Why Container Size Matters

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Container Size Impact                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Image Size    Pull Time     Cold Start    Registry Cost           │
│  ──────────   ──────────    ──────────    ────────────            │
│  10MB         ~1s           ~2s           $0.10/GB                 │
│  50MB         ~3s           ~4s           $0.10/GB                 │
│  100MB        ~5s           ~6s           $0.10/GB                 │
│  500MB        ~15s          ~17s          $0.10/GB                 │
│  1GB+         ~30s+         ~35s+         $0.10/GB                 │
│                                                                     │
│  Target for Rust MCP servers: <50MB                                │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Rust's Container Advantage

Rust produces statically-linked binaries that can run in minimal containers:

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Language Container Comparison                     │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Language      Base Image        Runtime Deps    Typical Size      │
│  ─────────    ──────────        ────────────    ────────────       │
│  Python       python:3.11       pip packages    500MB-1GB          │
│  Node.js      node:20           npm packages    300MB-800MB        │
│  Java         eclipse-temurin   JRE             400MB-600MB        │
│  Go           scratch/alpine    none            10MB-50MB          │
│  Rust         scratch/alpine    ca-certs only   5MB-30MB           │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Multi-Stage Build Patterns

### Basic Multi-Stage Dockerfile

```dockerfile
# Stage 1: Build
FROM rust:1.75-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy and build
COPY . .
RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/my-mcp-server /usr/local/bin/

CMD ["my-mcp-server"]
```

### Optimized Multi-Stage with Dependency Caching

This pattern separates dependency compilation from source compilation for much faster rebuilds:

```dockerfile
# Stage 1: Chef - prepare recipe
FROM rust:1.75-slim-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /app

# Stage 2: Planner - analyze dependencies
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Builder - build dependencies first, then source
FROM chef AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Build dependencies (cached layer)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Build application
COPY . .
RUN cargo build --release

# Stage 4: Runtime
FROM debian:bookworm-slim AS runtime

# Install only runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 appuser
USER appuser

WORKDIR /app
COPY --from=builder /app/target/release/my-mcp-server .

ENV PORT=8080
EXPOSE 8080

CMD ["./my-mcp-server"]
```

### Minimal Scratch-Based Container

For the smallest possible image when you don't need a shell:

```dockerfile
# Stage 1: Build with musl for static linking
FROM rust:1.75-alpine AS builder

RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static

WORKDIR /app

# Build with musl target
COPY . .
RUN RUSTFLAGS='-C target-feature=+crt-static' \
    cargo build --release --target x86_64-unknown-linux-musl

# Stage 2: Scratch runtime (no OS, just binary)
FROM scratch

# Copy CA certificates for HTTPS
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Copy binary
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/my-mcp-server /

# Set user (numeric, since scratch has no /etc/passwd)
USER 1000

ENV PORT=8080
EXPOSE 8080

ENTRYPOINT ["/my-mcp-server"]
```

### Distroless Runtime

Google's distroless images provide a middle ground - minimal but with some debugging capabilities:

```dockerfile
# Stage 1: Build
FROM rust:1.75-slim-bookworm AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .
RUN cargo build --release

# Stage 2: Distroless runtime
FROM gcr.io/distroless/cc-debian12

COPY --from=builder /app/target/release/my-mcp-server /

ENV PORT=8080
EXPOSE 8080

USER nonroot

ENTRYPOINT ["/my-mcp-server"]
```

## Build Optimization Strategies

### Cargo Configuration for Smaller Binaries

```toml
# Cargo.toml
[profile.release]
opt-level = "z"        # Optimize for size (smallest)
lto = true             # Link-time optimization
codegen-units = 1      # Single codegen unit
panic = "abort"        # No unwinding code
strip = true           # Strip symbols

# For production with balance of size and speed
[profile.release-optimized]
inherits = "release"
opt-level = 3          # Optimize for speed
lto = "thin"           # Faster LTO
```

### Reducing Binary Size

```bash
# Check binary size before optimization
cargo build --release
ls -lh target/release/my-mcp-server
# Before: 15MB

# After Cargo.toml optimizations
cargo build --release
ls -lh target/release/my-mcp-server
# After: 5MB

# Additional stripping (if strip=true not in Cargo.toml)
strip target/release/my-mcp-server
# After strip: 3MB

# UPX compression (optional, trades startup time for size)
upx --best target/release/my-mcp-server
# After UPX: 1.5MB (but slower startup)
```

### Dependency Audit

Remove unused dependencies to reduce compile time and binary size:

```bash
# Find unused dependencies
cargo install cargo-udeps
cargo +nightly udeps

# Analyze dependency tree
cargo tree --duplicates

# Check feature flags being used
cargo tree -e features
```

### Conditional Compilation

Use feature flags to include only what you need:

```toml
# Cargo.toml
[features]
default = ["postgres"]
postgres = ["sqlx/postgres"]
mysql = ["sqlx/mysql"]
sqlite = ["sqlx/sqlite"]
full = ["postgres", "mysql", "sqlite"]
```

```dockerfile
# Build with specific features
RUN cargo build --release --no-default-features --features postgres
```

## Cross-Compilation

### Building for Different Architectures

Cloud Run supports both AMD64 and ARM64. ARM64 can be cheaper and more efficient:

```dockerfile
# Cross-compilation for ARM64
FROM --platform=$BUILDPLATFORM rust:1.75-slim-bookworm AS builder

ARG TARGETPLATFORM
ARG BUILDPLATFORM

# Install cross-compilation tools
RUN case "$TARGETPLATFORM" in \
    "linux/arm64") \
        apt-get update && apt-get install -y \
            gcc-aarch64-linux-gnu \
            libc6-dev-arm64-cross \
        && rustup target add aarch64-unknown-linux-gnu \
        ;; \
    "linux/amd64") \
        apt-get update && apt-get install -y \
            gcc \
            libc6-dev \
        ;; \
    esac && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

# Build for target platform
RUN case "$TARGETPLATFORM" in \
    "linux/arm64") \
        CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
        cargo build --release --target aarch64-unknown-linux-gnu \
        && cp target/aarch64-unknown-linux-gnu/release/my-mcp-server target/release/ \
        ;; \
    "linux/amd64") \
        cargo build --release \
        ;; \
    esac

# Runtime stage
FROM --platform=$TARGETPLATFORM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/my-mcp-server /usr/local/bin/

CMD ["my-mcp-server"]
```

### Building Multi-Architecture Images

```bash
# Enable buildx
docker buildx create --use

# Build for multiple architectures
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t us-central1-docker.pkg.dev/PROJECT/mcp-servers/my-mcp-server:v1 \
  --push \
  .
```

### Deploying ARM64 to Cloud Run

```bash
# Deploy specifying ARM64
gcloud run deploy my-mcp-server \
  --image us-central1-docker.pkg.dev/PROJECT/mcp-servers/my-mcp-server:v1 \
  --platform managed \
  --cpu-boost \
  --execution-environment gen2  # Required for ARM
```

## Container Security

### Non-Root User

Always run as non-root:

```dockerfile
# Create user in builder stage if needed
FROM debian:bookworm-slim AS runtime

# Create non-root user with specific UID
RUN groupadd -r -g 1000 appgroup && \
    useradd -r -u 1000 -g appgroup -s /sbin/nologin appuser

# Set ownership of application files
COPY --from=builder --chown=appuser:appgroup /app/target/release/my-mcp-server /app/

# Switch to non-root user
USER appuser

WORKDIR /app
CMD ["./my-mcp-server"]
```

### Read-Only Filesystem

Configure Cloud Run to use read-only container filesystem:

```yaml
# service.yaml
spec:
  template:
    spec:
      containers:
        - image: my-image
          securityContext:
            readOnlyRootFilesystem: true
          volumeMounts:
            - name: tmp
              mountPath: /tmp
      volumes:
        - name: tmp
          emptyDir:
            medium: Memory
            sizeLimit: 100Mi
```

### Vulnerability Scanning

```bash
# Scan with Trivy
trivy image us-central1-docker.pkg.dev/PROJECT/mcp-servers/my-mcp-server:v1

# Scan with Google's scanner
gcloud artifacts docker images scan \
  us-central1-docker.pkg.dev/PROJECT/mcp-servers/my-mcp-server:v1

# Enable automatic scanning in Artifact Registry
gcloud artifacts repositories update mcp-servers \
  --location=us-central1 \
  --enable-vulnerability-scanning
```

### Security Labels

```dockerfile
# Add security-related labels
LABEL org.opencontainers.image.source="https://github.com/org/repo" \
      org.opencontainers.image.revision="abc123" \
      org.opencontainers.image.created="2024-01-15T10:00:00Z" \
      org.opencontainers.image.licenses="MIT"
```

## Health Checks and Probes

### Dockerfile Health Check

```dockerfile
# Install curl for health checks (debian-based)
RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1
```

### Native Rust Health Check (No curl)

Build a tiny health check binary:

```rust
// src/bin/healthcheck.rs
use std::net::TcpStream;
use std::process::exit;
use std::time::Duration;

fn main() {
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("127.0.0.1:{}", port);

    match TcpStream::connect_timeout(
        &addr.parse().unwrap(),
        Duration::from_secs(2),
    ) {
        Ok(_) => exit(0),
        Err(_) => exit(1),
    }
}
```

```dockerfile
# Copy both binaries
COPY --from=builder /app/target/release/my-mcp-server .
COPY --from=builder /app/target/release/healthcheck .

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["./healthcheck"]
```

### Cloud Run Probes

```yaml
# service.yaml
spec:
  template:
    spec:
      containers:
        - image: my-image
          # Startup probe - gives time for initialization
          startupProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 0
            periodSeconds: 2
            timeoutSeconds: 3
            failureThreshold: 30  # 60 seconds max startup
          # Liveness probe - restart if unhealthy
          livenessProbe:
            httpGet:
              path: /health
              port: 8080
            periodSeconds: 30
            timeoutSeconds: 3
            failureThreshold: 3
```

## Environment Configuration

### Build-Time vs Runtime Configuration

```dockerfile
# Build-time arguments (baked into image)
ARG RUST_VERSION=1.75
ARG BUILD_DATE
ARG GIT_COMMIT

FROM rust:${RUST_VERSION}-slim-bookworm AS builder

# Runtime environment variables (overridable at deploy)
ENV PORT=8080 \
    RUST_LOG=info \
    RUST_BACKTRACE=0

# Labels from build args
LABEL build.date="${BUILD_DATE}" \
      build.commit="${GIT_COMMIT}"
```

### Handling Secrets at Build Time

Never embed secrets in images. Use multi-stage builds to ensure secrets don't leak:

```dockerfile
# BAD - secret in final image
FROM rust:1.75 AS builder
ARG DATABASE_URL
ENV DATABASE_URL=$DATABASE_URL
RUN cargo build --release

# GOOD - secret only in builder, not in final image
FROM rust:1.75 AS builder
# Secret used only during build (e.g., private registry)
RUN --mount=type=secret,id=cargo_token \
    CARGO_REGISTRIES_MY_REGISTRY_TOKEN=$(cat /run/secrets/cargo_token) \
    cargo build --release

# Final image has no secrets
FROM debian:bookworm-slim
COPY --from=builder /app/target/release/my-mcp-server /
CMD ["/my-mcp-server"]
```

Build with secrets:

```bash
docker build --secret id=cargo_token,src=.cargo_token -t my-image .
```

## Local Development with Docker

### Development Dockerfile

```dockerfile
# Dockerfile.dev
FROM rust:1.75-slim-bookworm

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Install development tools
RUN cargo install cargo-watch

WORKDIR /app

# Mount source code, don't copy
VOLUME /app

ENV PORT=8080
EXPOSE 8080

# Auto-reload on changes
CMD ["cargo", "watch", "-x", "run"]
```

### Docker Compose for Development

```yaml
# docker-compose.yml
version: '3.8'

services:
  mcp-server:
    build:
      context: .
      dockerfile: Dockerfile.dev
    ports:
      - "8080:8080"
    volumes:
      - .:/app
      - cargo-cache:/usr/local/cargo/registry
    environment:
      - DATABASE_URL=postgres://postgres:postgres@db:5432/mcp
      - RUST_LOG=debug
    depends_on:
      - db

  db:
    image: postgres:15-alpine
    environment:
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: postgres
      POSTGRES_DB: mcp
    volumes:
      - postgres-data:/var/lib/postgresql/data
    ports:
      - "5432:5432"

volumes:
  cargo-cache:
  postgres-data:
```

```bash
# Start development environment
docker compose up

# Rebuild after dependency changes
docker compose up --build
```

## Build Performance

### Layer Caching Strategy

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Layer Caching Hierarchy                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Layer 1: Base image          (cached across all builds)           │
│     │                                                               │
│     ▼                                                               │
│  Layer 2: System packages     (cached if unchanged)                │
│     │                                                               │
│     ▼                                                               │
│  Layer 3: Cargo dependencies  (cached if Cargo.toml unchanged)     │
│     │                                                               │
│     ▼                                                               │
│  Layer 4: Source code         (rebuilt on code changes)            │
│     │                                                               │
│     ▼                                                               │
│  Layer 5: Final binary        (rebuilt if any above changed)       │
│                                                                     │
│  Key: Structure Dockerfile to maximize cache hits                   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Build Cache with Cloud Build

```yaml
# cloudbuild.yaml with caching
steps:
  - name: 'gcr.io/cloud-builders/docker'
    entrypoint: 'bash'
    args:
      - '-c'
      - |
        docker pull us-central1-docker.pkg.dev/$PROJECT_ID/mcp-servers/my-mcp-server:cache || true
        docker build \
          --cache-from us-central1-docker.pkg.dev/$PROJECT_ID/mcp-servers/my-mcp-server:cache \
          --build-arg BUILDKIT_INLINE_CACHE=1 \
          -t us-central1-docker.pkg.dev/$PROJECT_ID/mcp-servers/my-mcp-server:$COMMIT_SHA \
          -t us-central1-docker.pkg.dev/$PROJECT_ID/mcp-servers/my-mcp-server:cache \
          .

  - name: 'gcr.io/cloud-builders/docker'
    args: ['push', '--all-tags', 'us-central1-docker.pkg.dev/$PROJECT_ID/mcp-servers/my-mcp-server']
```

## Summary

Optimizing containers for Rust MCP servers involves:

1. **Multi-stage builds** - Separate build and runtime environments
2. **Dependency caching** - Use cargo-chef or similar for faster rebuilds
3. **Minimal base images** - scratch, distroless, or alpine
4. **Binary optimization** - LTO, strip symbols, size optimization
5. **Security hardening** - Non-root user, read-only filesystem, vulnerability scanning
6. **Cross-compilation** - Support multiple architectures for cost optimization

Target image sizes:
- **Scratch-based**: 5-15MB
- **Distroless**: 15-30MB
- **Debian-slim**: 30-50MB

The smaller your container, the faster your cold starts and the lower your costs.

## Practice Ideas

These informal exercises help reinforce the concepts.

### Practice 1: Size Reduction Challenge
Take an existing Rust project and create a Dockerfile that produces an image under 20MB.

### Practice 2: Build Time Optimization
Measure build times with and without cargo-chef caching. Document the improvement.

### Practice 3: Multi-Architecture Build
Create a CI/CD pipeline that builds and pushes images for both AMD64 and ARM64.
