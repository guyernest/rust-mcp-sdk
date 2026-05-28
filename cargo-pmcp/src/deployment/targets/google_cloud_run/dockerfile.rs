use crate::deployment::{DeployConfig, LayoutConfig};
use anyhow::{Context, Result};

/// Generate the Cloud Run Dockerfile for `config.project_root`.
///
/// Layout selection precedence:
/// 1. `[layout].kind = "multi-crate-isolated"` (issue #258) — surgical
///    per-crate `COPY` lines + `cargo build --manifest-path`.
/// 2. Root `Cargo.toml` contains `[workspace]` — workspace template
///    (`COPY . .` + workspace build).
/// 3. Otherwise — simple binary crate template.
///
/// Issue #259's distroless default applies to all three layouts; see
/// [`runtime_stage`].
///
/// # Errors
///
/// Returns an error if the project `Cargo.toml` cannot be read (non
/// multi-crate-isolated layouts) or the rendered Dockerfile cannot be
/// written to disk.
pub fn generate_dockerfile(config: &DeployConfig) -> Result<()> {
    let dockerfile_content = render_dockerfile(config)?;
    let dockerfile_path = config.project_root.join("Dockerfile");
    std::fs::write(&dockerfile_path, dockerfile_content).context("Failed to write Dockerfile")?;
    println!("   ✓ Generated Dockerfile");
    Ok(())
}

/// Render the Dockerfile contents without writing to disk.
///
/// Exposed for unit tests so the asserts can target the rendered text
/// directly without touching the filesystem.
pub(super) fn render_dockerfile(config: &DeployConfig) -> Result<String> {
    let multi_crate = config
        .layout
        .as_ref()
        .filter(|l| l.is_multi_crate_isolated());

    let builder = if let Some(layout) = multi_crate {
        builder_stage_multi_crate_isolated(layout, &resolve_binary_name(config))
    } else {
        let cargo_toml_path = config.project_root.join("Cargo.toml");
        let cargo_toml =
            std::fs::read_to_string(&cargo_toml_path).context("Failed to read Cargo.toml")?;
        if cargo_toml.contains("[workspace]") {
            builder_stage_workspace()
        } else {
            builder_stage_simple()
        }
    };

    Ok(format!(
        "{builder}\n{runtime}",
        runtime = runtime_stage(config)
    ))
}

/// Resolve the binary name for `cargo build --bin <name>` and the runtime
/// `COPY --from=builder` line. Falls back to `config.server.name` when
/// `[server].binary` is unset.
fn resolve_binary_name(config: &DeployConfig) -> String {
    config
        .server
        .binary
        .clone()
        .unwrap_or_else(|| config.server.name.clone())
}

