# Codebase Structure

**Analysis Date:** 2026-02-26

## Directory Layout

```
rust-mcp-sdk/
├── src/                           # Main SDK crate (pmcp v1.10.3)
│   ├── lib.rs                     # Public API and re-exports
│   ├── wasi.rs                    # WASI HTTP adapter stub
│   ├── assets/                    # Static assets (documentation, UI templates)
│   ├── client/                    # MCP client implementation
│   ├── composition/               # SDK composition for multi-server integration
│   ├── error/                     # Error types and definitions
│   ├── runtime/                   # Cross-platform runtime abstraction (native vs WASM)
│   ├── server/                    # MCP server implementation (core, handlers, features)
│   ├── shared/                    # Shared utilities (transport, protocol, middleware)
│   ├── simd/                      # SIMD optimizations (feature-gated)
│   ├── types/                     # MCP protocol type definitions
│   └── utils/                     # Utility functions and helpers
├── examples/                      # Numbered examples (01-60) demonstrating features
├── crates/                        # Workspace members
│   ├── pmcp-tasks/                # Tasks feature extension (experimental)
│   ├── mcp-tester/                # Testing utilities and scenarios
│   ├── mcp-preview/               # Preview/visualization server
│   └── mcp-e2e-tests/             # End-to-end testing infrastructure
├── examples/25-oauth-basic/       # OAuth example crate
├── examples/test-basic/           # Basic testing example
├── cargo-pmcp/                    # CLI tool for PMCP
├── pmcp-course/                   # Educational course materials
├── Cargo.toml                     # Main crate manifest with workspace definition
└── justfile                       # Task runner for common operations
```

## Directory Purposes

**src/client:**
- Purpose: MCP client implementation for consuming servers
- Contains: `Client<T>` generic type, request builders, transport adapters, middleware
- Key files: `mod.rs` (main client), `transport/` (transport integration), `auth.rs` (client auth), `oauth_middleware.rs`

**src/server:**
- Purpose: MCP server implementation and feature modules
- Contains: Core server logic, handler registration, all optional features
- Key subdirectories:
  - `auth/`: OAuth2, JWT, OIDC providers and authentication middleware
  - `workflow/`: Workflow-based prompt system with DSL and type handles
  - `observability/`: Tracing, metrics, and observability infrastructure
  - `transport/`: WebSocket and enhanced transport implementations
- Key files:
  - `mod.rs`: Main exports, handler trait definitions
  - `core.rs`: Protocol-agnostic `ServerCore` implementation
  - `builder.rs`: `ServerCoreBuilder` for fluent server configuration
  - `traits.rs`: Handler trait definitions (`ToolHandler`, `PromptHandler`, etc.)
  - `typed_tool.rs`: Schema-driven typed tools with auto-validation
  - `dynamic_resources.rs`: Pattern-based resource routing
  - `middleware_executor.rs`: Tool middleware chain execution
  - `cancellation.rs`: Request cancellation management
  - `progress.rs`: Progress notification tracking
  - `validation.rs`: Schema and input validation helpers
  - `simple_tool.rs`, `simple_resources.rs`, `simple_prompt.rs`: Basic handler implementations

**src/shared:**
- Purpose: Shared utilities for client and server
- Contains: Protocol implementation, transport abstraction, middleware, session management
- Key files:
  - `transport.rs`: `Transport` trait and `TransportMessage` type
  - `protocol.rs`: JSON-RPC protocol state machine
  - `protocol_helpers.rs`: Request/response/notification creation helpers
  - `middleware.rs`: `Middleware` trait, `EnhancedMiddlewareChain`, concrete middleware
  - `middleware_presets.rs`: Pre-configured middleware stacks
  - `stdio.rs`: Standard input/output transport
  - `websocket.rs`: Native WebSocket transport (feature: "websocket")
  - `wasm_websocket.rs`: WASM WebSocket transport (feature: "websocket-wasm")
  - `http.rs`: HTTP/SSE transport (feature: "http")
  - `wasm_http.rs`: WASM HTTP transport
  - `streamable_http.rs`: Streamable HTTP for Axum (feature: "streamable-http")
  - `sse_optimized.rs`: Optimized SSE transport (feature: "sse")
  - `logging.rs`: Logging infrastructure and correlation IDs
  - `session.rs`: Session state management
  - `event_store.rs`: Event sourcing for resumption/recovery
  - `batch.rs`: Batch request/response handling
  - `reconnect.rs`: Reconnection logic with exponential backoff
  - `connection_pool.rs`: Connection pooling for multiple transports
  - `simd_parsing.rs`: SIMD-optimized JSON/HTTP parsing
  - `uri_template.rs`: URI template expansion for resources
  - `context.rs`: Request context and context propagation

