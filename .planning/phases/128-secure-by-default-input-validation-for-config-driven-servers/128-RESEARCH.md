# Phase 128: Secure-by-default input validation for config-driven servers - Research

**Researched:** 2026-09-26
**Tree measured at:** `44d50b3a` (branch `fix/oauth-discovery-optional-fields`)
**Domain:** JSON Schema runtime enforcement, URL path-injection containment, Rust feature-graph severance, multi-crate coordinated release
**Confidence:** HIGH (every load-bearing claim is either a `file:line` read this session or a pasted probe output against the pinned dependency version)

> **Provenance discipline.** This phase's own source documents warn that "prose reasoning about
> versions and enforcement is unreliable here" and that "threat-ID comments are not evidence."
> Accordingly: nothing below is stated from training memory. Claims are tagged
> `[VERIFIED: path:lines]` (file opened this session, values quoted verbatim),
> `[MEASURED: probe]` (a program compiled and run this session against the *pinned* dependency,
> with its output pasted), or `[ASSUMED]` (training knowledge — see the Assumptions Log).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

Copied verbatim from `128-CONTEXT.md` § Implementation Decisions. **Do not re-litigate any of these.**

- **D-01:** **Core owns the input-validation entry point.** Generalize `src/server/output_validation.rs` into `server::schema_validation` with a public `validate_input(...)` that owns MCP input semantics too — missing `arguments` (`null`) treated as `{}`, and the already-emitted `additionalProperties: false` honoured rather than re-added. The toolkit calls it. Rejected: a toolkit-local second `jsonschema` path. **This moves part of D1 into `pmcp`, which the CR's rollout does not account for.** *Reversibility:* one-way.
- **D-02:** **Inputs compile under Draft 2020-12 on both eras.** Reuse `compile_2020_12` (`output_validation.rs:592`) for inputs regardless of `Era`, rather than routing through `compile_for_era`'s two arms (`:607-611`). Inputs have no shipped behaviour to preserve, and a config- or schemars-declared `$schema` must not be able to change input enforcement semantics. Inherits the existing warn at `:585`. *Reversibility:* costly.
- **D-03:** **`src/server/validation.rs` — harvest, then deprecate (not delete).** (a) harvest `validate_safe_path` (`:252`) as the basis for D4(a); (b) mark the module `#[deprecated]` **and** `#[doc(hidden)]`; (c) correct `ARCHITECTURE.md:200`; (d) book removal against the next major. *Reversibility:* reversible.
- **D-04:** **Split core's `validation` feature.** Add `schema-validation = ["dep:jsonschema"]` and redefine `validation = ["schema-validation", "dep:garde"]` (today: `Cargo.toml:327`). The toolkit's `input-validation` forwards only `pmcp/schema-validation`. Additive: existing `validation` users and `full` (`Cargo.toml:280`) see no change. *Reversibility:* costly.
- **D-05:** **Position-scoped enforcement, not uniform.** A string parameter in **path or query position** gets a hard default cap; a string in **body position** gets a warning only. Position is derivable from config today. *Reversibility:* costly.
- **D-06:** **Strict cap value is 256** for path/query position. It also refuses the CR-01 two-placeholder probe (180+200 = 380 chars) on this cap alone.
- **D-07:** **Warn teeth:** `ServerConfig::validate` and `cargo pmcp validate deploy` both warn on an uncapped body-position string; a `[server.validation]` strict flag turns the warning into a hard config error. A running server does not refuse to boot over this.
- **D-08:** **D4(c) carries its own always-on placeholder cap, independent of D3.** Without this, setting `default_max_length = 0` would silently disable the "Length via two placeholders" acceptance row.
- **D-09:** **Resolve placeholders in `pmcp-code-mode` BEFORE calling `execute_request`, AND export `validate_path_placeholder`.** Moving resolution ahead of dispatch makes **every** `HttpExecutor` implementor safe by construction, pmcp.run's built-in `openapi-api` server included. Breaking for implementors that resolve their own paths, hence `pmcp-code-mode` 0.5.4 → **0.6.0**. *Reversibility:* one-way.
- **D-10:** **The character denylist is an unconditional floor.** `?`, `#`, `/`, `..` and percent-encoded forms of these are always refused in a placeholder value; a `pattern` declared in the OpenAPI spec narrows further **on top**, never replaces.
- **D-11:** **`/` is liftable only by per-parameter config opt-in, never by the spec's `allowReserved`.** `..` and percent-encoded traversal are refused unconditionally with **no** escape.
- **D-12:** **E1 (`RequestPolicy`) runs BEFORE outgoing auth is applied.** The policy sees tool name, method, resolved path, query pairs and body, and never the credential.
- **D-13:** **Release is ~11 crate versions.** `pmcp` 2.20.4 → **2.21.0**; `pmcp-code-mode` 0.5.4 → **0.6.0**; `pmcp-code-mode-derive`; `pmcp-server-toolkit` 0.1.3 → **0.2.0**; and **seven** toolkit consumers including **`pmcp-workbook-compiler`**. Also to repin: root `Cargo.toml:263` and `crates/pmcp-server-toolkit/Cargo.toml:23-24`. **The `pmcp` minor needs no other downstream repin** — every `pmcp` pin in the tree is a caret on 2.x.
- **D-14:** **One tag, `release.yml`'s existing order.** Bump all ~11 in one commit; `scripts/check-release-coverage.sh` machine-checks the order inside `make quality-gate`. Known risk accepted: one failing crate mid-order strands the rest. *Reversibility:* reversible.
- **D-15:** **`ParamDecl`'s forward-incompatibility is a rollout note, not a design change.** `#[serde(deny_unknown_fields)]` at `config.rs:920` means a config written against 0.2.0 using `pattern` / `min_length` / `format` / `items` **fails to parse** on 0.1.3. Must appear in the CHANGELOG and the rollout note.

**Answered without discussion (do not re-litigate):**
- CR open question 3 (schema engine version) is ALREADY ANSWERED in-tree: `jsonschema = "0.49"` in both crates. The CR's "0.57 misses U+3000" figure must be re-measured on 0.49 before it drives anything. *(→ re-measured in this document, Finding 1.)*
- Review note B (warn-vs-refuse asymmetry) is a **deliverable**, not a fork — it must be written down in the docs deliverable.
- Review note F / SC-7 is locked: return the DECLARED expectations, never the rejected value and never the rejected key. Shape: `unknown argument(s): 2; allowed: cui, version`.
- Feature gate is the toolkit's existing `input-validation`, **not** `openapi-code-mode`.

### Claude's Discretion

> None — every gray area presented was decided by the user. Items left to the planner/researcher by
> nature rather than by deferral: E2's exact attach-point signature, the `[server.validation]` opt-out
> key names beyond those the CR already fixes, the startup-log format for enforced rules and opt-outs,
> and how `validate_path_placeholder`'s public signature is shaped.

### Deferred Ideas (OUT OF SCOPE)

- **Per-request total-time budget** (CR open question 6) — its own phase. Scope it against the **measured 127 s**.
- **Sweep other phases' threat records for the same false-mitigation class.** SC-6 still closes the three instances in `tools.rs`.
- **Wire `validate_input` into `src/server/mod.rs:2590`/`:2820`** so hand-written `ToolHandler`s get the same enforcement. This phase wires config-driven tools only.
- **Remove `server::validation` at the next major** — D-03 deprecates and hides it now; the removal is booked, not done.
- **Re-measure the `jsonschema` `\s`/`\S` U+3000 behaviour on 0.49** — *this research discharges that deferral; see Finding 1.*

</user_constraints>

<phase_requirements>
## Phase Requirements

No formal REQ-IDs (v2.7 has no REQUIREMENTS.md). The tracked set is D1–D4 / E1–E3 from the change
request plus SC-1..SC-8 from the ROADMAP. Both go in each plan's `requirements:` frontmatter.

| ID | Description | Research Support |
|----|-------------|------------------|
| D1 | Enforce the declared `inputSchema` at runtime for every config-declared tool; `additionalProperties: false` honoured; errors name the rule and JSON pointer, never the value; missing `arguments` (`null`) → `{}` | Findings 1 (jsonschema 0.49 semantics + the **value-echo hazard** in `Display`), 2 (the `validate_input` seam) |
| D2 | Richer `ParamDecl`: `pattern`, `min_length`, `format`, `items`/`max_items`; config validation rejects a non-compiling `pattern` and documents the engine's `\s`/`\S` coverage | Finding 1 (regex engine split, non-compiling-pattern surface, `format` is **annotative** by default), Finding 10 (`ParamDecl`/`build_param_property` shape) |
| D3 | Default string cap, position-scoped per D-05/D-06; `ServerConfig::validate` warns, strict mode fails | Finding 11 (`ServerConfig::validate` has **no warning channel** today), Finding 5 (position derivation) |
| D4 | Path-injection closure on BOTH HTTP surfaces: (a) character denylist, (b) spec-parameter validation, (c) placeholder cap | Findings 3 (D-09 blast radius), 5 (`validate_safe_path` gap), 6 (`substitute_path`) |
| E1 | `RequestPolicy` trait on the server builder, run after D1/D4 on every outbound request, before auth | Finding 3 (the two `execute_request` call sites + `HttpCodeExecutor` step ordering) |
| E2 | Per-tool `ArgumentValidator`, attachable by tool name from Rust | Finding 10 (`synthesize_*` entry points) |
| E3 | `garde` on `TypedTool<T>` where `T: garde::Validate` | Finding 4 (`garde` 0.23 measured API; the `T` bound cannot be tightened additively) |
| SC-1 | Config-declared tool refuses schema-violating args before any backend call, zero upstream requests, gated on `input-validation` | Findings 1, 8, 12 |
| SC-2 | `ParamDecl` accepts the new keywords; a non-compiling `pattern` fails **config** validation, not call time | Finding 1 (a bad pattern fails the WHOLE schema compile; `meta::is_valid` does **not** catch it) |
| SC-3 | Uncapped string surfaced by `ServerConfig::validate` and `cargo pmcp validate deploy` | Finding 11 (`cargo-pmcp` has **no** dependency on the toolkit and never reads a `ServerConfig`) |
| SC-4 | Placeholder denylist on BOTH surfaces; all five CR-01 probes pass | Findings 3, 5, 6 |
| SC-5 | `RequestPolicy` + `ArgumentValidator` registerable; `garde` runs on `TypedTool<T>` | Finding 4 |
| SC-6 | Three false enforcement claims corrected; no remaining toolkit comment claims an unimplemented mitigation | Finding 10 (all three located verbatim) |
| SC-7 | Refusal messages name the violated rule and the DECLARED parameters, never the value, never an attacker key | Finding 1 (`ValidationErrorKind` is the value-free seam; `Display` is **not**) |
| SC-8 | `make quality-gate` passes; fuzz, property, unit and example coverage | Finding 9 |

</phase_requirements>

<project_constraints>
## Project Constraints (from CLAUDE.md)

Extracted from `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/CLAUDE.md`. These carry the same
authority as the locked decisions above.

| Directive | Consequence for this phase |
|---|---|
| **ALWAYS requirements — fuzz, property, unit AND `cargo run --example` for every new feature** | SC-8 depends on this. See Finding 9 for where each lives and the two legs that currently measure almost nothing. |
| **Cognitive complexity ≤ 25 per function, CI-gated by PMAT 3.15.0** | Measured this session: `src/server/output_validation.rs` has **zero** functions over cog 25. Do not plan a refactor that merges its three-function split. |
| **Zero SATD comments** | No `TODO`/`FIXME`/`XXX` in new code; `make check-todos` is a gate leg. |
| **`make quality-gate` is the only CI-equivalent local gate** | Bare `cargo clippy -- -D warnings` is weaker than CI for the root crate and *stricter* than CI for the toolkit crates (which `make lint` does not reach at all — Finding 9). |
| **Contract-first: write/update the contract YAML in `../provable-contracts/contracts/<crate>/`, `pmat comply check` before and after** | `make comply` is a `quality-gate` leg. New public API (`validate_input`, `RequestPolicy`, `validate_path_placeholder`) needs contract entries. |
| **Release & Publish Workflow ledger (items 1–17) + Version Bump Rules + the caret exception** | D-13/D-14 lean on this directly. Finding 7 verifies every pin against the manifests and reports one D-13 miss. |
| **`.github/workflows/release.yml` is the AUTHORITY on publish order; the prose ledger is its mirror** | Do not reorder `release.yml` to match CLAUDE.md prose. |
| **Pre-commit hook blocks commits until quality gates pass** | Plan task granularity must allow `make quality-gate` to pass at each commit. |

</project_constraints>

## Summary

This phase is unusually well specified: CONTEXT.md carries 15 locked decisions, exact `file:line`
anchors, and an empty discretion list. What research adds is **measurement** — the five places where
the decisions rest on a dependency behaviour or a tree fact that nobody had actually checked.

Three of those measurements change the plan shape. **First**, `jsonschema` 0.49.2's
`ValidationError: Display` echoes the rejected value in *every* keyword's message — the 5,000-char
PHI dump, the attacker-supplied object key, all of it. The existing `schema_mismatch`
(`output_validation.rs:120-133`) renders errors with exactly `format!("{} (at {})", e, …)`, which is
acceptable for a warn-only output path and is **categorically wrong for D1**. SC-7 is therefore not
a formatting preference the implementation can satisfy by being careful with `format!`; it requires a
dedicated renderer that reads `ValidationError::kind()` — a public enum that exposes *declared-only*
data (`MaxLength { limit }`, `Pattern { pattern }`, `Enum { options }`, `Required { property }`) for
every keyword except `AdditionalProperties`, whose `unexpected: Vec<String>` carries the attacker's
keys and may only contribute its `.len()`. That is precisely the locked SC-7 message shape
(`unknown argument(s): 2; allowed: cui, version`), and the measurement explains *why* it has to be
that shape rather than merely permitting it.

