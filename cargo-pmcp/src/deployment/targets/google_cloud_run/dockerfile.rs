use crate::deployment::DeployConfig;
use anyhow::{Context, Result};

/// Generate optimized Dockerfile for Rust MCP server on Cloud Run
pub fn generate_dockerfile(config: &DeployConfig) -> Result<()> {
    // Detect if this is a workspace or simple binary crate
    let cargo_toml_path = config.project_root.join("Cargo.toml");
    let cargo_toml =
        std::fs::read_to_string(&cargo_toml_path).context("Failed to read Cargo.toml")?;

    let is_workspace = cargo_toml.contains("[workspace]");

    let dockerfile_content = if is_workspace {
        // Workspace-based Dockerfile - builds inside Docker
        format!(
            r#"# Multi-stage Dockerfile for Rust MCP Server on Google Cloud Run
# Workspace project structure - builds inside Docker to handle path dependencies

# Stage 1: Build the Rust binary
FROM rust:1.83-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy the entire project (including path dependencies)
COPY . .

# Build the release binary (exclude Lambda packages to avoid binary name collisions)
# Find all packages with 'lambda' in the name and exclude them
RUN LAMBDA_PKGS=$(cargo metadata --no-deps --format-version=1 2>/dev/null | \
    grep -o '"name":"[^"]*lambda[^"]*"' | \
    sed 's/"name":"\([^"]*\)"/--exclude \1/g' || echo ""); \
    cargo build --release --workspace $LAMBDA_PKGS

# Find the server binary (exclude Lambda binaries)
# This finds the actual executable binary for HTTP server
RUN find target/release -maxdepth 1 -type f -executable \
    ! -name "*lambda*" ! -name "*-lambda" \
    ! -name "*.so" ! -name "*.d" ! -name "build-script-*" \
    -exec cp {{}} /app/mcp-server \; || \
    (echo "No server binary found in target/release" && exit 1)

# Stage 2: Create minimal runtime image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 mcpserver

# Copy binary from builder
COPY --from=builder /app/mcp-server /usr/local/bin/mcp-server

# Ensure binary is executable
RUN chmod +x /usr/local/bin/mcp-server

# Change to non-root user
USER mcpserver

# Set up working directory
WORKDIR /home/mcpserver

# Cloud Run will set the PORT environment variable
# Default to 8080 if not set
ENV PORT=8080

# Expose the port
EXPOSE $PORT

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:$PORT/health || exit 1

# Run the MCP server
# Cloud Run expects the server to bind to 0.0.0.0:$PORT
CMD ["/usr/local/bin/mcp-server"]
"#
        )
    } else {
        // Simple binary crate Dockerfile - builds inside Docker
        r#"# Multi-stage Dockerfile for Rust MCP Server on Google Cloud Run
# Simple binary crate - builds inside Docker

# Stage 1: Build the Rust binary
FROM rust:1.83-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy project files
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/

# Build the release binary
RUN cargo build --release

# Copy the server binary (exclude Lambda binaries)
RUN find target/release -maxdepth 1 -type f -executable \
    ! -name "*lambda*" ! -name "*-lambda" \
    ! -name "*.so" ! -name "*.d" ! -name "build-script-*" \
    -exec cp {} /app/mcp-server \; || \
    (echo "No server binary found in target/release" && exit 1)

# Stage 2: Create minimal runtime image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 mcpserver

# Copy binary from builder
COPY --from=builder /app/mcp-server /usr/local/bin/mcp-server

# Ensure binary is executable
RUN chmod +x /usr/local/bin/mcp-server

# Change to non-root user
USER mcpserver

# Set up working directory
WORKDIR /home/mcpserver

# Cloud Run will set the PORT environment variable
# Default to 8080 if not set
ENV PORT=8080

# Expose the port
EXPOSE $PORT

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:$PORT/health || exit 1

# Run the MCP server
# Cloud Run expects the server to bind to 0.0.0.0:$PORT
CMD ["/usr/local/bin/mcp-server"]
"#
        .to_string()
    };

    let dockerfile_path = config.project_root.join("Dockerfile");
    std::fs::write(&dockerfile_path, dockerfile_content).context("Failed to write Dockerfile")?;

    println!("   ✓ Generated Dockerfile");

    Ok(())
}

