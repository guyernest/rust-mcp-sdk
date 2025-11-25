# Binary Naming Conventions for Multi-Target Deployments

## Problem Statement

When deploying MCP servers to multiple targets (AWS Lambda, Google Cloud Run, Kubernetes, etc.), binary naming conflicts can occur in Cargo workspaces. This happens when multiple packages define binaries with the same name, causing `cargo run` to fail with:

```
error: `cargo run` can run at most one executable, but multiple were specified
```

### Root Causes

1. **Platform Requirements vs. Conventions**
   - Some platforms have hard requirements (e.g., AWS Lambda requires `bootstrap`)
   - Others use conventions (e.g., Docker typically uses the package name)

2. **Workspace Binary Name Collision**
   - Cargo requires unique binary names across the entire workspace
   - Multiple deployment targets often want similar names (e.g., `server`, `app`)

3. **Multi-Target Deployments Are Common**
   - Modern applications deploy to multiple platforms (Lambda + Cloud Run + K8s)
   - Each target may have different binary name expectations
   - Developers need to run different targets locally for testing

### Example Conflict

```
reinvent/
├── crates/
│   ├── reinvent-server/         # Standalone HTTP server
│   │   └── src/main.rs          # bin: reinvent-server ✅
│   └── reinvent-lambda/         # AWS Lambda handler
│       └── src/main.rs          # bin: reinvent-server ❌ CONFLICT!
```

This causes the error shown above because both packages define a binary named `reinvent-server`.

## Platform Requirements and Conventions

### Comprehensive Deployment Target Table

| Deployment Target | Package Name Convention | Binary Name | Build Target | Platform Requirement | Notes |
|------------------|------------------------|-------------|--------------|---------------------|-------|
| **AWS Lambda** | `{app}-lambda` | `bootstrap` | `--bin bootstrap` | ✅ **MUST** be `bootstrap` | Custom Runtime API requires this exact name |
| **pmcp.run** | `{app}-lambda` | `bootstrap` | `--bin bootstrap` | ✅ **MUST** be `bootstrap` | Built on AWS Lambda |
| **Google Cloud Run** | `{app}-cloudrun` | `{app}-server` | `--bin {app}-server` | 🔵 Convention (Dockerfile CMD) | Can be anything; set in Dockerfile |
| **Kubernetes** | `{app}-k8s` | `{app}-server` | `--bin {app}-server` | 🔵 Convention (manifest) | Set in pod spec |
| **Docker Generic** | `{app}-docker` | `{app}` | `--bin {app}` | 🔵 Convention (Dockerfile) | Dockerfile CMD determines executable |
| **Standalone Binary** | `{app}-server` | `{app}-server` | `--bin {app}-server` | 🟢 Developer choice | Typically matches package name |
| **Azure Functions** | `{app}-azure` | `{app}-handler` | `--bin {app}-handler` | 🔵 Convention (host.json) | Custom handler executable name |
| **Fly.io** | `{app}-fly` | `{app}` | `--bin {app}` | 🔵 Convention (fly.toml) | Set in fly.toml `[[processes]]` |
| **Railway** | `{app}` | `{app}` | `--bin {app}` | 🔵 Convention (Procfile) | Set in Procfile or auto-detected |

**Legend:**
- ✅ **MUST** - Platform hard requirement (cannot be changed)
- 🔵 **Convention** - Platform convention (can be overridden in config)
- 🟢 **Choice** - Developer's preference (no platform constraint)

### Key Insights

1. **AWS Lambda and pmcp.run are special** - They require `bootstrap` (non-negotiable)
2. **Container platforms (Cloud Run, K8s, Docker)** - Flexible; name set in Dockerfile/manifest
3. **Other cloud functions** - Typically use conventions but allow override
4. **Local development** - Developers want meaningful names for `cargo run --bin X`

## Recommended Solutions

### 1. Convention-Based Naming with Detection

**Principle**: Use platform-specific naming conventions and auto-detect conflicts.

**Implementation**:

```rust
// cargo-pmcp/src/deployment/naming.rs
pub fn get_recommended_binary_name(target: DeploymentTarget, app_name: &str) -> String {
    match target {
        DeploymentTarget::AwsLambda | DeploymentTarget::PmcpRun => "bootstrap".to_string(),
        DeploymentTarget::GoogleCloudRun => format!("{}-server", app_name),
        DeploymentTarget::Kubernetes => format!("{}-k8s-server", app_name),
        DeploymentTarget::Docker => app_name.to_string(),
        DeploymentTarget::Standalone => format!("{}-server", app_name),
        DeploymentTarget::AzureFunctions => format!("{}-handler", app_name),
        DeploymentTarget::FlyIo => app_name.to_string(),
        DeploymentTarget::Railway => app_name.to_string(),
    }
}

pub fn get_recommended_package_name(target: DeploymentTarget, app_name: &str) -> String {
    match target {
        DeploymentTarget::AwsLambda | DeploymentTarget::PmcpRun => format!("{}-lambda", app_name),
        DeploymentTarget::GoogleCloudRun => format!("{}-cloudrun", app_name),
        DeploymentTarget::Kubernetes => format!("{}-k8s", app_name),
        DeploymentTarget::Docker => format!("{}-docker", app_name),
        DeploymentTarget::Standalone => format!("{}-server", app_name),
        DeploymentTarget::AzureFunctions => format!("{}-azure", app_name),
        DeploymentTarget::FlyIo => format!("{}-fly", app_name),
        DeploymentTarget::Railway => app_name.to_string(),
    }
}
```

### 2. Enhanced Package Structure

**Recommended Structure for Multi-Target Projects**:

```
my-app/
├── Cargo.toml                     # Workspace root
├── crates/
│   ├── my-app-core/              # Shared business logic
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   └── my-app-server/            # Standalone HTTP server
│       ├── Cargo.toml            # bin: my-app-server
│       └── src/main.rs
└── deployments/                  # 🆕 Deployment targets
    ├── lambda/                   # AWS Lambda deployment
    │   ├── Cargo.toml            # package: my-app-lambda, bin: bootstrap
    │   ├── src/main.rs
    │   ├── deploy/
    │   │   └── config.toml
    │   └── README.md
    ├── cloudrun/                 # Google Cloud Run
    │   ├── Cargo.toml            # package: my-app-cloudrun, bin: my-app-server
    │   ├── src/main.rs
    │   ├── Dockerfile
    │   ├── deploy/
    │   │   └── config.toml
    │   └── README.md
    └── k8s/                      # Kubernetes
        ├── Cargo.toml            # package: my-app-k8s, bin: my-app-k8s-server
        ├── src/main.rs
        ├── Dockerfile
        ├── k8s/
        │   └── deployment.yaml
        ├── deploy/
        │   └── config.toml
        └── README.md
```

**Benefits**:
- Clear separation between core logic and deployment targets
- Each deployment target is self-contained
- Easy to add new targets without conflicts
- Natural place for deployment-specific configs

### 3. Updated deploy/config.toml Format

**Enhanced Configuration**:

```toml
[deployment]
target = "aws-lambda"
# 🆕 Explicit binary name configuration
binary_name = "bootstrap"
package_name = "my-app-lambda"

[deployment.build]
# Platform requirement marker
binary_name_required = true  # If true, cannot be overridden
binary_name_reason = "AWS Lambda Custom Runtime requires 'bootstrap'"

[deployment.lambda]
runtime = "provided.al2023"
handler = "bootstrap"
architecture = "arm64"
```

**For Flexible Platforms**:

```toml
[deployment]
target = "google-cloud-run"
# Convention, can be overridden
binary_name = "my-app-server"
package_name = "my-app-cloudrun"

[deployment.build]
binary_name_required = false
binary_name_reason = "Convention - set in Dockerfile CMD"

[deployment.cloudrun]
region = "us-west1"
dockerfile = "Dockerfile"
```

### 4. Conflict Detection Command

**New Command**: `cargo pmcp validate`