**Second**, `\s` means two different things inside one `jsonschema` 0.49.2 process. A plain pattern
compiles on the linear-time `regex` path where `\s` is a partial ECMA-262 set that **misses U+3000**
(confirming the CR's 0.57 figure on the pinned version), while a pattern containing a lookaround or
backreference routes to `fancy-regex 0.18` where `\s` is exactly `\p{White_Space}` and **does** match
U+3000. SC-2's "documents that engine's `\s`/`\S` coverage" is therefore a two-column table, not one.
Separately, `format` is **annotative** in 0.49 — emitting `"format": "uuid"` into `inputSchema` buys
zero enforcement unless the validator is built through `jsonschema::options().should_validate_formats(true)`.

**Third**, three tree facts contradict what the CR and D-07 assume. The toolkit's `input-validation`
is **not** in its `default = ["code-mode"]`, so "forwarded from `default`" is a change to make, not a
state to rely on. `cargo-pmcp` has **no** dependency on `pmcp-server-toolkit` and `validate.rs` never
reads a `ServerConfig` — SC-3's second half needs a new dependency edge or a different surface. And
`ServerConfig::validate` returns a first-error-wins `Result<(), ConfigValidationError>` with no
warning channel at all, so D-07's "warns" needs a new return shape.

**Primary recommendation:** Build `validate_input` as a **new** `pub mod schema_validation` that
wraps the existing crate-private `output_validation` internals rather than renaming that file — the
rename has a measured cost (two test files carry the path as a string literal, a fuzz target and a
nextest selector carry the module path) and buys nothing D-01 needs. Give it a value-free renderer
driven by `ValidationError::kind()`, gate it `schema-validation`, and keep `compile_2020_12`,
`cached_validator` and the three-function complexity split exactly where they are.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| D1 schema compilation + `is_valid` | **Core `pmcp`** (`server::schema_validation`) | — | D-01 locks it there; the compiled-validator cache and draft pin already live in core and must not be duplicated |
| D1 value-free error rendering | **Core `pmcp`** | — | Same module: the renderer is the only safe consumer of `ValidationError`, so it cannot be left to each caller |
| D1 invocation per `tools/call` | **Toolkit** (`tools.rs` handlers) | Core (deferred: `mod.rs`/`core.rs` dispatch) | This phase wires config-driven tools only; the core dispatch wiring is a deferred item |
| D2 `ParamDecl` vocabulary + schema emission | **Toolkit** (`config.rs`, `tools.rs`) | — | `ParamDecl` is a toolkit config type; `build_param_property` is the only writer |
| D2 config-time `pattern` compile check | **Toolkit** (`ServerConfig::validate`) | Core (the compile fn) | The check must fail at *config* time (SC-2), which only the toolkit's validate path reaches |
| D3 position derivation (path/query vs body) | **Toolkit** | — | `operation.path_parameters()` and `[[tools.parameters]]` are toolkit-side; no new config surface needed |
| D4 placeholder denylist primitive | **`pmcp-code-mode`** (exported `validate_path_placeholder`) | Toolkit (calls it on both surfaces) | D-09 exports it so third-party `HttpExecutor` implementors can reach it |
| D4 Code Mode resolution ordering | **`pmcp-code-mode`** (`PlanExecutor`) | — | D-09: resolving before `execute_request` is what makes every implementor safe by construction |
| D4 single-call surface | **Toolkit** (`http/client.rs::substitute_path`) | — | The curated JS-engine-free build never touches `pmcp-code-mode` |
| E1 `RequestPolicy` | **Toolkit** (server builder) | — | It must see the resolved path *and* the toolkit's auth boundary (D-12: before auth) |
| E2 `ArgumentValidator` | **Toolkit** (by tool name) | — | Attaches to synthesized handlers |
| E3 `garde` on `TypedTool<T>` | **Core `pmcp`** | — | `TypedTool` is core; gated on `validation` (which retains `dep:garde` after the D-04 split) |
| D-07 deploy-time warning | **`cargo-pmcp`** | Toolkit (the rule) | ⚠ No dependency edge exists today — see Finding 11 |

---

# Measured Findings (research brief items 1–9)

## Finding 1 — `jsonschema` 0.49 behaviour, measured on the pinned version

**The pin is real and resolves to 0.49.2.** `[VERIFIED: Cargo.toml:220]` —
`jsonschema = { version = "0.49", optional = true, default-features = false }`;
`[VERIFIED: crates/pmcp-server-toolkit/Cargo.toml:54]` —
`jsonschema = { version = "0.49", default-features = false, optional = true }`;
`[VERIFIED: Cargo.lock:3661-3662]` — `name = "jsonschema"` / `version = "0.49.2"`.
Its regex dependencies: `fancy-regex v0.18.0`, `regex v1.12.3`, `jsonschema-regex v0.49.2`
`[VERIFIED: cargo tree -p jsonschema --depth 1]`.

**Method.** A standalone crate depending on `jsonschema = { version = "0.49", default-features = false }`
(the same requirement and the same `default-features = false` as both in-tree manifests) was compiled
and run this session. Every result below is pasted from its stdout. This is the positive
falsification the CONTEXT.md deferral asked for.

### 1a. `additionalProperties: false` — enforced, but `Display` leaks the attacker's keys

```
ap-extra-one:  valid=false errors=["MSG[Additional properties are not allowed ('apiKey' was unexpected)] PTR[] KW[/additionalProperties]"]
ap-extra-two:  valid=false errors=["MSG[Additional properties are not allowed ('Jane Doe DOB 1970-01-01', 'apiKey' were unexpected)] PTR[] KW[/additionalProperties]"]
ap-ok:         valid=true  errors=[]
```
`[MEASURED: probe — jsonschema 0.49.2]`

Draft 2020-12 enforces the keyword the toolkit already emits. **But the `Display` impl echoes the
rejected keys verbatim**, including the PHI-shaped one from SC-7's own worked example. Note
`instance_path()` is **empty** for this keyword — the JSON pointer gives you nothing to name, which is
exactly why SC-7's locked shape is a *count* plus the declared allow-list.

### 1b. Every other keyword's `Display` echoes the rejected VALUE

```
maxLength-5000-value: MSG["xxxxxx…(all 5000 chars)…xxx" is longer than 256 characters] KW[/maxLength]
enum:                 MSG["Jane Doe DOB 1970-01-01" is not one of "exact", "words" or "approximate"] KW[/enum]
pattern:              MSG["Jane Doe DOB 1970-01-01" does not match "^C[0-9]+$"] KW[/pattern]
type-mismatch:        MSG[{"secret":"PHI"} is not of type "string"] KW[/type]
maxItems:             MSG[["a","b","c"] has more than 2 items] KW[/maxItems]
minimum:              MSG[0 is less than the minimum of 1] KW[/minimum]
```
`[MEASURED: probe — jsonschema 0.49.2]`

> **This is the single most consequential measurement in this document.** The existing renderer at
> `[VERIFIED: src/server/output_validation.rs:128-131]` is
> `.map(|e| format!("{} (at {})", e, e.instance_path()))`. Reusing it for D1 would put the rejected
> value — a 5,000-char PHI dump in the CR-02 acceptance row — straight into a `tools/call` error
> returned to the client. **D-01's generalization must NOT reuse `schema_mismatch`'s renderer.**

### 1c. `ValidationErrorKind` is the value-free seam SC-7 needs

`kind()` is a **method**, not a field, in 0.49 (`e.kind()` — `e.kind` is a compile error
`E0615: attempted to take value of method`). Matching on it yields declared-only data:

```
path=      /cui  MaxLength limit=8
path=      /cui  Pattern pattern=^C[0-9]+$
path=     /kind  Enum options=["exact","words"]
path=        /n  Maximum limit=10
path=     /tags  MaxItems limit=2
path=   /tags/0  MaxLength limit=4
path=            AdditionalProperties unexpected.len()=2 (VALUES ARE ATTACKER KEYS: ["Jane Doe DOB 1970-01-01", "apiKey"])
```
`[MEASURED: probe — jsonschema 0.49.2]`

| Kind | Payload | SC-7 safe? |
|---|---|---|
| `MaxLength { limit }` / `MinLength { limit }` | the declared limit | ✅ |
| `Pattern { pattern }` | the declared pattern string | ✅ (author-supplied, in the config) |
| `Maximum { limit }` / `Minimum { limit }` | the declared bound | ✅ |
| `Enum { options }` | the declared option list | ✅ |
| `MaxItems { limit }` | the declared limit | ✅ |
| `Required { property }` | the declared property name | ✅ |
| `Type { kind }` | the declared type | ✅ |
| `AdditionalProperties { unexpected }` | **attacker-controlled keys** | ❌ — use `.len()` only |

`instance_path()` is safe for declared properties (`/cui`, `/tags/0`) because those names come from
the config; it is empty for `additionalProperties`. **Error iteration order is deterministic** —
three consecutive `iter_errors` runs produced byte-identical `(schema_path, instance_path)` sequences
`[MEASURED: probe]` — so a refusal message built from the first error is stable and testable.

### 1d. `maxLength` counts Unicode **code points**

```
ml-ascii3 ("abc", 3cp/3B):              valid=true
ml-latin1-3cp-6b ("ééé"):               valid=true      <- 6 bytes, passes maxLength 3
ml-astral-3cp-12b ("😀😀😀"):            valid=true      <- 12 bytes, 6 UTF-16 units, passes
ml-zwj-family-7cp (1 grapheme cluster): valid=false     <- one visible emoji, refused at cap 3
ml-combining-4cp ("é" x2 decomposed):   valid=false
```
`[MEASURED: probe — jsonschema 0.49.2]` (cross-check: `"👨‍👩‍👧‍👦" chars=7 bytes=25 utf16=11`)

This is spec-correct (`maxLength` is defined over code points) and matters for D-06's 256: it is
256 **code points**, not bytes and not grapheme clusters. A byte-oriented cap elsewhere in the
stack would disagree. A single user-perceived emoji can consume 7 of the 256.

### 1e. `pattern` — engine, syntax subset, and how a bad pattern surfaces

**Syntax accepted** (all compile under `draft202012::new`) `[MEASURED: probe]`:
`^(?=.*[A-Z]).*$` (lookahead), `^(a)\1$` (backreference), `(?<!a)b` (negative lookbehind),
`(?P<x>a)` (named group), `^\p{Greek}+$`, `^[[:alpha:]]+$`, `(?i)abc`, `(?x) a b`, `\b\w+\b`,
`(a+)+$`, `\p{White_Space}`.

**Rejected at compile:** `^[A-Z` → `"^[A-Z" is not a "regex"`; `a{2,1}`; `*`; `a{1000000}`
(regex size limit). `[MEASURED: probe]`

**A non-compiling nested pattern fails the WHOLE schema**, and the error names the offending
position:
```
WHOLE SCHEMA FAILS: "^[A-Z" is not a "regex"  | instance_path=  schema_path=/properties/bad/pattern
```
`[MEASURED: probe]` — this is a gift for SC-2: the `schema_path` points straight at the parameter.

**`jsonschema::meta::is_valid` does NOT catch it:**
```
meta::is_valid(bad pattern schema) = true
meta::is_valid(good)               = true
```
`[MEASURED: probe]` — **SC-2's config-time check must be a real `draft202012::new()` compile of the
synthesized `inputSchema`, not a meta-schema validation.**

**`pattern` is UNANCHORED** (spec behaviour, confirmed): `[A-Z0-9_]+` matches
`"Jane Doe DOB 1970-01-01"` (valid=true); `^[A-Z0-9_]+(,[A-Z0-9_]+)*$` refuses it. `[MEASURED: probe]`
The D2 docs must say this — a config author writing `[A-Z0-9_]+` gets no enforcement at all.
Useful nuance: `^[A-Z]+$` **does** refuse `"ABC\nevil"` (valid=false) — `$` is end-of-haystack, not
end-of-line. `[MEASURED: probe]`

### 1f. `\s` / `\S` coverage — the two-engine split (SC-2's documented table)

The CR's claim was "0.57 misses U+3000". **Re-measured on 0.49.2: it depends on which engine the
pattern routes to, and both engines are live in the same process.**

| Codepoint | Name | `\s` plain | `\S` plain | `\s` fancy¹ | `\p{White_Space}` |
|---|---|---|---|---|---|
| U+0009 | TAB | ✅ | ❌ | ✅ | ✅ |
| U+000A | LF | ✅ | ❌ | ✅ | ✅ |
| U+000B | VT | ✅ | ❌ | ✅ | ✅ |
| U+000C | FF | ✅ | ❌ | ✅ | ✅ |
| U+000D | CR | ✅ | ❌ | ✅ | ✅ |
| U+0020 | SPACE | ✅ | ❌ | ✅ | ✅ |
| U+0085 | NEL | **❌** | ✅ | ✅ | ✅ |
| U+00A0 | NBSP | ✅ | ❌ | ✅ | ✅ |
| U+1680 | OGHAM SPACE | **❌** | ✅ | ✅ | ✅ |
| U+2000 | EN QUAD | **❌** | ✅ | ✅ | ✅ |
| U+2007 | FIGURE SPACE | **❌** | ✅ | ✅ | ✅ |
| U+2028 | LINE SEPARATOR | **❌** | ✅ | ✅ | ✅ |
| U+2029 | PARA SEPARATOR | ✅ | ❌ | ✅ | ✅ |
| U+202F | NARROW NBSP | **❌** | ✅ | ✅ | ✅ |
| U+205F | MED MATH SPACE | **❌** | ✅ | ✅ | ✅ |
| **U+3000** | **IDEOGRAPHIC SPACE** | **❌** | ✅ | ✅ | ✅ |
| U+FEFF | ZWNBSP / BOM | ✅ | ❌ | **❌** | ❌ |
| U+200B | ZWSP | ❌ | ✅ | ❌ | ❌ |
| U+180E | MONGOLIAN VS | ❌ | ✅ | ❌ | ❌ |

¹ "fancy" = a pattern containing a lookaround or backreference, which routes to `fancy-regex 0.18`.
Forced in the probe with `(?=[\s\S])\s`. Unanchored throughout, so no `^`/`$` artifacts.
`[MEASURED: probe — jsonschema 0.49.2]`

**Conclusions for SC-2's documentation deliverable:**
1. **The CR's "misses U+3000" is CONFIRMED on the pinned 0.49.2** — for the default (plain) engine.
   It is a *pinned-version* property, not a 0.57 regression.
2. The plain-engine `\s` set is `{09, 0A, 0B, 0C, 0D, 20, A0, 2029, FEFF}` — a **partial** ECMA-262
   `\s` that includes PARA SEPARATOR but not LINE SEPARATOR, and includes the BOM. It is neither
   ASCII-only nor `\p{White_Space}`.
3. The fancy-regex `\s` is **exactly** `\p{White_Space}` (every row matches the last column).
4. **Therefore `\s` is not a single rule in this engine.** Adding a lookahead to a pattern silently
   changes what `\s` means. The D2 docs must carry both columns, and a config author who needs
   U+3000 refused must write an explicit class (e.g. `[^\p{White_Space}]`) rather than `\S`.

### 1g. `format` is ANNOTATIVE by default — D2's `format` field buys nothing without an opt-in

```
format-uri-bad:       valid=true    <- "!!!not-a-uri!!!" ACCEPTED
format-email-bad:     valid=true
format-date-time-bad: valid=true
format-uuid-bad:      valid=true
format-ipv4-bad:      valid=true
format-hostname-bad:  valid=true
format-regex-bad:     valid=true
format-date-bad:      valid=true
-- with should_validate_formats(true) --
format-uri-opt-in:    valid=false  errors=["\"!!!nope!!!\" is not a \"uri\""]
format-email-opt-in:  valid=false
format-uuid-opt-in:   valid=false
format-ipv4-opt-in:   valid=false
```
`[MEASURED: probe — jsonschema 0.49.2]`

**D2/SC-2 planning consequence.** Adding `format` to `ParamDecl` and emitting it into `inputSchema`
gives **zero** runtime enforcement on the path `compile_2020_12` takes today, because
`jsonschema::draft202012::new()` does not assert formats. Two viable plans:
- **(A)** Build input validators through `jsonschema::options().with_draft(Draft::Draft202012).should_validate_formats(true).build(&schema)` — enforces `format`, and note the opt-in messages ALSO echo the value, so 1c's renderer still applies.
- **(B)** Ship `format` as a documentation-only annotation and say so explicitly in the `ParamDecl` rustdoc and the docs deliverable.
Picking (A) silently changes `compile_2020_12`'s behaviour if the same function is shared with
outputs — so if (A) is chosen, **inputs need their own compile entry point**, not a shared one.
This is a genuine open question for the planner (see Open Questions Q1).

### 1h. Draft-07 constructs fail to compile under the 2020-12 pin

```
{"$schema":"…draft-07…","properties":{"a":{"exclusiveMinimum":true,"minimum":1}}}  -> FAILS: true is not of type "number"
{"$schema":"…draft-07…","type":"array","items":[{"type":"string"}]}               -> FAILS: [{"type":"string"}] is not of types "boolean", "object"
```
`[MEASURED: probe]` — consistent with `[VERIFIED: src/server/output_validation.rs:55-60]`
("`exclusiveMinimum: true` and array-form `items` are the measured cases"). **D2 consequence:** the
new `items` / `max_items` vocabulary must emit **object-form** `items`, never array-form, or the
whole tool's validator fails to compile.

A `$schema: draft-07` declaration on a schema with no incompatible constructs compiles fine and is
simply ignored by `draft202012::new` (`legacy-dollar-schema: valid=true` for a `maxLength:2` schema
against `"abc"` — i.e. the 2020-12 pin applied and the doc's own dialect did not) `[MEASURED: probe]`.
This is why D-02's "a config- or schemars-declared `$schema` must not be able to change input
enforcement semantics" holds for free once `compile_2020_12` is used.

### 1i. Cost model — the cache key is more expensive than the validation

```
100 compiles:                            1.040ms      (10.404µs/compile)
100k is_valid (conforming):              3.720ms      (37ns/call)
100k schema.to_string() (the CACHE KEY): 54.098ms     (540ns/call)
```
`[MEASURED: probe — release build]`

`cached_validator` keys on `(Era, schema.to_string())` `[VERIFIED: src/server/output_validation.rs:641-645]`.
So reusing it per `tools/call` costs **540ns of key serialization to save 10.4µs of compile** — still a
win, but the key is 14× the validation itself. All three numbers are negligible beside an HTTP
round-trip; the note matters only if someone later profiles. A per-tool `OnceLock<Arc<Validator>>` on
the synthesized handler would be cheaper still and is a legitimate alternative the planner may pick
(see Finding 2).

### 1j. No ReDoS blowup observed

```
(a+)+$  vs 20/28/64 'a's + 'b'  (plain engine): 2.79µs / 2.21µs / 2.08µs
(?=[\s\S])^(a+)+$ vs 20/24/28   (fancy engine): 3.25µs / 2.67µs / 3.79µs
```
`[MEASURED: probe]` — a positive falsification attempt on "a config-supplied `pattern` is a DoS
vector" that **did not** reproduce at n ≤ 28 on either engine. This is not proof that no pattern can
blow up (fancy-regex is a backtracking engine and I did not search the input space), so the residual
risk is `[ASSUMED]` and recorded in the Assumptions Log. Do not plan a ReDoS mitigation on the
strength of a hunch; if the planner wants one, a fuzz target over `(pattern, input)` pairs is the
cheap way to look (see Finding 9).

## Finding 2 — The D-01 generalization shape

### 2a. The three-function split, and exactly why it exists

`src/server/output_validation.rs` is **2155 lines** `[VERIFIED: wc -l]`. The split CONTEXT.md tells
you to preserve is documented in the source itself:

> `[VERIFIED: src/server/output_validation.rs:598-605]`
> ```
> /// This is deliberately split out of [`cached_validator`] rather than inlined
> /// into it: the process-global cache is unbounded by design (bounded in
> /// practice by the number of distinct DECLARED schemas), so a property or fuzz
> /// target that compiles arbitrary generated schemas needs a path that does not
> /// grow it without limit. Keep this function separate — do not inline it back.
> ///
> /// Splitting normalization, compilation and caching across three functions is
> /// also what keeps each of them under CI's cognitive-complexity cap.
> ```

**The three functions are:**

| Function | Line | Signature (verbatim) | Role |
|---|---|---|---|
| `normalize_schema_dialect` | `:553` | `fn normalize_schema_dialect(schema: &Value) -> std::borrow::Cow<'_, Value>` | rewrites legacy `$schema` declarations in schema positions |
| `compile_2020_12` | `:574` | `fn compile_2020_12(schema: &Value) -> Result<jsonschema::Validator, jsonschema::ValidationError<'static>>` | normalize → `tracing::warn!` if rewritten → `jsonschema::draft202012::new(&normalized)` |
| `cached_validator` | `:631` | `fn cached_validator(era: Option<Era>, schema: &Value) -> Result<std::sync::Arc<jsonschema::Validator>, std::sync::Arc<str>>` | the `(Era, String)`-keyed memo |

with `compile_for_era` (`:607`) as the fourth, thin, era-branching layer:
`[VERIFIED: src/server/output_validation.rs:607-615]`
```rust
fn compile_for_era(era: Era, schema: &Value) -> Result<jsonschema::Validator, std::sync::Arc<str>> {
    match era {
        Era::V1 => jsonschema::validator_for(schema),
        Era::V2 => compile_2020_12(schema),
    }
    .map_err(|e| std::sync::Arc::from(e.to_string().as_str()))
}
```

**PMAT confirms the split is currently doing its job.** `pmat analyze complexity --format json
--max-cognitive 25` (PMAT 3.15.0, the CI-pinned version) reports **21 violations repo-wide, every one
of them in a `tests/` file** — `crates/mcp-tester/tests/property_tests.rs`,
`crates/pmcp-server-toolkit/tests/sql_server_http_example.rs`, `tests/v2_schema_tripwires.rs`,
`tests/v1_severability_tripwire.rs`, and six others. **Zero violations in any `src/` directory of any
crate this phase touches.** `[MEASURED: pmat analyze complexity, this session]` Summary line:
`total_functions: 298, median_cognitive: 1.0, max_cognitive: 73, p90_cognitive: 16`.

> **Planner rule:** do not plan a task that merges `normalize_schema_dialect` / `compile_2020_12` /
> `cached_validator`, and do not plan one that adds branching inside any of them. A new
> `validate_input` should be a *fourth sibling*, not a fifth branch.

### 2b. ⚠ Do NOT rename the file or the module — measured blast radius

D-01's wording is "Generalize `src/server/output_validation.rs` into `server::schema_validation`".
Taken as a **rename**, it costs the following, all measured this session:

| Consumer | Line | What breaks |
|---|---|---|
| `tests/v2_schema_tripwires.rs` | `:98` | `const OUTPUT_VALIDATION: &str = "src/server/output_validation.rs";` — a **path-string literal** the tripwire opens |
| `tests/v2_schema_tripwires.rs` | `:1059`, `:1069` | two more `file: "src/server/output_validation.rs"` literals |
| `tests/property_tests.rs` | `:1000`, `:1137` | `use pmcp::server::output_validation::fuzz_support::normalize_bytes;` / `use pmcp::server::output_validation::fuzz_support;` |
| `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` | `:23`, `:30` | reaches the module through the `fuzzing`-gated `fuzz_support` seam |
| `src/server/output_validation.rs` | `:807` | documents a nextest selector `test(/output_validation::fuzz_support/)` — and this repo's memory records that `test()` vs `binary()` selectors **silently select zero tests** rather than failing |
| `src/server/core.rs` | `:1179` | `crate::server::output_validation::warn_on_schema_mismatch(` |
| `src/server/mod.rs` | `:2722` | `output_validation::warn_on_schema_mismatch(` |
| `src/types/tools.rs`, `src/types/protocol/version.rs` | `:810`, `:64` | rustdoc cross-references |

`[VERIFIED: grep -rn "output_validation" src tests fuzz Makefile .github scripts]`

**Recommended shape (additive, zero rename):**

```rust
// src/server/mod.rs — NEW, alongside the existing pair
/// Runtime enforcement of a tool's declared `inputSchema` (Phase 128, D-01).
#[cfg(not(target_arch = "wasm32"))]
pub mod schema_validation;
```

`schema_validation.rs` holds the **public** `validate_input` + its value-free renderer + its error
type, and calls into `output_validation`'s crate-private helpers (which become
`pub(in crate::server)` or move to a shared private submodule). `output_validation` keeps its name,
its path, its fuzz seam and its selector.

**Why this also matters for the public API surface.** `output_validation` is `pub(crate)` on every
normal build and only widened under the `fuzzing` feature:
`[VERIFIED: src/server/mod.rs:83-88]`
```rust
#[cfg(not(feature = "fuzzing"))]
pub(crate) mod output_validation;
/// Warn-only emit-time validation of `structuredContent` against a declared
/// `outputSchema` (no-op unless the `validation` feature is enabled).
#[cfg(feature = "fuzzing")]
pub mod output_validation;
```
Making that module unconditionally `pub` to expose one function would also publish
`schema_mismatch`, `normalize_schema_dialect`, `DATA_ONLY_KEYWORDS`, `SUBSCHEMA_MAP_KEYWORDS` and the
rest as shipped API. A separate narrow module avoids that entirely. (There is **no** `cargo
public-api` gate in this repo — `[VERIFIED: grep -rn "public-api|public_api" Makefile .github/workflows/*.yml]`
returns nothing — so nothing will *stop* an over-wide `pub`; the discipline has to come from the plan.)

### 2c. The concrete seam

```rust
// src/server/schema_validation.rs   (feature = "schema-validation")

/// Refusal detail for a `tools/call` argument-schema violation.
///
/// Deliberately carries DECLARED data only: no rejected value, and no
/// attacker-supplied key (SC-7). `unknown_count` is the ONLY thing taken from
/// `ValidationErrorKind::AdditionalProperties`, whose `unexpected` field is the
/// attacker's key list.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct InputViolation {
    /// JSON pointer into the arguments — always a DECLARED property name, and
    /// empty for `additionalProperties` (measured: 0.49.2 reports root).
    pub pointer: String,
    /// The violated JSON Schema keyword, e.g. `"maxLength"`, `"pattern"`.
    pub keyword: &'static str,
    /// The DECLARED expectation, rendered value-free.
    pub expected: String,
}

/// Check `arguments` against a tool's declared `inputSchema`.
///
/// - `arguments == None` or `Value::Null` is treated as `{}` (D-01 / the MCP
///   "missing arguments" semantics), so a zero-parameter tool ACCEPTS and a
///   tool with `required` params is REFUSED by `required` rather than by `type`.
/// - Compiled under Draft 2020-12 on BOTH eras (D-02) — `Era` is deliberately
///   NOT a parameter. See the module docs for why the v1/v2 asymmetry that
///   `output_validation::compile_for_era` carries is correct there and wrong here.
/// - `additionalProperties: false` is HONOURED as declared, never re-added (D-01).
///
/// # Errors
/// `Err(Vec<InputViolation>)` on violation; `Err` with a single
/// `keyword: "schema"` violation when the declared schema does not compile.
pub fn validate_input(
    schema: &serde_json::Value,
    arguments: Option<&serde_json::Value>,
) -> Result<(), Vec<InputViolation>>;

/// Render violations as ONE client-facing message.
/// Shape (locked by SC-7): `unknown argument(s): 2; allowed: cui, version`
pub fn render_refusal(violations: &[InputViolation], declared: &[&str]) -> String;
```

**How the existing assets are reused:**

| Asset | Reuse |
|---|---|
| `compile_2020_12` (`:574`) | called directly — D-02 says inputs bypass `compile_for_era`'s arms. Inherits the `tracing::warn!` at `:581-588` for free. |
| `cached_validator` (`:631`) | either call it with a fixed `Era::V2` (cheapest, no new cache), **or** add a sibling `cached_input_validator` keyed on schema text alone. **Prefer the latter** if Finding 1g option (A) is taken, because format-asserting validators must not collide with output validators under the same key. |
| the `(Era, String)` key | 540ns/call (Finding 1i). A per-handler `OnceLock<Arc<Validator>>` in the toolkit is the cheaper alternative and needs no core cache change. |
| `normalize_schema_dialect` (`:553`) | reached transitively through `compile_2020_12`; nothing new. |
| error renderer (`:128-131`) | **NOT reused** — see Finding 1b. |

**Expressing "missing `arguments` (null) treated as `{}`":** the measurement says this must be done
*before* handing the value to the validator, because 0.49.2 refuses `null` on `type: object`:
```
zeroparam-null:      valid=false  errors=["null is not of type \"object\""]
zeroparam-empty-obj: valid=true
required-empty-obj:  valid=false  errors=["\"cui\" is a required property"]
required-null:       valid=false  errors=["null is not of type \"object\""]
```
`[MEASURED: probe — jsonschema 0.49.2]` — i.e. `match arguments { None | Some(Value::Null) => &EMPTY_OBJ, Some(v) => v }`.
**This is exactly what makes the CR's two easily-conflated acceptance rows come out right:**
zero-parameter + no arguments → ACCEPTED as `{}`; required params + no arguments → REFUSED by
`Required { property }` (a declared name, SC-7-safe) rather than by `Type` (whose message would echo
`null`).

### 2d. Where the enforcement is NOT wired this phase (and where it would go)

Four unchecked dispatch sites exist, not two:
- `[VERIFIED: src/server/mod.rs:2590]` — `let result = handler.handle(req.arguments, extra).await;` (the `#[cfg(target_arch = "wasm32")]` arm)
- `[VERIFIED: src/server/mod.rs:2820]` — `let result = match handler.handle(req.arguments, extra).await {`
- `[VERIFIED: src/server/core.rs:1085]` — `handler.handle(args, extra).await` (wasm32 arm)
- `[VERIFIED: src/server/core.rs:1270]` — `handler.handle(req.arguments.clone(), extra).await`

The native `core.rs` path actually dispatches through `handle_output`:
`[VERIFIED: src/server/core.rs:1026]` — `let output = handler.handle_output(args, extra).await;`
— and the **one** input guard that exists today sits just above it:
`[VERIFIED: src/server/core.rs:1014-1023]`
```rust
if self.payload_limits.max_tool_args_bytes < usize::MAX {
    let args_size = json_serialized_len(&args)?;
    if args_size > self.payload_limits.max_tool_args_bytes {
        return Err(Error::validation(format!(
            "Tool arguments for '{}' exceed size limit ({} bytes > {} max)",
            req.name, args_size, self.payload_limits.max_tool_args_bytes
        )));
    }
}
```
That is a **byte-size** cap on the whole argument object — not a schema check, and not a per-string
cap. It is the natural neighbour for the deferred wiring, because it is already post-middleware
(so inflated args are caught) and pre-handler. Record it in the deferred item so the later phase
does not re-derive it.

## Finding 3 — D-09 blast radius

### 3a. The trait, verbatim

`[VERIFIED: crates/pmcp-code-mode/src/executor.rs:2421-2432]`
```rust
#[async_trait::async_trait]
pub trait HttpExecutor: Send + Sync {
    /// Execute an HTTP request.
    async fn execute_request(
        &self,
        method: &str,
        path: &str,
        body: Option<JsonValue>,
    ) -> Result<JsonValue, ExecutionError>;
}
```

### 3b. Every in-tree implementor — four, one of them public API

| Implementor | Location | Visibility | Impact of a signature change |
|---|---|---|---|
| `NoopHttpExecutor` | `crates/pmcp-code-mode/src/code_executor.rs:276,280` | `struct NoopHttpExecutor;` — **private** | mechanical |
| `MockHttpExecutor` | `crates/pmcp-code-mode/src/executor.rs:2515,2672` | **`pub struct`** — shipped API, documented for downstream tests (`:2496-2502`) | **breaking for downstream test code** |
| `MockHttpExecutor` (test-local) | `crates/pmcp-code-mode/src/executor.rs:3353,3370` | `#[cfg(test)]` | mechanical |
| `HttpCodeExecutor` | `crates/pmcp-server-toolkit/src/code_mode.rs:971` | `pub struct`, `#[cfg(feature = "openapi-code-mode")]` | **the one that matters** — it is where `resolve_path` lives |

`[VERIFIED: grep -rn "impl HttpExecutor for|HttpExecutor for " src crates cargo-pmcp examples tests]`

### 3c. Every `.execute_request(` call site — two in the library, five in tests

| Call site | Context |
|---|---|
| `crates/pmcp-code-mode/src/executor.rs:2843` | `PlanStep::ApiCall` arm of `PlanExecutor::execute_step` |
| `crates/pmcp-code-mode/src/executor.rs:3016` | `PlanStep::ParallelApiCalls` arm |
| `crates/pmcp-server-toolkit/tests/http_connector_props.rs:255` | property test |
| `crates/pmcp-server-toolkit/tests/http_executor.rs:40, 65, 95, 121` | four integration tests |

`[VERIFIED: grep -rn "\.execute_request(" src crates cargo-pmcp examples tests]`

**Both library call sites already have a `resolved_path` in hand** — but it is resolved at a
*different layer*:

`[VERIFIED: crates/pmcp-code-mode/src/executor.rs:2833-2848]`
```rust
let resolved_path = self.resolve_path(path)?;
let resolved_body = match body {
    Some(expr) => Some(self.evaluate(expr)?),
    None => None,
};

let call_start = std::time::Instant::now();
let raw_response = self
    .http
    .execute_request(method, &resolved_path, resolved_body.clone())
    .await
    .map_err(|e| ExecutionError::RuntimeError {
        message: format!("{} {} failed: {}", method, resolved_path, e),
    })?;
```

**There are TWO resolution layers, and the CR's blindness argument is about the second:**

| Layer | Function | Resolves | Where |
|---|---|---|---|
| 1 | `PlanExecutor::resolve_path(&self, path: &PathTemplate)` `[VERIFIED: executor.rs:3146]` | script-level `PathPart::Variable` / `PathPart::Expression` interpolation | `pmcp-code-mode`, **before** `execute_request` |
| 2 | `HttpCodeExecutor::resolve_path(path, &body)` `[VERIFIED: code_mode.rs:~905-935]` | OpenAPI `{key}` placeholders, from **body keys** | toolkit, **inside** the impl |

D-09 moves **layer 2** up next to layer 1. That is the change; layer 1 is untouched.

> ### ⚠ 3d. A defect D-09's implementation must fix at the same time
>
> `[VERIFIED: crates/pmcp-code-mode/src/executor.rs:2846, and identically at :3018]`
> ```rust
> .map_err(|e| ExecutionError::RuntimeError {
>     message: format!("{} {} failed: {}", method, resolved_path, e),
> })?;
> ```
>
> **`PlanExecutor` wraps every `execute_request` error with the RESOLVED PATH.** D4's contract is
> "Refusals use fixed messages with no path or value", and the CR-01 acceptance rows require "a
> refusal message containing neither the value nor the path". A refusal raised inside
> `execute_request` today is re-wrapped with the exact injected path the check refused. Moving
> placeholder resolution *up* to this layer means the refusal is raised **here**, so the fix is in
> the same function — but it is a distinct edit at two sites and will be missed if the plan does not
> name it. Note `ApiCallLog { path: resolved_path, body: resolved_body, … }` (`:2856-2861`) also
> records the resolved path and body; that is an internal execution log rather than a client-facing
> message, but a plan that surfaces `api_calls` to a caller inherits the same leak.

### 3e. `HttpCodeExecutor::execute_request` — the five numbered steps, today

`[VERIFIED: crates/pmcp-server-toolkit/src/code_mode.rs:972-1050]`, the inline step comments verbatim:

1. `// (1) Path-param substitution from the body object. A non-scalar {key} value is rejected (WR-03)`
   → `let (resolved_path, remaining_body) = Self::resolve_path(path, &body)?;`
2. `// (2) Shared join_url helper (Pitfall 2 — preserves an API-Gateway stage prefix …)`
   → `let url = crate::http::join_url(&self.base_url, &resolved_path);`
3. `// (3) Apply auth, threading the per-request inbound token (H1).`
   → `self.auth.apply(&mut headers, &mut auth_query, self.inbound_token.as_deref()).await`
4. `// (4) For GET-like requests, serialize remaining body fields as query params; otherwise keep them as the JSON body.`
5. `// (5) Send + read. Transport / status / parse errors NEVER echo the URL or token (Pitfall 5).`

**D-12's "E1 runs before outgoing auth" therefore lands between step (2) and step (3)** — after the
path is joined (so the policy sees the URL as it will be sent) and before the credential exists in
`headers`. This is a clean, already-numbered seam.

`resolve_path` (the layer-2 one) verbatim, for the planner moving it:
`[VERIFIED: crates/pmcp-server-toolkit/src/code_mode.rs:~907-932]`
```rust
fn resolve_path(
    path: &str,
    body: &Option<serde_json::Value>,
) -> std::result::Result<(String, Option<serde_json::Value>), ExecutionError> {
    let mut resolved_path = path.to_string();
    let remaining = if let Some(serde_json::Value::Object(obj)) = body {
        let mut remaining = serde_json::Map::new();
        for (key, value) in obj {
            let placeholder = format!("{{{key}}}");
            if resolved_path.contains(&placeholder) {
                resolved_path =
                    resolved_path.replace(&placeholder, &Self::scalar_str(key, value)?);
            } else {
                remaining.insert(key.clone(), value.clone());
            }
        }
        …
```
Note its own doc comment already records *why* it is a free helper:
`/// (kept a free helper so the trait method stays under the cog ≤25 budget)` — preserve that.

`scalar_str`, the WR-03 gate, verbatim:
`[VERIFIED: crates/pmcp-server-toolkit/src/code_mode.rs:~955-970]`
```rust
fn scalar_str(key: &str, value: &serde_json::Value)
    -> std::result::Result<String, ExecutionError> {
    match value {
        serde_json::Value::String(s) => Ok(s.clone()),
        serde_json::Value::Null => Ok("null".to_string()),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        serde_json::Value::Bool(b) => Ok(b.to_string()),
        serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
            Err(ExecutionError::RuntimeError {
                message: format!("path/query param '{key}' must be a scalar"),
            })
        },
    }
}
```
It rejects only non-scalars. **Any `String` passes through verbatim** — this is the Code Mode half of
CR-01. Its doc comment is also the in-tree statement of the SC-7 convention:
`/// Per Pitfall 5 the message names the KEY only — never the value.` D4's new refusals should match
that wording so the two surfaces read alike.

### 3f. Sizing the break

- **In-tree cost:** 4 impls + 2 library call sites + 5 test call sites = **11 edits**, all mechanical.
- **Published cost:** `pub trait HttpExecutor` and `pub struct MockHttpExecutor` are both shipped
  API of `pmcp-code-mode`. A signature change is breaking for any downstream implementor **and** for
  downstream code constructing mock expectations. This is what D-13 prices as `0.5.4 → 0.6.0`.
- **The thing D-09 buys:** pmcp.run's built-in `openapi-api` server is an out-of-repo
  `HttpExecutor` implementor. Resolving before dispatch makes it safe **without a platform change** —
  which is how CR open question 5 gets answered by construction. `[VERIFIED: 128-CONTEXT.md D-09]`

## Finding 4 — `garde` 0.23 on `TypedTool<T>` (E3 / SC-5)

### 4a. `garde` really is a zero-reference dependency

`[VERIFIED: grep -rn "garde" src/ crates/ cargo-pmcp/]` → **zero matches, repo-wide, outside
manifests.** `[VERIFIED: Cargo.toml:221]` — `garde = { version = "0.23", optional = true }`;
`[VERIFIED: Cargo.lock:2737-2738]` — `name = "garde"` / `version = "0.23.0"`;
`[VERIFIED: cargo tree -p pmcp --features validation -e normal -i garde]` →
`garde v0.23.0 └── pmcp v2.20.4` (the only edge). SC-5's "retiring `garde`'s zero-reference status"
is accurate.

### 4b. `garde` 0.23's API, measured

A probe crate on `garde = { version = "0.23", features = ["derive"] }` was compiled and run:

```
Display (DOES IT ECHO THE VALUE?):
cui: length is greater than 10
n: greater than 100

  path=cui  err=length is greater than 10  err_debug=Error { message: "length is greater than 10" }
  path=n    err=greater than 100           err_debug=Error { message: "greater than 100" }
good => true
PHI case Display:
cui: length is greater than 10
```
`[MEASURED: probe — garde 0.23.0]`

| Fact | Consequence for E3 |
|---|---|
| `Validate` carries an associated type `Context`; `fn validate(&self) -> Result<(), garde::Report>` exists for `Context = ()`, `validate_with(&self, ctx)` otherwise | the ergonomic bound is `T: garde::Validate<Context = ()>`; a context-carrying `T` needs a second entry point or is out of scope |
| `garde::Report::iter()` yields `(Path, Error)`; `Error` is `{ message: String }` | mapping to `InputViolation` is direct |
| **`Report`'s `Display` does NOT echo the value** — the PHI input `"Jane Doe DOB 1970-01-01"` rendered as `cui: length is greater than 10` | E3 is value-free *by construction*, unlike `jsonschema` (Finding 1b). **Write this asymmetry down** — a reviewer comparing D1 and E3 will otherwise ask why one needs a renderer and the other does not. |
| The **path** is echoed (`cui`, `n`) | safe: it is a declared Rust struct field, not an attacker-supplied key |
| A `#[garde(custom(…))]` validator supplies its own message | **residual leak surface** — the docs deliverable must say "a custom garde validator's message is echoed; do not put the value in it" |

### 4c. The attach point, and why the bound cannot simply be added

`[VERIFIED: src/server/typed_tool.rs:24-39]`
```rust
/// A typed tool implementation with automatic schema generation and validation.
pub struct TypedTool<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>>
        + Send
        + Sync,
{
    name: String,
    description: Option<String>,
    input_schema: Value,
    annotations: Option<ToolAnnotations>,
    ui_resource_uri: Option<String>,
    execution: Option<ToolExecution>,
    handler: F,
    _phantom: PhantomData<T>,
}
```

`[VERIFIED: src/server/typed_tool.rs:246-261]`
```rust
impl<T, F> ToolHandler for TypedTool<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    …
{
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        // Deserialize and validate the arguments
        let typed_args: T = serde_json::from_value(args).map_err(|e| {
            crate::Error::Validation(format!("Invalid arguments for tool '{}': {}", self.name, e))
        })?;

        // Call the handler with the typed arguments
        (self.handler)(typed_args, extra).await
    }
```

The comment says "Deserialize **and validate**". It does not validate — it deserializes. That is a
**fourth** place in the tree where a comment claims validation that does not exist. SC-6 is scoped to
the toolkit, so correcting this line is arguably in E3's own scope rather than SC-6's; either way,
name it in a plan or it stays.

**The bound `T: garde::Validate` CANNOT be added to the existing `impl` or `struct`** — that is a
breaking change for every existing `TypedTool<T>` whose `T` does not implement it, and Rust has no
stable specialization to make it conditional. The CR's "It's additive, since types that don't
implement `Validate` behave as today" is therefore a *goal*, not a free property.

**Three additive shapes, in order of recommendation:**

| Shape | Sketch | Cost |
|---|---|---|
| **A. Stored validator fn (recommended)** | add `validator: Option<Box<dyn Fn(&T) -> Result<(), garde::Report> + Send + Sync>>` to the struct; a new constructor `TypedTool::new_validated<T2>(…) where T: garde::Validate<Context = ()>` populates it; `handle` runs it after deserialize | one new field, one new constructor, zero bound changes. `TypedTool` is not `#[non_exhaustive]`-sensitive here because all fields are private. |
| **B. Wrapper type** | `pub struct ValidatedTypedTool<T, F>(TypedTool<T, F>)` with the extra bound | clean, but duplicates the `ToolHandler` impl and the builder surface |
| **C. Blanket via a marker trait** | a `MaybeValidate` trait with a default no-op impl | needs specialization or a negative-impl workaround — not stable Rust |

**Do not forget `TypedSyncTool<T, F>`** `[VERIFIED: src/server/typed_tool.rs:278-292]` — the same
struct exists with `F: Fn(T, RequestHandlerExtra) -> Result<Value>`. E3 should either cover both or
state explicitly that it covers only the async one. The ROADMAP's SC-5 says "`garde` runs on
`TypedTool<T>`", which is satisfiable either way; the plan must pick.

**Feature gate:** E3 stays on `validation`, which after the D-04 split is
`validation = ["schema-validation", "dep:garde"]` — so an existing `validation` consumer sees no
change, exactly as D-04 promises.

## Finding 5 — `validate_safe_path` harvest (D-03 / D4(a))

### 5a. What it checks today — verbatim

`[VERIFIED: src/server/validation.rs:252-281]`
```rust
pub fn validate_safe_path(field: &str, path: &str, allowed_prefix: Option<&str>) -> Result<()> {
    // Check for path traversal
    if path.contains("..") {
        return Err(ValidationError::elicit(
            "path_traversal",
            field,
            "Path must not contain '..'",
        ));
    }

    // Check for null bytes
    if path.contains('\0') {
        return Err(ValidationError::elicit(
            "invalid_path",
            field,
            "Path must not contain null bytes",
        ));
    }

    // Check allowed prefix
    if let Some(prefix) = allowed_prefix {
        if !path.starts_with(prefix) {
            return Err(ValidationError::elicit(
                "path_not_allowed",
                field,
                format!("Path must start with '{}'", prefix),
            ));
        }
    }

    Ok(())
}
```

It is also **already SC-7-shaped**: `ValidationError::elicit` builds
`format!("Validation failed for field '{}'", &field_str)` plus a machine-readable
`{code, field, expected, elicit}` JSON object `[VERIFIED: src/server/validation.rs:23-40]` — the
*expected* string, never the rejected value. That is why D-03 calls it "already the right primitive".

### 5b. The gap against D-10's unconditional floor

| D-10 floor item | Covered today? | Evidence |
|---|---|---|
| `..` | ✅ substring check | `:254` |
| `/` | ❌ | no check |
| `?` | ❌ | no check |
| `#` | ❌ | no check |
| `%2e%2e` / `%2E%2E` (encoded `..`) | ❌ | the check is a literal `contains("..")` |
| `%2f` / `%2F` (encoded `/`) | ❌ | — |
| `%3f` (encoded `?`) | ❌ | — |
| `%23` (encoded `#`) | ❌ | — |
| NUL byte | ✅ (bonus, not in D-10) | `:260` |
| `%00` (encoded NUL) | ❌ | — |
| allowed-prefix | ✅ (bonus; not applicable to a placeholder VALUE) | `:266` |

So the harvest covers **1 of the 4** unconditional denylist characters and **0 of the 4**
percent-encoded forms. `validate_path_placeholder` is mostly new code with `validate_safe_path`'s
*shape* borrowed, not its body. Two design notes the plan should fix:

- **Case-insensitive hex.** `%2E`, `%2e`, `%2F`, `%2f` all decode the same. A `contains("%2e")` is
  a bypass; the check must be ASCII-case-insensitive.
- **Double encoding.** `%252e` decodes to `%2e` which decodes to `.`. Refusing a bare `%` in a
  placeholder value is the simplest floor that closes this whole family; the CR does not name it,
  so it is an Open Question (Q3) rather than a decided rule.

Measured empirically against the two pattern styles D-10 contrasts:
```
2026AA?string=x               strict-unreserved=false  loose(^.*$)=true
current/../../search/current  strict-unreserved=false  loose(^.*$)=true
%2e%2e%2f                     strict-unreserved=false  loose(^.*$)=true
%3Fstring%3Dx                 strict-unreserved=false  loose(^.*$)=true
a%00b                         strict-unreserved=false  loose(^.*$)=true
a#frag                        strict-unreserved=false  loose(^.*$)=true
```
`[MEASURED: probe — jsonschema 0.49.2]` — **D-10's rationale is empirically confirmed**: a
spec-declared `pattern` of `^.*$` accepts every single CR-01 payload, so "pattern supersedes
denylist" would have silently disabled the check. (Aside, useful for the fixture author: `^.*$`
*does* refuse an embedded LF, because `.` excludes `\n`.)

