# Architecture

**Analysis Date:** 2026-02-26

## Pattern Overview

**Overall:** Layered architecture with protocol-agnostic core and pluggable transport implementations.

**Key Characteristics:**
- Transport abstraction enables deployment across native, WASM, and serverless environments
- Handler trait pattern for tools, prompts, resources, and sampling
- Middleware chain for cross-cutting concerns in both client and server
- Protocol-agnostic core (`ServerCore`) decoupled from transport mechanisms
- Feature-gated optional functionality (websocket, HTTP, streamable-http, validation, auth)

## Layers

**Protocol Layer:**
- Purpose: JSON-RPC request/response handling and validation
- Location: `src/shared/protocol.rs`, `src/shared/protocol_helpers.rs`
- Contains: Request/response state machine, pending request tracking, response routing
- Depends on: Transport layer
- Used by: Client, Server via transport adapters

**Types Layer:**
- Purpose: Protocol type definitions following MCP specification
- Location: `src/types/` (protocol.rs, capabilities.rs, jsonrpc.rs, auth.rs, ui.rs, elicitation.rs, completable.rs)
- Contains: Request/response types, capability definitions, notification types, auth schemas
- Depends on: serde for serialization
- Used by: All protocol, client, and server layers

**Transport Layer:**
- Purpose: Platform-specific message delivery (stdio, WebSocket, HTTP, SSE, streamable-HTTP)
- Location: `src/shared/transport.rs` (trait), `src/shared/stdio.rs`, `src/shared/websocket.rs`, `src/shared/http.rs`, `src/shared/sse_optimized.rs`, `src/shared/streamable_http.rs`, `src/shared/wasm_http.rs`, `src/shared/wasm_websocket.rs`
- Contains: Transport trait, concrete implementations per platform
- Depends on: Protocol layer, platform-specific dependencies (tokio, web-sys, hyper)
- Used by: Client, Server

**Middleware Layer:**
- Purpose: Cross-cutting concerns (logging, authentication, retry, rate limiting, compression)
- Location: `src/shared/middleware.rs`, `src/shared/middleware_presets.rs`, `src/client/http_middleware.rs`, `src/server/http_middleware.rs`, `src/server/tool_middleware.rs`
- Contains: `Middleware` trait, `MiddlewareChain`, `EnhancedMiddlewareChain` with priority-based execution, concrete middleware implementations
- Depends on: Shared types, logging framework
- Used by: Client (request/response pipeline), Server (tool execution, protocol handling)

**Server Core Layer:**
- Purpose: Protocol-agnostic business logic for MCP server operations
- Location: `src/server/core.rs`, `src/server/builder.rs`
- Contains: `ServerCore` struct, handler management (tools, prompts, resources, sampling), state management, request dispatching
- Depends on: Types layer, middleware layer, handler traits
- Used by: Transport adapters (native, WASM)

**Server Handler Traits:**
- Purpose: Interface contract for implementing MCP capabilities
- Location: `src/server/mod.rs`, `src/server/traits.rs`
- Contains: `ToolHandler`, `PromptHandler`, `ResourceHandler`, `SamplingHandler`, `ProtocolHandler`
- Depends on: Types layer, error handling
- Used by: Application code implementing MCP servers

**Server Feature Modules:**
- Purpose: Specialized server functionality
- Location: `src/server/`
- Contains:
  - `auth/`: OAuth2, JWT, OIDC, authentication middleware and providers
  - `workflow/`: Type-safe workflow-based prompt system with DSL
  - `typed_tool.rs`, `wasm_typed_tool.rs`: Schema-driven typed tool implementations
  - `dynamic_resources.rs`: Pattern-based resource routing
  - `task.rs`: MCP Tasks extension support
  - `observability/`: Tracing, metrics, and observability infrastructure
  - `validation.rs`: Schema validation helpers
  - `cancellation.rs`: Request cancellation management
  - `progress.rs`: Progress notification tracking
- Depends on: Core layer, types layer
- Used by: Server builder and applications

**Client Layer:**
- Purpose: MCP client for consuming MCP servers
- Location: `src/client/mod.rs`, `src/client/transport/`, `src/client/auth.rs`, `src/client/oauth_middleware.rs`
- Contains: `Client<T>` generic over transport, request dispatching, server capability tracking, initialization
- Depends on: Transport layer, protocol layer, types layer
- Used by: Client applications

**Shared Utilities:**
- Purpose: Common functionality for both client and server
- Location: `src/shared/` (batch.rs, session.rs, event_store.rs, uri_template.rs, logging.rs, context.rs, reconnect.rs, connection_pool.rs, simd_parsing.rs, sse_parser.rs)
- Contains: Batch operations, session management, event sourcing, URI template handling, logging infrastructure, context propagation, reconnection logic
- Depends on: Types layer, transport layer
- Used by: Client, Server

## Data Flow

**Client -> Server Request:**

1. Application calls `client.call_tool()`, `client.get_prompt()`, etc.
2. Client constructs request from types
3. Request passes through `EnhancedMiddlewareChain` (priority-ordered middleware)
4. Middleware may add auth headers, logging, retry logic, etc.
5. Protocol layer assigns `RequestId` and sends via `Transport`
6. Transport serializes to JSON and sends over platform-specific transport (stdio, WebSocket, HTTP, etc.)
7. Server receives message via Transport
8. Protocol layer deserializes and routes to `ProtocolHandler`
9. `ProtocolHandler` (in `ServerCore`) dispatches to appropriate handler (tool, prompt, resource, sampling)
10. Handler executes business logic, returns result
11. Result passes through tool middleware chain (if enabled)
12. Response sent back through transport

