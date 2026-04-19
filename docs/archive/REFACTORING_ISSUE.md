# PMCP-001: Major Server Architecture Refactoring - Protocol/Transport Split for WASM/WASI Deployment

## Summary

Refactor the Rust MCP SDK server architecture to separate protocol handling from transport management, enabling deployment to WASM/WASI environments (Cloudflare Workers, Vercel Edge, etc.) and aligning with the TypeScript SDK architecture.

## Background

The current Rust MCP SDK has a tightly coupled architecture where the server directly manages transport lifecycle and protocol handling. This design prevents deployment to WASM/WASI environments and limits portability. The TypeScript SDK demonstrates a clean separation between these concerns, enabling deployment across diverse environments.

### Previous Attempts

- **wit-bindgen Integration**: Previous refactoring attempt failed due to improper WASI HTTP interface integration and feature configuration issues
- **WASI Server Example**: Incomplete implementation in `examples/33_wasi_server/` with compilation errors

## Problem Statement

### Current Architecture Issues

1. **Tight Coupling**: Server directly manages transport through `Arc<RwLock<Transport>>`
2. **WASM Incompatibility**: Server excluded from WASM builds (`#[cfg(not(target_arch = "wasm32"))]`)
3. **Tokio Dependencies**: Threading model incompatible with WASM single-threaded execution
4. **Stateful Design**: Server lifecycle tied to transport connection
5. **No Protocol Abstraction**: Request handling directly calls transport methods

### Business Impact

- Cannot deploy to cost-effective serverless platforms (Cloudflare Workers, AWS Lambda@Edge)
- Limited scalability compared to edge computing solutions
- Higher operational costs for traditional server deployments
- Reduced portability compared to TypeScript SDK

## Proposed Solution

### Architecture Overview

Implement a three-layer architecture following the TypeScript SDK pattern:

```
┌─────────────────────────────────────────────────┐
│                Application Layer                 │
│         (Tools, Resources, Prompts, etc.)        │
└─────────────────────────────────────────────────┘
                        │
┌─────────────────────────────────────────────────┐
│                 Protocol Layer                   │
│     (Request Handling, Response Correlation)     │
└─────────────────────────────────────────────────┘
                        │
┌─────────────────────────────────────────────────┐
│                Transport Layer                   │
│    (STDIO, HTTP, WebSocket, WASI HTTP, etc.)     │
└─────────────────────────────────────────────────┘
```

### Core Components

#### 1. Protocol Handler Trait

```rust
/// Protocol-agnostic request handler
#[async_trait]
pub trait ProtocolHandler: Send + Sync {
    /// Handle a single request without transport knowledge
    async fn handle_request(&self, request: Request) -> Result<Response>;
    
    /// Handle notifications (no response expected)
    async fn handle_notification(&self, notification: Notification) -> Result<()>;
    
    /// Get server capabilities
    fn capabilities(&self) -> ServerCapabilities;
}
```

#### 2. Server Core (Transport-Independent)

```rust
/// Core server logic without transport dependencies
pub struct ServerCore {
    /// Method handlers
    handlers: HashMap<String, Arc<dyn RequestHandler>>,
    
    /// Server metadata
    info: ServerInfo,
    
    /// Capabilities
    capabilities: ServerCapabilities,
    
    /// Authentication provider (optional)
    auth_provider: Option<Arc<dyn AuthProvider>>,
}

impl ProtocolHandler for ServerCore {
    async fn handle_request(&self, request: Request) -> Result<Response> {
        // Pure request processing logic
        // No transport interaction
        // Stateless execution
    }
}
```

#### 3. Transport Adapter Pattern

```rust
/// Transport adapter for binding server to specific transport
#[async_trait]
pub trait TransportAdapter {
    /// Bind a protocol handler to this transport
    async fn serve(&self, handler: Arc<dyn ProtocolHandler>) -> Result<()>;
    
    /// Transport-specific configuration
    fn configure(&mut self, config: TransportConfig) -> Result<()>;
}

/// Example: STDIO adapter
pub struct StdioAdapter {
    input: Stdin,
    output: Stdout,
}

impl TransportAdapter for StdioAdapter {
    async fn serve(&self, handler: Arc<dyn ProtocolHandler>) -> Result<()> {
        loop {
            let request = self.read_request().await?;
            let response = handler.handle_request(request).await?;
            self.write_response(response).await?;
        }
    }
}
```