/// Generate .dockerignore for optimal build context
pub fn generate_dockerignore(config: &DeployConfig) -> Result<()> {
    let dockerignore_content = r#"# Rust build artifacts
target/debug/
target/release/.fingerprint/
target/release/build/
target/release/deps/
target/release/examples/
target/release/incremental/
target/release/*.d
target/release/*.rlib
**/*.rs.bk
*.pdb

# IDE files
.vscode/
.idea/
*.swp
*.swo
*~

# Git
.git/
.gitignore

# Documentation
README.md
docs/

# Test files
tests/
benches/

# CI/CD
.github/
.gitlab-ci.yml

# Deploy artifacts (CDK, Lambda, etc.)
deploy/
cdk.out/
.pmcp/
bootstrap

# Vendored dependencies (not needed with crates.io)
vendor/

# OS files
.DS_Store
Thumbs.db

# Logs
*.log

# Environment files
.env
.env.local
"#;

    let dockerignore_path = config.project_root.join(".dockerignore");
    std::fs::write(&dockerignore_path, dockerignore_content)
        .context("Failed to write .dockerignore")?;

    println!("   ✓ Generated .dockerignore");

    Ok(())
}

/// Generate optional Cloud Build configuration
pub fn generate_cloudbuild(config: &DeployConfig) -> Result<()> {
    let region = std::env::var("CLOUD_RUN_REGION").unwrap_or_else(|_| "us-central1".to_string());

    let cloudbuild_content = format!(
        r#"# Cloud Build configuration for automated deployments
# Build and deploy Rust MCP server to Cloud Run
#
# Usage: gcloud builds submit --config cloudbuild.yaml
#
# Or set up a trigger in Cloud Build to auto-deploy on git push

steps:
  # Build the Rust binary locally (to handle path dependencies)
  - name: 'rust:1.83-slim'
    entrypoint: bash
    args:
      - '-c'
      - |
        apt-get update && apt-get install -y pkg-config libssl-dev
        cargo build --release

  # Build the Docker image with the pre-built binary
  - name: 'gcr.io/cloud-builders/docker'
    args:
      - 'build'
      - '-t'
      - 'gcr.io/$PROJECT_ID/{}:$COMMIT_SHA'
      - '-t'
      - 'gcr.io/$PROJECT_ID/{}:latest'
      - '.'

  # Push the Docker image to Google Container Registry
  - name: 'gcr.io/cloud-builders/docker'
    args:
      - 'push'
      - 'gcr.io/$PROJECT_ID/{}:$COMMIT_SHA'

  # Deploy to Cloud Run
  - name: 'gcr.io/google.com/cloudsdktool/cloud-sdk'
    entrypoint: gcloud
    args:
      - 'run'
      - 'deploy'
      - '{}'
      - '--image'
      - 'gcr.io/$PROJECT_ID/{}:$COMMIT_SHA'
      - '--region'
      - '{}'
      - '--platform'
      - 'managed'
      - '--allow-unauthenticated'
      - '--memory'
      - '512Mi'
      - '--cpu'
      - '1'
      - '--max-instances'
      - '10'
      - '--min-instances'
      - '0'
      - '--port'
      - '8080'

# Store images in GCR
images:
  - 'gcr.io/$PROJECT_ID/{}:$COMMIT_SHA'
  - 'gcr.io/$PROJECT_ID/{}:latest'

# Build timeout
timeout: '1200s'

# Build options
options:
  machineType: 'E2_HIGHCPU_8'
  logging: CLOUD_LOGGING_ONLY
"#,
        config.server.name,
        config.server.name,
        config.server.name,
        config.server.name,
        config.server.name,
        region,
        config.server.name,
        config.server.name,
    );

    let cloudbuild_path = config.project_root.join("cloudbuild.yaml");
    std::fs::write(&cloudbuild_path, cloudbuild_content)
        .context("Failed to write cloudbuild.yaml")?;

    println!("   ✓ Generated cloudbuild.yaml");

    Ok(())
}