**Server -> Client Notification:**

1. Server calls `server.send_notification()` with notification type
2. Notification passes through protocol middleware
3. Transport sends to client
4. Client receives and delivers to registered notification handler

**State Management:**

- Client: `server_capabilities`, `client_capabilities`, `initialized` flag stored in `Client<T>`
- Server: Handler maps, capabilities, client capabilities stored in `ServerCore`
- Protocol: Pending request tracking in `Protocol` state machine
- Auth: Context passed from transport layer through `auth_context` parameter to `ProtocolHandler`
- Cancellation: `CancellationManager` tracks cancellation tokens across handler execution

## Key Abstractions

**Transport Abstraction:**
- Purpose: Enable deployment to multiple platforms without protocol changes
- Examples: `StdioTransport`, `WebSocketTransport`, `HttpTransport`, `WasmHttpTransport`, `WasmWebSocketTransport`, `StreamableHttpTransport`, `OptimizedSseTransport`
- Pattern: All implement `Transport` trait with `send()` and `receive()` async methods
- Enables: Native (tokio-based), WASM (futures-based), serverless (stateless HTTP) deployments

**Handler Abstraction:**
- Purpose: Allow applications to implement MCP capabilities without knowing transport/protocol details
- Examples: `ToolHandler`, `PromptHandler`, `ResourceHandler`, `SamplingHandler`
- Pattern: Traits with `async fn handle()` method, optional `metadata()` method
- Enables: Easy addition of new tools/prompts/resources via simple trait implementations

**Middleware Abstraction:**
- Purpose: Add cross-cutting concerns without modifying handler code
- Location: `src/shared/middleware.rs`
- Pattern: `Middleware` trait with `before_request()`, `after_response()`, `before_notification()`, optional priorities
- Examples: `AuthMiddleware`, `LoggingMiddleware`, `RetryMiddleware`, `RateLimitMiddleware`
- Enables: Authentication, observability, resilience without changing business logic

**Workflow System:**
- Purpose: Type-safe, ergonomic prompt composition
- Location: `src/server/workflow/`
- Pattern: Builder/DSL-based workflow definition with type handles
- Examples: `SequentialWorkflow`, `WorkflowPromptHandler`, `ToolHandle`, `ResourceHandle`
- Enables: Complex prompt building with automatic type safety and composition

**Typed Tools:**
- Purpose: Schema-driven tool definitions with automatic validation and code generation
- Location: `src/server/typed_tool.rs`, `src/server/wasm_typed_tool.rs`
- Pattern: Derives JSON schema from Rust types, generates input/output validators
- Examples: `TypedTool<Input, Output>`, `SyncTool<Input, Output>`
- Enables: Full type safety without manual schema definition

## Entry Points

**Server:**
- Location: `src/server/mod.rs` exports `ServerBuilder`, `Server`, traits
- Triggers: Application calls `Server::builder()` or `ServerCoreBuilder::new()`
- Responsibilities: Configure handlers, middleware, capabilities; select transport; initialize and run server

**Client:**
- Location: `src/client/mod.rs` exports `Client<T>`, `ClientBuilder`
- Triggers: Application instantiates `Client::new(transport)` or `ClientBuilder::new()`
- Responsibilities: Connect to server, initialize capabilities, issue requests, handle responses

**Examples:**
- Location: `examples/` numbered 01-60 (e.g., `01_client_initialize.rs`, `02_server_basic.rs`)
- Pattern: Each demonstrates specific capability (tools, resources, prompts, auth, workflow, WebSocket, etc.)

## Error Handling

**Strategy:** Hierarchical error types with JSON-RPC error codes and optional user elicitation.

**Patterns:**
- `pmcp::Error` wraps protocol errors with `ErrorCode`
- `ErrorCode` enum maps to JSON-RPC error codes (-32700 to -32000) plus MCP-specific codes
- `JSONRPCError` carries error code, message, data payload to client
- `elicitation` module provides structured error messages for client input prompts
- Workflow errors (`WorkflowError`) wrap step failures with step context
- Handler errors returned as `Result<Value, Error>` in tool/prompt/resource handlers

**Server Response:**
```
Match on error type → construct JSONRPCError with code/message → send via transport
```

## Cross-Cutting Concerns

**Logging:**
- Framework: `tracing` crate with optional file output
- Configuration: `src/shared/logging.rs`, initialized via `init_logging()`
- Middleware: `LoggingMiddleware` logs requests/responses with correlation IDs

**Validation:**
- Framework: Optional `jsonschema` + `garde` (feature-gated "validation")
- Locations: `src/server/validation.rs` for general validation, typed tools auto-validate via schema
- Usage: Server can validate tool inputs before calling handler

**Authentication:**
- Location: `src/server/auth/` (oauth2.rs, jwt.rs, middleware.rs, providers/)
- Pattern: `AuthProvider` trait implemented by OAuth2, JWT, mock providers
- Middleware: `AuthMiddleware` extracts tokens, `ToolAuthorizer` enforces per-tool auth
- Integration: `auth_context` passed from transport through protocol handler to middleware

**Cancellation:**
- Location: `src/server/cancellation.rs` (native), stub in `src/server/mod.rs` (WASM)
- Pattern: `CancellationManager` tracks tokens, `RequestHandlerExtra` carries cancellation info to handlers
- Usage: Handlers check cancellation status via `extra.cancellation_token()`

**Progress:**
- Location: `src/server/progress.rs`
- Pattern: Handlers report progress via `extra.send_progress()` calls
- Transport: Sends `ProgressNotification` to client

---

*Architecture analysis: 2026-02-26*