### 5c. ⚠ `#[deprecated]` on `pub mod validation` **DOES** break `make quality-gate`

This was the specific question asked, and the answer is **yes**.

`[VERIFIED: src/server/mod.rs:204-206]`
```rust
/// Validation helpers for typed tools.
#[cfg(not(target_arch = "wasm32"))]
pub mod validation;
```

**In-tree consumers of the module, outside its own doctests:**
`[VERIFIED: examples/s19_wasm_typed_tools.rs:210, 217, 224, 231, 317, 318, 319, 321]`
```rust
if let Err(e) = validation::validate_email("email", &args.email) {
if let Err(e) = validation::validate_url("url", &args.url) {
if let Err(e) = validation::validate_range("age", args.age, 18, 120) {
if let Err(e) = validation::validate_length("username", &args.username, Some(3), Some(20)) {
…
assert!(validation::validate_email("email", &args.email).is_ok());
```

**Why this fails the gate:** `[VERIFIED: Makefile:216-221]`
```make
lint:
	@echo "$(BLUE)Running clippy...$(NC)"
	RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) clippy --features "full" --lib --tests -- $(CLIPPY_POLICY)
	@echo "$(BLUE)Checking examples...$(NC)"
	RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) check --features "full" --examples
```
with `[VERIFIED: Makefile:11]` `RUSTFLAGS = -D warnings`, and `make lint` is a `quality-gate` leg
`[VERIFIED: Makefile:1936]`. A `#[deprecated]` module produces `deprecated` warnings at all eight
call sites in `s19_wasm_typed_tools.rs`, which `cargo check --features "full" --examples` compiles
under `-D warnings`. **`make lint` fails.**

