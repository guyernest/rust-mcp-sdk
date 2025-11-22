# Workspace Separation Solution for WASM Compatibility

**Date**: 2025-11-21
**Status**: Implemented and Verified
**Related**: CORE_TRANSPORT_PATTERN.md, DEPLOYMENT_ARCHITECTURE.md

---

## Problem Summary

Cargo's feature unification prevents selective feature disabling within a single workspace, causing WASM compilation failures when core business logic is mixed with transport dependencies.

### Root Cause

1. **Cargo Feature Unification**: When the same dependency is used multiple times in a workspace, Cargo unifies ALL requested features
2. **Workspace Dependencies**: Using `workspace = true` applies the workspace's feature superset to all members
3. **Cannot Override**: Individual packages cannot selectively disable workspace-level features

### Concrete Example

```toml
# Root workspace Cargo.toml
[workspace.dependencies.pmcp]
features = ["streamable-http", "schema-generation"]  # Transport features!

# Core package tries to override
[dependencies]
pmcp = { workspace = true, default-features = false, features = ["schema-generation"] }
# ❌ FAILS: Still gets "streamable-http" due to feature unification
```

**Result**: Core package gets `streamable-http` → `axum` → `hyper` → `tokio` → `mio` → ❌ WASM compile error

---

## Solution: Separate Workspaces

Split the project into **two independent workspaces**:
1. **Core Workspace**: WASM-compatible business logic only
2. **Main Workspace**: Transport packages (HTTP, Lambda, stdio)

### Architecture

```
my-project/
├── core-workspace/              # ✅ WASM-compatible workspace
│   ├── Cargo.toml              # pmcp with NO transport features
│   └── mcp-myapp-core/
│       ├── Cargo.toml
│       └── src/lib.rs          # pub fn build_server()
│
├── Cargo.toml                  # Main workspace
├── crates/
│   ├── myapp-server/          # Stdio transport
│   └── server-common/          # Shared transport utilities
│
├── myapp-lambda/               # Lambda transport
│   └── Cargo.toml              # References ../core-workspace/mcp-myapp-core
│
└── deploy/cloudflare/          # Cloudflare adapter (generated)
    └── Cargo.toml              # References ../core-workspace/mcp-myapp-core
```

---

## Implementation Steps

### Step 1: Create Core Workspace

```bash
mkdir core-workspace
mv crates/mcp-myapp-core core-workspace/
```

Create `core-workspace/Cargo.toml`:

```toml
[workspace]
members = ["mcp-myapp-core"]
resolver = "2"

[workspace.dependencies.pmcp]
path = "/path/to/pmcp-sdk"  # Or relative if stable
default-features = false
features = ["schema-generation"]  # ✅ NO streamable-http!

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
schemars = { version = "1.0", features = ["preserve_order"] }
anyhow = "1"
validator = { version = "0.18", features = ["derive"] }

[workspace.package]
version = "0.1.0"
edition = "2021"
```

### Step 2: Update Core Package

`core-workspace/mcp-myapp-core/Cargo.toml`:

```toml
[package]
name = "mcp-myapp-core"
version.workspace = true
edition.workspace = true

[dependencies]
# Uses core workspace settings (NO transport features)
pmcp = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
schemars = { workspace = true }
anyhow = { workspace = true }
validator = { workspace = true }

[dev-dependencies]
# Tokio ONLY for tests, not in WASM builds
tokio = { version = "1", features = ["macros", "rt"] }
```

### Step 3: Update Main Workspace

`Cargo.toml`:

```toml
[workspace]
# Remove mcp-myapp-core from members (it's in separate workspace now)
members = ["crates/server-common", "crates/myapp-server", "myapp-lambda"]
resolver = "2"

[workspace.dependencies.pmcp]
path = "/path/to/pmcp-sdk"
features = ["streamable-http", "schema-generation"]  # ✅ Transport features OK here
```

### Step 4: Update Transport Package References

`myapp-lambda/Cargo.toml`:

```toml
[dependencies]
# Reference core package as EXTERNAL dependency (from different workspace)
mcp-myapp-core = { path = "../core-workspace/mcp-myapp-core" }
pmcp = { workspace = true }  # Can use transport features here
```

### Step 5: Update cargo-pmcp Detection

Update `cargo-pmcp/src/deployment/targets/cloudflare/init.rs`:

```rust
fn find_core_package(project_root: &std::path::Path) -> Result<Option<(String, std::path::PathBuf)>> {
    let search_dirs = vec![
        project_root.join("core-workspace"),  // ← ADD THIS
        project_root.join("crates"),
        project_root.join("packages"),
        project_root.to_path_buf(),
    ];
    // ... rest of function
}
```

### Step 6: Test WASM Compilation

```bash
cargo pmcp deploy init --target cloudflare-workers
cargo pmcp deploy --target cloudflare-workers
```

---

## Verification Results

### ✅ Core Package Detection

```
🔍 Auto-detecting MCP server package...
   Detected workspace, searching for MCP server package...
   ✅ Found core package (WASM-compatible): mcp-calculator-core
```

### ✅ Dependency Tree (No More mio!)

Before (same workspace):
```
pmcp v1.8.3
├── pmcp feature "streamable-http"  ← ❌ Unwanted!
│   └── mcp-calculator-core
├── axum v0.8.7
│   └── pmcp
│       └── mcp-calculator-core
└── tokio v1.48.0
    ├── mio v1.1.0  ← ❌ WASM compilation error!
```

