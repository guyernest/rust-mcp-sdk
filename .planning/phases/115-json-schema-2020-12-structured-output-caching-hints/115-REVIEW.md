---
phase: 115-json-schema-2020-12-structured-output-caching-hints
reviewed: 2026-08-01T00:00:00Z
depth: standard
files_reviewed: 38
files_reviewed_list:
  - contracts/binding.yaml
  - contracts/mcp-protocol-sdk-v1.yaml
  - crates/pmcp-agent/Cargo.toml
  - crates/pmcp-server-toolkit/Cargo.toml
  - examples/s52_v2_caching_hints.rs
  - fuzz/.gitignore
  - fuzz/Cargo.toml
  - fuzz/fuzz_targets/fuzz_schema_draft_pin.rs
  - schema/vendored/core-2026-07-28/PROVENANCE.md
  - schema/vendored/ext-tasks/PROVENANCE.md
  - src/server/core_tests.rs
  - src/server/core.rs
  - src/server/mod.rs
  - src/server/output_validation.rs
  - src/server/simple_resources.rs
  - src/server/streamable_http_server.rs
  - src/server/traits.rs
  - src/server/wasm_server_tests.rs
  - src/server/wasm_server.rs
  - src/server/workflow/prompt_handler.rs
  - src/testing/mod.rs
  - src/types/caching.rs
  - src/types/mod.rs
  - src/types/prompts.rs
  - src/types/protocol/mod.rs
  - src/types/protocol/version.rs
  - src/types/resources.rs
  - src/types/tasks.rs
  - src/types/tools.rs
  - tests/common/duplex.rs
  - tests/phase115_contract_bindings.rs
  - tests/property_tests.rs
  - tests/structured_tool_output.rs
  - tests/v1_lists_golden.rs
  - tests/v2_caching_hints.rs
  - tests/v2_core_schema_facts.rs
  - tests/v2_schema_tripwires.rs
  - tests/vendored_schema_provenance.rs
findings:
  critical: 1
  warning: 10
  info: 4
  total: 15
status: issues_found
---

# Phase 115: Code Review Report

**Reviewed:** 2026-08-01
**Depth:** standard
**Files Reviewed:** 38
**Status:** issues_found

## Summary

Reviewed SCHM-01 (Draft 2020-12 pin + era-keyed compile cache), SCHM-02
(`structured_value`), and SCHM-03 (`ttlMs` / `cacheScope` projection) at standard
depth, with targeted cross-file tracing of every dispatcher that can serialize one
of the six `CacheableResult` extenders.

**What holds.** The era branching is sound at the chokepoints. All five production
call sites of `inject_v2_result_envelope` (`core.rs:1929`, `core.rs:3404`,
`mod.rs:1544`, `mod.rs:1737`, `streamable_http_server.rs:3106`) name a `Cacheable`
claim; the `#[cfg]`-free projector is genuinely reachable from both the
`not(wasm32)` and `wasm32` dispatchers; the era-keyed cache key is correct and its
two ordering tests are real; `request_is_cacheable` fails closed. I independently
verified the "six `CacheableResult` extenders" claim against
`schema/vendored/core-2026-07-28/schema.ts` (`DiscoverResult`, `ReadResourceResult`
directly; `ListResourcesResult`, `ListResourceTemplatesResult`, `ListPromptsResult`,
`ListToolsResult` via `extends PaginatedResult, CacheableResult`) — it is accurate.
`cargo check --lib --features full` and `cargo check -p pmcp-agent -p
pmcp-server-toolkit --all-features` both pass; `cargo run --example
s52_v2_caching_hints --features full` runs green.

**The headline concern.** The v2 Draft 2020-12 pin does **not** close the
vacuous-validator bypass it was written to close. `normalize_schema_dialect`
rewrites only the ROOT `$schema`, so an *embedded schema resource* (a subschema
carrying `$id` plus a legacy `$schema`) still resolves an empty vocabulary set and
validates its subtree vacuously. I reproduced this through pmcp's own
`output_validation::fuzz_support::validate_bytes` seam and measured the exact
regression direction the phase claims to have eliminated: `(v1 = Violates,
v2 = Conforms)`. Neither the fuzz target nor the new property test can reach the
case, and the module's "measured: a nested declaration does not trigger the bypass"
claim generalizes from the one nested shape that happens to be safe.

Secondary themes: a latent byte-identity hazard in the v1 strip
(`serde_json::Map::remove` is `swap_remove` under `preserve_order`, which this repo
enables), a new hard-parse failure surface on six client-side deserializable types,
a non-conformant `resources/read` body in the phase's own reference example, and
two never-compiled files that received blind edits.