#### 4. WASI HTTP Adapter

```rust
/// WASI HTTP adapter for serverless deployment
pub struct WasiHttpAdapter;

impl WasiHttpAdapter {
    /// Handle a single HTTP request in WASI environment
    pub async fn handle_http_request(
        handler: Arc<dyn ProtocolHandler>,
        request: HttpRequest,
    ) -> HttpResponse {
        // Parse MCP request from HTTP
        let mcp_request = parse_mcp_request(request)?;
        
        // Process through handler (stateless)
        let mcp_response = handler.handle_request(mcp_request).await?;
        
        // Convert to HTTP response
        to_http_response(mcp_response)
    }
}

// WASI HTTP export using wit-bindgen
wit_bindgen::generate!({
    world: "wasi:http/proxy@0.2.0",
});

impl Guest for WasiHttpAdapter {
    fn handle(request: IncomingRequest) -> OutgoingResponse {
        // Bridge to MCP handler
    }
}
```

### Implementation Phases

#### Phase 1: Core Refactoring (2-3 weeks)

1. **Extract ServerCore**:
   - Separate server logic from transport management
   - Create stateless request handling
   - Remove tokio dependencies from core

2. **Define Protocol Traits**:
   - `ProtocolHandler` trait
   - `TransportAdapter` trait
   - Message conversion traits

3. **Migrate Existing Functionality**:
   - Port handler registration
   - Port authentication/authorization
   - Port capability management

#### Phase 2: Transport Adapters (2 weeks)

1. **STDIO Adapter**: Port existing stdio transport
2. **HTTP Adapter**: Port streamable HTTP server
3. **WebSocket Adapter**: Port WebSocket server
4. **Test Adapter**: Mock transport for testing

#### Phase 3: WASI Integration (2-3 weeks)

1. **wit-bindgen Setup**:
   - Proper WASI HTTP world integration
   - Request/response mapping
   - Error handling

2. **WASI HTTP Adapter**:
   - Stateless request handling
   - Session management via headers
   - Cloudflare Workers compatibility

3. **Build Configuration**:
   - WASI target configuration
   - Feature flags for conditional compilation
   - Optimization for size

#### Phase 4: Testing & Validation (1 week)

1. **Compatibility Testing**:
   - Ensure backward compatibility
   - Validate all transports work
   - Performance benchmarks

2. **WASI Deployment Testing**:
   - Cloudflare Workers deployment
   - Vercel Edge deployment
   - AWS Lambda@Edge testing

## Success Criteria

### Functional Requirements

- [ ] Server core compiles for WASM/WASI targets
- [ ] All existing transports continue to work
- [ ] WASI HTTP adapter handles MCP requests
- [ ] Successfully deploy to Cloudflare Workers
- [ ] Performance parity with current implementation

### Quality Requirements

- [ ] Zero clippy warnings (Toyota Way compliance)
- [ ] 80%+ test coverage maintained
- [ ] Comprehensive documentation
- [ ] Working examples for each deployment target
- [ ] Property-based testing for protocol handling

### Architecture Alignment

- [ ] Protocol/transport separation matches TypeScript SDK
- [ ] Stateless request handling model
- [ ] Transport adapter pattern implemented
- [ ] Session management abstracted
- [ ] Capability system preserved

## Risk Mitigation

### Technical Risks

1. **WASM Async Complexity**:
   - **Risk**: Async trait limitations in WASM
   - **Mitigation**: Use wasm-bindgen-futures, avoid spawning

2. **Performance Regression**:
   - **Risk**: Additional abstraction layers impact performance
   - **Mitigation**: Benchmark-driven development, optimization passes

3. **Breaking Changes**:
   - **Risk**: API changes break existing users
   - **Mitigation**: Maintain compatibility layer, deprecation warnings

### Implementation Risks

1. **Scope Creep**:
   - **Risk**: Refactoring expands beyond initial scope
   - **Mitigation**: Strict phase boundaries, incremental delivery

2. **wit-bindgen Instability**:
   - **Risk**: WASI tooling changes during development
   - **Mitigation**: Pin versions, maintain fallback approach

## Alternative Approaches Considered

### 1. Minimal WASI Wrapper

Create a thin WASI wrapper around existing server without refactoring.

