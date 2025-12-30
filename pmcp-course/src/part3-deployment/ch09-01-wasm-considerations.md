# WASM Considerations for Rust MCP Servers

WebAssembly (WASM) enables running Rust code on Cloudflare Workers' edge network, but it comes with specific constraints and patterns you need to understand. This lesson covers everything you need to know about building WASM-compatible MCP servers.

## Learning Objectives

By the end of this lesson, you will:
- Understand WASM compilation targets and toolchains
- Identify crate compatibility issues and workarounds
- Master async patterns in the WASM environment
- Handle WASM limitations (filesystem, networking, threads)
- Test and debug WASM locally
- Optimize memory usage and binary size

## Understanding the WASM Runtime

### V8 Isolates vs Traditional Containers

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Traditional Container                             │
├─────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Operating System                          │   │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐           │   │
│  │  │  Process 1  │ │  Process 2  │ │  Process 3  │           │   │
│  │  │  (Your App) │ │  (Runtime)  │ │  (Deps)     │           │   │
│  │  └─────────────┘ └─────────────┘ └─────────────┘           │   │
│  │  Full syscall access, filesystem, threads                   │   │
│  └─────────────────────────────────────────────────────────────┘   │
│  Startup: 50-500ms │ Memory: 128MB-4GB │ Isolation: Process      │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                    V8 Isolate (Workers)                             │
├─────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    V8 JavaScript Engine                      │   │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐           │   │
│  │  │  Isolate 1  │ │  Isolate 2  │ │  Isolate 3  │           │   │
│  │  │  (WASM)     │ │  (WASM)     │ │  (WASM)     │           │   │
│  │  └─────────────┘ └─────────────┘ └─────────────┘           │   │
│  │  Sandboxed, no syscalls, Web APIs only                      │   │
│  └─────────────────────────────────────────────────────────────┘   │
│  Startup: <5ms │ Memory: 128MB max │ Isolation: V8 Sandbox        │
└─────────────────────────────────────────────────────────────────────┘
```

### What WASM Provides

| Capability | Available | Notes |
|------------|-----------|-------|
| CPU compute | Yes | Full Rust performance |
| Memory allocation | Yes | Up to 128MB |
| Async/await | Yes | Via JavaScript promises |
| HTTP fetch | Yes | Via Workers Fetch API |
| Time/Date | Yes | Via JavaScript Date |
| Crypto | Yes | Via Web Crypto API |
| JSON parsing | Yes | Native Rust serde |

### What WASM Cannot Do

| Capability | Available | Alternative |
|------------|-----------|-------------|
| Filesystem | No | Workers KV, R2 |
| Raw sockets | No | HTTP via fetch |
| Threads | No | Single-threaded async |
| System calls | No | Workers APIs |
| FFI/C libraries | Limited | Pure Rust only |
| Environment vars | No | Workers secrets |

## Compilation Setup

### Toolchain Installation

```bash
# Install wasm32 target
rustup target add wasm32-unknown-unknown

# Install wasm tooling
cargo install worker-build
cargo install wasm-pack
cargo install wasm-opt  # For optimization
```

### Project Structure

```
my-mcp-worker/
├── Cargo.toml
├── wrangler.toml
├── src/
│   ├── lib.rs           # Worker entry point
│   ├── server.rs        # MCP server logic
│   ├── tools/           # Tool implementations
│   │   ├── mod.rs
│   │   └── database.rs
│   └── bindings.rs      # Workers API bindings
├── build.rs             # Build script for WASM
└── tests/
    └── wasm.rs          # WASM-specific tests
```

### Cargo.toml Configuration

```toml
[package]
name = "my-mcp-worker"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
# Workers runtime
worker = "0.4"
worker-macros = "0.4"

# MCP SDK (WASM-compatible)
pmcp-sdk = { version = "0.1", features = ["wasm"] }

# Async runtime (WASM-compatible)
futures = "0.3"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# WASM-compatible utilities
getrandom = { version = "0.2", features = ["js"] }
chrono = { version = "0.4", features = ["wasmbind"] }