**Remediations, in order of preference:**

| Option | Effect |
|---|---|
| **(a) Add `#[allow(deprecated)]` to the example** (module-level or at the eight sites), with a `// Why:` comment pointing at Phase 128 D-03 and the booked removal | smallest, honest — the example genuinely demonstrates a deprecated API |
| **(b) Rewrite `s19_wasm_typed_tools.rs`** to demonstrate `garde` on `TypedTool` instead | larger, but arguably *correct*: E3 is precisely the replacement for what that example teaches, and SC-8's "example coverage" could be discharged here |
| (c) `#[doc(hidden)]` only, skip `#[deprecated]` | gives up half of D-03(b) — not recommended, D-03 says "**and**" |

**`#[doc(hidden)]` alone is safe** — it changes rustdoc output, not compilation, and `make doc-check`
(a gate leg, `[VERIFIED: Makefile:1943]`) has no hidden-item rule.

**The 10 doctests inside `validation.rs` itself** (`:54, 80, 111, 137, 180, 214, 246, 290, 312`, +1)
each `use pmcp::server::validation::validate_*`. `make test-doc` runs
`RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) test --doc --features "full"` `[VERIFIED: Makefile:791-794]`.
Whether `RUSTFLAGS` reaches rustdoc's doctest compilation is **`[ASSUMED]` — not measured this
session**; budget a plan task to run `make test-doc` after applying the attribute, and add
`# #![allow(deprecated)]` hidden lines to the doctests if it bites.

### 5d. The fourth false claim (D-03(c))

`[VERIFIED: .planning/codebase/ARCHITECTURE.md:197-201]`
```
**Validation:**
- Framework: Optional `jsonschema` + `garde` (feature-gated "validation")
- Locations: `src/server/validation.rs` for general validation, typed tools auto-validate via schema
- Usage: Server can validate tool inputs before calling handler
```
Both the second and third bullets are false today: typed tools do **not** auto-validate via schema
(Finding 4c), and the server does **not** validate tool inputs before calling the handler
(Finding 2d). D-03(c) names only line 200; line 199's "typed tools auto-validate via schema" is the
same defect one line up and should be corrected in the same edit.

## Finding 6 — `substitute_path`, the curated single-call half of CR-01

`[VERIFIED: crates/pmcp-server-toolkit/src/http/client.rs:143-163]`
```rust
/// Substitute path parameters into the operation path template.
///
/// # Errors
///
/// Returns [`HttpConnectorError::Backend`] (via [`render_scalar`]) when a path
/// parameter value is a non-scalar (`Object`/`Array`) — such a value would
/// otherwise be JSON-stringified into the URL (WR-03).
fn substitute_path(
    operation: &Operation,
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<String, HttpConnectorError> {
    let mut path = operation.path.clone();
    for param in operation.path_parameters() {
        let placeholder = format!("{{{}}}", param.name);
        if let Some(value) = args.get(&param.name) {
            let value_str = render_scalar(&param.name, value)?;
            path = path.replace(&placeholder, &value_str);
        }
    }
    Ok(path)
}
```

**`operation.path_parameters()` is the D-05 position oracle.** It is already the loop driver here,
so "path position" is derivable with no new config surface, exactly as CONTEXT.md's
`<code_context>` claims. Query position comes from `build_query`'s query-located-param filter
(`render_query_value` at `:353-372` comma-joins arrays; the doc comment at `:350-352` records the
`form`/`explode:false` style). Everything else is body position.

`render_scalar` verbatim `[VERIFIED: crates/pmcp-server-toolkit/src/http/client.rs:354-372]`:
```rust
fn render_scalar(
    param_name: &str,
    value: &serde_json::Value,
) -> Result<String, HttpConnectorError> {
    match value {
        serde_json::Value::String(s) => Ok(s.clone()),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        serde_json::Value::Bool(b) => Ok(b.to_string()),
        serde_json::Value::Null => Ok("null".to_string()),
        // Object OR Array: non-scalar in a path/query/header position is rejected
        // rather than silently JSON-stringified. Name the param ONLY (Pitfall 5).
        serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
            Err(HttpConnectorError::Backend(format!(
                "param '{param_name}' must be a scalar (non-scalar values are \
                 not supported in path/query/header position)"
            )))
        },
    }
}
```

This is the exact twin of `code_mode.rs`'s `scalar_str` (Finding 3e) — same rule, same redaction
convention, different error type (`HttpConnectorError::Backend` vs `ExecutionError::RuntimeError`).
**D4's placeholder check belongs in `render_scalar`'s neighbourhood on this surface and in
`scalar_str`'s on the other**, so the two stay parallel. Both already carry the Pitfall-5 comment,
so the new refusals should use the same wording.

**Consequence for D-09's shared helper.** `validate_path_placeholder` is exported from
`pmcp-code-mode` (D-09). `http/client.rs` is behind the toolkit's `http` feature, which does **not**
pull `pmcp-code-mode` `[VERIFIED: crates/pmcp-server-toolkit/Cargo.toml:112-124]` — the `http`
feature list is `["dep:reqwest", "dep:url", "dep:openapiv3", "dep:serde_yaml", "dep:base64",
"dep:regex", "dep:tokio", "pmcp/streamable-http"]`, with no `pmcp-code-mode`. **So the curated
single-call build cannot call the `pmcp-code-mode` helper without a new feature edge that would
violate SC-1's "no JS engine in curated builds".**

Three ways out — this is a real fork the plan must resolve (Open Question Q2):

| Option | Effect on SC-1 |
|---|---|
| **A. Put the primitive in core `pmcp`** (e.g. `server::schema_validation::validate_path_placeholder`) and have **both** `pmcp-code-mode` and the toolkit's `http` surface call it | ✅ clean; the toolkit already depends on `pmcp` unconditionally (`Cargo.toml:23`), and `pmcp-code-mode` pins `pmcp = ">=2.2.0"` (`crates/pmcp-code-mode/Cargo.toml:29`). D-09's "export `validate_path_placeholder`" is satisfied by re-exporting it from `pmcp-code-mode`. |
| B. Duplicate the rule in both crates | ❌ two copies of a security rule drift; this repo already has a documented three-way-drift incident (`output_validation.rs:667-691`) |
| C. Add `dep:pmcp-code-mode` (without `js-runtime`) to the toolkit's `http` feature | ⚠ `code-mode = ["dep:pmcp-code-mode", "pmcp-code-mode/sql-code-mode"]` is the bare edge and does **not** forward `js-runtime` (`Cargo.toml:128-137`), so this is *technically* SWC-free — but it widens the curated build's dependency graph and the purity gate bans `pmcp-code-mode` in some served trees (`Makefile:1642` `BAN='umya\|calamine\|quick-xml\|swc_\|pmcp-code-mode'`) |

**Recommend A.** It is the only option that keeps one copy of the rule and leaves the purity gate
untouched. Note it means the primitive ships in `pmcp` 2.21.0 and the re-export in
`pmcp-code-mode` 0.6.0 — consistent with D-13's version set.

## Finding 7 — The ~11-crate release (D-13 / D-14), verified pin by pin

### 7a. Actual in-tree versions (all read from the manifests this session)

| Crate | Version today | D-13 target | Verified |
|---|---|---|---|
| `pmcp` | **2.20.4** | 2.21.0 | `Cargo.toml` `[package].version` |
| `pmcp-code-mode` | **0.5.4** | 0.6.0 | `crates/pmcp-code-mode/Cargo.toml` |
| `pmcp-code-mode-derive` | **0.3.0** | (bump) | `crates/pmcp-code-mode-derive/Cargo.toml` |
| `pmcp-server-toolkit` | **0.1.3** | 0.2.0 | `crates/pmcp-server-toolkit/Cargo.toml:3` |
| `pmcp-toolkit-postgres` | 0.1.0 | bump | |
| `pmcp-toolkit-mysql` | 0.1.1 | bump | |
| `pmcp-toolkit-athena` | 0.1.0 | bump | |
| `pmcp-sql-server` | 0.1.1 | bump | |
| `pmcp-openapi-server` | 0.1.2 | bump | |
| `pmcp-workbook-server` | 0.1.1 | bump | |
| `pmcp-workbook-compiler` | 0.1.3 | bump | |

`[VERIFIED: per-manifest [package] version extraction across all tracked Cargo.toml]`
**All eleven of D-13's crates and versions are CONFIRMED.**

*(Incidental correction for the release ledger, not this phase's scope: CLAUDE.md § Release
item 15a says `cargo-pmcp` is at 0.23.0. It is at **0.24.3** `[VERIFIED: cargo-pmcp/Cargo.toml]`.)*

### 7b. Every `pmcp-server-toolkit` pin — D-13's "seven" is EXACT

| Manifest:line | Requirement | Admits 0.2.0? |
|---|---|---|
| `crates/pmcp-toolkit-postgres/Cargo.toml:28` | `pmcp-server-toolkit = { version = "0.1.0", path = "../pmcp-server-toolkit" }` | ❌ |
| `crates/pmcp-toolkit-mysql/Cargo.toml:28` | `version = "0.1.0"` | ❌ |
| `crates/pmcp-toolkit-athena/Cargo.toml:28` | `version = "0.1.0"` | ❌ |
| `crates/pmcp-sql-server/Cargo.toml:33` | `version = "0.1.0", features = ["code-mode", "sqlite"]` | ❌ |
| `crates/pmcp-workbook-server/Cargo.toml:43` | `version = "0.1.0", default-features = false, features = ["workbook", "http"]` | ❌ |
| `crates/pmcp-openapi-server/Cargo.toml:47` | `version = "0.1.2", features = ["openapi-code-mode"]` | ❌ |
| **`crates/pmcp-workbook-compiler/Cargo.toml:107`** | `version = "0.1.0", default-features = false, features = ["workbook"]` | ❌ |
| `crates/pmcp-server-toolkit/fuzz/Cargo.toml:13` | `pmcp-server-toolkit = { path = ".." }` — **path-only, no version** | n/a — no repin |

`[VERIFIED: grep -rn "pmcp-server-toolkit" --include='Cargo.toml' .]`
**D-13's seven is right, review note D's six was wrong, and `pmcp-workbook-compiler` is indeed the
one review note D missed.** Its publish step is `[VERIFIED: .github/workflows/release.yml:396]`
`- name: Publish pmcp-workbook-compiler`, and `pmcp-server-toolkit`'s is
`[VERIFIED: .github/workflows/release.yml:291]` — both D-13 line numbers confirmed.

There is also a feature-forward to check, not just a version pin:
`[VERIFIED: crates/pmcp-openapi-server/Cargo.toml:29]` — `openapi-code-mode = ["pmcp-server-toolkit/openapi-code-mode"]`
and `[VERIFIED: crates/pmcp-sql-server/Cargo.toml:51]` — `sqlite = ["pmcp-server-toolkit/sqlite"]`.
**If `input-validation` is to reach these binaries, each needs a forward or the toolkit must carry
it in `default`** — see Finding 8b.

### 7c. `pmcp-code-mode` pins

| Manifest:line | Requirement | Admits 0.6.0? |
|---|---|---|
| `Cargo.toml:263` | `pmcp-code-mode = { version = "0.5.3", path = "crates/pmcp-code-mode" }` — root **`[dev-dependencies]`**, for example `s41` | ❌ |
| `crates/pmcp-code-mode-derive/Cargo.toml:27` | `pmcp-code-mode = { version = "0.5.0", path = "../pmcp-code-mode" }` — **`[dev-dependencies]`** | ❌ |
| `crates/pmcp-server-toolkit/Cargo.toml:24` | `pmcp-code-mode = { version = "0.5.3", path = "../pmcp-code-mode", default-features = false, optional = true }` | ❌ |
| `fuzz/Cargo.toml:62-63` | `[dependencies.pmcp-code-mode] path = "../crates/pmcp-code-mode"` — **path-only** | n/a |

`[VERIFIED: grep -rn "pmcp-code-mode" --include='Cargo.toml' .]`

> ### 🔎 The pin D-13 missed: root `Cargo.toml:264`
>
> `[VERIFIED: Cargo.toml:264]`
> ```toml
> pmcp-code-mode-derive = { version = "0.3.0", path = "crates/pmcp-code-mode-derive" }  # For code mode example (s41)
> ```
>
> D-13 names `Cargo.toml:263` (`pmcp-code-mode = "0.5.3"`) but not `:264`. Whether `:264` must move
> depends on how `pmcp-code-mode-derive` is bumped:
>
> - **If `0.3.0 → 0.3.1` (patch):** `^0.3.0` admits `0.3.1`, the CLAUDE.md caret exception applies,
>   and `:264` stays. A patch is defensible — the crate's only change is a **`[dev-dependencies]`**
>   requirement (`:27` is inside `[dev-dependencies]`, `[VERIFIED: crates/pmcp-code-mode-derive/Cargo.toml:25-27]`),
>   and `grep -rn "HttpExecutor|execute_request" crates/pmcp-code-mode-derive/src/` returns **zero
>   matches** — the derive does not emit `HttpExecutor` impls `[VERIFIED]`.
> - **If `0.3.0 → 0.4.0` (minor):** `^0.3.0` does **not** admit `0.4.0`, so `:264` MUST move, and
>   the release grows to 12 versions.
>
> Because the dep is dev-only and path-carrying, Cargo strips it at publish time and the published
> `pmcp-code-mode-derive` manifest is unaffected either way — **so a strong case exists that
> `pmcp-code-mode-derive` needs no bump at all**, shrinking the release to ten. D-13 asserts it must
> bump ("`^0.5` does not admit 0.6.0"), which is true of the *workspace* build but not of the
> *published* artifact. **Flag for the planner (Open Question Q4).** Whichever way it goes,
> `scripts/check-release-coverage.sh` does not adjudicate it — its order assertion is scoped to
> `pmcp-package` and its four consumers only `[VERIFIED: scripts/check-release-coverage.sh:247-314,
> and the header at :26 — "Plus a bounded ORDER assertion (see the D-10 region below)"]`.

### 7d. `pmcp` 2.20.4 → 2.21.0 needs no downstream repin — CONFIRMED

Every `pmcp` requirement in the tree, read this session
`[VERIFIED: grep -rn "^pmcp = " --include='Cargo.toml' .]`:
`2.8.0`, `2.8.1` (×3), `2.9.0` (×4), `2.17.0` (×4), `2.19.0`, `2.20.3`, `2.7.0` (×2),
`>=2.2.0` (×2), `>=1.20.0`, plus a dozen path-only entries. **All carets on the 2.x line; `^2.20.3`
is the tightest and admits 2.21.0.** D-13's claim holds exactly.

The one that must move for a *functional* reason, not a semver one:
`[VERIFIED: crates/pmcp-server-toolkit/Cargo.toml:23]`
```toml
pmcp = { version = "2.9.0", path = "../..", default-features = false }
```
→ `"2.21"`, because the toolkit must *reach* `validate_input`. Note `default-features = false`:
`pmcp`'s `default = ["logging", "v1-compat"]` is off in the toolkit, so
`pmcp/schema-validation` must be forwarded **explicitly** by `input-validation`.

### 7e. `release.yml` order — the whole ledger, with line numbers

`[VERIFIED: grep -n "name: Publish" .github/workflows/release.yml]`

```
136 pmcp-widget-utils      154 pmcp-macros-support     172 pmcp-macros
190 pmcp-code-mode         208 pmcp-code-mode-derive   226 pmcp (core SDK)
255 pmcp-workbook-runtime  273 pmcp-workbook-dialect   291 pmcp-server-toolkit
309 pmcp-toolkit-postgres  324 pmcp-toolkit-mysql      339 pmcp-toolkit-athena
357 pmcp-sql-server        372 pmcp-openapi-server     396 pmcp-workbook-compiler
411 pmcp-workbook-server   429 mcp-tester              444 mcp-preview
492 pmcp-package           519 pmcp-cfn-renderer       537 pmcp-agent
552 pmcp-team-servers      573 cargo-pmcp              591 pmcp-server
```

Every one of D-13's eleven crates has a step, all of them in a dependency-correct order for this
phase's change set: `pmcp-code-mode` (190) before `pmcp` (226) before `pmcp-server-toolkit` (291)
before its seven consumers (309–411). **D-14's "one tag, existing order" is sound as-is; no
`release.yml` edit is needed.**

## Finding 8 — Feature-split mechanics (D-04 / SC-1)

### 8a. The split is unusually clean — all 18 gates live in one file

`[VERIFIED: Cargo.toml:327]` — `validation = ["dep:jsonschema", "dep:garde"]`
`[VERIFIED: Cargo.toml:280]` — `full = ["websocket", "http", "streamable-http", "sse", "validation", "resource-watcher", "rayon", "schema-generation", "jwt-auth", "composition", "mcp-apps", "http-client", "logging", "macros", "testing", "v1-compat"]`
`[VERIFIED: Cargo.toml:293]` — `full-v2 = [… "validation" …]` (same list minus `v1-compat`)

`[VERIFIED: grep -rn 'feature = "validation"' src/]` → **18 matches, every one of them in
`src/server/output_validation.rs`.** Zero in `core.rs`, zero in `mod.rs`, zero anywhere else.
So D-04's mechanical work is: rename 18 attributes in one file, and split the feature line.

**Proposed manifest edit:**
```toml
# Cargo.toml
schema-validation = ["dep:jsonschema"]
validation = ["schema-validation", "dep:garde"]   # unchanged meaning for every existing consumer
```
`full` / `full-v2` keep naming `validation` and are unaffected.

### 8b. Every feature-graph edge a split touches