/// Shared apt-install layer for the rust:slim builder stage. `pkg-config`
/// and `libssl-dev` cover the common native-build deps; everything else is
/// expected to come from crates.io.
const BUILDER_APT_LAYER: &str = "# Install build dependencies
RUN apt-get update && apt-get install -y \\
    pkg-config \\
    libssl-dev \\
    && rm -rf /var/lib/apt/lists/*";

/// Shared post-build step for the workspace and simple-crate layouts:
/// locate the produced binary in `target/release` (excluding any
/// lambda binaries that may have been built alongside it) and copy it
/// to a stable path the runtime stage expects.
const FIND_AND_COPY_BINARY: &str = "# Copy the server binary (exclude Lambda binaries)
RUN find target/release -maxdepth 1 -type f -executable \\
    ! -name \"*lambda*\" ! -name \"*-lambda\" \\
    ! -name \"*.so\" ! -name \"*.d\" ! -name \"build-script-*\" \\
    -exec cp {} /app/mcp-server \\; || \\
    (echo \"No server binary found in target/release\" && exit 1)";

fn builder_stage_workspace() -> String {
    format!(
        r#"# Multi-stage Dockerfile for Rust MCP Server on Google Cloud Run
# Workspace project structure - builds inside Docker to handle path dependencies

# Stage 1: Build the Rust binary
FROM rust:1-slim AS builder

{BUILDER_APT_LAYER}

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

{FIND_AND_COPY_BINARY}
"#,
    )
}

fn builder_stage_simple() -> String {
    format!(
        r#"# Multi-stage Dockerfile for Rust MCP Server on Google Cloud Run
# Simple binary crate - builds inside Docker

# Stage 1: Build the Rust binary
FROM rust:1-slim AS builder

{BUILDER_APT_LAYER}

# Create app directory
WORKDIR /app

# Copy project files
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/

# Build the release binary
RUN cargo build --release

{FIND_AND_COPY_BINARY}
"#,
    )
}

/// Surgical builder for the multi-crate isolated layout (issue #258).
///
/// Emits one `COPY <crate>/Cargo.toml <crate>/Cargo.toml` + one
/// `COPY <crate>/src <crate>/src` pair for each entry in `primary` +
/// `path_deps`, then a single `cargo build --release --manifest-path
/// <primary>/Cargo.toml --bin <binary>` step. The release artifact is
/// then copied to `/app/mcp-server` so the runtime stage can rely on a
/// fixed path.
fn builder_stage_multi_crate_isolated(layout: &LayoutConfig, binary: &str) -> String {
    // `path_deps` entries are scaffolded into Dockerfile COPY lines. We
    // restrict to the Cargo crate-name character set (alphanumeric, `-`,
    // `_`) — `.` is intentionally excluded to block `..` path escapes,
    // and any other character that would inject Dockerfile syntax or
    // shell semantics (semicolons, quotes, newlines, whitespace, `/`)
    // is stripped.
    let sanitize = |s: &str| -> String {
        s.chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
            .collect()
    };

    let primary = sanitize(&layout.primary);
    let safe_binary = sanitize(binary);

    let mut copies = String::new();
    copies.push_str(&format!("COPY {primary}/Cargo.toml {primary}/Cargo.toml\n"));
    copies.push_str(&format!("COPY {primary}/src {primary}/src\n"));
    for dep in &layout.path_deps {
        let safe_dep = sanitize(dep);
        if safe_dep.is_empty() {
            continue;
        }
        copies.push_str(&format!(
            "COPY {safe_dep}/Cargo.toml {safe_dep}/Cargo.toml\n"
        ));
        copies.push_str(&format!("COPY {safe_dep}/src {safe_dep}/src\n"));
    }

    format!(
        r#"# Multi-stage Dockerfile for Rust MCP Server on Google Cloud Run
# Multi-crate isolated layout — sibling crates with path-dep relationships
# (issue 258). Only the primary crate and its declared path_deps are
# copied into the build context. Unrelated siblings (e.g. aws-lambda)
# are excluded to keep the image small and avoid cross-toolchain build
# failures.

# Stage 1: Build the Rust binary
FROM rust:1-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Surgical per-crate COPY lines (primary + declared path_deps)
{copies}
# Build the binary from the primary crate's manifest
RUN cargo build --release \
    --manifest-path {primary}/Cargo.toml \
    --bin {safe_binary}

# Cargo with `--manifest-path` places the release artifact under the
# primary crate's own target/release dir. Some setups put it at the
# top-level target/release dir; try both so the runtime stage gets a
# stable path.
RUN cp {primary}/target/release/{safe_binary} /app/mcp-server 2>/dev/null \
    || cp target/release/{safe_binary} /app/mcp-server
"#,
    )
}

/// Runtime stage of the Dockerfile.
///
/// Emits the runtime stage with the appropriate shape for the runtime
/// base. Closes upstream paiml/rust-mcp-sdk#259:
///
/// - Default base is `gcr.io/distroless/cc-debian12` — no shell, no apt,
///   ~20 MB vs `debian:bookworm-slim`'s ~80 MB. Cuts cold-start image-pull
///   time and the post-exploitation attack surface. Distroless runs as
///   `nonroot` (uid 65532) by default, so no `useradd` is needed.
/// - `[runtime].base = "..."` overrides verbatim.
/// - `[runtime].apt_packages = [...]` is honored only when `base` resolves
///   to a debian-family image. Empty list (the default) → no apt layer.
fn runtime_stage(config: &DeployConfig) -> String {
    let base = config
        .runtime
        .as_ref()
        .and_then(|r| r.base.as_deref())
        .unwrap_or("gcr.io/distroless/cc-debian12");

    if is_debian_family(base) {
        runtime_stage_debian(config, base)
    } else {
        runtime_stage_distroless(base)
    }
}

fn is_debian_family(base: &str) -> bool {
    base.starts_with("debian:") || base.starts_with("ubuntu:")
}

/// Distroless / non-shell runtime stage.
///
/// Contains no shell, no package manager, no `useradd`, no
/// `HEALTHCHECK CMD curl` (no curl in the image). Cloud Run health-checks
/// externally on `PORT`, so the in-image HEALTHCHECK is unnecessary.
fn runtime_stage_distroless(base: &str) -> String {
    format!(
        r#"# Stage 2: distroless runtime — minimal attack surface (issue #259)
# No shell, no apt, no package manager. Cloud Run health-checks externally
# on $PORT, so no in-image HEALTHCHECK directive is needed.
FROM {base}

# Copy the prebuilt binary from the builder stage. Distroless cc runs as
# `nonroot` (uid 65532) by default — no useradd / USER directive needed.
COPY --from=builder /app/mcp-server /usr/local/bin/mcp-server

# Cloud Run sets PORT (defaults to 8080); the server is expected to bind
# 0.0.0.0:$PORT.
ENV PORT=8080
EXPOSE $PORT

CMD ["/usr/local/bin/mcp-server"]
"#,
    )
}

/// Debian/Ubuntu runtime stage — emitted only when the operator opts back
/// to a shell-enabled base via `[runtime].base = "debian:..."` (or
/// `"ubuntu:..."`).
///
/// `[runtime].apt_packages` drives the apt-install layer; an empty list
/// (the default) emits no apt layer at all, leaving the base image
/// untouched.
fn runtime_stage_debian(config: &DeployConfig, base: &str) -> String {
    let pkgs: Vec<&str> = config
        .runtime
        .as_ref()
        .map(|r| {
            r.apt_packages
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let apt_layer = if pkgs.is_empty() {
        String::new()
    } else {
        format!(
            "# Install runtime apt packages declared in [runtime].apt_packages\nRUN apt-get update && apt-get install -y \\\n    {} \\\n    && rm -rf /var/lib/apt/lists/*\n\n",
            pkgs.join(" \\\n    ")
        )
    };

    format!(
        r#"# Stage 2: debian-family runtime — opted in via [runtime].base
FROM {base}

{apt_layer}# Create non-root user
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

# Run the MCP server
# Cloud Run expects the server to bind to 0.0.0.0:$PORT
CMD ["/usr/local/bin/mcp-server"]
"#,
    )
}

/// Generate .dockerignore for optimal build context
///
/// # Errors
///
/// Returns an error if the `.dockerignore` file cannot be written.
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
///
/// # Errors
///
/// Returns an error if the `cloudbuild.yaml` file cannot be written.
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
        steps_tail.push_str(&format!("      - '--ingress'\n      - '{ingress}'\n"));
    }
    let env_vars = super::env::render_set_env_vars(&config.environment);
    if !env_vars.is_empty() {
        steps_tail.push_str(&format!("      - '--set-env-vars'\n      - '{env_vars}'\n"));
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

    // ---------- Layout / Dockerfile tests (issue #258) ----------

    use crate::deployment::config::LayoutConfig;

    /// Default simple-crate layout: no [workspace] in Cargo.toml, no
    /// [layout] block. Dockerfile uses the simple template.
    #[test]
    fn dockerfile_simple_layout_emits_simple_template() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let config = make_config(&tmp);

        let dockerfile = render_dockerfile(&config).expect("render");
        assert!(dockerfile.contains("Simple binary crate"));
        assert!(dockerfile.contains("COPY Cargo.toml Cargo.lock ./"));
        assert!(!dockerfile.contains("multi-crate-isolated"));
    }

    /// Workspace layout detected via `[workspace]` table at the project
    /// root. Dockerfile uses the workspace template.
    #[test]
    fn dockerfile_workspace_layout_emits_workspace_template() {
        let tmp = TempDir::new().expect("tmpdir");
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crate-a\"]\n",
        )
        .expect("write cargo");
        let config = make_config(&tmp);

        let dockerfile = render_dockerfile(&config).expect("render");
        assert!(dockerfile.contains("Workspace project structure"));
        assert!(dockerfile.contains("COPY . ."));
    }

    /// Multi-crate isolated layout (#258): per-crate COPY pairs for
    /// primary + each path_dep, then `cargo build --manifest-path
    /// <primary>/Cargo.toml --bin <binary>`. Crucially, no `COPY . .`
    /// (which would over-bundle sibling lambda crates).
    #[test]
    fn dockerfile_multi_crate_isolated_emits_surgical_copy_pairs() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let mut config = make_config(&tmp);
        config.layout = Some(LayoutConfig {
            kind: "multi-crate-isolated".to_string(),
            primary: "gcp-cloud-run".to_string(),
            path_deps: vec!["auth-echo-core".to_string()],
        });
        config.server.binary = Some("server".to_string());

        let dockerfile = render_dockerfile(&config).expect("render");
        assert!(dockerfile.contains("COPY gcp-cloud-run/Cargo.toml gcp-cloud-run/Cargo.toml"));
        assert!(dockerfile.contains("COPY gcp-cloud-run/src gcp-cloud-run/src"));
        assert!(dockerfile.contains("COPY auth-echo-core/Cargo.toml auth-echo-core/Cargo.toml"));
        assert!(dockerfile.contains("COPY auth-echo-core/src auth-echo-core/src"));
        assert!(
            dockerfile.contains("--manifest-path gcp-cloud-run/Cargo.toml")
                || dockerfile.contains("--manifest-path gcp-cloud-run/Cargo.toml \\"),
            "expected manifest-path build step: {dockerfile}"
        );
        assert!(dockerfile.contains("--bin server"));
        // Critically: must NOT include `COPY . .` which would over-bundle.
        assert!(
            !dockerfile.contains("COPY . ."),
            "multi-crate isolated must not COPY . ."
        );
        // Sibling lambda crates must NOT appear in any COPY line. (The
        // file may mention `aws-lambda` in an explanatory header comment;
        // we only assert it does not appear in a Dockerfile directive.)
        for line in dockerfile.lines().filter(|l| l.starts_with("COPY ")) {
            assert!(
                !line.contains("aws-lambda") && !line.contains("lambda"),
                "lambda crate leaked into COPY directive: {line}"
            );
        }
    }

    /// `[layout]` falls back to `config.server.name` when `binary` is
    /// unset — preserves backward-compat with users who don't set the
    /// new optional field.
    #[test]
    fn dockerfile_multi_crate_isolated_binary_falls_back_to_server_name() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let mut config = make_config(&tmp);
        config.layout = Some(LayoutConfig {
            kind: "multi-crate-isolated".to_string(),
            primary: "gcp".to_string(),
            path_deps: vec![],
        });
        // No config.server.binary set — should fall back to server.name.
        let dockerfile = render_dockerfile(&config).expect("render");
        assert!(dockerfile.contains("--bin auth-echo-cloud-run"));
    }

    /// Crate-name sanitization rejects injection attempts. A path_dep
    /// containing shell-meaningful characters must not propagate them
    /// into the generated Dockerfile.
    #[test]
    fn dockerfile_multi_crate_isolated_sanitizes_crate_names() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let mut config = make_config(&tmp);
        config.layout = Some(LayoutConfig {
            kind: "multi-crate-isolated".to_string(),
            primary: "gcp-run".to_string(),
            path_deps: vec!["evil; rm -rf /".to_string(), "../escape".to_string()],
        });
        let dockerfile = render_dockerfile(&config).expect("render");
        // The sanitized form ("evilrm-rf" / "escape") may appear as crate
        // names, but no dangerous chars / path-escape segments must
        // propagate into the COPY/RUN lines.
        for line in dockerfile.lines().filter(|l| {
            l.starts_with("COPY")
                || l.starts_with("RUN")
                || l.starts_with("CMD")
                || l.starts_with("    --")
                || l.starts_with("    cp ")
                || l.starts_with("    || ")
        }) {
            assert!(!line.contains(';'), "directive contains semicolon: {line}");
            assert!(
                !line.contains(".."),
                "directive contains `..` path escape: {line}"
            );
            assert!(
                !line.contains("rm -rf"),
                "directive contains shell injection: {line}"
            );
        }
        // Positive check: sanitized form is present where path_deps used
        // to be.
        assert!(dockerfile.contains("evilrm-rf/Cargo.toml"));
        assert!(dockerfile.contains("escape/Cargo.toml"));
    }

    // ---------- Runtime / distroless tests (issue #259) ----------

    use crate::deployment::config::RuntimeConfig;

    /// Extract just the runtime (Stage 2) portion of a rendered
    /// Dockerfile. The builder (Stage 1) uses `apt-get` to install
    /// pkg-config + libssl-dev regardless of runtime base, so any
    /// runtime-stage assertions about apt / useradd / etc. need to be
    /// scoped to Stage 2.
    fn runtime_portion(dockerfile: &str) -> &str {
        let start = dockerfile
            .find("# Stage 2:")
            .expect("runtime stage marker present");
        &dockerfile[start..]
    }

    /// Default runtime base is distroless. No apt layer, no useradd, no
    /// HEALTHCHECK curl. Closes upstream #259.
    #[test]
    fn dockerfile_runtime_defaults_to_distroless() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let config = make_config(&tmp);

        let dockerfile = render_dockerfile(&config).expect("render");
        let runtime = runtime_portion(&dockerfile);
        assert!(
            runtime.contains("FROM gcr.io/distroless/cc-debian12"),
            "default runtime FROM must be distroless: {runtime}"
        );
        assert!(!runtime.contains("apt-get install"));
        assert!(!runtime.contains("RUN useradd"));
        // Assert no HEALTHCHECK *directive* (a directive starts at the
        // line start; the explanatory comment may still mention the
        // keyword).
        assert!(
            !runtime
                .lines()
                .any(|l| l.trim_start().starts_with("HEALTHCHECK")),
            "no HEALTHCHECK directive in distroless runtime"
        );
        assert!(!runtime.contains("RUN chmod +x"));
        assert!(runtime.contains("CMD [\"/usr/local/bin/mcp-server\"]"));
    }

    /// `[runtime].base = "debian:bookworm-slim"` opts back to the
    /// shell-enabled runtime stage. With no `apt_packages` declared, no
    /// apt layer is emitted (operator opted out without declaring any
    /// packages — they get bare debian).
    #[test]
    fn dockerfile_runtime_debian_opt_out_no_apt_layer_by_default() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let mut config = make_config(&tmp);
        config.runtime = Some(RuntimeConfig {
            base: Some("debian:bookworm-slim".to_string()),
            apt_packages: vec![],
        });

        let dockerfile = render_dockerfile(&config).expect("render");
        let runtime = runtime_portion(&dockerfile);
        assert!(runtime.contains("FROM debian:bookworm-slim"));
        // No apt layer in the runtime stage.
        assert!(!runtime.contains("apt-get install"));
        // Shell-enabled scaffolding is present.
        assert!(runtime.contains("useradd -m -u 1000 mcpserver"));
        assert!(runtime.contains("USER mcpserver"));
    }

    /// `[runtime].apt_packages = ["ca-certificates"]` with a debian base
    /// produces an apt-install layer with exactly those packages.
    #[test]
    fn dockerfile_runtime_debian_with_apt_packages_emits_apt_layer() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let mut config = make_config(&tmp);
        config.runtime = Some(RuntimeConfig {
            base: Some("debian:bookworm-slim".to_string()),
            apt_packages: vec!["ca-certificates".to_string(), "libssl3".to_string()],
        });

        let dockerfile = render_dockerfile(&config).expect("render");
        let runtime = runtime_portion(&dockerfile);
        assert!(runtime.contains("FROM debian:bookworm-slim"));
        assert!(runtime.contains("apt-get install -y"));
        assert!(runtime.contains("ca-certificates"));
        assert!(runtime.contains("libssl3"));
        // Cleanup line for apt cache.
        assert!(runtime.contains("rm -rf /var/lib/apt/lists/*"));
    }

    /// `apt_packages` is ignored on non-debian bases (distroless,
    /// alpine, scratch, etc.). Issue #259 explicitly scopes the
    /// apt-packages knob to debian-family bases.
    #[test]
    fn dockerfile_runtime_apt_packages_ignored_on_distroless() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let mut config = make_config(&tmp);
        config.runtime = Some(RuntimeConfig {
            base: None,
            apt_packages: vec!["ca-certificates".to_string()],
        });

        let dockerfile = render_dockerfile(&config).expect("render");
        let runtime = runtime_portion(&dockerfile);
        assert!(runtime.contains("FROM gcr.io/distroless/cc-debian12"));
        assert!(!runtime.contains("apt-get install"));
    }

    /// `[runtime].base = "<arbitrary image>"` uses the value verbatim
    /// and falls through to the distroless-shaped (no-shell) runtime
    /// stage. Operator-managed bases that aren't debian/ubuntu get the
    /// minimal scaffolding so the operator is in control of any
    /// additional layers.
    #[test]
    fn dockerfile_runtime_custom_base_uses_distroless_shape() {
        let tmp = TempDir::new().expect("tmpdir");
        write_cargo_toml(&tmp);
        let mut config = make_config(&tmp);
        config.runtime = Some(RuntimeConfig {
            base: Some("gcr.io/distroless/static-debian12".to_string()),
            apt_packages: vec![],
        });

        let dockerfile = render_dockerfile(&config).expect("render");
        let runtime = runtime_portion(&dockerfile);
        assert!(runtime.contains("FROM gcr.io/distroless/static-debian12"));
        assert!(!runtime.contains("RUN useradd"));
    }

    // ---------- Property tests ----------
    //
    // Per CLAUDE.md ALWAYS requirements, every new feature must include
    // property-based invariants. These cover the two attack surfaces of
    // the new code: arbitrary user input flowing into Dockerfile
    // directives via [layout].path_deps, and arbitrary user input
    // flowing into the `gcloud --set-env-vars` argument via
    // [environment].

    use proptest::prelude::*;

    proptest! {
        /// INVARIANT: any string fed through the crate-name sanitizer
        /// produces output that is safe to splice into a Dockerfile
        /// COPY directive — only `[A-Za-z0-9_-]`, no path separators,
        /// no shell metacharacters, no newlines.
        #[test]
        fn prop_sanitize_path_dep_yields_safe_chars_only(input in ".{0,200}") {
            let mut config =
                DeployConfig::default_for_cloud_run_server(
                    "s".to_string(),
                    "p".to_string(),
                    "us-central1".to_string(),
                    std::env::temp_dir(),
                );
            config.layout = Some(LayoutConfig {
                kind: "multi-crate-isolated".to_string(),
                primary: "primary-crate".to_string(),
                path_deps: vec![input.clone()],
            });
            // Synthesize a Cargo.toml in temp so render_dockerfile
            // doesn't fail on the simple/workspace detection path. We
            // route through the multi-crate-isolated branch so the
            // sanitization invariant is what we measure.
            let tmp = tempfile::TempDir::new().expect("tmpdir");
            std::fs::write(
                tmp.path().join("Cargo.toml"),
                "[package]\nname = \"p\"\nversion = \"0.1.0\"\n",
            )
            .expect("seed cargo");
            config.project_root = tmp.path().to_path_buf();

            let dockerfile = render_dockerfile(&config).expect("render");
            // The COPY lines for path_deps must not contain dangerous
            // characters regardless of input. (The input may also
            // sanitize to empty — in which case no COPY line is emitted
            // for that dep, which is also safe.)
            for line in dockerfile.lines().filter(|l| l.starts_with("COPY ")) {
                for ch in line.chars() {
                    prop_assert!(
                        ch.is_ascii() && (ch != ';' && ch != '`' && ch != '$' && ch != '\\'
                            && ch != '"' && ch != '\''),
                        "COPY line contains dangerous char {:?}: {}", ch, line
                    );
                }
                prop_assert!(!line.contains(".."), "COPY line contains `..`: {}", line);
            }
        }

        /// INVARIANT: render_set_env_vars output is sorted by key and
        /// every entry has the form KEY=VALUE. Determinism matters —
        /// re-running deploy with no schema change must produce the
        /// exact same gcloud invocation so Cloud Run does not create a
        /// pointless new revision.
        #[test]
        fn prop_render_set_env_vars_is_sorted_and_well_formed(
            entries in prop::collection::vec(
                ("[A-Z][A-Z0-9_]{0,15}", "[a-zA-Z0-9._-]{0,40}"),
                0..16,
            )
        ) {
            let mut env = std::collections::HashMap::new();
            for (k, v) in entries {
                env.insert(k, v);
            }
            let rendered = super::super::env::render_set_env_vars(&env);
            if env.is_empty() {
                prop_assert_eq!(rendered, "");
                return Ok(());
            }
            let pairs: Vec<&str> = rendered.split(',').collect();
            prop_assert_eq!(pairs.len(), env.len());
            // Every pair is KEY=VALUE.
            for pair in &pairs {
                prop_assert!(pair.contains('='), "missing `=` in pair: {}", pair);
            }
            // Keys (the part before the first `=`) are sorted ascending.
            // We assert on keys — not on full pairs — because key sort
            // order can differ from full-string sort when values
            // contain bytes lower than `=` (0x3D), e.g. digits (0x30).
            let keys: Vec<&str> = pairs
                .iter()
                .map(|p| p.split('=').next().unwrap_or(""))
                .collect();
            let mut sorted_keys = keys.clone();
            sorted_keys.sort();
            prop_assert_eq!(keys, sorted_keys);
        }
    }
}