# Console logging for WASM
console_error_panic_hook = "0.1"

[dev-dependencies]
wasm-bindgen-test = "0.3"

[profile.release]
# Optimize for size (important for cold starts)
opt-level = "s"
lto = true
codegen-units = 1
panic = "abort"

[profile.release.package."*"]
opt-level = "s"
```

### Build Script

```rust
// build.rs
fn main() {
    // Ensure we're building for the correct target
    #[cfg(target_arch = "wasm32")]
    {
        println!("cargo:rerun-if-changed=src/");
    }
}
```

## Crate Compatibility

### Common Incompatibility Patterns

Many Rust crates assume a traditional runtime environment. Here's how to identify and handle incompatibilities:

```
┌────────────────────────────────────────────────────────────────────┐
│                  Crate Compatibility Matrix                         │
├────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ✅ Pure Rust, no std dependencies                                 │
│     serde, serde_json, thiserror, anyhow                           │
│                                                                     │
│  ✅ WASM-aware crates                                              │
│     getrandom (with js feature), chrono (with wasmbind)            │
│     uuid (with js feature), rand (with getrandom)                  │
│                                                                     │
│  ⚠️  Async crates (need configuration)                             │
│     tokio (NOT compatible), futures (compatible)                    │
│     async-std (limited), wasm-bindgen-futures (recommended)        │
│                                                                     │
│  ❌ System-dependent crates                                        │
│     tokio (uses mio), std::fs, std::net, std::thread               │
│     ring (uses assembly), openssl, native-tls                      │
│                                                                     │
└────────────────────────────────────────────────────────────────────┘
```

### Handling tokio Dependencies

Many crates depend on tokio, which doesn't compile to WASM. Use conditional compilation:

```rust
// In Cargo.toml, use feature flags
[features]
default = ["native"]
native = ["tokio/full"]
wasm = ["wasm-bindgen-futures"]

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
tokio = { version = "1", features = ["full"] }

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen-futures = "0.4"
```

```rust
// In your code, use conditional imports
#[cfg(not(target_arch = "wasm32"))]
use tokio::time::sleep;

#[cfg(target_arch = "wasm32")]
async fn sleep(duration: std::time::Duration) {
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::JsFuture;

    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let window = web_sys::window().unwrap();
        window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                &resolve,
                duration.as_millis() as i32,
            )
            .unwrap();
    });
    JsFuture::from(promise).await.unwrap();
}
```

### Random Number Generation

```rust
// Cargo.toml
[dependencies]
getrandom = { version = "0.2", features = ["js"] }
uuid = { version = "1.0", features = ["v4", "js"] }
rand = { version = "0.8", features = ["getrandom"] }

// Usage - works on both native and WASM
use uuid::Uuid;
use rand::Rng;

fn generate_request_id() -> String {
    Uuid::new_v4().to_string()
}

fn generate_random_number() -> u32 {
    let mut rng = rand::thread_rng();
    rng.gen()
}
```

### Date/Time Handling

```rust
// Cargo.toml
[dependencies]
chrono = { version = "0.4", features = ["wasmbind"] }

// For Workers-specific time
use worker::Date;

fn get_current_time() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        Date::now().to_string()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        chrono::Utc::now().to_rfc3339()
    }
}
```

### Crypto Operations

```rust
// Native crypto won't work - use Web Crypto API
use worker::*;

async fn hash_data(data: &[u8]) -> Result<Vec<u8>> {
    let crypto = Crypto::new();
    let digest = crypto
        .subtle()
        .digest("SHA-256", data)
        .await?;
    Ok(digest.to_vec())
}