**src/types:**
- Purpose: Protocol type definitions
- Contains: All MCP protocol request/response/notification types
- Key files:
  - `protocol.rs`: Main protocol types (Initialize, ListTools, CallTool, etc.)
  - `capabilities.rs`: Client and server capability definitions
  - `jsonrpc.rs`: JSON-RPC request/response envelope types
  - `auth.rs`: Authentication information types
  - `ui.rs`: UI resource types (SEP-1865 MCP Apps)
  - `mcp_apps.rs`: Extended UI metadata for ChatGPT Apps/MCP-UI
  - `elicitation.rs`: User input elicitation types
  - `completable.rs`: Completion request/result types
  - `mod.rs`: Re-exports of commonly used types

**src/error:**
- Purpose: Error type definitions
- Contains: `Error` enum, `ErrorCode`, `Result` type alias
- Key aspects: Maps to JSON-RPC error codes, supports detailed error information

**src/runtime:**
- Purpose: Cross-platform runtime abstraction
- Contains: Conditional imports for native (tokio) vs WASM (futures) runtimes
- Enables: Same code to run on both platforms without conditional compilation

**src/composition:**
- Purpose: Multi-server SDK composition capabilities
- Contains: Utilities for composing multiple MCP servers into unified interfaces
- Files: Error types specific to composition

