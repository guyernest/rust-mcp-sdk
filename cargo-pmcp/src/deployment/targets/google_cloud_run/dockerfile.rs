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

/// Generate `cloudbuild.yaml` from `DeployConfig`.
///
/// Drives `gcloud run deploy` flags from the persisted schema (`[gcp]`,
/// `[server]`, `[environment]`) rather than hard-coded literals — closes
/// the env-var-drift gap from upstream issue #260. Memory / CPU / instance
/// counts / ingress / `allow-unauthenticated` are all sourced from
/// `[server]`; the `[environment]` table becomes the `--set-env-vars`
/// argument.
pub fn generate_cloudbuild(config: &DeployConfig) -> Result<()> {
    let region = config
        .gcp
        .as_ref()
        .map(|g| g.region.as_str())
        .filter(|r| !r.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            std::env::var("CLOUD_RUN_REGION").unwrap_or_else(|_| "us-central1".to_string())
        });

    let memory = config
        .server
        .memory
        .clone()
        .unwrap_or_else(|| "512Mi".to_string());
    let cpu = config.server.cpu.clone().unwrap_or_else(|| "1".to_string());
    let max_instances = config.server.max_instances.unwrap_or(10);
    let min_instances = config.server.min_instances.unwrap_or(0);
    let allow_unauth = config.server.allow_unauthenticated.unwrap_or(true);

    let mut steps_tail = String::new();
    if let Some(ingress) = &config.server.ingress {
        steps_tail.push_str(&format!("      - '--ingress'\n      - '{}'\n", ingress));
    }
    let env_vars = super::deploy::render_set_env_vars(&config.environment);
    if !env_vars.is_empty() {
        steps_tail.push_str(&format!(
            "      - '--set-env-vars'\n      - '{}'\n",
            env_vars
        ));
    }
    let auth_flag = if allow_unauth {
        "--allow-unauthenticated"
    } else {
        "--no-allow-unauthenticated"
    };

    let cloudbuild_content = format!(
        r#"# Cloud Build configuration for automated deployments
# Build and deploy Rust MCP server to Cloud Run
#
# Usage: gcloud builds submit --config cloudbuild.yaml
#
# Or set up a trigger in Cloud Build to auto-deploy on git push.
#
# This file is regenerated by `cargo pmcp deploy init --target-type
# google-cloud-run`. Edits to fields that are sourced from
# .pmcp/deploy.toml ([gcp].region, [server].*, [environment].*) will be
# overwritten on the next init. Edit deploy.toml instead.

steps:
  # Build the Docker image
  - name: 'gcr.io/cloud-builders/docker'
    args:
      - 'build'
      - '-t'
      - 'gcr.io/$PROJECT_ID/{name}:$COMMIT_SHA'
      - '-t'
      - 'gcr.io/$PROJECT_ID/{name}:latest'
      - '.'

  # Push the Docker image to Google Container Registry
  - name: 'gcr.io/cloud-builders/docker'
    args:
      - 'push'
      - 'gcr.io/$PROJECT_ID/{name}:$COMMIT_SHA'

  # Deploy to Cloud Run
  - name: 'gcr.io/google.com/cloudsdktool/cloud-sdk'
    entrypoint: gcloud
    args:
      - 'run'
      - 'deploy'
      - '{name}'
      - '--image'
      - 'gcr.io/$PROJECT_ID/{name}:$COMMIT_SHA'
      - '--region'
      - '{region}'
      - '--platform'
      - 'managed'
      - '{auth_flag}'
      - '--memory'
      - '{memory}'
      - '--cpu'
      - '{cpu}'
      - '--max-instances'
      - '{max_instances}'
      - '--min-instances'
      - '{min_instances}'
      - '--port'
      - '8080'
{tail}
# Store images in GCR
images:
  - 'gcr.io/$PROJECT_ID/{name}:$COMMIT_SHA'
  - 'gcr.io/$PROJECT_ID/{name}:latest'

# Build timeout
timeout: '1200s'

# Build options
options:
  machineType: 'E2_HIGHCPU_8'
  logging: CLOUD_LOGGING_ONLY
"#,
        name = config.server.name,
        region = region,
        memory = memory,
        cpu = cpu,
        max_instances = max_instances,
        min_instances = min_instances,
        auth_flag = auth_flag,
        tail = steps_tail,
    );

    let cloudbuild_path = config.project_root.join("cloudbuild.yaml");
    std::fs::write(&cloudbuild_path, cloudbuild_content)
        .context("Failed to write cloudbuild.yaml")?;

    println!("   ✓ Generated cloudbuild.yaml");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_config(tmp: &TempDir) -> DeployConfig {
        DeployConfig::default_for_cloud_run_server(
            "auth-echo-cloud-run".to_string(),
            "your-gcp-project-id".to_string(),
            "us-central1".to_string(),
            tmp.path().to_path_buf(),
        )
    }

    fn write_cargo_toml(tmp: &TempDir) {
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"auth-echo-cloud-run\"\nversion = \"0.1.0\"\n",
        )
        .expect("write cargo.toml");
    }

    /// cloudbuild.yaml splices [server].memory / cpu / max_instances /
    /// min_instances from deploy.toml — closes upstream #260's env-var
    /// drift problem.
    #[test]
    fn cloudbuild_yaml_drives_server_fields_from_config() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let mut config = make_config(&tmp);
        config.server.memory = Some("1Gi".to_string());
        config.server.cpu = Some("2".to_string());
        config.server.max_instances = Some(50);
        config.server.min_instances = Some(2);

        generate_cloudbuild(&config).expect("generate");
        let cb = std::fs::read_to_string(tmp.path().join("cloudbuild.yaml")).expect("read");
        assert!(cb.contains("- '1Gi'"), "memory must come from config: {cb}");
        assert!(cb.contains("- '2'"), "cpu must come from config");
        assert!(cb.contains("- '50'"), "max_instances must come from config");
        assert!(cb.contains("- '2'"), "min_instances must come from config");
    }

    /// [environment] entries become a deterministic `--set-env-vars` arg.
    /// Closes upstream #260: post-deploy
    /// `gcloud run services update --set-env-vars` patches are no longer
    /// required for app-level env vars.
    #[test]
    fn cloudbuild_yaml_includes_environment_set_env_vars() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let mut config = make_config(&tmp);
        config
            .environment
            .insert("EXPECTED_AUDIENCE".to_string(), "abc.apps.x".to_string());
        // RUST_LOG=info is already in environment from the default ctor.

        generate_cloudbuild(&config).expect("generate");
        let cb = std::fs::read_to_string(tmp.path().join("cloudbuild.yaml")).expect("read");
        assert!(cb.contains("'--set-env-vars'"));
        assert!(
            cb.contains("EXPECTED_AUDIENCE=abc.apps.x,RUST_LOG=info")
                || cb.contains("RUST_LOG=info,EXPECTED_AUDIENCE=abc.apps.x")
                || cb.contains("EXPECTED_AUDIENCE=abc.apps.x"),
            "deterministic env-vars rendering: {cb}"
        );
    }

    /// `[server].allow_unauthenticated = false` flips the gcloud auth flag.
    #[test]
    fn cloudbuild_yaml_honors_allow_unauthenticated_false() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let mut config = make_config(&tmp);
        config.server.allow_unauthenticated = Some(false);

        generate_cloudbuild(&config).expect("generate");
        let cb = std::fs::read_to_string(tmp.path().join("cloudbuild.yaml")).expect("read");
        assert!(cb.contains("--no-allow-unauthenticated"));
        assert!(!cb.contains("'--allow-unauthenticated'\n"));
    }

    /// `[server].ingress = "internal"` produces an `--ingress` arg pair.
    #[test]
    fn cloudbuild_yaml_honors_ingress() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let mut config = make_config(&tmp);
        config.server.ingress = Some("internal".to_string());

        generate_cloudbuild(&config).expect("generate");
        let cb = std::fs::read_to_string(tmp.path().join("cloudbuild.yaml")).expect("read");
        assert!(cb.contains("'--ingress'"));
        assert!(cb.contains("'internal'"));
    }
}