async fn generate_hmac(key: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    let crypto = Crypto::new();

    // Import the key
    let crypto_key = crypto
        .subtle()
        .import_key_raw(
            key,
            "HMAC",
            &HmacImportParams::new("SHA-256"),
            false,
            &["sign"],
        )
        .await?;

    // Sign the data
    let signature = crypto
        .subtle()
        .sign("HMAC", &crypto_key, data)
        .await?;

    Ok(signature.to_vec())
}
```

## Async Patterns in WASM

### Understanding the Event Loop

Workers use JavaScript's event loop, not tokio's runtime:

```
┌─────────────────────────────────────────────────────────────────────┐
│                    JavaScript Event Loop                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│    Request Arrives                                                  │
│          │                                                          │
│          ▼                                                          │
│    ┌─────────────┐     ┌─────────────┐     ┌─────────────┐        │
│    │   WASM      │────▶│  JS Promise │────▶│  Event Loop │        │
│    │   Code      │     │   Queue     │     │  (V8)       │        │
│    └─────────────┘     └─────────────┘     └─────────────┘        │
│          │                    │                   │                 │
│          │                    │                   │                 │
│          ▼                    ▼                   ▼                 │
│    Synchronous          Async I/O            Microtasks            │
│    computation          (fetch, KV)          scheduled             │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Spawning Async Tasks

```rust
use worker::*;
use futures::future::join_all;

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    // Parallel async operations (no tokio::spawn needed!)
    let results = join_all(vec![
        fetch_from_kv(&env, "key1"),
        fetch_from_kv(&env, "key2"),
        fetch_from_kv(&env, "key3"),
    ])
    .await;

    // Process results
    let combined: Vec<String> = results
        .into_iter()
        .filter_map(|r| r.ok())
        .collect();

    Response::from_json(&combined)
}

async fn fetch_from_kv(env: &Env, key: &str) -> Result<String> {
    let kv = env.kv("MY_KV")?;
    kv.get(key)
        .text()
        .await?
        .ok_or_else(|| Error::from("Key not found"))
}
```

### Timeouts and Cancellation

```rust
use worker::*;
use futures::future::{select, Either};
use std::time::Duration;

async fn with_timeout<T, F>(future: F, timeout_ms: u64) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    let timeout = create_timeout(timeout_ms);

    match select(Box::pin(future), Box::pin(timeout)).await {
        Either::Left((result, _)) => result,
        Either::Right(_) => Err(Error::from("Operation timed out")),
    }
}

async fn create_timeout(ms: u64) {
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::JsFuture;

    let promise = js_sys::Promise::new(&mut |resolve, _| {
        // Workers have a built-in scheduler
        let _ = js_sys::Reflect::set(
            &js_sys::global(),
            &"__timeout_resolve".into(),
            &resolve,
        );
    });

    // Use the scheduler API
    let _ = JsFuture::from(promise).await;
}

// Usage in MCP tool
async fn database_query_tool(env: &Env, query: &str) -> Result<String> {
    with_timeout(
        execute_d1_query(env, query),
        5000, // 5 second timeout
    )
    .await
}
```

### Error Handling Patterns

```rust
use worker::*;
use std::fmt;

// Custom error type that works in WASM
#[derive(Debug)]
pub enum McpError {
    InvalidRequest(String),
    DatabaseError(String),
    Timeout,
    Unauthorized,
}

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpError::InvalidRequest(msg) => write!(f, "Invalid request: {}", msg),
            McpError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            McpError::Timeout => write!(f, "Operation timed out"),
            McpError::Unauthorized => write!(f, "Unauthorized"),
        }
    }
}

impl From<McpError> for worker::Error {
    fn from(e: McpError) -> Self {
        worker::Error::from(e.to_string())
    }
}

// Result type alias for cleaner code
pub type McpResult<T> = std::result::Result<T, McpError>;
```

## Memory Management

### Understanding WASM Memory

```
┌─────────────────────────────────────────────────────────────────────┐
│                    WASM Linear Memory (128MB Max)                   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ Stack (grows down)                                    4MB    │  │
│  │ ▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼▼          │  │
│  ├──────────────────────────────────────────────────────────────┤  │
│  │                                                              │  │
│  │                    Free Space                                │  │
│  │                                                              │  │
│  ├──────────────────────────────────────────────────────────────┤  │
│  │ ▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲▲          │  │
│  │ Heap (grows up)                                     ~120MB   │  │
│  ├──────────────────────────────────────────────────────────────┤  │
│  │ Static Data (strings, constants)                    ~4MB     │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                     │
│  Note: Memory is NOT freed between requests in the same isolate!   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Memory-Efficient Patterns

```rust
use worker::*;

