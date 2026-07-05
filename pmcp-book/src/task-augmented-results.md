# Task-Augmented Tool Results (SEP-1686)

A normal, synchronous tool sometimes needs to hand the caller a pointer to a
task it just kicked off — "I started the export; poll THIS task for the result."
In MCP that pointer lives in the tool result's `_meta` under
`io.modelcontextprotocol/related-task`. This chapter shows how to emit it the
native way (pmcp 2.12+) and how to migrate off the hand-rolled workarounds that
used to lose it silently.

> This is the SEP-1686 junction. It is the complement of
> [MCP Tasks — Long-Running Operations](ch12-7-tasks.md), which covers exposing a
> tool AS an async task. Here the tool returns a normal result that merely
> *references* a task.

## The bug this replaces

A `ToolHandler` returns a `serde_json::Value`, and before 2.12 dispatch
UNCONDITIONALLY stringified that value into `content[0].text`. A handler that
hand-built a `CallToolResult`-shaped `Value` (with its own `content` and `_meta`)
had the whole object serialized into one text block — the top-level `_meta`
never reached the wire, so a `_meta`-sniffing client saw no related task.
**Silently.** One real incident ran that way in production for two weeks.

## The native pattern

Return a real `CallToolResult` and attach the related task with
`with_related_task`. Register the tool with `tool_with_result` so the envelope
reaches the wire VERBATIM:

```rust,ignore
use pmcp::types::{CallToolResult, Content};
use pmcp::types::tasks::TaskMetadata;

let server = Server::builder()
    .name("exporter")
    .version("1.0.0")
    // `store_minted_id` came from TaskStore::create — a REAL store-minted id,
    // resolvable via tasks/get, NOT a hand-written literal.
    .tool_with_result::<StartArgs>("start_export", move |_args, _extra| {
        let task_id = store_minted_id.clone();
        Box::pin(async move {
            Ok(CallToolResult::new(vec![Content::text("export started")])
                .with_related_task(TaskMetadata::new(task_id)))
        })
    })
    .task_store(store)
    .build()?;
```

On the client, detection is one accessor — no manual `_meta` parsing:

```rust,ignore
let result = client.call_tool("start_export".into(), json!({})).await?;
if let Some(meta) = result.related_task() {
    // Poll to terminal, then fetch the result — SDK-owned, wasm-safe.
    let task_result = client.wait_for_related_task(&meta, Default::default()).await?;
}
```

> **Note:** `wait_for_task` / `wait_for_related_task` return an error if the
> task enters `input_required` — that state is not terminal and needs
> client-side action (elicitation) the poller cannot provide. Handle the
> required input, then resume polling.

## Migrating a hand-written handler

If you have a hand-written `ToolHandler`, override `handle_output` to return
`ToolOutput::Result`. `handle()` stays as a serialize fallback; the default
`handle_output` delegates to it, so nothing else changes:

```rust,ignore
async fn handle_output(&self, args: Value, extra: RequestHandlerExtra)
    -> pmcp::Result<pmcp::ToolOutput>
{
    Ok(pmcp::ToolOutput::Result(
        CallToolResult::new(vec![Content::text("done")])
            .with_related_task(TaskMetadata::new(task_id)),
    ))
}
```

If the handler is otherwise happy on the normal path and just needs to ADD
`_meta`, one call retrofits it — no `handle_output` impl required:

```rust,ignore
extra.set_result_meta(related_task_meta_map); // merges, handler-key-wins
```

## Wire-compat: existing `_meta`-sniffing clients keep working

The native task create-path already emits `_meta[related-task]` with the
store-minted id, proven against real dispatch output in
`src/server/core_tests.rs`. A durable client that reads
`result._meta["io.modelcontextprotocol/related-task"]` detects a native
`with_task_store()` server UNCHANGED — no client migration required. The SDK
client also WARNs (never silently swallows) on a malformed task payload.

## Try it

The runnable BEFORE/AFTER migration lives in
`examples/s47_task_augmented_result.rs`:

```bash
cargo run --example s47_task_augmented_result --features full
```

It registers the BEFORE tool with `suppress_double_wrap_check` ONLY so the broken
shape can be demonstrated without aborting on the debug-build tripwire — real
tools should migrate, not suppress. The wire shape is locked in CI by
`tests/tool_output_result_http.rs` over a real HTTP round-trip.

> ⚠️ **A `tool_with_result` / `ToolOutput::Result` tool bypasses RESPONSE
> middleware.** Redaction, sanitization, and audit hooks (`ToolMiddleware`
> `on_response`) DO NOT run for a verbatim result — the handler owns its OWN
> redaction of both `content` and `_meta`, at the same trust level as returning a
> raw `Value`. This is a deliberate, locked design decision (D-04a): "keep the
> bypass, harden it." **Request** middleware still runs, and handler errors still
> route through the normal error path. Redact inside the handler BEFORE building
> the `CallToolResult`. See
> [`docs/design/sep-1686-task-augmented-results.md`](https://github.com/paiml/rust-mcp-sdk/blob/main/docs/design/sep-1686-task-augmented-results.md)
> for the full rationale.