Findings already booked in `deferred-items.md` (warn-only mismatch handling,
post-projection middleware mutation, the unexecuted wasm strip path,
`decide.rs:218`, no builder override, `structuredContent: null` re-read) are **not**
re-reported.

---

## Critical Issues

### CR-01 (BLOCKER): The v2 Draft 2020-12 pin still admits the vacuous-validator bypass through embedded schema resources

**File:** `src/server/output_validation.rs:146-165` (the normalizer), `:142-145`
(the false "measured" claim), `:113-141` (the safety narrative),
`contracts/mcp-protocol-sdk-v1.yaml` (`output_schema_draft_pin` invariant 1)

**Issue:**
`normalize_schema_dialect` inspects and rewrites **only** `schema.get("$schema")` at
the document root. Under JSON Schema 2020-12, `$schema` is legal at the root of any
*embedded schema resource* — i.e. any subschema that also carries `$id` — and
`jsonschema` honours it. A legacy declaration on such a resource therefore survives
the pin and yields the empty-vocabulary, accept-everything sub-validator that the
whole normalize-then-pin step exists to prevent.

Measured through pmcp's own seam (`cargo run` against
`pmcp::server::output_validation::fuzz_support::validate_bytes`, `jsonschema`
0.49.2, this working tree):

```
schema:   {"type":"object",
           "properties":{"n":{"$ref":"#/$defs/Inner"}},
           "$defs":{"Inner":{"$id":"https://example.test/inner",
                             "$schema":"http://json-schema.org/draft-07/schema#",
                             "type":"integer"}}}
instance: {"n":"NOT-AN-INTEGER"}

embedded-legacy-resource (v1,v2) = Some((Conforms, Conforms))   <-- `type: integer` silently dropped
control-no-nested-schema (v1,v2) = Some((Violates, Violates))   <-- enforcement works without it
root-draft07 + embedded  (v1,v2) = Some((Violates, Conforms))   <-- v2 is WEAKER than v1
```

The third row is the regression direction the phase explicitly claims to have
eliminated. `src/server/output_validation.rs:26-28` says "On v2 the pin wins
UNCONDITIONALLY"; `:142-145` says "Only the ROOT key is touched. A nested `$schema`
(inside `properties.*`, say) is left untouched — measured: a nested declaration does
not trigger the bypass." That measurement was taken only against
`normalization_cases()` case (d) at `:841-848`, which is a nested `$schema`
**without** `$id` — precisely the shape that is safe. The `$id` case, which is the
one 2020-12 actually sanctions, was never measured and does trigger it.

The contract invariant is consequently false as written:

> On Era::V2 the compiled validator enforces the schema's keywords even when the
> document declares a legacy `$schema` (draft-04/06/07); a legacy declaration is
> normalized to the 2020-12 URI before compilation, never honoured and never used
> as a vocabulary source

**Blast radius, stated honestly:** the module is warn-only on both eras and
`outputSchema` is author-declared rather than attacker-supplied, so the impact is a
silently-lost dev/CI diagnostic, not a wire fault or a remote exploit. It is
classified BLOCKER because (a) it is a validation bypass in the exact function
written to prevent that bypass, (b) it makes v2 measurably more permissive than v1,
which SCHM-01's stated purpose forbids, and (c) both the rustdoc and the
provable-contract assert a safety property the code does not have — so `pmat comply
check` and any reader are actively misled.

**Fix:** normalize recursively, not just at the root. Either strip every `$schema`
below the root, or rewrite each to the 2020-12 URI, before handing the document to
`draft202012::new`:

```rust
#[cfg(feature = "validation")]
fn normalize_schema_dialect(schema: &Value) -> std::borrow::Cow<'_, Value> {
    use std::borrow::Cow;
    if !declares_legacy_dialect_anywhere(schema) {
        return Cow::Borrowed(schema);
    }
    let mut pinned = schema.clone();
    pin_dialect_in_place(&mut pinned); // walks every object node
    Cow::Owned(pinned)
}

/// Rewrite EVERY `$schema` (root and every embedded resource) to 2020-12.
#[cfg(feature = "validation")]
fn pin_dialect_in_place(node: &mut Value) {
    match node {
        Value::Object(map) => {
            if map.contains_key("$schema") {
                map.insert("$schema".into(), Value::String(DRAFT_2020_12.into()));
            }
            for value in map.values_mut() {
                pin_dialect_in_place(value);
            }
        },
        Value::Array(items) => items.iter_mut().for_each(pin_dialect_in_place),
        _ => {},
    }
}
```