// BAD: Accumulates memory across requests
static mut CACHE: Option<Vec<String>> = None;

// GOOD: Use Workers KV for caching
async fn cached_fetch(env: &Env, url: &str) -> Result<String> {
    let kv = env.kv("CACHE")?;
    let cache_key = format!("fetch:{}", url);

    // Check cache first
    if let Some(cached) = kv.get(&cache_key).text().await? {
        return Ok(cached);
    }

    // Fetch and cache
    let response = Fetch::Url(url.parse()?).send().await?;
    let body = response.text().await?;

    // Cache for 5 minutes
    kv.put(&cache_key, &body)?
        .expiration_ttl(300)
        .execute()
        .await?;

    Ok(body)
}
```

### Streaming Large Responses

```rust
use worker::*;

// For large responses, use streaming instead of buffering
async fn stream_large_result(env: &Env, query: &str) -> Result<Response> {
    let d1 = env.d1("DB")?;

    // Create a streaming response
    let (mut tx, rx) = futures::channel::mpsc::unbounded();

    // Spawn the query (conceptually - actual implementation varies)
    wasm_bindgen_futures::spawn_local(async move {
        let results = d1.prepare(query).all().await;

        match results {
            Ok(rows) => {
                for row in rows.results::<serde_json::Value>().unwrap_or_default() {
                    let json = serde_json::to_string(&row).unwrap_or_default();
                    let _ = tx.unbounded_send(json);
                }
            }
            Err(e) => {
                let _ = tx.unbounded_send(format!("Error: {}", e));
            }
        }
    });

    // Return streaming response
    let stream = rx.map(|chunk| Ok(chunk.into_bytes()));
    Response::from_stream(stream)
}
```

### Avoiding Memory Leaks

```rust
use worker::*;
use std::cell::RefCell;

// Use RefCell for request-scoped state (dropped after request)
thread_local! {
    static REQUEST_CONTEXT: RefCell<Option<RequestContext>> = RefCell::new(None);
}

struct RequestContext {
    request_id: String,
    start_time: f64,
}

fn init_request_context(request_id: String) {
    REQUEST_CONTEXT.with(|ctx| {
        *ctx.borrow_mut() = Some(RequestContext {
            request_id,
            start_time: js_sys::Date::now(),
        });
    });
}

fn cleanup_request_context() {
    REQUEST_CONTEXT.with(|ctx| {
        *ctx.borrow_mut() = None;
    });
}

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let request_id = uuid::Uuid::new_v4().to_string();
    init_request_context(request_id);

    let result = handle_request(req, env).await;

    // Always cleanup, even on error
    cleanup_request_context();

    result
}
```

## Binary Size Optimization

### Why Size Matters

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Cold Start Impact                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Binary Size    Parse Time    Compile Time    Total Cold Start     │
│  ────────────   ──────────    ────────────    ────────────────     │
│  100KB          ~1ms          ~2ms            ~3ms                 │
│  500KB          ~3ms          ~8ms            ~11ms                │
│  1MB            ~5ms          ~15ms           ~20ms                │
│  3MB            ~12ms         ~40ms           ~52ms                │
│  5MB+           ~20ms         ~70ms           ~90ms+               │
│                                                                     │
│  Target: <1MB for sub-20ms cold starts                             │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Cargo.toml Optimization

```toml
[profile.release]
# Size optimization
opt-level = "s"        # Optimize for size ('z' for even smaller)
lto = true             # Link-time optimization
codegen-units = 1      # Single codegen unit for better optimization
panic = "abort"        # Don't include panic unwinding code
strip = true           # Strip symbols

[profile.release.package."*"]
opt-level = "s"
```

### Code-Level Optimizations

```rust
// AVOID: Generic functions create code bloat
fn process_generic<T: Serialize>(item: T) -> String {
    serde_json::to_string(&item).unwrap()
}