| Edge | Today | After D-04 |
|---|---|---|
| `pmcp/full` → `validation` → `jsonschema` + `garde` | `Cargo.toml:280` | unchanged (transitively via `schema-validation`) |
| `pmcp/full-v2` → `validation` | `Cargo.toml:293` | unchanged |
| `fuzz/Cargo.toml:60` `features = ["oauth", "streamable-http", "fuzzing", "validation", "skills"]` | `[VERIFIED]` | unchanged — `validation` still forwards `jsonschema` |
| `tests/property_tests.rs:996` `#[cfg(all(test, feature = "fuzzing", feature = "validation"))]` | `[VERIFIED]` | unchanged |
| `tests/v2_schema_tripwires.rs:36` — "`cargo metadata --features validation` → the RESOLVED graph's …" and `:804` "`cargo metadata --features validation` resolved ZERO nodes" | `[VERIFIED]` | **still resolves**, because `validation` ⊇ `schema-validation`. ⚠ But the tripwire then covers *only* the superset path — a consumer enabling **only** `schema-validation` would get the same `jsonschema` resolver-feature exposure with no tripwire. **Add a `--features schema-validation` arm** to keep the RESOLVER_FEATURES fence (`:98-110`, `const JSONSCHEMA: &str = "jsonschema";` / `RESOLVER_FEATURES = ["resolve-http", "resolve-file", "resolve-async", "tls-aws-lc-rs", …]`) honest. |
| `Makefile:1546` `--features composition,http,…,validation,websocket,v1-compat` (the feature-powerset / purity leg) | `[VERIFIED]` | consider adding `schema-validation` so the new arm is compiled somewhere |
| toolkit `input-validation = ["dep:jsonschema"]` `[VERIFIED: crates/pmcp-server-toolkit/Cargo.toml:102]` | pulls the toolkit's OWN `jsonschema` | → `input-validation = ["dep:jsonschema", "pmcp/schema-validation"]` (or drop `dep:jsonschema` entirely if the toolkit stops compiling schemas itself, which D-01 implies it should) |

> ### ⚠ 8c. `input-validation` is **NOT** in the toolkit's `default`
>
> The CR says D1 ships "behind the toolkit's existing `input-validation` feature …, **forwarded from
> `default`**". It is not forwarded today.
>
> `[VERIFIED: crates/pmcp-server-toolkit/Cargo.toml:93-102]`
> ```toml
> [features]
> default = ["code-mode"]
> # Why: the toolkit's code-mode wiring needs `ValidationPipeline::validate_sql_query` …
> code-mode = ["dep:pmcp-code-mode", "pmcp-code-mode/sql-code-mode"]
> aws = ["dep:aws-config", "dep:aws-sdk-secretsmanager", "dep:aws-sdk-ssm", "dep:tokio"]
> input-validation = ["dep:jsonschema"]
> ```
>
> **Adding `input-validation` to `default` is a plan task, not a given**, and it is the mechanism
> by which "the defaults ship on" (the whole CR compatibility story) actually happens. Two knock-on
> effects the plan must handle:
>
> 1. **Every consumer that sets `default-features = false` loses it.** Measured: `pmcp-workbook-server`
>    (`:43`) and `pmcp-workbook-compiler` (`:107`) both set `default-features = false` on the toolkit.
>    They would need `input-validation` added to their feature lists explicitly — or the phase accepts
>    that workbook servers are unenforced, which contradicts D1's "every config-declared tool".
> 2. **`pmcp-sql-server` (`:33`) and `pmcp-openapi-server` (`:47`) do NOT set `default-features = false`**,
>    so they inherit it automatically. Good.

### 8d. ⚠ The severance-proof trap this repo has already been bitten by

CONTEXT.md and this project's memory both flag it, and the Makefile records the measurement:

`[VERIFIED: Makefile:576-583]`
```make
# `--features http` is REQUIRED, not decorative. The toolkit's `default` is
# `["code-mode"]`, and `tests/base_url_expansion.rs` is `#![cfg(feature =
# "http")]` — MEASURED: a default `cargo test -p pmcp-server-toolkit` compiles
# it to `running 0 tests` and exits 0, so the whole file asserts nothing while
# looking green.
```

**Rules for any D-04 severance claim in a plan:**
1. Prove the *negative* with a **dev-dependency-free** build: `cargo build -p pmcp --no-default-features --features schema-validation` and assert `garde` is absent from `cargo tree`. `cargo test --all-features` re-enables everything via dev-dep unification and proves nothing.
2. Prove the *positive* with a **nonzero test count**. `make test-server-toolkit` already implements exactly this discipline and is the template to extend:
   `[VERIFIED: Makefile:592-596]`
   ```make
   ran=$$(echo "$$out" | awk '/^test result:/ { total += $$4 } END { print total+0 }'); \
   if [ "$$ran" -eq 0 ]; then \
       echo "$(RED)✗ pmcp-server-toolkit reported 0 tests — the gate is not reaching this crate$(NC)"; \
       exit 1; \
   fi; \
   REQUIRED_TEST_BINARIES="env_ref_grammar_parity base_url_expansion"; \
   ```
   **Add the new acceptance binary to `REQUIRED_TEST_BINARIES`, and add `input-validation` to that
   leg's `--features`** (`[VERIFIED: Makefile:587]` — today it is `--features http` only, so a new
   `#![cfg(feature = "input-validation")]` test file would compile to `running 0 tests` and the gate
   would pass green on zero coverage).
3. `make unused-deps` **cannot** catch a dead feature — `[VERIFIED: Makefile:236-241]` the
   `cargo machete` call is commented out and the target prints
   `⚠ cargo machete not installed - skipping`. This is precisely why `input-validation` survived
   with zero references. Do not rely on it.

### 8e. The jsonschema resolver-feature invariant must survive the split

`[VERIFIED: Cargo.toml:216-220]`
```
# `optional` and `default-features = false` are LOAD-BEARING and must survive verbatim:
# jsonschema's defaults are ["resolve-http", "resolve-file", "tls-aws-lc-rs"], which pull
# reqwest + rustls, break the wasm build, and turn an external `$ref` from a hard error
# into a live network fetch (SEP-2106).
jsonschema = { version = "0.49", optional = true, default-features = false }
```
The toolkit's pin carries the same two keys `[VERIFIED: crates/pmcp-server-toolkit/Cargo.toml:54]`.
**A new feature must never add a jsonschema feature**, and the `v2_schema_tripwires.rs` fence is
what enforces it — see 8b for the arm it needs.

## Finding 9 — ALWAYS coverage (SC-8): where each leg lives, and two that measure almost nothing

### 9a. Fuzz

**Five fuzz crates exist**, each its own one-crate workspace
`[VERIFIED: crates/pmcp-server-toolkit/fuzz/Cargo.toml:18-21 — "# Why: keeps the fuzz crate isolated
from the main workspace per Phase 77 PATTERNS §17 — fuzz/* declares its own one-crate workspace"]`:

| Crate | Targets |
|---|---|
| `fuzz/` (root, `pmcp-fuzz`) | **27 targets**, incl. `fuzz_schema_draft_pin.rs` (48.5K — the closest analogue for a new schema fuzzer), `fuzz_javascript_code_mode.rs`, `fuzz_token_code_mode.rs` |
| `crates/pmcp-server-toolkit/fuzz/` | **1 target** — `pmcp_server_toolkit_config_parser.rs` |
| `cargo-pmcp/fuzz/`, `crates/pmcp-team-servers/fuzz/`, `crates/pmcp-workbook-compiler/fuzz/` | 1 each |

**Registration** is a `[[bin]]` stanza per target
`[VERIFIED: crates/pmcp-server-toolkit/fuzz/Cargo.toml:23-28]`:
```toml
[[bin]]
name = "pmcp_server_toolkit_config_parser"
path = "fuzz_targets/pmcp_server_toolkit_config_parser.rs"
doc = false
test = false
bench = false
```
Root fuzz features `[VERIFIED: fuzz/Cargo.toml:60]`:
`features = ["oauth", "streamable-http", "fuzzing", "validation", "skills"]`.

**The `fuzzing`-gated seam pattern** is the in-tree convention for reaching crate-private internals
from a fuzz target `[VERIFIED: src/server/output_validation.rs:663-690]` — `pub mod fuzz_support`
under `#[cfg(all(feature = "fuzzing", feature = "validation"))]`, which "adds NOTHING to the shipped
public API". If a new fuzz target needs to drive `validate_input`, note `validate_input` is
*already* public, so no seam is needed.

> ⚠ **`make test-fuzz` cannot fail the gate.** `[VERIFIED: Makefile:803-814]`
> ```make
> cd fuzz && $(CARGO) fuzz list | while read target; do \
>     timeout 30s $(CARGO) fuzz run $$target || echo "$(YELLOW)Fuzz target $$target completed$(NC)"; \
> done;
> ```
> A crash is swallowed by `|| echo`, and only the **root** `fuzz/` directory is visited — the
> toolkit's own fuzz crate is never run by this target. A new toolkit fuzz target is therefore
> *documentation* unless the plan also gives it a runner. Budget a task for that, or place the new
> target in root `fuzz/` where at least the 30s smoke runs.

**Recommended new targets:** (1) `fuzz_input_schema_enforcement` in root `fuzz/` — arbitrary
`(schema, arguments)` pairs, asserting the rendered refusal **never contains a byte of the input**
(the direct SC-7 invariant, and the cheapest guard against the Finding-1b class); (2) a
`(pattern, input)` target to look for the ReDoS residual (Finding 1j).

### 9b. Property

`proptest` is the framework; `quickcheck` is not used.
`[VERIFIED: crates/pmcp-server-toolkit/Cargo.toml [dev-dependencies] — "proptest = \"1.7\""]`
Toolkit property files already exist: `tests/http_connector_props.rs`, `tests/tool_synthesis_props.rs`,
`tests/workbook_tool_name_prop.rs`, `tests/env_expansion.rs`, `tests/http_auth.rs`
`[VERIFIED: grep -rln "proptest!"]`. Root has 29 files using `proptest!`.

> ⚠ **`make test-property` selects almost nothing.** `[VERIFIED: Makefile:797-800]`
> ```make
> PROPTEST_CASES=1000 RUST_LOG=$(RUST_LOG) $(CARGO) test --features "full" -- --ignored property_
> ```
> `--ignored` runs **only** `#[ignore]`d tests. Measured: `tests/property_tests.rs` contains **19**
> `property_*` / `prop_*` functions and **zero** `#[ignore]` attributes
> `[VERIFIED: grep -n "#\[ignore" tests/property_tests.rs → 0 matches]`. The only tests this leg
> selects are the **two** in `tests/log_emitter.rs`, which carry the marker explicitly:
> `[VERIFIED: tests/log_emitter.rs:169 and :2224]`
> ```rust
> #[ignore = "property arm — selected by `make test-property` (--ignored property_)"]
> ```
> and whose own rustdoc says why (`:165-167`): *"the CLAUDE.md ALWAYS-property requirement is
> discharged by a test the `validate-always` target actually SELECTS, rather than by an always-run
> test that target never runs."*
>
> **Consequence for SC-8:** a new property test named `property_*` without `#[ignore]` runs under
> `make test-integration` (`cargo test --test '*' --features "full"`) but is **invisible to the
> ALWAYS-property leg**. To discharge SC-8 honestly, copy `log_emitter.rs`'s pattern verbatim:
> `#[test]` + `#[ignore = "property arm — selected by \`make test-property\` (--ignored property_)"]`
> + a `property_`-prefixed name. Note also that `--features "full"` is a **root-package** selector,
> so a *toolkit* property test is never reached by this leg at all — it lands in
> `make test-server-toolkit` instead.

**Recommended properties:** (1) *no input byte ever appears in a refusal* over arbitrary
`(schema, args)`; (2) *refusal ⇒ zero upstream requests* over arbitrary placeholder values
(pairs with the acceptance harness); (3) *`validate_path_placeholder` is closed under
percent-encoding* — if `decode(v)` is refused then `v` is refused.

### 9c. Unit + example

- **Unit:** `make test-unit` = `cargo test --lib --features "full"` — root package only
  `[VERIFIED: Makefile:250-253]`. Core unit tests for `schema_validation` land here; toolkit unit
  tests land in `make test-server-toolkit`.
- **Example numbering.** The `s`-series is the SDK-example series and the highest slot taken is
  **`s56_workflow_skill_projection.rs`** `[VERIFIED: ls examples/ | grep -E '^s[0-9]' | sort -V | tail]`.
  **The next slot is `s57_`.** Declaration shape `[VERIFIED: Cargo.toml:714-718]`:
  ```toml
  # taken by `s45_tool_as_task_lifecycle` below, hence `s56`.
  [[example]]
  name = "s56_workflow_skill_projection"
  path = "examples/s56_workflow_skill_projection.rs"
  required-features = ["skills", "full"]
  ```
  Recommended: **`examples/s57_typed_tool_garde_validation.rs`** with
  `required-features = ["validation", "schema-generation", "full"]` — it demonstrates E3 (the core
  half) and is the natural replacement for what `s19_wasm_typed_tools.rs` teaches (see Finding 5c
  option (b), which would let one example discharge both the SC-8 example requirement *and* the
  deprecation-warning problem).
  For the **toolkit** half, the existing convention is a crate-local `[[example]]` with
  `required-features` naming toolkit features only
  `[VERIFIED: crates/pmcp-server-toolkit/Cargo.toml:150-152]`:
  ```toml
  [[example]]
  name = "e01_toolkit_minimal"
  required-features = ["code-mode"]
  ```
  So a second example, e.g. `e05_input_validation` with `required-features = ["input-validation", "http"]`.
- `make test-examples` runs `./scripts/run-example-builds.sh` and **builds every example, failing
  when one does not** `[VERIFIED: Makefile:879-881 — "# Phase 119 (D-13/D-14) — BUILD every example,
  and FAIL when one does not"]`. It builds; it does not run. `cargo run --example` is the CLAUDE.md
  requirement and is a manual/UAT step.

## Finding 10 — The toolkit surfaces D1/D2/SC-6 touch

### 10a. The three false claims, located verbatim (SC-6)

**Claim 1 — T-83-05-02**, module doc `[VERIFIED: crates/pmcp-server-toolkit/src/tools.rs:11-17]`:
```
//! - **JSON Schema object envelope.** Every synthesized [`ToolInfo`] carries an
//!   `input_schema` with `"type": "object"`, an explicit `properties` map, a
//!   `required` array, and `"additionalProperties": false`. Unknown argument
//!   keys are rejected by pmcp's request-validation path at `tools/call` time —
//!   defence-in-depth against arg-injection (threat T-83-05-02).
```

**Claim 2 — T-90-03-01**, `HttpToolHandler::handle` `[VERIFIED: crates/pmcp-server-toolkit/src/tools.rs:556-558]`:
```rust
        // T-90-03-01: arg injection is bounded by the object-envelope schema
        // (additionalProperties:false) enforced upstream; path substitution in the
        // connector touches only declared `{params}`.
```

**Claim 3 — T-90-05-03**, `ScriptToolHandler.tool_info` field doc `[VERIFIED: crates/pmcp-server-toolkit/src/tools.rs:612-615]`:
```rust
    /// The synthesized `ToolInfo` (object-envelope schema from
    /// `[[tools.parameters]]`, `additionalProperties:false`) — `args` are
    /// schema-validated against this BEFORE the script runs (T-90-05-03).
    tool_info: ToolInfo,
```

The test carrying T-90-05-03 asserts binding, not validation
`[VERIFIED: crates/pmcp-server-toolkit/tests/script_tool.rs:184-188]`:
```rust
/// (c) The client args from `[[tools.parameters]]` are bound to `args` inside
/// the script (T-90-05-03): a script that reads `args.maxLines` after a backend
/// call observes the exact value the caller supplied.
#[tokio::test]
async fn script_tool_args_max_lines_binding_is_honored() {
```

Note that **after this phase, claims 1–3 become TRUE** for config-driven tools — so SC-6's edit is
not "delete the claim" but "restate it accurately and point at the enforcing code". Claim 2's
"enforced upstream" specifically becomes "enforced by `pmcp::server::schema_validation::validate_input`,
called at `<line>`", and claim 1's "by pmcp's request-validation path at `tools/call` time" becomes
"by the toolkit's synthesizer before the backend call" (the core dispatch wiring is deferred).
SC-6's second half — "no remaining comment in the toolkit claims a mitigation the code does not
implement" — needs a deliberate sweep task; three known extra candidates are Finding 4c's
"Deserialize and validate" (core, not toolkit) and the two `ARCHITECTURE.md` bullets in Finding 5d.

### 10b. `additionalProperties: false` is already emitted, with two asserting tests

`[VERIFIED: crates/pmcp-server-toolkit/src/tools.rs:198-208]`
```rust
fn build_input_schema(params: &[ParamDecl]) -> Value {
    let mut props = Map::new();
    let mut required = Vec::new();
    for p in params {
        props.insert(p.name.clone(), build_param_property(p));
        if p.required {
            required.push(Value::String(p.name.clone()));
        }
    }
    json!({
        "type": "object",
        "properties": props,
        "required": required,
        "additionalProperties": false,
    })
}
```
D1 **honours** this, never re-adds it. The `required` array is built from `p.required`, which is what
makes the CR's two `arguments`-missing rows separable (Finding 2c).

### 10c. `build_param_property` — the D2 extension point

`[VERIFIED: crates/pmcp-server-toolkit/src/tools.rs:216-243]`
```rust
fn build_param_property(p: &ParamDecl) -> Value {
    let ty = p.param_type.as_deref().unwrap_or("string");
    let mut prop = json!({ "type": ty });
    if let Some(desc) = &p.description { prop["description"] = Value::String(desc.clone()); }
    if let Some(min) = p.minimum      { prop["minimum"]   = json!(min); }
    if let Some(max) = p.maximum      { prop["maximum"]   = json!(max); }
    if let Some(max_len) = p.max_length { prop["maxLength"] = json!(max_len); }
    if let Some(default) = &p.default { if let Ok(v) = serde_json::to_value(default) { prop["default"] = v; } }
    if let Some(enum_vals) = &p.enum_values { if let Ok(v) = serde_json::to_value(enum_vals) { prop["enum"] = v; } }
    prop
}
```
**Emits 5 constraint keywords; D2 adds 5 more** (`pattern`, `minLength`, `format`, `items`,
`maxItems`). The function is a straight-line `if let` chain — adding five more keeps it well under
cog 25, but the plan should still check with `pmat` rather than assume (the chain is already 7 arms).

### 10d. `ParamDecl` — nine fields, `deny_unknown_fields`, and D-15's real scope