Then extend the fences so the case cannot regress:

1. Add a fixed case to `normalization_cases()` (`:832-849`) with `$id` + nested
   `$schema`, asserted `expected_owned == true`.
2. Add a `v2_pin_still_enforces_an_embedded_legacy_resource` behavioural test using
   the three-row measurement above.
3. Widen `arb_schema_document()` in `tests/property_tests.rs` to inject nested
   `$schema`/`$id` pairs, and relax `is_dialect_neutral`'s exclusion of
   `$ref`/`$defs`/`$id` in `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` (or add a
   second predicate) so the fuzzer can reach the case.
4. Correct the "measured" sentence at `:142-145` and the contract invariant to state
   what is actually true after the fix.

---

## Warnings

### WR-01 (WARNING): The v1 strip's "byte-identical legacy response" guarantee holds only by accidental field ordering

**File:** `src/types/caching.rs:248-251`

**Issue:**
This repo enables `serde_json`'s `preserve_order` (`Cargo.toml:55`), under which
`Map::remove` is documented to be equivalent to `swap_remove` — it moves the map's
LAST entry into the removed slot, reordering the remaining keys. Demonstrated:

```
before: {"a":1,"ttlMs":2,"b":3,"c":4,"cacheScope":"public"}
after : {"a":1,"c":4,"b":3}
```

Today the strip is order-preserving only because `ttl_ms` and `cache_scope` are
declared **last** on all six result structs, so `skip_serializing_if` leaves them as
the final two emitted keys. But `project_caching_hints` is typed and documented as a
total projector over an arbitrary `serde_json::Value`, and `wasm_server.rs`'s
`cacheable_result_to_value<T: Serialize>` will hand it whatever a caller serializes.
Adding any field after `cache_scope` on any of the six — or ever calling the
projector after `own_reserved_result_fields` — silently reorders the v1 wire, which
is exactly what D-11 forbids.

This is unfenced: `tests/v1_lists_golden.rs` never sets a hint (the fixture handlers
at `:400-427` do not call `with_ttl_ms`/`with_cache_scope`), and
`v2_caching_hints_v1_strips_handler_set_values`
(`tests/v2_caching_hints.rs:851-862`) asserts only key **absence**, never key order.

**Fix:**
```rust
} else {
    // `remove` is `swap_remove` under serde_json's `preserve_order` feature
    // (enabled at Cargo.toml:55), which REORDERS the surviving keys. D-11
    // promises a byte-identical legacy response, so the order-preserving
    // variant is the only correct one here.
    object.shift_remove("ttlMs");
    object.shift_remove("cacheScope");
}
```
And extend the strip test to compare the full serialized string against the same
response produced by a handler that set no hints, not just `get(key).is_none()`.

---

### WR-02 (WARNING): `Option<u64>` `ttlMs` is a new hard-parse failure surface on six client-deserializable result types

**File:** `src/types/resources.rs:174`, `:427`, `:616`; `src/types/prompts.rs:300`;
`src/types/tools.rs:483`; `src/types/protocol/mod.rs:679`

**Issue:**
All six types derive `Deserialize` and are what the pmcp **client** parses
(`Client::list_tools`, `read_resource`, …). Before this phase, `ttlMs` on the wire
was an unknown key and was silently ignored. Now a peer emitting `ttlMs: -1`,
`ttlMs: 3.5`, `ttlMs: "300000"` or a value above `u64::MAX` fails deserialization of
the **entire result**, turning a tolerable non-conformance into a hard client-side
error on `tools/list` / `resources/read` / `prompts/list`. The rustdoc enumerates
exactly one residual ("the absent upper bound … ~584 million years") and does not
mention this one.

A second, quieter half: `with_ttl_ms(ms: u64)` (`resources.rs:223`, `:495`, `:660`)
range-checks nothing, and any value above 2^53 loses precision when a JavaScript
peer — the majority of MCP clients — parses it with `JSON.parse`. The SDK will
happily emit `"ttlMs":18446744073709551615`.

**Fix:** make the parse lenient in the direction the field's own semantics allow (a
malformed cache *hint* should not destroy the payload), and range-check the builder:

```rust
#[serde(default, deserialize_with = "lenient_ttl_ms", skip_serializing_if = "Option::is_none")]
pub ttl_ms: Option<u64>,

/// A malformed or out-of-range `ttlMs` degrades to "no hint" rather than
/// failing the whole result — the field is advisory, the payload is not.
fn lenient_ttl_ms<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
    Ok(Option::<serde_json::Value>::deserialize(d)?.and_then(|v| v.as_u64()))
}
```
and either document the 2^53 JS ceiling on `with_ttl_ms` or clamp to it.

---

### WR-03 (WARNING): The phase's reference example emits a non-conformant `resources/read` body

**File:** `examples/s52_v2_caching_hints.rs:134-137`

**Issue:**
`read()` returns `Content::text(format!("contents of {uri}"))`. `ReadResourceResult`
serializes `contents` through `resource_contents_serde`
(`src/types/resources.rs:557-561`), which strips the `type` tag, so the emitted
entry is `{"text": "contents of docs://me/profile"}` — **no `uri`**. The vendored
`2026-07-28` schema declares `$defs.TextResourceContents.required = ["text","uri"]`.
The example prints exactly this body as its own demonstration of a conformant v2
response:

```
raw = {"contents":[{"text":"contents of docs://me/profile"}],"ttlMs":0,"cacheScope":"private",...}
```

Examples are the copy-paste surface for server authors, and this one is the phase's
headline artifact.

**Fix:**
```rust
Ok(ReadResourceResult::new(vec![Content::resource_with_text(
    uri,
    format!("contents of {uri}"),
    "text/plain",
)]))
```
(`tests/v1_lists_golden.rs:407-411` already does exactly this.) Consider asserting
`read["contents"][0]["uri"]` in the example so the conformance claim is self-checked.

---

### WR-04 (WARNING): The example re-declares SDK constants as local literals instead of importing the ones this phase exported

**File:** `examples/s52_v2_caching_hints.rs:80-84`

**Issue:**
```rust
const DEFAULT_TTL_MS: u64 = 0;
const DEFAULT_CACHE_SCOPE: &str = "private";
```
`pmcp::types::DEFAULT_TTL_MS` is newly public (`src/types/mod.rs:43`) and
`CacheScope::default()` is the authoritative scope — the example already imports
`CacheScope` on line 64. Hardcoding both re-introduces exactly the drift the rest of
the phase is careful to avoid (`tests/common/duplex.rs:210-215` sources its
constants from pmcp "never string literals — so the harness cannot drift from the
crate"; `src/types/caching.rs:245-247` serializes the enum rather than typing
`"private"`). If a future phase changed the SDK default, this example would fail with
a confusing message instead of demonstrating the new default.

**Fix:**
```rust
use pmcp::types::{CacheScope, DEFAULT_TTL_MS};
// ...
let default_scope = serde_json::to_value(CacheScope::default()).expect("unit enum serializes");
assert_eq!(result.get("cacheScope"), Some(&default_scope), ...);
```

---

### WR-05 (WARNING): `src/server/traits.rs` is a never-compiled orphan that received a blind edit and cannot compile if ever wired in

**File:** `src/server/traits.rs:28-33` (the edit), `:4` (the broken import)

**Issue:**
No `mod traits;` declaration exists for this path — `grep -rn '^\s*\(pub \)\?mod traits;' src/`
returns only `src/server/auth/mod.rs:59`, which resolves to `src/server/auth/traits.rs`.
The file is compiled on no target. Worse, `:4` reads
`use crate::shared::cancellation::RequestHandlerExtra;` and `src/shared/mod.rs`
declares no `cancellation` module, so wiring the file in today is an immediate
compile error. It also declares a second `pub trait ToolHandler` that shadows the
real one in `src/server/mod.rs`.

Adding `ttl_ms: None, cache_scope: None` here produced no verification signal
whatsoever, and leaves the file looking maintained.

**Fix:** delete `src/server/traits.rs`. If any of it is wanted, wire it in and fix
the import to `crate::server::cancellation::RequestHandlerExtra` first, then resolve
the `ToolHandler` name collision.

---

### WR-06 (WARNING): `src/server/wasm_server_tests.rs` is bit-rotted dead code that also received a blind edit

**File:** `src/server/wasm_server_tests.rs:3` (double gate), `:7-13` (stale types),
`:225-238` (the edit)

