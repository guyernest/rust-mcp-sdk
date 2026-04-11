# pmcp-macros include_str! POC

This file is the proof-of-concept gate for Phase 66. Its sole purpose is to
prove that:

1. `#![doc = include_str!("../POC_README.md")]` wires a Markdown file into
   `cargo test --doc -p pmcp-macros`.
2. A `rust,no_run` code block that imports `pmcp::mcp_tool` via the crate's
   `pmcp` dev-dependency (re-exported under `features = ["macros"]`) compiles
   successfully from the same `pmcp-macros` crate that defines the macro.

If this doctest compiles, Wave 1+ plans can safely wire the real
`pmcp-macros/README.md` via `include_str!` and trust the cycle.

The POC uses the zero-argument `#[mcp_tool]` form with an untyped
`serde_json::Value` return — the minimal shape that exercises the macro
without pulling in `schemars` argument derive boilerplate.

```rust,no_run
use pmcp::mcp_tool;

#[mcp_tool(description = "Get server version")]
async fn version() -> pmcp::Result<serde_json::Value> {
    Ok(serde_json::json!({ "version": "0.5.0" }))
}
```

Success criterion: `cargo test --doc -p pmcp-macros` reports at least one
passing doctest (the block above) and zero failures.

This file will be deleted or replaced in a subsequent plan (66-02+) once the
real `README.md` rewrite lands and the crate-level doc attribute is pointed
at `../README.md` instead. It intentionally does not duplicate any user-facing
documentation — it is a gate, not content.