**examples/**
- Purpose: Demonstrate SDK features with working code
- Structure: Numbered `NN_feature_name.rs` files (01-60)
- Coverage:
  - 01-02: Basic client/server
  - 03-07: Tools, resources, prompts
  - 08-12: Logging, auth, progress, cancellation, error handling
  - 13-20: WebSocket, sampling, middleware, OAuth, OIDC
  - 22-24: Streamable HTTP (stateful/stateless/client)
  - 27-29: Enhanced WebSocket, SSE, connection pooling
  - 30-37: Middleware, advanced errors, typed tools, schema generation
  - 49-57: Tool composition, workflows, dynamic resources
  - 60: Tasks basic example

**crates/pmcp-tasks/**
- Purpose: Experimental MCP Tasks extension
- Structure: `src/types/`, `src/domain/`, `src/store/` for task domain and storage
- Status: Separate crate for feature isolation

**crates/mcp-tester/**
- Purpose: Testing utilities and scenario runners
- Contains: Test framework, scenarios directory for test cases

**crates/mcp-preview/**
- Purpose: Interactive preview/visualization server
- Contains: Handlers for UI rendering and interaction

**crates/mcp-e2e-tests/**
- Purpose: End-to-end testing with full client-server communication
- Contains: Test harness for validating complete workflows

**cargo-pmcp/**
- Purpose: Command-line interface for PMCP tasks
- Contains: CLI tool implementation

## Key File Locations

**Entry Points:**
- `src/lib.rs`: Public SDK API - exports `Client`, `Server`, types, traits, middleware
- `examples/01_client_initialize.rs`: Minimal client example
- `examples/02_server_basic.rs`: Minimal server example

**Configuration:**
- `Cargo.toml`: Workspace definition, feature flags, dependencies
- `src/shared/protocol.rs`: Protocol configuration via `ProtocolOptions`
- `src/shared/logging.rs`: Logging configuration via `LogConfig`

**Core Logic:**
- `src/server/core.rs`: `ServerCore` - protocol-agnostic server implementation
- `src/server/builder.rs`: `ServerCoreBuilder` - fluent server construction
- `src/client/mod.rs`: `Client<T>` - generic client over any transport
- `src/shared/protocol.rs`: `Protocol` - JSON-RPC state machine
- `src/shared/middleware.rs`: Middleware trait and chain implementation

**Testing:**
- `src/server/core_tests.rs`: Core server tests
- `src/server/adapter_tests.rs`: Transport adapter tests
- `src/server/wasm_core_tests.rs`: WASM-specific tests
- `crates/mcp-e2e-tests/tests/`: End-to-end test suite

## Naming Conventions

**Files:**
- `mod.rs`: Module re-export and documentation
- `*_tests.rs`: Test module (integration tests in same file)
- `*_middleware.rs`: Middleware implementations
- `*.rs` (uppercase): Trait/type definitions
- Snake_case: File names for modules

**Directories:**
- Lower-case: Standard modules (server, client, types, shared)
- Feature-specific (auth/, workflow/, observability/): Grouped by functionality

**Module Structure:**
- Public re-exports in `mod.rs`
- Private implementation files with `_` prefix when multiple files define related functionality
- Example: `server/auth/mod.rs` re-exports from `oauth2.rs`, `jwt.rs`, `provider.rs`

**Types:**
- `PascalCase`: All public types (Client, Server, ServerCore, ToolHandler)
- `camelCase`: JSON serialization via `#[serde(rename_all = "camelCase")]`
- Nested: Capability types nest under `ServerCapabilities`, `ClientCapabilities`

**Traits:**
- `*Handler` suffix: Request/response handlers (ToolHandler, PromptHandler, ResourceHandler)
- `*Middleware` suffix: Middleware implementations (AuthMiddleware, LoggingMiddleware)
- `*Provider` suffix: Strategy implementations (AuthProvider, ObservabilityBackend)

## Where to Add New Code

**New Tool/Prompt/Resource:**
- Implementation: Create handler in application code or `src/server/simple_tool.rs`
- Registration: Use `ServerBuilder::tool()`, `.prompt()`, `.resource()` methods
- Tests: Add tests in same file or `src/server/core_tests.rs`

**New Transport:**
- Implementation: Create file `src/shared/{transport_name}.rs`
- Trait: Implement `Transport` trait from `src/shared/transport.rs`
- Feature: Add feature flag to `Cargo.toml` if optional
- Registration: Add to `lib.rs` re-exports, update `mod.rs`
- Examples: Add example in `examples/NN_{transport_name}.rs`

**New Middleware:**
- Implementation: Add to `src/shared/middleware.rs` or `src/shared/middleware_presets.rs`
- Trait: Implement `Middleware` or `AdvancedMiddleware` trait
- Integration: Construct middleware in builder or add to preset chain
- Tests: Add tests in same file

**New Handler Feature:**
- Implementation: Create file `src/server/{feature_name}.rs`
- Trait: Define as `*Handler` trait or extend existing handlers
- Builder: Add builder methods to `ServerCoreBuilder`
- Examples: Add to `examples/NN_{feature_name}.rs`

**New Server Capability:**
- Types: Add types to appropriate file in `src/types/`
- Handling: Implement in `ServerCore` request dispatch (src/server/core.rs)
- Builder: Add configuration to `ServerCoreBuilder`
- Example: Add working example in `examples/`

**Utilities:**
- Location: `src/utils/` for helper functions, `src/shared/` for shared infrastructure
- Export: Re-export from appropriate module's `mod.rs`

## Special Directories

**src/server/auth/:**
- Purpose: Authentication and authorization system
- Generated: No, hand-written
- Committed: Yes
- Contents:
  - `mod.rs`: Re-exports and architecture overview
  - `oauth2.rs`: OAuth2 provider implementation
  - `jwt.rs`: JWT token validation
  - `jwt_validator.rs`: Token signature verification
  - `middleware.rs`: Auth middleware for request processing
  - `provider.rs`: `AuthProvider` trait definition
  - `traits.rs`: `ToolAuthorizer` for per-tool authorization
  - `config.rs`: Configuration types
  - `mock.rs`: Mock provider for testing
  - `providers/`: Provider-specific implementations (Cognito, OIDC)

**src/server/workflow/:**
- Purpose: Type-safe, ergonomic prompt workflows
- Generated: No, hand-written
- Committed: Yes
- Contents:
  - `mod.rs`: Re-exports and architecture documentation
  - `dsl.rs`: Domain-specific language for workflows
  - `handles.rs`: Type-safe tool/resource handles
  - `prompt_handler.rs`: Workflow prompt execution
  - `task_prompt_handler.rs`: Task-specific workflow handler
  - `sequential.rs`: Sequential workflow builder
  - `workflow_step.rs`: Step definition types
  - `data_source.rs`: Data binding for workflow inputs
  - `conversion.rs`: Type conversion and expansion logic
  - `into_prompt_content.rs`: Protocol type conversion
  - `newtypes.rs`: Newtype wrappers for type safety
  - `error.rs`: Workflow-specific error types

**src/server/observability/:**
- Purpose: Tracing, metrics, and observability
- Generated: No, hand-written
- Committed: Yes
- Contents:
  - `mod.rs`: Re-exports and high-level overview
  - `config.rs`: Observability configuration
  - `backend.rs`: `ObservabilityBackend` trait with CloudWatch, console, null implementations
  - `middleware.rs`: Observability middleware for protocol/tool execution
  - `events.rs`: Event definitions for observability
  - `types.rs`: Type definitions for metrics and traces

**examples/:**
- Purpose: Working demonstrations of features
- Generated: No, hand-written source code
- Committed: Yes
- Pattern: `NN_feature_description.rs` with `[[example]]` entries in Cargo.toml
- Feature flags: Many examples require specific features (websocket, streamable-http, etc.)

**target/:**
- Purpose: Build artifacts
- Generated: Yes, by cargo build
- Committed: No (in .gitignore)

**docs/**
- Purpose: Design documents and planning
- Generated: No
- Committed: Yes
- Contents: Feature designs, architecture decisions, implementation plans

---

*Structure analysis: 2026-02-26*