```bash
# Check for naming conflicts
cargo pmcp validate

# Output:
✅ No binary naming conflicts detected

Deployment targets:
  - lambda (my-app-lambda) → binary: bootstrap
  - cloudrun (my-app-cloudrun) → binary: my-app-server
  - server (my-app-server) → binary: my-app-server

All binary names are unique across workspace.
```

**With Conflicts**:

```bash
cargo pmcp validate

# Output:
❌ Binary naming conflicts detected!

Conflict: binary 'my-app-server' is defined in multiple packages:
  - my-app-server (crates/my-app-server)
  - my-app-cloudrun (deployments/cloudrun)

Recommendation:
  Rename one of the binaries:
  - my-app-server → my-app-server (standalone)
  - my-app-cloudrun → my-app-cloudrun-server

Run: cargo pmcp fix-conflicts --auto
```

**Implementation**:

```rust
// cargo-pmcp/src/commands/validate.rs
pub async fn validate_workspace() -> Result<()> {
    let metadata = MetadataCommand::new().exec()?;
    let mut binary_names: HashMap<String, Vec<String>> = HashMap::new();

    for package in metadata.packages {
        for target in &package.targets {
            if target.kind.contains(&"bin".to_string()) {
                binary_names
                    .entry(target.name.clone())
                    .or_default()
                    .push(package.name.clone());
            }
        }
    }

    let conflicts: Vec<_> = binary_names
        .iter()
        .filter(|(_, packages)| packages.len() > 1)
        .collect();

    if conflicts.is_empty() {
        println!("✅ No binary naming conflicts detected");
        // Show summary...
    } else {
        println!("❌ Binary naming conflicts detected!");
        for (binary, packages) in conflicts {
            println!("\nConflict: binary '{}' in packages:", binary);
            for package in packages {
                println!("  - {}", package);
            }
            // Show recommendations...
        }
        std::process::exit(1);
    }

    Ok(())
}
```

### 5. Helper Commands

**New Command**: `cargo pmcp list`

```bash
cargo pmcp list

# Output:
📦 Deployment Targets:

AWS Lambda (my-app-lambda)
  Binary: bootstrap (required by platform)
  Location: deployments/lambda
  Run: cargo run --bin bootstrap
  Deploy: cargo pmcp deploy lambda

Google Cloud Run (my-app-cloudrun)
  Binary: my-app-server (convention)
  Location: deployments/cloudrun
  Run: cargo run --bin my-app-server
  Deploy: cargo pmcp deploy cloudrun

Standalone Server (my-app-server)
  Binary: my-app-server
  Location: crates/my-app-server
  Run: cargo run --bin my-app-server
```

**New Command**: `cargo pmcp run`

```bash
# Smart run command that understands deployment targets
cargo pmcp run lambda
# Equivalent to: cargo run --bin bootstrap

cargo pmcp run cloudrun
# Equivalent to: cargo run --bin my-app-server

# With args
cargo pmcp run lambda -- --help
```

### 6. Enhanced cargo pmcp deploy init

**Updated Initialization**:

```bash
cargo pmcp deploy init \
  --target aws-lambda \
  --location deployments/lambda \
  --auto-name

# Creates:
# - deployments/lambda/
# - Cargo.toml with package: my-app-lambda, bin: bootstrap
# - README.md with naming explanation
# - deploy/config.toml with binary_name_required = true
```

**Interactive Mode**:

```bash
cargo pmcp deploy init

# Prompts:
? Select deployment target: (Use arrow keys)
  ❯ AWS Lambda (requires binary: bootstrap)
    Google Cloud Run (convention: {app}-server)
    Kubernetes (convention: {app}-k8s-server)
    Standalone Server

? Package location:
  ❯ deployments/lambda (recommended)
    crates/my-app-lambda

? Binary name: bootstrap
  ⚠️  This is required by AWS Lambda and cannot be changed

? Check for conflicts? Yes
  ✅ No conflicts with existing binaries

Creating deployment package...
✅ Created deployments/lambda with binary 'bootstrap'
```

