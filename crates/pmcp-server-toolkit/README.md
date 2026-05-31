# pmcp-server-toolkit

Runtime library for config-driven MCP servers — auth, secrets, static resources/prompts, `[[tools]]` synthesizer, code-mode wiring.

**Status:** 0.1.0 — early access. Public API may evolve as DX matures across Phases 84–89.

## What this crate is

`pmcp-server-toolkit` lifts the operational glue that pmcp-run servers share into the public SDK, so a `main.rs` can wire a config-driven MCP server in ~15 lines. The crate is organised as a flat module set (Phase 83 decision D-15):

- `auth` — `AuthProvider` implementations (`StaticAuthProvider`, `BearerAuthProvider`).
- `secrets` — `SecretsProvider` trait + env/AWS implementations and the `SecretValue` newtype that never leaks via `Debug`/`Display`/`Serialize`.
- `config` — `ServerConfig` types with `#[serde(deny_unknown_fields)]` strictness.
- `prompts` — `StaticPromptHandler` adapter for static prompt templates.
- `resources` — `StaticResourceHandler` adapter for shipped resources.
- `tools` — `synthesize_from_config` builder turning `[[tools]]` config rows into `ToolInfo` + `Arc<dyn ToolHandler>` pairs.
- `sql` — `SqlConnector` trait + dialect enum for backend-agnostic SQL toolkits.
- `builder_ext` — `ServerBuilderExt` trait adding `tools_from_config` / `code_mode_from_config` to `pmcp::ServerBuilder`.
- `code_mode` *(feature `code-mode`, default-on)* — re-exports from `pmcp-code-mode` plus toolkit-side wiring (`executor_from_config`, `assemble_code_mode_prompt`).
- `http` *(feature `http`)* — OpenAPI/REST backend connector + outgoing `[backend.auth]` providers (`HttpAuthProvider`), including the `oauth_passthrough` relay.
- `error` — `ToolkitError` enum and the crate-level `Result<T>` alias.

### `oauth_passthrough` trust boundary (WR-04)

The `http` module's `oauth_passthrough` `[backend.auth]` variant relays a **client-controlled** credential into an **operator-controlled** destination, and the trust posture must be explicit:

- The **MCP client controls the forwarded token VALUE** — it is the raw inbound `Authorization` header the client sent, captured by the binary's token-capture provider and forwarded verbatim (a bare token is prefixed with `Bearer `).
- The **operator controls the destination header NAME** — `target_header` is fixed in the committed config; the client cannot redirect the token elsewhere.

Forwarding the client's own credential to the backend is the **intended** SSO-passthrough behavior — use it only when the backend should receive the MCP client's own identity. The `HeaderValue::try_from` control-character rejection is the protection against header injection. For a server-side service credential, use a static variant (`bearer` / `oauth2_client_credentials`) instead.

## What this crate is NOT

- **Not a DynamoDB toolkit.** Phase 83 dropped `ddb` / `dynamo-config` from the feature matrix (D-14). Use `pmcp-tasks` for DynamoDB-backed task storage.
- **Not a generic OpenAPI / JavaScript runtime.** The `openapi-code-mode`, `js-runtime`, and `mcp-code-mode` features stay in `pmcp-code-mode` and are not re-exported via the toolkit (D-14).
- **Not WASM-ready in 0.1.0.** No `wasm32-*` target support yet — Phase 83 RESEARCH §"Open Questions" #3 defers WASM until Phase 84+.

## Quickstart

The eventual Shape C target is a ~15-line `main.rs` that loads a config TOML and ships a working MCP server with synthesized tools, static prompts/resources, and code-mode wiring. Phase 86 owns the runnable example; until then the API sketch is:

```rust,ignore
use pmcp::ServerBuilder;
use pmcp_server_toolkit::{builder_ext::ServerBuilderExt, config::ServerConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ServerConfig::from_toml_path("config.toml")?;
    let server = ServerBuilder::new()
        .tools_from_config(&config)?
        .code_mode_from_config(&config)?
        .build();
    server.serve_stdio().await
}
```

## Design context

For the architectural responsibility map, review notes, and locked decisions, see [`.claude/skills/spike-findings-rust-mcp-sdk/SKILL.md`](../../.claude/skills/spike-findings-rust-mcp-sdk/SKILL.md) and the `.planning/phases/83-toolkit-core-lift-pmcp-server-toolkit/` design log.