`[VERIFIED: crates/pmcp-server-toolkit/src/config.rs:913-949]`
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct ParamDecl {
    #[serde(default)] pub name: String,
    #[serde(default, rename = "type")] pub param_type: Option<String>,
    #[serde(default)] pub description: Option<String>,
    #[serde(default)] pub required: bool,
    #[serde(default)] pub default: Option<toml::Value>,
    #[serde(default)] pub max_length: Option<u64>,
    #[serde(default)] pub minimum: Option<f64>,
    #[serde(default)] pub maximum: Option<f64>,
    #[serde(default, rename = "enum")] pub enum_values: Option<Vec<toml::Value>>,
}
```

> ### 🔎 D-15's forward-incompatibility is WIDER than `ParamDecl`
>
> `ServerSection` carries the same attribute `[VERIFIED: crates/pmcp-server-toolkit/src/config.rs:328-333]`:
> ```rust
> /// `[server]` section — identity and version metadata.
> #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
> #[serde(deny_unknown_fields)]
> pub struct ServerSection {
>     #[serde(default)] pub id: Option<String>,
>     #[serde(default)] pub name: String,
> ```
> and so does `ServerConfig` itself (`:100-102`). D-07's `[server.validation]` block therefore adds a
> field to `ServerSection`, and a config carrying `[server.validation]` **fails to parse on 0.1.3**
> exactly as a `pattern` does. **D-15's CHANGELOG/rollout note must cover `[server.validation]`, not
> just `ParamDecl`.**

### 10e. Synthesizer entry points (E2's attach surface)

`[VERIFIED: grep -n "fn synthesize" crates/pmcp-server-toolkit/src/tools.rs]`

| Fn | Line | Kind |
|---|---|---|
| `pub fn synthesize_from_config` | `:85` | public |
| `pub fn synthesize_from_config_with_connector` | `:122` | public |
| `fn synthesize_inner` | `:136` | private shared core |
| `pub fn synthesize_from_config_with_http_connector` | `:387` | public |
| `pub fn synthesize_from_config_with_http_connector_and_scripts` | `:431` | public |
| `fn synthesize_http_inner` | `:453` | private shared core |

**Two private cores, four public wrappers.** D1's validation wrapper and E2's `ArgumentValidator`
lookup both belong in `synthesize_inner` / `synthesize_http_inner`, so all four public entry points
inherit them and none can be forgotten. E2's registration surface (a name→validator map) needs a
fifth constructor or a builder-side registry; the four-wrapper pattern suggests the latter, to avoid
a sixth `synthesize_from_config_with_*` name.

## Finding 11 — D-07 / SC-3 have two structural gaps

### 11a. `ServerConfig::validate` has no warning channel

`[VERIFIED: crates/pmcp-server-toolkit/src/config.rs:246-252]`
```rust
    pub fn validate(&self) -> std::result::Result<(), ConfigValidationError> {
        if self.server.name.trim().is_empty() {
            return Err(ConfigValidationError::EmptyServerName);
        }
        if self.server.version.trim().is_empty() {
            return Err(ConfigValidationError::EmptyServerVersion);
        }
```
Its own rustdoc: *"Returns a [`ConfigValidationError`] variant identifying the **first** rule
violated. Iteration order matches struct field order."* `[VERIFIED: config.rs:242-245]`

It is **first-error-wins and has no `Vec<Warning>`**. D-07's "`ServerConfig::validate` … warns on an
uncapped body-position string" cannot be expressed in this signature. Options: a new
`pub fn validate_with_warnings(&self) -> (Vec<ConfigWarning>, Result<(), ConfigValidationError>)`,
or `validate` gains a `&mut Vec<_>` out-param (ugly), or a separate
`pub fn lint(&self) -> Vec<ConfigWarning>`. **Recommend the separate `lint`** — additive, no change
to `validate`'s 16 existing tests (`config.rs:1095-1371`), and it gives `cargo-pmcp` something to
call that is not entangled with boot-time validation. The `[server.validation] strict` flag then
promotes `lint` findings into `validate` errors.

### 11b. ⚠ `cargo-pmcp` cannot see a toolkit config at all

`[VERIFIED: grep -n "pmcp-server-toolkit" cargo-pmcp/Cargo.toml]` → **no match.** `cargo-pmcp` has
no dependency on `pmcp-server-toolkit`.

`[VERIFIED: grep -rn "ServerConfig|config.toml" cargo-pmcp/src/commands/validate.rs]` → **no match.**

What `validate deploy` actually does `[VERIFIED: cargo-pmcp/src/commands/validate.rs:29-42, 584-607]`:
```rust
    /// Validate `.pmcp/deploy.toml` — focuses on IAM footgun detection.
    ///
    /// Hard-errors on wildcard-`Allow`, malformed actions, empty resource
    /// lists, bad effects, and sugar-keyword typos. …
    Deploy { … }
…
pub fn validate_deploy(server: Option<String>, verbose: bool) -> Result<()> {
    …
    let config = crate::deployment::config::DeployConfig::load(&project_root)
        .context("failed to load .pmcp/deploy.toml")?;
    let warnings = crate::deployment::iam::validate(&config.iam)
```
It reads **`.pmcp/deploy.toml`**, an IAM file — an entirely different document from the toolkit's
server `config.toml`. `ValidateCommand` has exactly two variants, `Workflows` and `Deploy`
`[VERIFIED: validate.rs:13-43]`.

**Four routes, for the planner to choose (Open Question Q5):**

| Route | Cost |
|---|---|
| **A. New `cargo-pmcp → pmcp-server-toolkit` dependency**, reuse `ServerConfig::lint` | Order is safe (`pmcp-server-toolkit` at `release.yml:291`, `cargo-pmcp` at `:573`), but adds an **eighth** toolkit pin to bump on every future toolkit release, and `cargo-pmcp` is the crate CLAUDE.md item 15a already calls the most-pinned in the tree |
| **B. A new `cargo pmcp validate config` subcommand** doing the same, so the IAM command keeps its scope | Same dependency cost, cleaner semantics — SC-3 says "`cargo pmcp validate deploy`" verbatim, so this is a deviation to record |
| **C. Re-implement the lint in `cargo-pmcp`** against a minimal local TOML shape | No dependency, but **two copies of a security rule** — the drift class this repo has been bitten by |
| **D. Descope the `cargo-pmcp` half**, ship only `ServerConfig::lint` + a boot-time `tracing::warn!` | SC-3 not fully met; needs a ROADMAP amendment |

**Recommend B.** `cargo-pmcp` already parses toolkit-shaped config in five other places — `deployment/builder.rs`,
`templates/{sql,openapi,workbook}_server.rs`, `commands/deploy/mod.rs`, `commands/package/save.rs`
`[VERIFIED: grep -rln "server-toolkit|ParamDecl|ToolDecl" cargo-pmcp/src/]` — so the domain is not
foreign to it, and a dedicated subcommand keeps the IAM command's contract ("a failing
`validate deploy` guarantees a failing `deploy`", `validate.rs:36-38`) intact.

## Finding 12 — The acceptance harness

### 12a. What already exists

`wiremock = "0.6"` is a toolkit dev-dependency with a recorded rationale
`[VERIFIED: crates/pmcp-server-toolkit/Cargo.toml [dev-dependencies]]`:
```toml
# Why: Phase 90 Plans 01/03/04/05 drive the HttpConnector against a mock REST
# backend (GET/POST + auth-header assertions) without a live server. Dev-only.
wiremock = "0.6"
```
Also present: `proptest = "1.7"`, `trybuild = "1"`, `tempfile = "3"`,
`mcp-tester = { version = "0.8.0", path = "../mcp-tester" }`,
`tokio = { version = "1", features = ["macros", "rt-multi-thread"] }`.

**24 test binaries** in `crates/pmcp-server-toolkit/tests/`, four of them wiremock-driven:
`http_auth.rs`, `http_executor.rs`, `script_tool.rs`, `script_tool_engine_parity.rs`
`[VERIFIED: ls + grep -rln wiremock]`. `crates/pmcp-openapi-server/tests/` adds six more.

### 12b. Both "zero upstream requests" idioms are already in use

**Idiom 1 — `.expect(0)`, verified on `MockServer` drop:**
`[VERIFIED: crates/pmcp-openapi-server/tests/oauth_passthrough_e2e.rs:117, 130, 167, 172]`
```rust
        .expect(0)
…
    // expect(0) on drop proves the backend was never contacted.
```
**Idiom 2 — count `received_requests()`:**
`[VERIFIED: crates/pmcp-server-toolkit/tests/script_tool_engine_parity.rs:85-87]`
```rust
        .received_requests()
        .await
        .expect("wiremock records requests")
```
also at `crates/pmcp-openapi-server/tests/{contoso_m365_parity.rs:453, parity_replay.rs:274, roundtrip_e2e.rs:1078}`.

**Prefer idiom 2 for the acceptance matrix** — `received_requests()` returns the full recorded list,
so one assertion can prove *zero requests* **and** that the refusal message contains neither the
value nor the path, in the same test body. `.expect(0)` only fires on drop, which makes a failure
harder to attribute to a row.

### 12c. The template for the five CR-01 probes

`[VERIFIED: crates/pmcp-server-toolkit/tests/http_executor.rs:16, 27-44]`
```rust
#![cfg(feature = "openapi-code-mode")]
…
#[tokio::test]
async fn http_executor_get_substitutes_path_param_and_returns_json() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/7"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": 7, "name": "Ada"})))
        .mount(&server)
        .await;

    let auth = create_auth_provider(&AuthConfig::None).expect("noauth");
    let exec = HttpCodeExecutor::new(reqwest::Client::new(), server.uri(), auth);

    let result = exec
        .execute_request("GET", "/users/{id}", Some(json!({"id": "7"})))
        .await
        .expect("GET with path-param substitution must succeed");
```

Its module header also records the **run incantation and a naming rule** worth copying
`[VERIFIED: http_executor.rs:11-14]`:
```
//! Run with: `cargo test -p pmcp-server-toolkit --features openapi-code-mode \
//! --test http_executor -- --test-threads=1`. The test fns are
//! `http_executor_`-prefixed so the positional `http_executor` verify filter
//! resolves (Plan 01 verify-filter lesson).
```

**Mapping the 13 rows to surfaces:**

| Rows | Surface | Feature gate | Harness |
|---|---|---|---|
| 3 Code Mode CR-01 probes (query-via-placeholder, two-placeholder length, traversal) | `HttpCodeExecutor` + `PlanExecutor` | `openapi-code-mode` | `http_executor.rs` template |
| 2 curated single-call CR-01 probes | `HttpClient::substitute_path` | `http` **only** — no JS engine (SC-1's whole point) | `http_auth.rs` / `base_url_expansion.rs` templates |
| CR-02 uncapped filter, free-text code-list, undeclared argument, both `arguments`-missing rows, compliant call | D1 through a synthesized handler | `input-validation` (+ `http`) | new binary |
| E1 policy hook | `RequestPolicy` between steps (2) and (3) | `http` | new binary |
| E3 `garde` | core `TypedTool` | `validation` | **root** `tests/`, not the toolkit |

> ⚠ **The two curated rows must be in a binary that does NOT require `openapi-code-mode`,** or SC-1's
> "including the two that need no JS engine" is untested on the build it is about. Give them their own
> `#![cfg(all(feature = "http", feature = "input-validation"))]` file.

> ⚠ **Add every new binary to `REQUIRED_TEST_BINARIES` and add `input-validation` to
> `make test-server-toolkit`'s `--features`** (`[VERIFIED: Makefile:587]` today: `--features http`),
> or the whole matrix compiles to `running 0 tests` and the gate stays green. Finding 8d.

---

# Standard Stack

## Core

**No new external dependency is required by this phase.** Every library it needs is already pinned
in-tree; the work is wiring, not adoption.

| Library | Version (locked) | Purpose | Why standard |
|---------|------------------|---------|--------------|
| `jsonschema` | **0.49.2** (`Cargo.toml:220`, `crates/pmcp-server-toolkit/Cargo.toml:54`, `Cargo.lock:3662`) | D1 schema enforcement | Already the in-tree validator with five versions of recorded measurements; D-01 forbids a second path |
| `garde` | **0.23.0** (`Cargo.toml:221`, `Cargo.lock:2738`) | E3 struct field validation | Already an (unused) optional dep; the CR names it |
| `regex` / `fancy-regex` | 1.12.3 / 0.18.0 (transitive via `jsonschema`) | `pattern` evaluation | Not a direct dep and must not become one — Finding 1f's two-engine split is a property to *document*, not to work around |
| `serde_json` | 1.x | argument values | existing |
| `tracing` | 0.1 | the D-07 startup log | existing; `output_validation.rs:581` is the in-tree precedent for a `tracing::warn!` on a validation decision |

## Supporting (dev / test only)

| Library | Version | Purpose | When to use |
|---------|---------|---------|-------------|
| `wiremock` | 0.6 (`crates/pmcp-server-toolkit/[dev-dependencies]`) | the acceptance matrix's upstream | every one of the 13 rows |
| `proptest` | 1.7 (toolkit), 1.x (root) | SC-8 property arm | the three properties in Finding 9b |
| `libfuzzer-sys` | 0.4 (fuzz crates) | SC-8 fuzz arm | the two targets in Finding 9a |
| `trybuild` | 1 | compile-fail tests | if E3 needs to prove a bound is *not* required |
| `tempfile` | 3 | fixture configs | config-parse tests |

## Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `jsonschema` in core (D-01) | a toolkit-local `jsonschema` path | **Rejected by D-01.** Would let input and output be checked under different dialects and duplicate the cache/pin/renderer. |
| `cached_validator`'s `(Era, String)` key | per-handler `OnceLock<Arc<Validator>>` in the toolkit | 540 ns/call cheaper (Finding 1i) and needs no core cache change; but puts a second lifetime story next to the existing one. Either is defensible — the planner should pick one and say why. |
| `draft202012::new` | `jsonschema::options().should_validate_formats(true).build()` | Required if D2's `format` is to enforce (Finding 1g). Must be a *separate* compile entry point from the output path. |
| deprecating `server::validation` | deleting it | **Rejected by D-03** — removing a `pub mod` from a 2.x line is a semver break. |
| `#[deprecated]` on the module | `#[deprecated]` on each of the 11 functions | Same gate failure (Finding 5c), no benefit. |

**Installation:** none — no `cargo add` in this phase.

**Version verification performed:**
```bash
grep -n -A3 '^name = "jsonschema"' Cargo.lock   # -> 0.49.2, checksum f8a77951…
grep -n -A3 '^name = "garde"' Cargo.lock        # -> 0.23.0, checksum 5d7f479d…
cargo tree -p jsonschema --depth 1              # -> fancy-regex 0.18.0, regex 1.12.3, jsonschema-regex 0.49.2
cargo tree -p pmcp --features validation -e normal -i garde   # -> garde v0.23.0 └── pmcp v2.20.4
```
`[VERIFIED: run this session]`

## Package Legitimacy Audit

**No new external packages are introduced by this phase**, so no slopsquatting surface is added. The
four libraries above are all pre-existing, workspace-locked dependencies whose presence, version and
checksum were read from `Cargo.lock` this session.

| Package | Registry | Version | In-tree since | Source repo | Verdict | Disposition |
|---|---|---|---|---|---|---|
| `jsonschema` | crates.io | 0.49.2 | pre-existing (Phase 115) | github.com/Stranger6667/jsonschema | OK — locked + checksummed | Approved (no change) |
| `garde` | crates.io | 0.23.0 | pre-existing (optional, unused) | github.com/jprochazk/garde | OK — locked + checksummed | Approved (activated, not added) |
| `wiremock` | crates.io | 0.6 | pre-existing (Phase 90) | github.com/LukeMathWalker/wiremock-rs | OK | Approved (dev-only) |
| `proptest` | crates.io | 1.7 | pre-existing | github.com/proptest-rs/proptest | OK | Approved (dev-only) |

**Packages removed due to [SLOP] verdict:** none.
**Packages flagged as suspicious [SUS]:** none.