// BETTER: Use trait objects for smaller binary
fn process_dynamic(item: &dyn erased_serde::Serialize) -> String {
    serde_json::to_string(item).unwrap()
}

// AVOID: Large match statements with many arms
match tool_name {
    "tool1" => handle_tool1(),
    "tool2" => handle_tool2(),
    // ... 50 more tools
}

// BETTER: Use a lookup table
lazy_static! {
    static ref TOOL_HANDLERS: HashMap<&'static str, fn() -> Result<Value>> = {
        let mut m = HashMap::new();
        m.insert("tool1", handle_tool1 as fn() -> Result<Value>);
        m.insert("tool2", handle_tool2 as fn() -> Result<Value>);
        m
    };
}
```

### Measuring Binary Size

```bash
# Build for release
wrangler build

# Check size
ls -lh build/worker/shim.mjs
wasm-opt --print-size build/*.wasm

# Analyze what's taking space
cargo install twiggy
twiggy top build/*.wasm
twiggy dominators build/*.wasm
```

## Local Testing

### Setting Up the Test Environment

```bash
# Install wrangler
npm install -g wrangler

# Install wasm testing tools
cargo install wasm-pack

# Create test configuration
cat > wrangler.test.toml << 'EOF'
name = "my-mcp-worker-test"
main = "build/worker/shim.mjs"
compatibility_date = "2024-01-01"

[dev]
port = 8787
local_protocol = "http"

[[kv_namespaces]]
binding = "TEST_KV"
id = "test-kv-id"
preview_id = "test-kv-preview"

[[d1_databases]]
binding = "TEST_DB"
database_name = "test-db"
database_id = "local"
EOF
```

### Unit Tests with wasm-bindgen-test

```rust
// tests/wasm.rs
#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn test_json_parsing() {
    let json = r#"{"name": "test"}"#;
    let value: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(value["name"], "test");
}

#[wasm_bindgen_test]
async fn test_async_operation() {
    use wasm_bindgen_futures::JsFuture;

    // Test that async operations work
    let promise = js_sys::Promise::resolve(&42.into());
    let result = JsFuture::from(promise).await.unwrap();
    assert_eq!(result, 42);
}

#[wasm_bindgen_test]
fn test_uuid_generation() {
    // Ensure getrandom works in WASM
    let id = uuid::Uuid::new_v4();
    assert!(!id.is_nil());
}
```

```bash
# Run WASM tests
wasm-pack test --headless --chrome
wasm-pack test --headless --firefox
```

### Integration Testing with Miniflare

```javascript
// test/integration.mjs
import { Miniflare } from 'miniflare';

const mf = new Miniflare({
  scriptPath: './build/worker/shim.mjs',
  modules: true,
  kvNamespaces: ['KV'],
  d1Databases: ['DB'],
});

// Test MCP initialize
const initResponse = await mf.dispatchFetch('http://localhost/mcp', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    jsonrpc: '2.0',
    id: 1,
    method: 'initialize',
    params: {
      protocolVersion: '2024-11-05',
      capabilities: {},
      clientInfo: { name: 'test', version: '1.0' }
    }
  })
});

const result = await initResponse.json();
console.assert(result.result.protocolVersion === '2024-11-05');
console.log('Initialize test passed!');
```

```bash
# Run integration tests
npx wrangler dev --test
node test/integration.mjs
```

### Local Development Server

```bash
# Start local dev server
wrangler dev

# In another terminal, test with curl
curl -X POST http://localhost:8787/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/list",
    "params": {}
  }'
```

## Debugging WASM

### Console Logging

```rust
use worker::console_log;

// Simple logging
console_log!("Processing request: {}", request_id);

// Structured logging
fn log_json(label: &str, value: &impl serde::Serialize) {
    let json = serde_json::to_string_pretty(value).unwrap_or_default();
    console_log!("{}: {}", label, json);
}

// Debug logging (only in dev)
#[cfg(debug_assertions)]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        console_log!("[DEBUG] {}", format!($($arg)*))
    };
}

#[cfg(not(debug_assertions))]
macro_rules! debug_log {
    ($($arg:tt)*) => {};
}
```

### Panic Handling

```rust
use console_error_panic_hook;

// Set up panic hook at worker start
pub fn init_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

// In your main function
#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    init_panic_hook();
    // ... rest of handler
}
```

### Source Maps

```toml
# wrangler.toml
[build]
command = "cargo install worker-build && worker-build --release"

[build.upload]
format = "modules"
main = "./build/worker/shim.mjs"

# Enable source maps for debugging
[env.dev]
[env.dev.build]
command = "worker-build --dev"
```

### Performance Profiling

```rust
use worker::*;

struct Timer {
    label: String,
    start: f64,
}

impl Timer {
    fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            start: js_sys::Date::now(),
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        let elapsed = js_sys::Date::now() - self.start;
        console_log!("[PERF] {}: {:.2}ms", self.label, elapsed);
    }
}

// Usage
async fn handle_tool_call(env: &Env, tool: &str) -> Result<Value> {
    let _timer = Timer::new(&format!("tool:{}", tool));

    // ... tool implementation

    Ok(json!({"result": "done"}))
} // Timer logs duration when dropped
```

## Complete WASM-Compatible MCP Server

Here's a complete example bringing all concepts together:

```rust
// src/lib.rs
use worker::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

mod tools;
mod error;

use error::{McpError, McpResult};

// Initialize panic hook for better error messages
fn init() {
    console_error_panic_hook::set_once();
}

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    init();

    // CORS headers for browser clients
    let cors_headers = Headers::new();
    cors_headers.set("Access-Control-Allow-Origin", "*")?;
    cors_headers.set("Access-Control-Allow-Methods", "POST, OPTIONS")?;
    cors_headers.set("Access-Control-Allow-Headers", "Content-Type")?;

    // Handle CORS preflight
    if req.method() == Method::Options {
        return Response::empty()
            .map(|r| r.with_headers(cors_headers));
    }

    // Only accept POST to /mcp
    if req.method() != Method::Post || req.path() != "/mcp" {
        return Response::error("Not Found", 404);
    }

    // Parse and handle MCP request
    let result = handle_mcp_request(req, &env).await;

    match result {
        Ok(response) => Response::from_json(&response)
            .map(|r| r.with_headers(cors_headers)),
        Err(e) => {
            console_log!("Error: {}", e);
            Response::from_json(&json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -32603,
                    "message": e.to_string()
                }
            }))
            .map(|r| r.with_headers(cors_headers))
        }
    }
}

async fn handle_mcp_request(mut req: Request, env: &Env) -> McpResult<Value> {
    let body: Value = req.json().await
        .map_err(|e| McpError::InvalidRequest(e.to_string()))?;

    let method = body["method"].as_str()
        .ok_or_else(|| McpError::InvalidRequest("Missing method".into()))?;
    let id = &body["id"];
    let params = &body["params"];

    let result = match method {
        "initialize" => handle_initialize(params),
        "tools/list" => handle_tools_list(),
        "tools/call" => handle_tool_call(env, params).await,
        _ => Err(McpError::InvalidRequest(format!("Unknown method: {}", method))),
    }?;

    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    }))
}

fn handle_initialize(_params: &Value) -> McpResult<Value> {
    Ok(json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "wasm-mcp-server",
            "version": "1.0.0"
        }
    }))
}

fn handle_tools_list() -> McpResult<Value> {
    Ok(json!({
        "tools": [
            {
                "name": "query_data",
                "description": "Query the D1 database",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "sql": {
                            "type": "string",
                            "description": "SQL query to execute"
                        }
                    },
                    "required": ["sql"]
                }
            },
            {
                "name": "store_value",
                "description": "Store a value in KV",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "key": { "type": "string" },
                        "value": { "type": "string" }
                    },
                    "required": ["key", "value"]
                }
            }
        ]
    }))
}

async fn handle_tool_call(env: &Env, params: &Value) -> McpResult<Value> {
    let tool_name = params["name"].as_str()
        .ok_or_else(|| McpError::InvalidRequest("Missing tool name".into()))?;
    let arguments = &params["arguments"];

    match tool_name {
        "query_data" => tools::query_data(env, arguments).await,
        "store_value" => tools::store_value(env, arguments).await,
        _ => Err(McpError::InvalidRequest(format!("Unknown tool: {}", tool_name))),
    }
}
```

```rust
// src/tools.rs
use worker::*;
use serde_json::{json, Value};
use crate::error::{McpError, McpResult};

pub async fn query_data(env: &Env, args: &Value) -> McpResult<Value> {
    let sql = args["sql"].as_str()
        .ok_or_else(|| McpError::InvalidRequest("Missing sql parameter".into()))?;

    // Validate query (read-only)
    let sql_upper = sql.to_uppercase();
    if !sql_upper.starts_with("SELECT") {
        return Err(McpError::InvalidRequest("Only SELECT queries allowed".into()));
    }

    let d1 = env.d1("DB")
        .map_err(|e| McpError::DatabaseError(e.to_string()))?;

    let results = d1.prepare(sql)
        .all()
        .await
        .map_err(|e| McpError::DatabaseError(e.to_string()))?;

    let rows: Vec<Value> = results.results()
        .map_err(|e| McpError::DatabaseError(e.to_string()))?;

    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&rows).unwrap_or_default()
        }]
    }))
}

pub async fn store_value(env: &Env, args: &Value) -> McpResult<Value> {
    let key = args["key"].as_str()
        .ok_or_else(|| McpError::InvalidRequest("Missing key".into()))?;
    let value = args["value"].as_str()
        .ok_or_else(|| McpError::InvalidRequest("Missing value".into()))?;

    let kv = env.kv("KV")
        .map_err(|e| McpError::DatabaseError(e.to_string()))?;

    kv.put(key, value)
        .map_err(|e| McpError::DatabaseError(e.to_string()))?
        .execute()
        .await
        .map_err(|e| McpError::DatabaseError(e.to_string()))?;

    Ok(json!({
        "content": [{
            "type": "text",
            "text": format!("Stored value at key: {}", key)
        }]
    }))
}
```

```rust
// src/error.rs
use std::fmt;

#[derive(Debug)]
pub enum McpError {
    InvalidRequest(String),
    DatabaseError(String),
    Timeout,
}

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpError::InvalidRequest(msg) => write!(f, "Invalid request: {}", msg),
            McpError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            McpError::Timeout => write!(f, "Operation timed out"),
        }
    }
}

pub type McpResult<T> = std::result::Result<T, McpError>;
```

## Summary

Building WASM-compatible Rust MCP servers requires understanding:

1. **Runtime constraints** - No filesystem, threads, or system calls
2. **Crate compatibility** - Use WASM-aware crates with correct features
3. **Async patterns** - JavaScript event loop, not tokio
4. **Memory management** - 128MB limit, no automatic cleanup between requests
5. **Binary optimization** - Keep under 1MB for fast cold starts
6. **Testing strategies** - Combine unit tests, WASM tests, and Miniflare integration tests

The constraints push you toward cleaner, more portable code that runs efficiently at the edge.

## Exercises

### Exercise 1: Crate Audit
Review your existing Rust project's dependencies and identify which crates need WASM-specific configuration or replacement.

### Exercise 2: Memory Profiling
Build a test Worker that processes large JSON payloads and measure memory usage across multiple requests.

### Exercise 3: Binary Size Optimization
Take an existing Worker and reduce its binary size by 50% while maintaining functionality.

## Additional Resources

- [WebAssembly Specification](https://webassembly.github.io/spec/)
- [Cloudflare Workers Rust Documentation](https://developers.cloudflare.com/workers/languages/rust/)
- [wasm-bindgen Guide](https://rustwasm.github.io/wasm-bindgen/)
- [The `worker` Crate Documentation](https://docs.rs/worker)