### 7. Enhanced Documentation in Generated READMEs

**Auto-Generated README Template**:

```markdown
# my-app Lambda Deployment

This package deploys my-app to AWS Lambda.

## Binary Naming

**Binary Name**: `bootstrap`

**Platform Requirement**: ✅ **REQUIRED**

AWS Lambda Custom Runtime API requires the binary to be named exactly `bootstrap`.
This is a hard requirement and cannot be changed. The Lambda service looks for
an executable named `bootstrap` in the deployment package.

### Running Locally

```bash
# Run the Lambda handler locally (for testing)
cargo run --bin bootstrap

# Or use the helper command
cargo pmcp run lambda
```

### Building for Deployment

```bash
# Build for Lambda (ARM64)
cargo build --release --target aarch64-unknown-linux-musl --bin bootstrap

# Or use cargo-pmcp
cargo pmcp deploy lambda --build
```

## Other Deployment Targets

If you need to deploy to multiple targets, see:
- `/deployments/cloudrun` - Google Cloud Run (binary: my-app-server)
- `/crates/my-app-server` - Standalone HTTP server (binary: my-app-server)

Each target uses a different binary name to avoid conflicts.
```

### 8. Quality Gate Integration

**Add to Pre-Commit Hook**:

```bash
# .git/hooks/pre-commit
make quality-gate
cargo pmcp validate  # 🆕 Check for naming conflicts
```

**Add to CI/CD**:

```yaml
# .github/workflows/quality.yml
- name: Validate Deployment Naming
  run: cargo pmcp validate
```

## Priority Implementation Plan

### P0: Critical (Block on Conflicts)

**Goal**: Prevent conflicts in new projects

1. ✅ **Update `cargo pmcp deploy init`**
   - Use `bootstrap` for Lambda/pmcp-run by default
   - Use `{app}-server` for Cloud Run
   - Add `--binary-name` flag for override (with warning)

2. ✅ **Add Basic Conflict Detection**
   - Check workspace for duplicate binary names
   - Warn on `deploy init` if conflict would occur
   - Fail with clear error message

3. ✅ **Update README Templates**
   - Explain platform binary name requirements
   - Show how to run locally with correct binary name
   - Link to this document

**Timeline**: Immediate (sprint 1)

### P1: Enhanced Experience

**Goal**: Make multi-target deployments smooth

1. 🔜 **Implement `cargo pmcp validate`**
   - Full workspace analysis
   - Conflict detection with recommendations
   - Summary of all deployment targets

2. 🔜 **Implement `cargo pmcp list`**
   - Show all deployment targets
   - Display binary names and locations
   - Quick reference for developers

3. 🔜 **Implement `cargo pmcp run`**
   - Smart dispatch to correct binary
   - Tab completion for target names
   - Pass-through arguments

4. 🔜 **Enhanced `deploy/config.toml`**
   - Add `binary_name_required` field
   - Add `binary_name_reason` field
   - Validate against platform requirements

**Timeline**: Short-term (sprint 2-3)

### P2: Advanced Features

**Goal**: Enterprise-grade multi-target support

1. 📋 **Auto-Fix Command**
   - `cargo pmcp fix-conflicts --auto`
   - Automatically rename conflicting binaries
   - Update all references

2. 📋 **Migration Assistant**
   - `cargo pmcp migrate --to-deployments-folder`
   - Move existing deployment packages to new structure
   - Preserve git history

3. 📋 **Deployment Matrix Testing**
   - Test all deployment targets in CI
   - Ensure each binary builds correctly
   - Validate deployment configs

4. 📋 **Quality Gate Enhancement**
   - Add binary naming validation to pre-commit
   - CI check for conflicts
   - Documentation coverage for all targets

**Timeline**: Medium-term (sprint 4-6)

## Migration Guide

### For Existing Projects with Conflicts

**Scenario**: You have both `my-app-server` and `my-app-lambda` with conflicting binaries.

#### Option 1: Rename Lambda Binary (Recommended)