> If a plan proposes any *new* crate (e.g. a percent-decoding helper for D4(a)), it must run the
> legitimacy gate before the install task. **Recommendation: do not.** `percent-encoding 2.3.2` is
> already in the graph transitively via `jsonschema`, and `url` is already a direct toolkit dep under
> `http` (`crates/pmcp-server-toolkit/Cargo.toml` `http` list includes `"dep:url"`) — but reaching a
> *transitive* crate directly requires a new manifest entry, so prefer hand-rolling the ASCII
> case-insensitive `%XX` scan (it is ~15 lines and Finding 5b's table is its full spec).

# Architecture Patterns

## System Architecture Diagram

```
 MCP client
     │  tools/call { name, arguments }
     ▼
┌─────────────────────────── pmcp (core) ────────────────────────────┐
│  dispatch (mod.rs:2820 / core.rs:1026)                             │
│     │   [max_tool_args_bytes cap — core.rs:1014, the ONLY          │
│     │    input guard that exists today]                            │
│     │   ⋯ schema check NOT wired here this phase (deferred) ⋯      │
│     ▼                                                              │
│  ToolHandler::handle(args, extra)                                  │
└──────┬─────────────────────────────────────────┬───────────────────┘
       │ config-driven handler                   │ hand-written TypedTool<T>
       ▼                                         ▼
┌─ pmcp-server-toolkit ───────────────┐   ┌─ E3: garde ────────────┐
│                                     │   │ serde_json::from_value │
│ (D1) schema_validation::            │   │        ↓               │
│      validate_input(inputSchema,    │   │ T: garde::Validate     │
│                     arguments)      │   │   .validate()          │
│    arguments None|Null ──► {}       │   │        ↓ Report        │
│    compile_2020_12 (D-02, no Era)   │   │ value-free by          │
│    cache: (Era,String) or OnceLock  │   │ construction           │
│         │                           │   └────────────────────────┘
│    REFUSE ──► render_refusal()      │
│         │     from ValidationErrorKind ONLY  ─────► client
│         │     (NEVER Display — it echoes the value)  [0 upstream]
│         ▼ pass
│ (E2) ArgumentValidator (per tool, by name)
│         ▼ pass
│    ┌────────────────┴─────────────────┐
│    │ curated single-call              │ script / Code Mode
│    ▼                                  ▼
│ HttpClient::substitute_path      pmcp-code-mode PlanExecutor
│  for param in operation                 │ (1) PlanExecutor::resolve_path
│      .path_parameters()                 │     — script ${var} interpolation
│   (D4a/b/c) validate_path_              │ (D-09) ‹NEW› resolve {key}
│     placeholder(value)  ◄───────────────┼──── placeholders HERE, then
│   REFUSE ──► fixed msg, no path/value   │     validate_path_placeholder
│         ▼ pass                          │     REFUSE ──► fixed msg
│   path.replace(placeholder, value)      │        ⚠ and STOP wrapping the
│         │                               │          error with resolved_path
│         │                               │          (executor.rs:2846/:3018)
│         ▼                               ▼ pass
│      join_url(base, path)  ◄──── HttpExecutor::execute_request(method, RESOLVED path, body)
│         ▼                                   │ step (2) join_url
│ (E1) RequestPolicy ◄────── D-12 ────────────┤ ‹HERE, before step (3)›
│      sees: tool, method, resolved path,     │
│            query pairs, body                │
│      NEVER sees: the credential             │
│         ▼ allow                             ▼ allow
│      apply outgoing auth (step 3)  ─────────┘
└─────────┬───────────────────────────────────────────────────────────┘
          ▼
   upstream REST backend   ◄── reached only on a full pass;
                               every refusal above = 0 requests
```

## Component Responsibilities

| File | Responsibility this phase |
|---|---|
| `src/server/schema_validation.rs` **(new)** | `validate_input`, `InputViolation`, `render_refusal`, `validate_path_placeholder` (per Finding 6's Option A) |
| `src/server/output_validation.rs` | unchanged except: 18 `feature = "validation"` → `"schema-validation"`; helpers widened to `pub(in crate::server)` |
| `src/server/mod.rs` | `pub mod schema_validation;` declaration; `#[deprecated]` + `#[doc(hidden)]` on `pub mod validation` (`:206`) |
| `src/server/validation.rs` | deprecation attributes; `validate_safe_path` stays as the harvest source |
| `src/server/typed_tool.rs` | E3 attach point; fix the "Deserialize and validate" comment |
| `Cargo.toml` | `schema-validation` feature; version 2.21.0 |
| `crates/pmcp-code-mode/src/executor.rs` | D-09 trait signature + move layer-2 resolution to `:2833`/`:3006`; **stop echoing `resolved_path` at `:2846`/`:3018`**; re-export `validate_path_placeholder` |
| `crates/pmcp-server-toolkit/src/code_mode.rs` | `HttpCodeExecutor::execute_request` loses `resolve_path` step (1); E1 hook between steps (2) and (3) |
| `crates/pmcp-server-toolkit/src/http/client.rs` | D4 in `substitute_path`/`render_scalar`'s neighbourhood |
| `crates/pmcp-server-toolkit/src/tools.rs` | D1 call in `synthesize_inner`/`synthesize_http_inner`; D2 in `build_param_property`; SC-6 three comments |
| `crates/pmcp-server-toolkit/src/config.rs` | D2 `ParamDecl` fields; `[server.validation]` on `ServerSection`; `lint()` |
| `cargo-pmcp/src/commands/validate.rs` | SC-3's deploy-time half — see Finding 11b's fork |
| `.planning/codebase/ARCHITECTURE.md` | D-03(c), lines 199 **and** 200 |

## Pattern 1: Value-free refusal rendering (SC-7)

**What:** never let a validator's `Display` reach the client; render from the typed error kind.
**When to use:** every D1/D4/E2 refusal.

```rust
// Source: measured against jsonschema 0.49.2 this session (Finding 1c)
use jsonschema::error::ValidationErrorKind as K;

fn expectation(e: &jsonschema::ValidationError<'_>) -> Option<(&'static str, String)> {
    Some(match e.kind() {
        K::MaxLength { limit }  => ("maxLength", format!("at most {limit} characters")),
        K::MinLength { limit }  => ("minLength", format!("at least {limit} characters")),
        K::Pattern { pattern }  => ("pattern",   format!("must match {pattern}")),
        K::Maximum { limit }    => ("maximum",   format!("at most {limit}")),
        K::Minimum { limit }    => ("minimum",   format!("at least {limit}")),
        K::Enum { options }     => ("enum",      format!("one of {options}")),
        K::MaxItems { limit }   => ("maxItems",  format!("at most {limit} items")),
        K::Required { property} => ("required",  format!("`{property}` is required")),
        // NEVER read `unexpected` — it is the attacker's key list.
        K::AdditionalProperties { unexpected } =>
            ("additionalProperties", format!("unknown argument(s): {}", unexpected.len())),
        _ => return None,   // fall back to a generic "does not match the declared schema"
    })
}
```

**Anti-pattern (the exact line that must NOT be copied):**
```rust
// src/server/output_validation.rs:128-131 — correct for warn-only OUTPUT, FATAL for INPUT
.map(|e| format!("{} (at {})", e, e.instance_path()))
//               ^^ Display: echoes the 5,000-char PHI value verbatim
```

## Pattern 2: Denylist-floor-then-narrow (D-10)

**What:** the character denylist runs unconditionally; a declared `pattern` is an additional `AND`,
never a replacement.
**When to use:** `validate_path_placeholder`, both surfaces.

```rust
pub fn validate_path_placeholder(
    param: &str,                 // DECLARED name — safe to echo
    value: &str,
    declared_pattern: Option<&str>,
    declared_max_length: Option<usize>,
    allow_slash: bool,           // D-11: per-parameter CONFIG opt-in only, never spec allowReserved
) -> Result<(), PlaceholderRefusal> {
    // 1. UNCONDITIONAL floor — runs first, cannot be relaxed by any pattern (D-10).
    //    `..` and percent-encoded traversal have NO escape (D-11).
    //    Case-insensitive on hex: %2E and %2e both decode to '.' (Finding 5b).
    // 2. Always-on placeholder cap, independent of D3 (D-08).
    // 3. THEN the declared pattern narrows further (never replaces).
    // 4. THEN the D3 position-scoped cap.
}
```

**Why the ordering is load-bearing, measured:** `^.*$` — a pattern "specs in the wild are full of" —
accepts `2026AA?string=x`, `current/../../search/current`, `%2e%2e%2f`, `%3Fstring%3Dx`, `a%00b` and
`a#frag`, all of them `[MEASURED: probe]`. Pattern-supersedes-denylist would have been a no-op.

## Pattern 3: Feature severance proved by a nonzero count

**What:** a `#[cfg(feature = …)]` test file that the gate's feature list does not enable compiles to
`running 0 tests` and exits 0.
**When to use:** every new gated test binary in this phase.

```make
# Source: Makefile:587-616, the existing test-server-toolkit leg
@out=$$($(CARGO) test -p pmcp-server-toolkit --features http,input-validation -- --test-threads=1 2>&1); \
 ran=$$(echo "$$out" | awk '/^test result:/ { total += $$4 } END { print total+0 }'); \
 if [ "$$ran" -eq 0 ]; then echo "gate is not reaching this crate"; exit 1; fi; \
 REQUIRED_TEST_BINARIES="env_ref_grammar_parity base_url_expansion input_validation_acceptance"; \
 …per-binary count assertion via scripts/named-test-binary-count.awk…
```

## Anti-Patterns to Avoid

- **Renaming `output_validation.rs`.** Two test files hold its path as a string literal, a fuzz
  target and a documented nextest selector hold its module path, and the selector failure mode is
  *silently zero tests* (Finding 2b).
- **Merging the three-function split** in `output_validation.rs` — it exists to stay under the PMAT
  cog-25 CI gate and says so in its own doc comment.
- **Using `compile_for_era` for inputs.** D-02 forbids it, and the reason must be written into the
  code, or a maintainer will "fix" the asymmetry (CONTEXT.md § Established Patterns).
- **Emitting array-form `items`** for D2 — it fails to compile under the 2020-12 pin (Finding 1h).
- **Trusting `jsonschema::meta::is_valid`** for SC-2 — it returns `true` for a non-compiling
  `pattern` (Finding 1e).
- **Assuming `format` enforces** — it is annotative by default (Finding 1g).
- **Tightening `T`'s bound on `TypedTool`** — breaking, and not fixable by specialization on stable
  Rust (Finding 4c).
- **Relying on `make unused-deps`** to notice a dead feature — it is a hardcoded no-op
  (`Makefile:236-241`).
- **Relying on `make test-property`** to run a new `property_*` test — it selects only `#[ignore]`d
  ones (Finding 9b).
- **Relying on `make test-fuzz`** to fail on a crash — it swallows failures with `|| echo`
  (Finding 9a).

# Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON Schema evaluation | a keyword interpreter over `ParamDecl` | `jsonschema` 0.49.2 via core's `compile_2020_12` | D-01 locks it; `additionalProperties`, `enum`, `maxLength`-over-code-points and `pattern` anchoring semantics are each a bug farm — all four measured in Finding 1 |
| Compiled-validator caching | a `HashMap<String, Validator>` in the toolkit | `output_validation::cached_validator` (`:631`), or a per-handler `OnceLock` | the era-keying rationale at `:619-628` explains a first-writer-wins bug already found and fixed here |
| Draft pinning / `$schema` normalization | stripping `$schema` by hand | `normalize_schema_dialect` (`:553`) | it walks six subschema-map keywords and a data-only exclusion list; two narrower versions shipped and were both wrong (`:40-53`) |
| Value-free error text | `format!("{e}")` with a sanitizer regex over it | `ValidationErrorKind` matching (Pattern 1) | a redaction pass over a message you do not control is a denylist; the typed kind is an allowlist |
| Path traversal detection | `path.contains("..")` | the D-10 floor built on `validate_safe_path`'s shape | `..` alone covers 1 of 8 required checks (Finding 5b) |
| Struct field validation | a `#[derive]` of your own | `garde` 0.23 | already a pinned optional dep; `Report` is value-free by construction (Finding 4b) |
| A mock HTTP upstream | a hand-rolled `TcpListener` | `wiremock` 0.6 | already a dev-dep with two established zero-request idioms (Finding 12b) |
| Publish-order reasoning | reading CLAUDE.md prose | `.github/workflows/release.yml` + `scripts/check-release-coverage.sh` | the workflow is the AUTHORITY; CLAUDE.md records three corrections where the prose was wrong — and its `cargo-pmcp` version is stale right now (Finding 7a) |

**Key insight:** every "don't hand-roll" item here is one this repository has *already* got wrong
once and documented the scar. The phase's cheapest risk reduction is to read those scars rather than
re-derive them.

# Common Pitfalls

### Pitfall 1: The refusal message leaks the thing you refused
**What goes wrong:** D1 refuses a 5,000-char PHI dump and returns it to the client inside the error.
**Why it happens:** `jsonschema::ValidationError`'s `Display` echoes the value for *every* keyword
(Finding 1b), and the module the phase is told to generalize already renders errors that way
(`output_validation.rs:128-131`). Copying the obvious line is the failure.
**How to avoid:** Pattern 1 — render from `ValidationErrorKind` only. Read `AdditionalProperties`'
`unexpected` **only** through `.len()`.
**Warning signs:** any `format!("{e}")`, `e.to_string()`, or `{}`-formatting of a `ValidationError`
on an input path. Grep for them in review. A fuzz target asserting "no input byte appears in the
output" is the durable guard (Finding 9a).

### Pitfall 2: A gated test file compiles to `running 0 tests` and the gate goes green
**What goes wrong:** the acceptance matrix is written, the gate passes, nothing ran.
**Why it happens:** `make test-server-toolkit` pins `--features http` (`Makefile:587`); a file gated
`#![cfg(feature = "input-validation")]` is compiled empty. The Makefile documents this exact
measurement for `base_url_expansion.rs` at `:576-583`.
**How to avoid:** add the feature to the leg **and** add each new binary to `REQUIRED_TEST_BINARIES`,
which count-asserts per binary via `scripts/named-test-binary-count.awk` and distinguishes
"never ran" (-1) / "no result line" (-2) / "ran but zero tests" (0).
**Warning signs:** a `test result:` line reading `0 passed` for your file; a gate that got *faster*
after you added tests.

### Pitfall 3: `\s` silently changes meaning when a pattern gains a lookahead
**What goes wrong:** a config author adds `(?=...)` to a `pattern`, and `\s` quietly starts matching
U+3000 — or `\S` stops.
**Why it happens:** two regex engines in one `jsonschema` process (Finding 1f). Plain patterns take
the linear `regex` path with a partial ECMA `\s`; lookaround/backreference patterns take
`fancy-regex`, where `\s` is `\p{White_Space}`.
**How to avoid:** SC-2's documentation must carry **both columns**. Advise explicit character
classes over `\s`/`\S` for anything security-relevant.
**Warning signs:** a test that passes for one pattern and fails for a "logically equivalent" one.

### Pitfall 4: `format` in the config buys nothing
**What goes wrong:** `format = "uuid"` ships, a reviewer believes it is enforced, it is not.
**Why it happens:** 0.49 is annotative by default; `"!!!not-a-uuid!!!"` validates `true`
(Finding 1g).
**How to avoid:** decide Q1 explicitly. If enforcing, build through
`jsonschema::options().should_validate_formats(true)` on a **separate** entry point from outputs.
If not, say so in the `ParamDecl` rustdoc, the docs deliverable **and** the config reference.
**Warning signs:** a `format` acceptance row that passes without any code change.

### Pitfall 5: A non-compiling `pattern` takes down the whole tool, at call time
**What goes wrong:** one bad regex in one parameter and the tool's entire validator fails to
compile, so either every call is refused or (worse, if the error is swallowed) every call passes.
**Why it happens:** `draft202012::new` fails the whole document; the compile error carries
`schema_path=/properties/bad/pattern` (Finding 1e).
**How to avoid:** SC-2 — compile the synthesized `inputSchema` during `ServerConfig::validate` and
fail there, surfacing `schema_path` to name the parameter. **Not** `meta::is_valid`, which returns
`true`.
**Warning signs:** a tool that 500s on every call after a config edit.

### Pitfall 6: `#[deprecated]` breaks `make lint` through an example
**What goes wrong:** D-03(b) lands, `make quality-gate` fails at `cargo check --features "full"
--examples` with eight `-D warnings` deprecation errors.
**Why it happens:** `examples/s19_wasm_typed_tools.rs` uses `validation::validate_*` at eight sites,
and `RUSTFLAGS = -D warnings` (Finding 5c).
**How to avoid:** handle the example in the same commit as the attribute. `#[doc(hidden)]` alone is
safe; `#[deprecated]` is not.
**Warning signs:** the gate failing in `lint` rather than `test`.

### Pitfall 7: The resolved path is re-attached to the refusal one layer up
**What goes wrong:** D4 returns a fixed message with no path; `PlanExecutor` wraps it as
`"{method} {resolved_path} failed: {e}"` and the injected path reaches the client anyway.
**Why it happens:** `executor.rs:2846` and `:3018` (Finding 3d).
**How to avoid:** fix both sites in the same plan task as D-09's resolution move. Assert it in an
acceptance row, not just by reading.
**Warning signs:** an acceptance row asserting "message contains neither value nor path" that you
only ran against the *toolkit* surface.

### Pitfall 8: `--all-features` "proves" a severance that does not hold
**What goes wrong:** the D-04 split is declared severable; dev-dependency unification had silently
re-enabled `garde`.
**Why it happens:** documented in this project's memory and reproduced in-tree
(`crates/pmcp-server-toolkit` Phase 109 note).
**How to avoid:** a dev-dep-free `cargo build --no-default-features --features schema-validation`
plus an explicit `cargo tree` absence assertion. Finding 8d.

### Pitfall 9: `ServerConfig::validate` cannot warn
**What goes wrong:** D-07's warning is written as a `return Err(...)` because that is the only
channel, and servers stop booting on an uncapped free-text field — the exact breakage D-05 exists to
avoid.
**Why it happens:** `validate` returns first-error-wins `Result<(), ConfigValidationError>`
(Finding 11a).
**How to avoid:** add `lint() -> Vec<ConfigWarning>` and keep `validate` unchanged; promote only
under the `[server.validation] strict` flag.

### Pitfall 10: `[server.validation]` breaks 0.1.3 parsing, and D-15 does not mention it
**What goes wrong:** the rollout note warns about `ParamDecl` only; an operator adds
`[server.validation]` and 0.1.3 fails to parse for a reason nobody documented.
**Why it happens:** `ServerSection` and `ServerConfig` both carry `#[serde(deny_unknown_fields)]`
(Finding 10d).
**How to avoid:** widen D-15's CHANGELOG entry to name both.

# Code Examples

### Treating missing `arguments` as `{}` (the two conflatable acceptance rows)
```rust
// Source: measured — jsonschema 0.49.2 refuses `null` on `type: object`
// (`zeroparam-null: valid=false, "null is not of type \"object\""`), so the
// substitution MUST happen before the validator sees the value.
static EMPTY: once_cell::sync::Lazy<Value> = /* json!({}) */;

let args: &Value = match arguments {
    None | Some(Value::Null) => &EMPTY,
    Some(v) => v,
};
// zero-parameter tool + no arguments  -> ACCEPTED  (measured: zeroparam-empty-obj valid=true)
// required params   + no arguments    -> REFUSED by Required { property }, a DECLARED name
//                                        (measured: required-empty-obj -> "\"cui\" is a required property")
```

### SC-2's config-time pattern check
```rust
// Source: measured — a bad nested pattern fails the WHOLE schema and names the position:
//   "^[A-Z" is not a "regex"  | schema_path=/properties/bad/pattern
// and jsonschema::meta::is_valid returns TRUE for the same document, so it is NOT the check.
fn check_tool_schema_compiles(tool: &ToolDecl) -> Result<(), ConfigValidationError> {
    let schema = build_input_schema(&tool.parameters);
    jsonschema::draft202012::new(&schema).map_err(|e| {
        ConfigValidationError::UncompilableParamSchema {
            tool: tool.name.clone(),
            // schema_path points at /properties/<param>/pattern
            position: e.schema_path().to_string(),
            detail: e.to_string(),   // SAFE here: config-time, author-facing, not client-facing
        }
    })?;
    Ok(())
}
```

### Zero-upstream-request acceptance row
```rust
// Source: crates/pmcp-server-toolkit/tests/script_tool_engine_parity.rs:85-87 idiom
#[tokio::test]
async fn input_validation_refuses_uncapped_filter_without_contacting_upstream() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(200))
        .mount(&server).await;

    let value = "x".repeat(5000);
    let err = call_tool("search", json!({ "sabs": value.clone() }), &server).await
        .expect_err("D3 must refuse a 5000-char path/query string at cap 256");

    let requests = server.received_requests().await.expect("wiremock records requests");
    assert!(requests.is_empty(), "refusal must not reach upstream");

    let msg = err.to_string();
    assert!(!msg.contains(&value),   "SC-7: never echo the value");
    assert!(!msg.contains("/search"), "D4: never echo the path");
    assert!(msg.contains("256"),      "name the DECLARED expectation");
}
```

# State of the Art

| Old approach | Current approach | When changed | Impact |
|---|---|---|---|
| `$schema` auto-detect for output validation | Draft 2020-12 pinned on `Era::V2` | Phase 115 (SCHM-01), `output_validation.rs:16-34` | D-02 extends the pin to inputs on **both** eras, since inputs have no shipped behaviour to freeze |
| `jsonschema` 0.46.10 → 0.48.5 | **0.49.2** | `Cargo.toml:205-220` records the rationale | 0.49.0 is "purely ADDITIVE over 0.48 (`options_for`, `meta::validate_for`, one `multipleOf` fix)"; an exact `=` pin was deliberately declined |
| `ValidationError.kind` as a field | `.kind()` as a **method** | ≤ 0.49.2 | `&e.kind` is `E0615`; any pre-0.49 snippet must be adapted `[MEASURED]` |
| Config *describes* limits | Config *binds* limits | **this phase** | the whole point |
| `openapi-code-mode` as the validation gate (CR first draft) | `input-validation` | review finding 3 / SC-1 | keeps SWC/JS out of curated builds |
| Review note D's seven crates | D-13's eleven | this phase | `pmcp-workbook-compiler` confirmed as the missed one (Finding 7b) |

**Deprecated / outdated:**
- `pmcp::server::validation` (11 public validators + `Validator`/`FieldValidator` builder) — zero
  callers in `src/`, being `#[deprecated]` + `#[doc(hidden)]` here, removal booked for 3.0.
- `ARCHITECTURE.md:199-200`'s validation bullets — both false today (Finding 5d).
- CLAUDE.md § Release item 15a's "`cargo-pmcp` 0.23.0" — actually 0.24.3 (Finding 7a). Out of scope,
  recorded for whoever next edits the ledger.

# Assumptions Log

| # | Claim | Section | Risk if wrong |
|---|---|---|---|
| A1 | `RUSTFLAGS="-D warnings"` does **not** reach rustdoc's doctest compilation, so the 10 doctests inside `validation.rs` will warn rather than fail under `make test-doc`. **Not measured** — I did not run `make test-doc` with the attribute applied. | Finding 5c | If wrong, D-03(b) additionally fails the `test-doc` gate leg and each doctest needs a hidden `# #![allow(deprecated)]`. Cheap to check: one plan task running `make test-doc` after the attribute lands. |
| A2 | No `pattern` accepted by `jsonschema` 0.49.2 can cause catastrophic backtracking. **Falsification attempted and did NOT reproduce** at n ≤ 28 on both engines (Finding 1j), which is evidence, not proof — I did not search the input space. | Finding 1j | If wrong, a config-supplied `pattern` is a DoS vector. Mitigation is cheap and already recommended: a `(pattern, input)` fuzz target. |
| A3 | `#[deprecated]` is permitted on a `pub mod` in current stable Rust. | Finding 5c | If wrong, D-03(b) must attach to each of the 11 items instead — same gate consequence, more edits. |
| A4 | Adding five `if let` arms to `build_param_property` keeps it under PMAT cog 25. Reasoned from its straight-line shape, not measured post-change. | Finding 10c | If wrong, CI's `quality-gate` job fails; the fix is the documented P1–P6 refactor set or a `// Why:`-annotated allow. Re-run `pmat analyze complexity` after the edit. |
| A5 | `garde 0.23`'s `Validate` associated type is named `Context` and `validate(&self)` is available when `Context = ()`. Measured indirectly — my probe's `fn check<T: Validate<Context = ()>>(v: &T)` compiled and ran. | Finding 4b | Low risk — it compiled. |
| A6 | pmcp.run's built-in `openapi-api` server implements `HttpExecutor` out-of-repo. Taken from CONTEXT.md D-09 / the CR; **not verifiable from this tree**. | Finding 3f | If wrong, D-09's "answers CR open question 5 by construction" is weaker than stated, but the in-repo safety argument is unaffected. |
| A7 | The plain-engine `\s` set I measured is `jsonschema-regex`'s ECMA-262 translation. The *values* are measured; the *attribution to a specific crate* is inference from the dependency graph. | Finding 1f | None for planning — SC-2 documents observed behaviour, not its cause. |

# Open Questions

1. **Q1 — Does `format` enforce?** `format` is annotative in 0.49 (Finding 1g). SC-2 requires
   `ParamDecl` to *accept* `format`; it does not say it must enforce.
   - *Known:* enforcement requires `jsonschema::options().should_validate_formats(true)`, which
     would have to be a separate compile entry point from the output path.
   - *Unclear:* whether the phase wants a third acceptance row for it.
   - *Recommendation:* **ship `format` as enforcing**, on a dedicated input-only builder. A config
     keyword that silently does nothing is the same defect class as the three false comments SC-6
     exists to fix.

2. **Q2 — Where does `validate_path_placeholder` live?** D-09 says "export" it from
   `pmcp-code-mode`, but the toolkit's curated `http` feature cannot reach that crate without either
   a new dependency edge or violating SC-1 (Finding 6).
   - *Recommendation:* implement in core `pmcp` (`server::schema_validation`), re-export from
     `pmcp-code-mode` so D-09's published-helper obligation is met, and have both surfaces call the
     core one. One copy of the rule; purity gate untouched.

3. **Q3 — Is a bare `%` refused in a placeholder value?** D-10 names "percent-encoded forms of
   these". `%252e` double-decodes to `.`.
   - *Recommendation:* refuse any `%` not followed by two hex digits, and refuse `%25` outright.
     A path *placeholder value* has no legitimate need for a literal percent sign, and this closes
     the double-encoding family in one rule. Flag it as a deviation from the CR's literal wording.

4. **Q4 — Does `pmcp-code-mode-derive` need a bump at all?** Its `pmcp-code-mode` pin is in
   **`[dev-dependencies]`** (`crates/pmcp-code-mode-derive/Cargo.toml:25-27`), which Cargo strips at
   publish time when path-carrying, and the crate emits no `HttpExecutor` code (Finding 7c).
   - *Recommendation:* bump it as a **patch** (0.3.0 → 0.3.1) with the dev-dep repinned. That keeps
     the workspace building, keeps root `Cargo.toml:264`'s `^0.3.0` valid via the caret exception,
     and leaves the release at eleven versions. Record the reasoning in the release note so a future
     releaser does not re-derive it.

5. **Q5 — How does SC-3's deploy-time warning reach `cargo-pmcp`?** It has no toolkit dependency and
   `validate deploy` reads a different file (Finding 11b).
   - *Recommendation:* a new `cargo pmcp validate config` subcommand backed by a real
     `pmcp-server-toolkit` dependency. Record the ROADMAP deviation (SC-3 names `validate deploy`).

6. **Q6 — Which cache does `validate_input` use?** The existing `(Era, String)` cache costs 540 ns of
   key serialization per call (Finding 1i); a per-handler `OnceLock<Arc<Validator>>` costs nothing.
   - *Recommendation:* if Q1 resolves to "enforce formats", a **separate** cache is mandatory
     anyway (a format-asserting and a non-asserting validator must not collide on the same key), so
     resolve Q1 first. Otherwise either is fine; state the choice in the module docs.

7. **Q7 — Does `input-validation` go in the toolkit's `default`?** It is not there today
   (Finding 8c), and two consumers set `default-features = false`.
   - *Recommendation:* yes, add it to `default` **and** add it explicitly to
     `pmcp-workbook-server:43` and `pmcp-workbook-compiler:107`, or D1 is silently off for workbook
     servers.

# Environment Availability

| Dependency | Required by | Available | Version | Fallback |
|---|---|---|---|---|
| `cargo` / `rustc` (stable) | everything | ✓ | — (CI uses `dtolnay/rust-toolchain@stable`; run `rustup update stable` before the release per the Pre-Flight Checklist) | none |
| `pmat` | CI cognitive-complexity gate; local verification | ✓ | **3.15.0** — matches the CI pin exactly `[VERIFIED: pmat --version]` | none |
| `cargo nextest` | `make test` | assumed present (the Makefile invokes it unconditionally) | — | `cargo test` |
| `cargo audit` | `make audit` gate leg | assumed present | — | none |
| `cargo fuzz` + nightly | `make test-fuzz` | not probed | — | the target swallows failures anyway (Finding 9a) |
| `cargo machete` | `make unused-deps` | **✗ — and the call is commented out** `[VERIFIED: Makefile:236-241]` | — | none; this is why the dead feature survived |
| `wiremock` upstream | the 13-row acceptance matrix | ✓ (in-process, no network) | 0.6 | none needed |
| crates.io network | D-13/D-14 publish only | n/a at plan time | — | the release workflow skips already-published crates gracefully |
| `jq`, `awk`, bash 3.2 | `scripts/check-release-coverage.sh`, `scripts/named-test-binary-count.awk` | ✓ (script explicitly avoids bash-4-isms for stock macOS bash) | — | none |

**Missing dependencies with no fallback:** none that block this phase.
**Missing dependencies with fallback:** `cargo machete` — absent and disabled; do not plan any task
that depends on it detecting an unused dependency or feature.

## Validation Architecture

> Required: `.planning/config.json` has `"nyquist_validation": true`
> `[VERIFIED: .planning/config.json workflow.nyquist_validation]`.

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `libtest` + `cargo nextest` (root `make test`), `proptest 1.7` for properties, `libfuzzer-sys 0.4` for fuzz, `wiremock 0.6` + `tokio 1` for integration |
| Config file | none per se — the Makefile *is* the config. Root: `Makefile:244-253, 791-814, 917-920`. Toolkit: `Makefile:585-616`. |
| Quick run command (core) | `cargo test --lib --features "full" <filter>` |
| Quick run command (toolkit) | `cargo test -p pmcp-server-toolkit --features http,input-validation --test <binary> -- --test-threads=1` |
| Quick run command (code-mode) | `cargo test -p pmcp-code-mode --lib` |
| Full suite command | `make quality-gate` |
| ⚠ `--test-threads=1` | Mandatory per CLAUDE.md ("Tests run with `--test-threads=1` — race condition prevention") and already baked into `make test-server-toolkit`. |

### Phase Requirements → Test Map

| Req | Behavior | Test type | Automated command | File exists? |
|---|---|---|---|---|
| D1 | Schema-violating args refused before any backend call, 0 upstream requests | integration | `cargo test -p pmcp-server-toolkit --features http,input-validation --test input_validation_acceptance -- --test-threads=1` | ❌ Wave 0 |
| D1 | Missing `arguments` on a zero-param tool ACCEPTED as `{}` | integration | same binary, `-- arguments_missing_zero_param` | ❌ Wave 0 |
| D1 | Missing `arguments` on a required-param tool REFUSED by `required` | integration | same binary, `-- arguments_missing_required` | ❌ Wave 0 |
| D1 | `additionalProperties:false` honoured, not re-added | unit | `cargo test --lib --features full schema_validation::` | ❌ Wave 0 |
| D2 | `ParamDecl` round-trips `pattern`/`min_length`/`format`/`items`/`max_items` | unit | `cargo test -p pmcp-server-toolkit --lib config::` | ✅ (`config.rs` has 16 `validate_*` unit tests to extend) |
| D2/SC-2 | A non-compiling `pattern` fails **config** validation | unit | `cargo test -p pmcp-server-toolkit --lib config::validate_rejects_uncompilable_pattern` | ❌ Wave 0 |
| D2 | `items` emitted in object form (array form fails the 2020-12 pin) | unit | `cargo test -p pmcp-server-toolkit --lib tools::build_param_property` | ❌ Wave 0 |
| D3/D-05 | Path/query string over 256 refused; body string over 256 only warns | integration | `--test input_validation_acceptance -- position_scoped_cap` | ❌ Wave 0 |
| D3/SC-3 | `ServerConfig::lint()` surfaces an uncapped body string | unit | `cargo test -p pmcp-server-toolkit --lib config::lint_` | ❌ Wave 0 |
| D4(a)/SC-4 | `?`,`#`,`/`,`..` + percent-encoded refused — **Code Mode** (3 CR-01 probes) | integration | `cargo test -p pmcp-server-toolkit --features openapi-code-mode --test http_executor -- --test-threads=1` | ✅ extend existing binary |
| D4(a)/SC-4 | Same — **curated single-call**, no JS engine (2 CR-01 probes) | integration | `cargo test -p pmcp-server-toolkit --features http,input-validation --test curated_path_injection -- --test-threads=1` | ❌ Wave 0 |
| D4(b) | Spec `pattern` narrows on top of the floor; `^.*$` does not disable it | property | `--test path_placeholder_props -- --ignored property_` (see naming rule below) | ❌ Wave 0 |
| D4(c)/D-08 | Placeholder cap holds even with `default_max_length = 0` | integration | `--test curated_path_injection -- cap_independent_of_d3` | ❌ Wave 0 |
| D-09 | Every `HttpExecutor` receives an already-resolved path | unit | `cargo test -p pmcp-code-mode --lib executor::` | ✅ extend |
| D-09/Pitfall 7 | `PlanExecutor` error wrap no longer echoes `resolved_path` | unit | `cargo test -p pmcp-code-mode --lib executor::error_does_not_echo_path` | ❌ Wave 0 |
| E1/SC-5 | `RequestPolicy` refuses `/search` prefix, 0 requests, runs before auth | integration | `--test request_policy -- --test-threads=1` | ❌ Wave 0 |
| E2/SC-5 | Per-tool `ArgumentValidator` runs after D1 | integration | `--test request_policy -- argument_validator` | ❌ Wave 0 |
| E3/SC-5 | `#[garde(length(max = 10))]`, 11 chars → `Error::Validation`, value not echoed | unit | `cargo test --lib --features full typed_tool::garde` | ❌ Wave 0 |
| SC-6 | Three `tools.rs` comments corrected; no toolkit comment claims an unimplemented mitigation | manual + grep | `grep -rn "enforced upstream\|schema-validated\|rejected by pmcp" crates/pmcp-server-toolkit/src/` — **manual review, justified:** a comment's truth is not machine-checkable |
| SC-7 | No refusal message contains any byte of the rejected value or key | property + fuzz | `--ignored property_refusal_never_echoes_input` + `cargo fuzz run fuzz_input_schema_enforcement` | ❌ Wave 0 |
| SC-8 | Gate green; fuzz/property/unit/example present | gate | `make quality-gate` | ✅ |
| D-13/D-14 | Publish ledger stays coherent | gate | `./scripts/check-release-coverage.sh` (runs inside `make quality-gate`) | ✅ |

### Sampling Rate

- **Per task commit:** the narrowest relevant command from the table above, plus `make fmt-check`
  and `make lint`. The pre-commit hook enforces the Toyota Way gate, so a commit that cannot pass
  `make quality-gate` cannot land — task granularity must respect that.
- **Per wave merge:** `make test-server-toolkit && make test-unit && cargo test -p pmcp-code-mode`
  (the three crates this phase touches), plus `make lint` and `./scripts/check-release-coverage.sh`.
- **Phase gate:** full `make quality-gate` green before `/gsd-verify-work`, plus a manual
  `cargo run --example s57_typed_tool_garde_validation` (the CLAUDE.md ALWAYS requirement that
  `make test-examples` only *builds*).

### Wave 0 Gaps

- [ ] `crates/pmcp-server-toolkit/tests/input_validation_acceptance.rs` — gated
      `#![cfg(all(feature = "http", feature = "input-validation"))]`; covers D1's six rows
- [ ] `crates/pmcp-server-toolkit/tests/curated_path_injection.rs` — gated **without**
      `openapi-code-mode`; covers the two JS-engine-free CR-01 rows and D-08
- [ ] `crates/pmcp-server-toolkit/tests/request_policy.rs` — E1 + E2
- [ ] `crates/pmcp-server-toolkit/tests/path_placeholder_props.rs` — proptest arms
- [ ] `tests/typed_tool_garde.rs` (root) — E3
- [ ] `fuzz/fuzz_targets/fuzz_input_schema_enforcement.rs` + its `[[bin]]` stanza in `fuzz/Cargo.toml`
- [ ] **Makefile edits (these are the gap that makes the others real):**
      - `Makefile:587` — add `input-validation` to `test-server-toolkit`'s `--features http`
      - `Makefile:596` — add the new binaries to `REQUIRED_TEST_BINARIES`
      - consider a `test-code-mode` leg; `pmcp-code-mode` has **no** dedicated `quality-gate` leg
        today (`test-all` at `Makefile:1481` lists `test-tester`, `test-cargo-pmcp`,
        `test-server-toolkit`, `test-openapi-server` — not `pmcp-code-mode`), so D-09's tests would
        otherwise be gate-invisible
- [ ] **Property-test naming:** every new `property_*` fn must carry
      `#[ignore = "property arm — selected by \`make test-property\` (--ignored property_)"]`
      verbatim (`tests/log_emitter.rs:169`) or `make test-property` will not select it
- [ ] Framework install: none required

# Security Domain

> Required — `.planning/config.json` has no `security_enforcement` key, so it is enabled by default.

## Applicable ASVS Categories

| ASVS Category | Applies | Standard control in this phase |
|---|---|---|
| **V5 Input Validation** | **yes — this is the phase** | D1 via `jsonschema` 0.49.2 Draft 2020-12; D2's vocabulary; D3's position-scoped cap. Positive/allow-list validation from the declared schema, denylist only as the D-10 floor where no allow-list exists. |
| **V5 — Injection (path/URL)** | **yes** | D4 on both HTTP surfaces. This is CWE-22 (path traversal) and CWE-74/CWE-113-adjacent (URL structure injection via `?`/`#`). |
| V7 Error Handling & Logging | **yes** | SC-7: refusals carry declared expectations only. The startup log lists enforced rules and opt-outs once — and must not log argument values. |
| V2 Authentication | no | unchanged by this phase |
| V3 Session Management | no | unchanged |
| V4 Access Control | **partially** | E1 `RequestPolicy` is an authorization hook (endpoint allowlists, rate budgets) but the phase ships the *hook*, not a policy |
| V6 Cryptography | no | no crypto is written; D-12 exists specifically to keep the credential *out* of E1 |
| V8 Data Protection | **yes** | the PHI-non-echo requirement (SC-7) is a data-protection control, not a UX one |
| V13 API & Web Service | **yes** | "the server enforces the contract it publishes" is the V13 schema-validation control |

## Known Threat Patterns

| Pattern | STRIDE | Standard mitigation | Status after this phase |
|---|---|---|---|
| Unenforced published `inputSchema` → arbitrary arguments reach the backend | **Tampering** | validate against the declared schema before dispatch | **closed** for config-driven tools by D1; **open** for hand-written `ToolHandler`s (deferred) |
| Path/URL injection via a placeholder (`?`, `#`) — CR-01 | Tampering / **Elevation of Privilege** (reaches an undeclared endpoint) | resolve-then-validate; unconditional character floor | closed on both surfaces by D4 + D-09 |
| Path traversal via a placeholder (`..`, `%2e%2e`) — CR-01 | Tampering / EoP | unconditional, no-escape refusal | closed by D-10/D-11 |
| Unbounded string → PHI/free text forwarded upstream — CR-02 | **Information Disclosure** | position-scoped default cap | closed in path/query by D-06; body position is warn-only by D-05 — **an accepted residual** |
| Refusal message echoes the rejected value (PHI) | **Information Disclosure** | render from `ValidationErrorKind`, never `Display` | closed by SC-7 **only if** Pitfall 1 is avoided — this is the phase's own most likely self-inflicted vulnerability |
| Refusal message echoes an attacker-controlled key that is itself PHI | Information Disclosure | count + declared allow-list (`unknown argument(s): 2; allowed: cui, version`) | closed by the locked SC-7 shape |
| Resolved path re-attached to a refusal one layer up | Information Disclosure | fix `executor.rs:2846` / `:3018` | **open until explicitly planned** — Pitfall 7 |
| Credential visible to third-party policy code | Information Disclosure | E1 runs before auth is applied | closed by D-12 |
| Loose spec `pattern` (`^.*$`) silently disables the injection check | Tampering | denylist is a floor, pattern narrows on top | closed by D-10; **empirically justified** in Finding 5b |
| Documented-but-absent mitigation (the P0 sub-goal) | **Repudiation** of the threat model itself | correct the claims; assert no remaining false claim | closed by SC-6 for the toolkit; the cross-phase sweep is deferred |
| External `$ref` in a schema becomes a live network fetch | Information Disclosure / SSRF | `jsonschema` `default-features = false` (no `resolve-http`) | already closed; `tests/v2_schema_tripwires.rs` fences it — **extend the fence to the new `schema-validation` feature** (Finding 8b) |
| Config-supplied `pattern` as a ReDoS vector | **Denial of Service** | — | **residual, A2**: falsification attempted at n ≤ 28 on both engines and did not reproduce. Recommend a `(pattern, input)` fuzz target rather than a speculative mitigation. |
| Retry×timeout budget (127 s vs a 29 s gateway) | Denial of Service | per-request total-time budget | **explicitly deferred** to its own phase |

## Threat-model discipline note

This phase exists because three `T-*` comments asserted mitigations that did not exist, and
`/gsd-secure-phase` passed Phases 83 and 90 with them recorded as present. **Any new `T-*` comment
written in this phase must name the enforcing function and be backed by an acceptance row that fails
when the enforcement is removed** — a comment is not evidence, and a test that asserts *binding*
(like `script_tool.rs:185`) is not a test that asserts *validation*.

# Sources

### Primary (HIGH confidence) — measured in this tree/session
- `src/server/output_validation.rs` (lines 16-34, 55-68, 93-133, 149-150, 235-236, 553, 574-592, 598-628, 631-650, 663-691, 807) — the module D-01 generalizes
- `src/server/validation.rs` (1-40, 252-281, and the 11 `pub fn` inventory) — the D4(a) harvest
- `src/server/mod.rs` (70-95, 204-206, 2590, 2722, 2820) — module visibility + dispatch
- `src/server/core.rs` (995-1030, 1085, 1179, 1270) — the native dispatch + the only existing input guard
- `src/server/typed_tool.rs` (20-45, 246-261, 278-292) — E3 attach point
- `crates/pmcp-server-toolkit/src/tools.rs` (1-30, 190-250, 545-625, and the six `synthesize*` fns)
- `crates/pmcp-server-toolkit/src/config.rs` (95-115, 240-275, 325-360, 905-949)
- `crates/pmcp-server-toolkit/src/code_mode.rs` (800-1070)
- `crates/pmcp-server-toolkit/src/http/client.rs` (130-200, 340-380)
- `crates/pmcp-code-mode/src/executor.rs` (2421-2432, 2515, 2672, 2790-2870, 2960-3030, 3146-3175, 3353-3400)
- `crates/pmcp-code-mode-derive/Cargo.toml` (20-35), `crates/pmcp-code-mode/Cargo.toml` (29)
- `Cargo.toml` (205-230, 260-340, 714-720, 845), `Cargo.lock` (2737-2740, 3661-3664)
- `crates/pmcp-server-toolkit/Cargo.toml` (1-40, 45-60, 85-175, `[dev-dependencies]`)
- all seven toolkit-consumer manifests (postgres:28, mysql:28, athena:28, sql-server:33/51, workbook-server:43, openapi-server:29/47, workbook-compiler:107)
- `Makefile` (10-11, 180-216, 216-260, 562-616, 791-814, 879-881, 917-920, 1481, 1546, 1927-1975)
- `.github/workflows/release.yml` (the 24 `name: Publish` steps, 136–591)
- `scripts/check-release-coverage.sh` (1-80, 247-314)
- `tests/v2_schema_tripwires.rs` (36, 90-110, 804, 1059, 1069), `tests/property_tests.rs` (991-1000, 1137), `tests/log_emitter.rs` (165-170, 2220-2226)
- `crates/pmcp-server-toolkit/tests/` (http_executor.rs 1-60, script_tool.rs 175-200, script_tool_engine_parity.rs 85-87) and `crates/pmcp-openapi-server/tests/oauth_passthrough_e2e.rs` (117-172)
- `.planning/codebase/ARCHITECTURE.md` (195-205)
- `cargo-pmcp/src/commands/validate.rs` (1-60, 571-620), `cargo-pmcp/Cargo.toml`
- `CLAUDE.md` § Toyota Way, § ALWAYS Requirements, § Release & Publish Workflow
- `.claude/skills/spike-findings-rust-mcp-sdk/SKILL.md` + `references/schema-server-architecture.md`

### Primary (HIGH confidence) — probes compiled and run this session
- `jsonschema 0.49.2` (`default-features = false`, matching both in-tree manifests): additionalProperties, required/null, maxLength code-point counting, pattern syntax + anchoring + compile failure, `meta::is_valid`, the 19-codepoint `\s`/`\S`/`\p{White_Space}` sweep across both engines, `format` annotative-vs-assertive, `ValidationErrorKind` discriminants, error-order stability, draft-07 construct rejection, ReDoS timing, compile/validate/cache-key cost
- `garde 0.23.0` (`features = ["derive"]`): `Validate<Context = ()>`, `Report::iter()`, `Error { message }`, and the value-non-echo check against a PHI-shaped input
- `pmat 3.15.0`: `analyze complexity --max-cognitive 25` over the whole repo
- `cargo tree` inverse queries for `garde` and `jsonschema`

### Secondary (MEDIUM confidence)
- `128-CHANGE-REQUEST.md` (rev 12) and `128-REVIEW-NOTES.md` — the phase's own prior measurement,
  re-checked here wherever it drove a decision (D-13's pin list: confirmed; the "0.57 misses U+3000"
  figure: re-measured and confirmed for 0.49.2's default engine)

### Tertiary (LOW confidence)
- None. No web search or external documentation was consulted; every finding is grounded in this
  tree or in a probe against its pinned dependencies.

# Metadata

**Confidence breakdown:**
- Standard stack: **HIGH** — no new dependency; every version read from `Cargo.lock` with checksums
- `jsonschema` 0.49 semantics: **HIGH** — probe compiled against the identical requirement and
  feature flags, output pasted verbatim
- `garde` 0.23 semantics: **HIGH** — same
- Architecture / seams: **HIGH** — every file:line opened this session, values quoted verbatim
- Release pin verification: **HIGH** — all 11 crates and all 11 pins read from the manifests;
  `release.yml`'s 24 steps enumerated
- Gate behaviour (`make lint`/`test-property`/`test-fuzz`/`unused-deps` blind spots): **HIGH** —
  read from the Makefile, with the Makefile's own recorded measurements as corroboration
- Doctest deprecation behaviour (A1): **LOW** — reasoned, not run
- ReDoS residual (A2): **MEDIUM** — falsification attempted and did not reproduce; not exhaustive

**Research date:** 2026-09-26
**Tree:** `44d50b3a`
**Valid until:** ~2026-10-26 for the tree facts; the `jsonschema`/`garde` measurements are pinned to
`0.49.2` / `0.23.0` and stay valid until those pins move — **re-measure Finding 1 if either does.**