**Rejected because**:
- Doesn't solve fundamental architecture issues
- Limited scalability and maintainability
- No alignment with TypeScript SDK

### 2. Complete Rewrite

Start fresh with WASM-first architecture.

**Rejected because**:
- High risk of regression
- Loss of battle-tested code
- Longer development timeline

### 3. Conditional Compilation

Use extensive `#[cfg]` attributes for WASM support.

**Rejected because**:
- Code complexity increases significantly
- Difficult to maintain and test
- Poor developer experience

## Dependencies and Prerequisites

### Technical Dependencies

- Rust 1.75+ (for improved async trait support)
- wasm-bindgen 0.2.95+
- wit-bindgen 0.36+
- wasmtime 28.0+ (for testing)

### Knowledge Prerequisites

- Team familiarity with WASI component model
- Understanding of TypeScript SDK architecture
- Experience with serverless deployment

## Timeline Estimate

| Phase | Duration | Dependencies |
|-------|----------|--------------|
| Phase 1: Core Refactoring | 2-3 weeks | None |
| Phase 2: Transport Adapters | 2 weeks | Phase 1 |
| Phase 3: WASI Integration | 2-3 weeks | Phase 2 |
| Phase 4: Testing & Validation | 1 week | Phase 3 |
| **Total** | **6-8 weeks** | |

## Implementation Checklist

### Preparation

- [ ] Review TypeScript SDK architecture documentation
- [ ] Set up WASI development environment
- [ ] Create refactoring branch
- [ ] Write architecture decision record (ADR)

### Phase 1 Tasks

- [ ] Extract `ServerCore` from current `Server`
- [ ] Define `ProtocolHandler` trait
- [ ] Define `TransportAdapter` trait
- [ ] Create message conversion utilities
- [ ] Port handler registration system
- [ ] Port authentication system
- [ ] Port capability management
- [ ] Write unit tests for ServerCore

### Phase 2 Tasks

- [ ] Implement `StdioAdapter`
- [ ] Implement `HttpAdapter`
- [ ] Implement `WebSocketAdapter`
- [ ] Create `MockAdapter` for testing
- [ ] Update existing examples to use adapters
- [ ] Write integration tests for each adapter

### Phase 3 Tasks

- [ ] Configure wit-bindgen for WASI HTTP
- [ ] Implement `WasiHttpAdapter`
- [ ] Create WASI build configuration
- [ ] Write WASI-specific examples
- [ ] Test in wasmtime
- [ ] Deploy to Cloudflare Workers (test)
- [ ] Optimize WASM bundle size

### Phase 4 Tasks

- [ ] Run full test suite
- [ ] Benchmark performance
- [ ] Update documentation
- [ ] Create migration guide
- [ ] Deploy examples to production
- [ ] Gather feedback and iterate

## References

- [TypeScript SDK Architecture](https://github.com/modelcontextprotocol/sdk/tree/main/typescript)
- [WASI HTTP Proxy World](https://github.com/WebAssembly/wasi-http)
- [wit-bindgen Documentation](https://github.com/bytecodealliance/wit-bindgen)
- [Cloudflare Workers WASI Support](https://developers.cloudflare.com/workers/runtime-apis/webassembly/)
- [Previous wit-bindgen attempt (failed)](examples/33_wasi_server/)

## Success Metrics

- **Performance**: < 10% regression in request handling latency
- **Size**: WASM bundle < 5MB (optimized)
- **Deployment**: Successfully running on 3+ serverless platforms
- **Adoption**: 5+ example deployments within first month
- **Quality**: Zero defects (Toyota Way compliance)

## Open Questions

1. Should we maintain a compatibility layer for the old API?
2. How do we handle stateful operations (subscriptions) in stateless environments?
3. Should transport adapters be in separate crates?
4. What's the migration path for existing users?
5. How do we handle authentication in serverless environments?

## Next Steps

1. **Review and approve this design document**
2. **Create feature branch**: `feature/protocol-transport-split`
3. **Set up WASI development environment**
4. **Begin Phase 1 implementation**
5. **Weekly progress reviews**

---

**Priority**: P0 (Critical for SDK competitiveness)
**Labels**: `refactoring`, `wasm`, `architecture`, `performance`
**Milestone**: v1.0.0
**Assignee**: TBD