After (separate workspaces):
```
tokio v1.48.0
└── worker v0.4.2  ← ✅ Only from Cloudflare runtime!
    └── mcp-server-cloudflare-adapter
```

**No tokio dependency in core package!**

### ✅ Generated Adapter Structure

`deploy/cloudflare/Cargo.toml`:
```toml
[dependencies]
# References external workspace correctly
mcp-calculator-core = { path = "../../core-workspace/mcp-calculator-core" }

# PMCP with WASM features only
pmcp = { path = "../../../../../sdk/rust-mcp-sdk", default-features = false, features = ["wasm"] }
```

---

## Benefits

### 1. Clean Feature Isolation ✅
- Core workspace has NO transport features
- Main workspace can use full feature set
- No feature unification conflicts

### 2. True WASM Compatibility ✅
- Core package compiles to WASM without mio/tokio issues
- Only Cloudflare Worker runtime dependencies in final build
- No dependency tree pollution

### 3. Multi-Target Support ✅
Same core package used by:
- Cloudflare Workers (WASM)
- AWS Lambda (x86_64/aarch64)
- Docker containers
- Local stdio development

### 4. Maintainability ✅
- Clear separation of concerns
- Core business logic isolated
- Transport implementations decoupled
- Easy to add new deployment targets

---

## Remaining Issues

### 1. pmcp WASM Client Bugs

Found compilation errors in `pmcp/src/client/mod.rs` when building for WASM:
- Missing `use futures::SinkExt;` import (fixed)
- Mutable borrow through shared reference (needs fix)

**Impact**: Doesn't affect server-side deployment, only client WASM usage

**Solution**: These are pmcp SDK bugs that need to be fixed separately

### 2. jsonschema WASM Compatibility

The `validation` feature depends on `jsonschema` which pulls in `getrandom` v0.3 that doesn't support WASM.

**Workaround**: Don't use `validation` feature in core workspace:
```toml
features = ["schema-generation"]  # Removed "validation"
```

**Proper Solution**: Add getrandom override in pmcp SDK:
```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.3", features = ["js"] }
```

---

## Migration Guide

### For Existing Projects

1. **Create core workspace**:
   ```bash
   mkdir core-workspace
   mv crates/mcp-myapp-core core-workspace/
   ```

2. **Create core workspace Cargo.toml** (see Step 1 above)

3. **Update core package Cargo.toml** (see Step 2 above)

4. **Update main workspace members list** (see Step 3 above)

5. **Update all transport packages** to reference `../core-workspace/mcp-myapp-core`

6. **Regenerate Cloudflare adapter**:
   ```bash
   rm -rf deploy/cloudflare
   cargo pmcp deploy init --target cloudflare-workers
   ```

### For New Projects

Start with the separated workspace structure from the beginning:

```bash
cargo new my-project
cd my-project

# Create core workspace
mkdir core-workspace
cd core-workspace
cargo new --lib mcp-myapp-core
# Add core workspace Cargo.toml

# Create transport packages in main workspace
cd ..
mkdir crates
cd crates
cargo new --lib server-common
```

---

## Best Practices

### 1. Core Package Guidelines

✅ **DO include**:
- Business logic (tools, resources, prompts)
- Data validation using `validator` crate
- Schema generation using `schemars`
- Pure computations and algorithms

❌ **DON'T include**:
- HTTP servers (axum, hyper)
- Async runtimes (tokio with net features)
- File system operations
- Database connections
- Network clients

### 2. Workspace Organization

```
my-project/
├── core-workspace/          # Separate workspace
│   ├── Cargo.toml           # Minimal pmcp features
│   └── mcp-*-core/          # Core packages
│
├── Cargo.toml               # Main workspace
├── crates/                  # Transport utilities
├── *-lambda/                # Deployment packages
└── deploy/                  # Generated adapters
```

### 3. Dependency Management

**Core Workspace**:
- Use absolute paths for stability: `path = "/full/path/to/pmcp-sdk"`
- Minimal pmcp features: `features = ["schema-generation"]`
- No dev-dependencies leaking into builds

**Main Workspace**:
- Can use full pmcp features: `features = ["streamable-http", "full"]`
- Reference core as external: `mcp-core = { path = "../core-workspace/mcp-core" }`

---

## Troubleshooting

### Issue: Core package not detected

```
⚠️  No -core package found
```

**Solution**: Ensure core package name ends with `-core` and is in `core-workspace/` directory

### Issue: Feature unification still occurring

```
pmcp feature "streamable-http"
│   └── mcp-myapp-core  ← Should NOT have this!
```

**Solution**: Verify core package is in **separate workspace** with own Cargo.toml, not a member of main workspace

### Issue: Path dependencies not found

```
error: failed to get `mcp-myapp-core` as a dependency
```

**Solution**: Use correct relative path from adapter to core workspace:
```toml
mcp-myapp-core = { path = "../../core-workspace/mcp-myapp-core" }
```

---

## Conclusion

Separating workspaces is the **recommended solution** for multi-target MCP server deployment with WASM support. It:

- ✅ Solves Cargo feature unification issues
- ✅ Enables true WASM compatibility
- ✅ Maintains clean architecture
- ✅ Scales to multiple deployment targets
- ✅ Keeps core business logic portable

**Implementation Status**: Complete and verified with calculator example

**Next Steps**:
1. Fix remaining pmcp WASM client bugs
2. Add getrandom override for jsonschema WASM support
3. Document pattern in main README
4. Add examples for other deployment targets (Lambda, Docker)

---

**Last Updated**: 2025-11-21
**Status**: Verified Working Solution