**Issue:**
Declared `#[cfg(all(test, target_arch = "wasm32"))]` at `src/server/mod.rs:245-246`
and gated again by an inner `#[cfg(all(test, target_arch = "wasm32"))]` at `:3`, so
it compiles on no build this repo runs. Its body references types that no longer
exist: `CallToolParams`, `ListToolsParams`, `ListResourcesParams`,
`ListPromptsParams`, `GetPromptParams`, `InitializeParams`, `crate::shared::ClientInfo`,
and `WasmMcpServer::map_error_code`. Adding `ttl_ms: None, cache_scope: None` at
`:228-229`/`:235-236` implies wasm-side coverage of the new fields that does not
exist — the only runnable proof of the wasm strip is the native unit test
`no_context_strips_both_keys_which_is_the_wasm_path`
(`src/types/caching.rs:324-350`).

**Fix:** either delete the file and its `mod` declaration, or restore it against the
current type names and add a real wasm test job. Leaving a file that cannot compile
under a `cfg` nobody exercises is worse than having no file: it absorbs edits and
returns no signal.

---

### WR-07 (WARNING): The vendored-core PROVENANCE record contradicts the tree it describes

**File:** `schema/vendored/core-2026-07-28/PROVENANCE.md:28-29`, `:33-41`
(vs. `src/types/caching.rs:474-475`)

**Issue:**
The record states, in a section headed "THESE FILES ARE A READ-ONLY REFERENCE
ARTIFACT":

> - **Nothing in the build reads them.** They are not compiled, not code-generated
>   from, not `include_str!`'d, not parsed at runtime by any pmcp crate.
> - **Their only consumers are (a) human reviewers, (b) `tests/vendored_schema_provenance.rs`
>   … and (c) `tests/v2_core_schema_facts.rs`.**

Both are false. `src/types/caching.rs:474-475` does:

```rust
const CORE_SCHEMA_JSON: &str =
    include_str!("../../schema/vendored/core-2026-07-28/schema.json");
```

so `schema.json` is a **compile-time input to the crate's own unit-test build** and
there is a fourth consumer (`cacheable_result_serde_locks`). This is not cosmetic:
it changes the packaging consequence the same paragraph reasons about at `:46-53` —
excluding `schema/` would now break `cargo test --lib` compilation, not merely one
integration test. A provenance record whose entire value is precision should not be
wrong about its own consumers.

**Fix:** amend bullets 1 and 3 to name `src/types/caching.rs`'s `include_str!` and
state the stronger packaging consequence, in the same `> **Amended …**` style the
`ext-tasks` record already uses.

---

### WR-08 (WARNING): `CallToolResult::structured` and `structured_value` are byte-identical implementations with no shared body

**File:** `src/types/tools.rs:754-757` and `:845-848`