**Before**:
```toml
# deployments/lambda/Cargo.toml
[[bin]]
name = "my-app-server"  # ❌ Conflicts with standalone
path = "src/main.rs"
```

**After**:
```toml
# deployments/lambda/Cargo.toml
[[bin]]
name = "bootstrap"  # ✅ AWS Lambda requirement
path = "src/main.rs"
```

**Update deploy/config.toml**:
```toml
[deployment.build]
binary_name = "bootstrap"
```

#### Option 2: Move to deployments/ Folder Structure

```bash
# Create new structure
mkdir -p deployments/lambda
mkdir -p deployments/cloudrun

# Move existing packages
git mv my-app-lambda deployments/lambda
git mv my-app-cloudrun deployments/cloudrun

# Update workspace Cargo.toml
# members = [
#     "crates/*",
#     "deployments/*",  # 🆕
# ]

# Rename binaries as needed
cargo pmcp validate
cargo pmcp fix-conflicts --auto
```

#### Option 3: Use Package-Specific Binary Names

If you can't use `bootstrap` (e.g., not Lambda):

```toml
# Keep each binary name unique
# my-app-server/Cargo.toml
[[bin]]
name = "my-app-server"

# my-app-cloudrun/Cargo.toml
[[bin]]
name = "my-app-cloudrun"

# Update Dockerfile CMD accordingly
CMD ["/usr/local/bin/my-app-cloudrun"]
```

### Testing Your Migration

```bash
# 1. Validate no conflicts
cargo pmcp validate

# 2. Build all targets
cargo build --workspace

# 3. Run each target
cargo run --bin bootstrap
cargo run --bin my-app-server
cargo run --bin my-app-cloudrun

# 4. Deploy to staging
cargo pmcp deploy lambda --env staging
cargo pmcp deploy cloudrun --env staging
```

## Examples

### Example 1: Simple AWS Lambda Project

```
reinvent/
├── Cargo.toml
├── crates/
│   └── reinvent-server/
│       └── src/
│           ├── lib.rs        # Shared server logic
│           └── bin/
│               └── reinvent-server.rs  # Standalone server
└── deployments/
    └── lambda/
        ├── Cargo.toml        # package: reinvent-lambda
        └── src/
            └── main.rs       # bin: bootstrap
```

**lambda/Cargo.toml**:
```toml
[package]
name = "reinvent-lambda"

[[bin]]
name = "bootstrap"
path = "src/main.rs"

[dependencies]
reinvent-server = { path = "../../crates/reinvent-server" }
lambda_runtime = "0.13"
```

### Example 2: Multi-Cloud Deployment

```
saas-platform/
├── Cargo.toml
├── crates/
│   ├── platform-core/        # Business logic
│   └── platform-server/      # HTTP server (local dev)
└── deployments/
    ├── lambda/               # AWS (US customers)
    │   ├── Cargo.toml        # bin: bootstrap
    │   └── deploy/config.toml
    ├── cloudrun/             # GCP (EU customers)
    │   ├── Cargo.toml        # bin: platform-server
    │   ├── Dockerfile
    │   └── deploy/config.toml
    └── azure/                # Azure (APAC customers)
        ├── Cargo.toml        # bin: platform-handler
        └── deploy/config.toml
```

**Each deployment**:
- Uses shared `platform-core` for business logic
- Has unique binary name (no conflicts)
- Self-contained with deployment configs
- Can be tested independently

### Example 3: Enterprise Multi-Region Setup

```
enterprise-api/
├── deployments/
│   ├── lambda-us-west/
│   │   └── Cargo.toml        # bin: bootstrap
│   ├── lambda-us-east/
│   │   └── Cargo.toml        # bin: bootstrap (different package!)
│   ├── lambda-eu-west/
│   │   └── Cargo.toml        # bin: bootstrap
│   └── k8s-apac/
│       └── Cargo.toml        # bin: enterprise-api-server
```

**Key Point**: Even multiple Lambda deployments work because each is a separate **package** with the same **binary name**. Cargo only requires binary names to be unique **within** each package, not across packages.

