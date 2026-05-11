# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.1] - 2026-05-10

### Changed
- **Phase 75.5 — Cognitive-complexity refactors.** `expand_mcp_tool`,
  `expand_mcp_prompt`, `expand_mcp_resource`, `expand_mcp_server`, and the
  `collect_{tool,prompt,resource}_methods` helpers all refactored to ≤ cog 25
  per workspace quality gate. Cargo-expand snapshot baselines added to lock the
  generated output. No behavioral changes intended.

## [0.5.0] - 2026-04-11

### Removed (breaking)

- **`#[tool]` macro removed.** Deprecated since 0.3.0. Use `#[mcp_tool]`.
  `#[mcp_tool]` provides compile-time `description` enforcement, `State<T>`
  injection, async/sync auto-detection, and `annotations(...)` support.
- **`#[tool_router]` macro removed.** Deprecated since 0.3.0. Use `#[mcp_server]`.
  `#[mcp_server]` collects tools on an `impl` block and exposes them via
  `ServerBuilder::mcp_server(...)`.
- **`#[prompt]` zero-op stub removed.** This macro was a placeholder identity
  function that generated no code. Use `#[mcp_prompt]` for the functional
  equivalent.
- **`#[resource]` zero-op stub removed.** This macro was a placeholder identity
  function that generated no code. Use `#[mcp_resource]` for the functional
  equivalent.
- **`tool_router_dev` Cargo feature removed.** Gated the deleted
  `#[tool_router]` integration tests — no longer referenced by any source.

A total of 898 lines of deprecated/stub source were removed across 6 files
(`src/tool.rs`, `src/tool_router.rs`, `tests/tool_tests.rs`,
`tests/tool_router_tests.rs`, `tests/ui/tool_missing_description.rs` plus its
`.stderr` snapshot). The `lib.rs` crate root shrank from 374 to 226 lines after
deleting the four deprecated `pub fn` exports and their documentation.

### Migration from 0.4.x

#### `#[tool]` → `#[mcp_tool]`

Before:

```rust,ignore
#[tool(description = "Add two numbers")]
async fn add(params: AddParams) -> Result<AddResult, String> {
    Ok(AddResult { sum: params.a + params.b })
}
```

After:

```rust,ignore
#[mcp_tool(description = "Add two numbers")]
async fn add(args: AddArgs) -> pmcp::Result<AddResult> {
    Ok(AddResult { sum: args.a + args.b })
}
```

Behavioral differences worth calling out:

- **`description` is enforced at compile time** — no more runtime
  `Option<String>`. A `#[mcp_tool]` without a description fails to build with
  a clear error pointing at the attribute.
- **Return type is `pmcp::Result<T>`** instead of `Result<T, String>`. This
  unifies tool errors with the rest of the SDK and removes boilerplate string
  conversions.
- **Shared state via `State<T>` parameter** — no `Arc::clone` boilerplate.
  Declare `state: State<MyStateType>` in the signature and the macro injects
  the registered state from `ServerBuilder::state(...)`.
- **Async/sync auto-detection** from the `fn` signature — write `async fn` or
  `fn`, the macro does the right thing. No more manual `Box::pin(async move {
  ... })` wrapping.
- **MCP annotations via `annotations(read_only = true, destructive = true,
  ...)`** on the attribute — supports the full MCP annotation surface
  including `title`, `idempotent`, `open_world`, etc.
- **Registration via `.tool("add", add())`** — the macro generates a zero-arg
  constructor returning a `ToolHandler`-implementing struct. No more
  hand-writing `Arc::new(Mutex::new(state))` wiring at registration time.

#### `#[tool_router]` → `#[mcp_server]`

Before:

```rust,ignore
#[tool_router]
impl Calculator {
    #[tool(description = "Add")]
    async fn add(&self, a: i32, b: i32) -> Result<i32, String> {
        Ok(a + b)
    }
}
```

After:

```rust,ignore
#[mcp_server]
impl Calculator {
    #[mcp_tool(description = "Add")]
    async fn add(&self, args: AddArgs) -> pmcp::Result<AddResult> {
        Ok(AddResult { sum: args.a + args.b })
    }
}

// Register all tools on the impl block in one call:
let builder = ServerBuilder::new().mcp_server(calculator);
```

`#[mcp_server]` inherits the same compile-time checking and state-injection
semantics as `#[mcp_tool]` — every `#[mcp_tool]`-annotated method on the `impl`
block becomes a registered tool automatically.

#### `#[prompt]` and `#[resource]` zero-op stubs

These macros never generated any code in 0.4.x — they were placeholder identity
functions left over from an early scaffolding phase. Use `#[mcp_prompt]` and
`#[mcp_resource]` for the real, functional equivalents. See the rewritten
[README](README.md) for usage, attributes, and URI-template examples.

### Changed

- **Crate-level documentation is now sourced from `README.md`** via
  `#![doc = include_str!("../README.md")]`. docs.rs and GitHub render the
  exact same 355-line document from a single source — no more stale
  `pmcp = "1.1"` crate-root docs drifting behind the current API.
- **README and per-macro `///` doc comments use `rust,no_run` code blocks**
  (compiled under `cargo test --doc`) instead of `rust,ignore`. API drift is
  caught automatically by the quality gate — any re-name or signature change
  that invalidates a documented example will fail `cargo test --doc -p
  pmcp-macros` and block the commit.
- **Per-macro examples reference the renamed `examples/s23_mcp_tool_macro.rs`
  and `examples/s24_mcp_prompt_macro.rs`** files from Phase 65. The previous
  `63_`/`64_` numbers have been removed from all rustdoc comments and from
  the runnable example headers themselves.

## [0.4.1] - 2026-04-06

Prior history was tracked only in the workspace root `CHANGELOG.md`. See the
root [`CHANGELOG.md`](../CHANGELOG.md) for entries predating this file.