**Issue:**
```rust
pub fn structured(value: Value) -> Self {
    let text = value.to_string();
    Self::new(vec![Content::text(text)]).with_structured_content(value)
}
pub fn structured_value(value: Value) -> Self {
    let text = value.to_string();
    Self::new(vec![Content::text(text)]).with_structured_content(value)
}
```
The rustdoc says the difference is intentional and purely documentary ("The body is
identical to `structured`; the two differ only in what they SAY"). That is a fine
API decision and a poor implementation of it: two copies drift, and D-06 freezes
`structured`'s behaviour, so a future edit to one is a silent behavioural divergence
the contract says cannot happen.

**Fix:**
```rust
/// … (the widening sibling; see D-06 for why the NAME differs)
pub fn structured_value(value: Value) -> Self {
    // Deliberately delegates: D-06 freezes `structured`'s behaviour and the two
    // constructors must never diverge. The difference is the NAME, not the body.
    Self::structured(value)
}
```

---

### WR-09 (WARNING): Neither the new fuzz target nor the new property test can reach the CR-01 bypass, so both provide false confidence

**File:** `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs:154-234`,
`tests/property_tests.rs` (`arb_schema_document`),
`src/server/output_validation.rs:832-849`

**Issue:**
- `is_dialect_neutral` / `is_neutral_subschema` allowlist only
  `type, properties, required, enum, const, minimum, maximum, minLength, maxLength,
  pattern, additionalProperties, minItems, maxItems`. `$ref`, `$defs` and `$id` are
  absent, so any document containing an embedded resource is rejected as
  non-neutral and invariant 3 (the equality that would catch the bypass) is skipped.
- `arb_schema_document()` injects a `$schema` at the **root only** and explicitly
  removes any other (`object.remove("$schema")`), so the generated space contains no
  nested declaration at all.
- `normalization_cases()` case (d) is the only nested-`$schema` fixture and is the
  `$id`-free shape that does not trigger the bypass.

The result is three layers of testing that all agree the pin is safe while the pin
is not safe. This is worth recording separately from CR-01 because fixing the
normalizer without widening these generators leaves the same blind spot for the next
dialect edge case.

**Fix:** as listed under CR-01 items 1–3. At minimum, add `$id`/`$schema` pairs to
`arb_schema_document()` and add a second fuzz invariant that holds specifically over
documents containing embedded resources.

---

### WR-10 (WARNING): `WasmServerCore` is a fourth dispatcher that never reaches the projector, so the "every cacheable serialization site" claim is broader than what is enforced

**File:** `src/server/wasm_core.rs:73-90` (hand-built `tools/list`),
`src/server/wasm_server.rs:20-52` (the uniformity claim),
`tests/v2_schema_tripwires.rs:1633`
(`v2_schema_tripwires_every_cacheable_serialization_site_routes_through_the_projector`)

**Issue:**
`WasmServerCore::handle_request` answers `tools/list` by hand-building
`json!({"tools": tools})` and never calls `project_caching_hints`. There is no leak
today — the value is assembled from a literal and can carry no hint — but the wasm
module's rustdoc calls `WasmMcpServer` "the THIRD dispatcher" and the tripwire's
name asserts *every* cacheable serialization site is covered, when this one is
neither covered nor enumerated as an exception.

**Fix:** either route it through `cacheable_result_to_value` for uniformity, or add
`src/server/wasm_core.rs` to the tripwire's accounted-for list with a `// Why:`
comment stating that its results are literal-built and cannot carry a hint. Silence
is the one option that lets the next author add a typed result there and leak.

---

## Info

### IN-01: `pub use super::caching::*;` is a redundant glob that creates a future accidental-export path

**File:** `src/types/protocol/mod.rs:23`

`src/types/mod.rs:43` already names the two intended public items explicitly
(`pub use caching::{CacheScope, DEFAULT_TTL_MS};`) with a comment describing it as a
"NARROW re-export: only the two PUBLIC items … The projector and its classification
enum are `pub(crate)` dispatcher plumbing and deliberately stay off the public
surface." The glob at `protocol/mod.rs:23` reaches the same items through a second
path and will automatically surface anything public added to `types::caching` later —
defeating the stated narrowness by construction.

**Fix:** replace with `pub use super::caching::{CacheScope, DEFAULT_TTL_MS};`, or drop
it entirely if no caller uses the `types::protocol::CacheScope` path.

### IN-02: `pub mod output_validation` under `feature = "fuzzing"` is public API for any downstream that enables the feature

**File:** `src/server/mod.rs:64-80`

`fuzzing` is a normal, publicly-selectable Cargo feature. Enabling it promotes
`pmcp::server::output_validation` to a public module. With `fuzzing` but **without**
`validation` it is a public module with zero public items. The `cargo public-api`
argument in the comment is about the default/`full` surface only.

**Fix:** consider `#[doc(hidden)]` on the `fuzzing` arm, and gate the widening on
both features (`#[cfg(all(feature = "fuzzing", feature = "validation"))]`) so the
empty-public-module case cannot arise.

### IN-03: The contract-binding gate resolves symbols crate-wide and never compares the recorded `signature:`

**File:** `tests/phase115_contract_bindings.rs:401-419`

`module_path` is used only to pick a source root (`split("::").next()`), and
`declares()` searches every `.rs` file under that root for `fn <name>`. So a binding
whose `module_path` is wrong still resolves, and a `signature:` that drifts from the
shipped source is never detected. `contracts/binding.yaml` claims "115-10 … verified
each `signature:` against the shipped source" — that verification is manual and
ungated, so it decays from the next commit onward.

**Fix:** at minimum, resolve within the file whose path matches `module_path`; ideally
compare the `fn <name>(` argument list textually.

### IN-04: `cached_validator` compiles JSON Schemas while holding a process-global `std::sync::Mutex`

**File:** `src/server/output_validation.rs:244-249`

`map.entry(key).or_insert_with(|| compile_for_era(...))` runs the whole compile
inside the lock, on an async request-handling path, for a map shared by every server
in the process. Also, `schema.to_string()` allocates the full schema text on every
call just to build the lookup key. Flagged as Info because `outputSchema` is
server-authored (not attacker-supplied) and performance is out of v1 review scope —
but a pathological author-supplied `pattern` would stall every other tool call in the
process, which edges into robustness.

**Fix:** compile outside the lock and insert under it (double-checked), accepting the
rare duplicate compile.

---

_Reviewed: 2026-08-01_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