## Best Practices

### 1. Use Descriptive Package Names

✅ Good:
```
my-app-lambda
my-app-cloudrun
my-app-k8s
```

❌ Avoid:
```
deployment1
deployment2
lambda_thing
```

### 2. Keep Platform Requirements Visible

Always document why a binary has a specific name:

```toml
# Cargo.toml
[[bin]]
name = "bootstrap"
# ⚠️  REQUIRED: AWS Lambda Custom Runtime API requires this exact name
path = "src/main.rs"
```

### 3. Validate Early and Often

```bash
# Before committing new deployment
cargo pmcp validate

# In CI/CD
make quality-gate && cargo pmcp validate
```

### 4. Use Consistent Naming Conventions

Follow the table in this document for consistency across projects.

### 5. Test Each Binary Independently

```bash
# Don't just build - run to verify
cargo run --bin bootstrap -- --help
cargo run --bin my-app-server -- --version
```

## Troubleshooting

### Q: Can I use the same binary name in different packages?

**A**: No, Cargo workspaces require unique binary names across all packages. However, you can have multiple packages with the same binary name if they're in different workspaces (rare).

### Q: What if I need multiple Lambda functions?

**A**: Create separate packages for each:

```
deployments/
├── lambda-api/           # bin: bootstrap
│   └── deploy/config.toml  # handler: api
├── lambda-worker/        # bin: bootstrap (different package!)
│   └── deploy/config.toml  # handler: worker
└── lambda-auth/          # bin: bootstrap
    └── deploy/config.toml  # handler: auth
```

Each is a separate Cargo package, so all can use `bootstrap`.

### Q: Can I override the binary name for Lambda?

**A**: No. AWS Lambda Custom Runtime API requires `bootstrap`. This is non-negotiable. If you need a different name, you're probably not using Lambda correctly.

### Q: How do I handle this in CI/CD?

**A**: Build each target explicitly:

```bash
# .github/workflows/deploy.yml
- name: Build Lambda
  run: cargo build --release --bin bootstrap

- name: Build Cloud Run
  run: cargo build --release --bin my-app-server
```

### Q: What about local testing with docker-compose?

**A**: Use the actual binary names:

```yaml
# docker-compose.yml
services:
  lambda:
    command: ["/usr/local/bin/bootstrap"]

  cloudrun:
    command: ["/usr/local/bin/my-app-server"]
```

## References

- **AWS Lambda Custom Runtime**: https://docs.aws.amazon.com/lambda/latest/dg/runtimes-custom.html
- **Cargo Binary Naming**: https://doc.rust-lang.org/cargo/reference/cargo-targets.html#binaries
- **Cargo Workspaces**: https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html
- **pmcp.run Documentation**: `cargo-pmcp/src/deployment/targets/pmcp_run/README.md`

## Contributing

When adding new deployment targets to cargo-pmcp:

1. Update the platform requirements table in this document
2. Add binary name conventions to `naming.rs`
3. Update template generation in `deploy init`
4. Add examples to this document
5. Update quality gate validation

## Summary

**Key Takeaways**:

1. ✅ **AWS Lambda requires `bootstrap`** - Non-negotiable
2. ✅ **Use deployments/ folder** for multi-target projects
3. ✅ **Validate early** with `cargo pmcp validate`
4. ✅ **Follow naming conventions** from the table
5. ✅ **Document platform requirements** in READMEs
6. ✅ **Test each binary independently**
7. ✅ **Integrate with quality gates**

**Quick Command Reference**:

```bash
# Validate workspace
cargo pmcp validate

# List deployment targets
cargo pmcp list

# Run specific target
cargo pmcp run lambda

# Initialize new deployment
cargo pmcp deploy init --target aws-lambda --auto-name

# Fix conflicts automatically
cargo pmcp fix-conflicts --auto
```

---

**Last Updated**: 2025-01-25
**Version**: 1.0.0
**Status**: Draft - Ready for Implementation
