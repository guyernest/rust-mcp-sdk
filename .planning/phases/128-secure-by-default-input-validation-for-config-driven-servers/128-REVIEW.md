---
phase: 128-secure-by-default-input-validation-for-config-driven-servers
reviewed: 2026-09-27T00:00:00Z
depth: standard
files_reviewed: 54
files_reviewed_list:
  - src/server/schema_validation.rs
  - src/server/mod.rs
  - src/server/output_validation.rs
  - src/server/typed_tool.rs
  - src/server/validation.rs
  - crates/pmcp-server-toolkit/src/tools.rs
  - crates/pmcp-server-toolkit/src/policy.rs
  - crates/pmcp-server-toolkit/src/builder_ext.rs
  - crates/pmcp-server-toolkit/src/code_mode.rs
  - crates/pmcp-server-toolkit/src/config.rs
  - crates/pmcp-server-toolkit/src/error.rs
  - crates/pmcp-server-toolkit/src/http/client.rs
  - crates/pmcp-server-toolkit/src/http/schema.rs
  - crates/pmcp-server-toolkit/src/http/mod.rs
  - crates/pmcp-server-toolkit/src/lib.rs
  - crates/pmcp-code-mode/src/executor.rs
  - crates/pmcp-code-mode/src/code_executor.rs
  - crates/pmcp-code-mode/src/lib.rs
  - crates/pmcp-openapi-server/src/assemble.rs
  - crates/pmcp-openapi-server/src/dispatch.rs
  - crates/pmcp-openapi-server/src/lib.rs
  - cargo-pmcp/src/commands/validate.rs
  - cargo-pmcp/src/templates/workspace.rs
  - cargo-pmcp/src/templates/sql_server.rs
  - cargo-pmcp/src/templates/openapi_server.rs
  - cargo-pmcp/src/templates/workbook_server.rs
  - fuzz/fuzz_targets/fuzz_input_schema_enforcement.rs
  - fuzz/fuzz_targets/fuzz_placeholder_pattern_redos.rs
  - tests/schema_validation_props.rs
  - tests/root_dev_dep_path_only.rs
  - tests/typed_tool_garde.rs
  - tests/server_validation_deprecated.rs
  - tests/v2_schema_tripwires.rs
  - crates/pmcp-server-toolkit/tests/input_validation_acceptance.rs
  - crates/pmcp-server-toolkit/tests/curated_path_injection.rs
  - crates/pmcp-server-toolkit/tests/path_placeholder_props.rs
  - crates/pmcp-server-toolkit/tests/request_policy.rs
  - crates/pmcp-server-toolkit/tests/http_executor.rs
  - crates/pmcp-server-toolkit/tests/http_connector_props.rs
  - crates/pmcp-server-toolkit/tests/script_tool.rs
  - cargo-pmcp/tests/validate_server_config.rs
  - crates/pmcp-openapi-server/tests/contoso_m365_code_mode.rs
  - crates/pmcp-server-toolkit/examples/e05_input_validation.rs
  - examples/s57_typed_tool_garde_validation.rs
  - scripts/check-release-coverage.sh
  - .github/workflows/release.yml
  - .github/workflows/ci.yml
  - .github/workflows/fuzz.yml
  - Makefile
  - Cargo.toml
  - crates/pmcp-server-toolkit/Cargo.toml
  - crates/pmcp-code-mode/Cargo.toml
  - cargo-pmcp/Cargo.toml
  - server.json
findings:
  critical: 4
  warning: 11
  info: 0
  total: 15
status: issues_found
---

# Phase 128: Code Review Report

**Reviewed:** 2026-09-27
**Depth:** standard
**Files Reviewed:** 54
**Status:** issues_found

## Summary

The three enforcement layers are real and, in the narrow sense the phase set out to
prove, correct: `validate_input` never `Display`-formats a validation error,
`AdditionalProperties.unexpected` is read only through `.len()`, `safe_pointer`
genuinely projects onto the declared schema, the floor runs before any declared
pattern, `check_registered_validator` is called strictly after `check_schema`, and
the E1 hook sits between `join_url` and `auth.apply` on both HTTP surfaces. I found
no route by which a rejected argument VALUE reaches a client, and no new
`tracing` line that carries request data. SC-7's core property holds.

The defects are of two other kinds, and both are the phase's own signature class.

**First: the tightening breaks working shapes, silently and at runtime.** Three
independent hard refusals now fire on configurations that worked before and that
nothing detects at config time — the root path `/` and any trailing-slash path
(CR-01), the entire JSON request body of every curated `POST`/`PUT`/`PATCH` tool
(CR-02), and several ordinary operator-written query strings (WR-01). CR-02 is the
one I would fix first: `additionalProperties: false` was published-but-unenforced
before this phase, and the only route a curated mutating tool ever had to a request
body was through an undeclared argument. Enforcing the keyword closes that route
with no replacement and no test that would notice, because the only POST coverage in
the tree drives `HttpConnector::execute` directly, below the new decorator.

**Second: two comments assert data flows that the adjacent code contradicts, and
one security gate no longer sees the site it exists to guard.** `QUERY_BEARING_METHODS`
and `ParamPosition::Body` both say a `POST` tool's non-path inputs are payload
fields; `build_operation` marks every one of them `ParameterLocation::Query` and
they travel in the URL, so the D3 default length cap is absent exactly where the
comments say it is unnecessary (CR-03). And `v2_schema_tripwires`'s
validator-construction allowlist scans for `validator_for` / `draft202012`, while
the new `compile_input_2020_12` builds through `jsonschema::options()` — so the
gate whose failure message reads "Every site that builds a validator has to state
which dialect policy it is under" cannot see this phase's validator (CR-04).

Release mechanics check out. Every pin I traced matches its target's new version,
the four root dev-deps that must be path-only are, `pmcp` now precedes
`pmcp-code-mode` in `release.yml`, and `check-release-coverage.sh`'s new ordinal
assertion is boundary-correct (`( |$)`) and fails closed when a pattern does not
resolve. `pmcp-macros`'s pre-existing `pmcp = { version = ">=1.20.0", path = ".." }`
dev-dep still resolves against the published 2.20.4 and is unaffected by the
reorder.

---

## Critical Issues

### CR-01: `validate_resolved_path` refuses the root path `/`, so a `GET /` operation cannot be called at all

**File:** `src/server/schema_validation.rs:781-825` (`check_resolved_segments` /
`check_one_resolved_segment`), reached from
`crates/pmcp-server-toolkit/src/http/client.rs:532-546` and
`crates/pmcp-code-mode/src/executor.rs:2513-2524`

**Issue:** `check_one_resolved_segment` treats only *one* empty segment as
legitimate — index 0 of a path starting with `/`. Trace `path = "/"`:

- `decode_once` yields `[b'/']`; no byte is denied (`allow_slash = true`), no brace.
- `check_resolved_segments`: `leading_slash = true`; `[b'/'].split(|b| *b == b'/')`
  yields **two** empty slices (before and after the separator).
- index 0 → `leading == true` → `Ok`.
- index 1 → `segment.is_empty()`, `leading == (1 == 0 && true) == false` → `Err`.

So `validate_resolved_path("/")` refuses, and `substitute_path` calls
`check_composed_path` unconditionally (`client.rs:255`) on every request. An
OpenAPI spec that declares `paths: { "/": { get: … } }` — a root/health/index
endpoint, which is common — produces an `Operation` with `path == "/"` whose every
call now fails with the param-agnostic message `param 'path segment' must not be
empty`. The same applies to any path ending in `/`.

The trailing-slash case *is* acknowledged as deliberate (`ResolvedPath::from_checked`
rustdoc: "a dangling `/x?` is … the same class as a trailing `/`"), but the root
path is not mentioned anywhere and is almost certainly unintended: it is not a
doubled separator, it is the shortest legal absolute path. There is no coverage —
`grep` for `validate_resolved_path("/")` across `src/`, `crates/` and `tests/`
returns nothing — and `ServerConfig::validate`'s `check_path_template_segments`
accepts `"/"` happily (both segments are brace-free), so a config author gets no
config-time signal and discovers this on the first `tools/call`.

**Fix:** treat a decoded path that is exactly `/` as the one-segment absolute-root
case, and decide the trailing-slash question explicitly rather than by falling out
of the index arithmetic:

```rust
fn check_resolved_segments(decoded: &[u8]) -> Result<(), PlaceholderRefusal> {
    // The absolute root is a legal path with no segments at all; the split below
    // reports it as two empty segments and would refuse the second.
    if decoded == b"/" {
        return Ok(());
    }
    let leading_slash = decoded.first() == Some(&b'/');
    for (index, segment) in decoded.split(|byte| *byte == b'/').enumerate() {
        check_one_resolved_segment(segment, index == 0 && leading_slash)?;
    }
    Ok(())
}
```

Add both rows to the unit tests (`resolved_path_accepts_the_absolute_root`, and an
explicit assertion that a trailing `/` is refused *by decision*), and add a
`lint()` finding for a configured `path` whose composed form the runtime check will
refuse, so a trailing slash is reported before deploy rather than per request.

---

### CR-02: enforcing `additionalProperties: false` makes the JSON request body unreachable for every curated POST/PUT/PATCH tool

**File:** `crates/pmcp-server-toolkit/src/tools.rs:508` (schema),
`crates/pmcp-server-toolkit/src/tools.rs:1018-1029` (`build_operation`'s query
loop), `crates/pmcp-server-toolkit/src/http/client.rs:333-359` (`build_body`)

**Issue:** Three facts compose into a functional regression.

1. `build_operation` marks **every** non-path declared parameter
   `ParameterLocation::Query`, regardless of method (`tools.rs:1018-1029`; the
   rustdoc at `:988-994` states this divergence deliberately).
2. `build_body` returns a body only from args that are **not** in
   `operation.parameters`, plus the special `args.get("body")` key
   (`client.rs:337-359`).
3. `build_input_schema` emits `additionalProperties: false` by default
   (`tools.rs:508`) and Phase 128 is the first release that **enforces** it, via
   `ValidatingToolHandler::check_schema`.

Therefore, on a curated single-call `POST`/`PUT`/`PATCH` tool with the default
`[server.validation]`:

- every declared non-path parameter is appended to the **query string**, not the body;
- every *undeclared* argument — which is how a body was populated before this
  phase — is now refused by D1 before the handler runs;
- so `build_body` finds nothing and `has_request_body == true` sends an empty body.

The only surviving route to a body is to declare a parameter literally named
`body`, which `build_operation` also marks `Query`, so that value is sent **twice**:
once percent-encoded in the URL and once as the entire JSON payload (see WR-10).

Nothing catches this. The only POST coverage in the tree
(`client.rs:1385-1409`) constructs `Operation { parameters: vec![], … }` and calls
`HttpConnector::execute` directly, i.e. *below* the decorator, so it proves the
connector still works and says nothing about a synthesized tool.
`grep -rn 'method = "POST"' crates/pmcp-server-toolkit/tests` returns zero hits,
and `tests/input_validation_acceptance.rs` has no mutating-tool row.

This is a data-loss risk in the literal sense: a client sends a payload, the tool
accepts or refuses it, and no byte of it reaches the backend.

**Fix:** make body position a first-class parameter location rather than an
absence. `ParamDecl` already carries enough information, and `ToolDecl::param_position`
already computes `Body` for a mutating tool's non-path parameters — reuse it in
`build_operation` instead of deriving a second, contradictory answer:

```rust
// tools.rs::build_operation — remaining declared params
for p in &decl.parameters {
    if path_param_names.iter().any(|n| *n == p.name) { continue; }
    let location = match decl.param_position(&p.name) {
        ParamPosition::Query => ParameterLocation::Query,
        ParamPosition::Body  => ParameterLocation::Body,   // new variant
        ParamPosition::Path  => unreachable!("filtered above"),
    };
    parameters.push(Parameter::new(p.name.clone(), location, p.required)
        .with_rules(p.pattern.clone(), p.max_length, p.allow_slash));
}
```

and have `build_body` collect the `Body`-located declared parameters. If adding a
`ParameterLocation::Body` variant is out of scope for a patch release, the minimum
acceptable fix is a **hard config-time error**: refuse a `POST`/`PUT`/`PATCH`
`[[tools]]` entry that declares non-path parameters while `additional_properties`
is `false`, naming the shape and telling the author to set
`additional_properties = true` — so the breakage is discovered at boot rather than
by a silently-empty backend request. Either way, add an end-to-end acceptance row
that drives a synthesized POST tool through `handle` and asserts the wiremock
server observed the expected JSON body.

---

### CR-03: the D3 default length cap is absent exactly where the value travels in the query string, and two comments assert the opposite data flow

**File:** `crates/pmcp-server-toolkit/src/config.rs:1713-1724` (`param_position`,
`QUERY_BEARING_METHODS`), `crates/pmcp-server-toolkit/src/config.rs:1759-1765`
(`ParamPosition::Body`), against
`crates/pmcp-server-toolkit/src/tools.rs:1023-1029` (`build_operation`)

**Issue:** `QUERY_BEARING_METHODS` is `["GET", "HEAD", "DELETE"]`, documented as
"HTTP methods whose non-path inputs **genuinely travel in the query string**, and
where an unbounded value is therefore dangerous rather than merely large"
(`config.rs:1722-1724`). `ParamPosition::Body` is documented as "a `POST`/`PUT`/`PATCH`
**payload field** … NOT capped by default (D-05 — free text must keep working)"
(`config.rs:1759-1765`).

Both statements are false for this surface. `build_operation` gives every non-path
declared parameter `ParameterLocation::Query` for *every* method, and
`HttpClient::build_query` (`client.rs:290-307`) builds the wire query from
`operation.query_parameters()` unconditionally. So a `POST` tool's non-path
parameters travel in the URL — and, per CR-02, **only** in the URL. The D3 default
`maxLength` is therefore withheld from precisely the parameters that land in a
query string, for the stated reason that they are payload fields, which they are not.

`build_operation`'s own rustdoc (`tools.rs:988-994`) sees half of this — it says
the two functions "answer different questions — where a value TRAVELS versus where
LENGTH is dangerous" and instructs "Do not 'reconcile' them" — but the rationale
does not survive the fact that a `POST` non-path value travels in the URL. Where a
value travels *is* where length is dangerous: an unbounded query value means an
unbounded request line, which is a 414 on some gateways, a truncation on others, and
an access-log amplification everywhere. The consequence is an uncapped
`inputSchema` string in a query position, which is the one case the D3 cap exists
for.

This is the eighth-to-ninth claim class the phase intent asks for: a comment that
asserts an enforcement boundary the enforcing function does not implement.

**Fix:** reconcile in the direction the data actually flows. If CR-02 is fixed by
introducing `ParameterLocation::Body`, `param_position` and `build_operation` agree
by construction and the cap lands correctly. If not, `QUERY_BEARING_METHODS` must
be deleted and `param_position` must return `Query` for every non-path parameter of
a tool that has a `path`/`method` pair, with the D-05 free-text concern handled by
the *existing* per-parameter `max_length` escape rather than by a
method-conditional. Either way, rewrite both doc comments to state the measured
flow, and make `render_param_rules`' startup line say `Query` for a POST tool's
parameters so the enforcement report does not report a position the request does
not use.

---

### CR-04: the validator-construction dialect gate cannot see this phase's new validator

**File:** `tests/v2_schema_tripwires.rs:130` (`VALIDATOR_NEEDLES`),
`tests/v2_schema_tripwires.rs:1099-1131` (`VALIDATOR_SITES`), against
`src/server/schema_validation.rs:373-381` (`compile_input_2020_12`)

**Issue:** `v2_schema_tripwires_validator_construction_sites_are_accounted_for`
exists so that "Every site that builds a `jsonschema` validator has to state which
dialect policy it is under: pinned to 2020-12 (v2), frozen at auto-detection (v1),
or out of scope with a written reason" — quoting its own failure message
(`:1167-1172`). Its population is whatever matches `VALIDATOR_NEEDLES`, which is
`["validator_for", "draft202012"]`.

This phase's new compile site uses neither idiom:

```rust
// src/server/schema_validation.rs:377
jsonschema::options()
    .with_draft(jsonschema::Draft::Draft202012)
    .should_validate_formats(true)
    .build(&normalized)
```

A workspace scan confirms it is the only validator construction outside the
allowlisted three (`grep -rn "jsonschema::options()\|validator_for(\|draft202012::"`
over `src/` and `crates/*/src/` yields exactly `output_validation.rs:602`,
`output_validation.rs:621`, `pmcp-agent/.../decide.rs:218` and
`schema_validation.rs:377`). `VALIDATOR_SITES` has no entry for it, and because the
needles do not match, the test passes — the site is invisible rather than
unaccounted-for.

The consequence is concrete: `.with_draft(jsonschema::Draft::Draft202012)` is the
only thing pinning D-02 ("Compiled under Draft 2020-12 regardless of protocol
era"). Delete that one line and the builder falls back to `jsonschema`'s default
draft resolution, inputs silently start honouring a declared `$schema` on any
dialect `first_legacy_dialect` does not rewrite, and **no test in the repository
fires** — not this tripwire (wrong needle), not the unit tests (they assert
outcomes under 2020-12-compatible schemas), not the fuzz target (its oracle is
provenance-based, not dialect-based). Phase 128 removed a
documented-but-unenforced claim from `tools.rs` and installed a new one in the
gate that was supposed to prevent exactly that.

**Fix:** add the construction idiom to the needles and register the site:

```rust
// tests/v2_schema_tripwires.rs
const VALIDATOR_NEEDLES: &[&str] = &["validator_for", "draft202012", "jsonschema::options"];

// … and in VALIDATOR_SITES:
ValidatorSite {
    file: "src/server/schema_validation.rs",
    function: "compile_input_2020_12",
    hits: 1,
    disposition: ValidatorDisposition::PinnedByPolicy,
    why: "Phase 128 D-02: inputs pin Draft 2020-12 on BOTH protocol eras, on a \
          document normalized by `normalize_schema_dialect` first, and additionally \
          turn format ASSERTION on (Q1). `Era` is deliberately not a parameter.",
},
```

Verify the added needle does not pick up the existing two sites twice (it should
not — neither `output_validation.rs` site uses `options()`), and confirm the
`hits: 1` count fails if a second `options()` chain is added inside
`compile_input_2020_12`.

---

## Warnings

### WR-01: the `?`-split applies path-segment rules to the query portion, refusing ordinary operator-written query strings at runtime

**File:** `crates/pmcp-server-toolkit/src/http/client.rs:532-546`
(`check_composed_path`), `crates/pmcp-code-mode/src/executor.rs:2513-2524`
(`ResolvedPath::from_checked`)

**Issue:** Both sites pass the query portion through the *unmodified*
`validate_resolved_path`, and the rustdoc presents that as pure upside ("a second
`?`, a fragment marker, traversal on either side, an over-cap query and an empty
query portion all stay refused for free"). What neither doc states is that the
**path**-segment rules also apply to a string that is not a path. Working through
`validate_resolved_path` on realistic query portions:

| configured `path` | query portion checked | verdict | why |
| --- | --- | --- | --- |
| `/r?cb=https%3A%2F%2Fex.com` | `cb=https://ex.com` (after decode-once) | **refused** | `split('/')` yields `["cb=https:", "", "ex.com"]` → empty interior segment |
| `/r?pct=100%25` | — | **refused** | `%25` is rejected before decoding |
| `/r?f=a/./b` | `f=a/./b` | **refused** | segment `.` → "must not be a relative path reference" |
| `/r?f=a//b` | `f=a//b` | **refused** | empty interior segment |
| `/r?tag=<260 chars>` | | **refused** | `segmentMaxLength` |

None of these is an injection: they all come from the operator-authored
`[[tools]] path` or a `PathPart::Literal`, which is precisely the provenance the
`?` narrowing exists to trust. They fail per-request, with the param-agnostic
message `param 'path segment' …`, and `ServerConfig::validate` does not run the
composed check, so there is no config-time signal. Test coverage is the single
happy case `?string=x`
(`crates/pmcp-server-toolkit/tests/curated_path_injection.rs:355`).

**Fix:** check the query portion against a query-appropriate rule set rather than
the path rule set. The properties that matter there are: no second `?`, no `#`, no
backslash, no control byte, no residual `{`/`}`, and a total length cap. Traversal
and empty-segment rules are meaningless in a query and should not run. Add a sibling
in core next to `validate_resolved_path`:

```rust
/// The composed-QUERY half of the `?` narrowing: the byte denylist and the cap,
/// without the path-segment rules (a `/` or a `.` in a query VALUE is ordinary).
pub fn validate_resolved_query(query: &str) -> Result<(), PlaceholderRefusal> { … }
```

and call it from both `check_composed_path` and `ResolvedPath::from_checked`. Keep
the empty-query refusal if that is the decision, but state it in the rustdoc as a
decision rather than as a by-product. Add the five rows above as tests.

---

### WR-02: `HttpClient::from_config` builds a redirect-FOLLOWING client, re-opening the T-128-39a bypass inside the toolkit

**File:** `crates/pmcp-server-toolkit/src/http/client.rs:137-160`

**Issue:** `dispatch.rs:147-153` now builds the shared client with
`reqwest::redirect::Policy::none()` and documents why: every hop after the first
happens inside `reqwest`, after the E1 hook ran, so a followed redirect is an
outbound request no policy ever saw. `RequestPolicy`'s own rustdoc
(`policy.rs:205-211`) says "A client built elsewhere with reqwest's default
redirect policy re-opens that gap."

`HttpClient::from_config` is that client, in the same crate:

```rust
let client = reqwest::Client::builder()
    .timeout(Duration::from_secs(http_config.timeout_seconds))
    .default_headers(headers)
    .build()
```

No `.redirect(…)`, so reqwest's default (follow up to 10 hops) applies. It has
**zero callers** anywhere in the workspace — a `grep` for
`HttpClient::from_config` returns only its own definition — so it is simultaneously
dead code and the one first-party constructor that ships the bypass. Anyone reading
the phase's redirect story would reasonably reach for the `from_config` constructor
(it is the one that honours `[backend.http]`) and get the un-hardened behaviour.

**Fix:** add `.redirect(reqwest::redirect::Policy::none())` to the builder in
`from_config`, with the same `// Why:` comment `dispatch.rs` carries, and add a
`from_config`-driven variant of
`dispatch::tests::the_dispatched_client_does_not_follow_a_redirect`. Separately,
decide whether `from_config` should be deleted or wired in — `dispatch.rs:165`
calls `HttpClient::new`, which means `[backend.http]`'s `timeout_seconds`,
`retries`, `user_agent` and `default_headers` are silently ignored by the OpenAPI
binary today (pre-existing, but worth booking now that the constructor's security
posture is in scope).

---

### WR-03: layer-1 `${var}` interpolation has no narrowing/widening seam and treats a query-position value as a path placeholder

**File:** `crates/pmcp-code-mode/src/executor.rs:3509-3541` (`resolve_path`),
`crates/pmcp-code-mode/src/executor.rs:2938-2941`
(`floor_layer_one_contribution`)

**Issue:** Layer 2 gets `HttpExecutor::placeholder_rules`, so an implementor can
supply a spec-declared `pattern`/`maxLength` and, via config, `allow_slash`. Layer 1
is hard-wired to `PlaceholderRules::default()` with no seam at all. Two consequences
the rustdoc does not state:

1. **A script that composes a path prefix from a variable is now refused.**
   `` api.get(`${base}/users`) `` with `base = "/api/v1"` hits
   `characterFloor` ("must not contain a path separator"). There is no opt-in;
   `allow_slash` is unreachable on this route. Given that Code Mode scripts
   routinely build paths from variables (a paginated `next` path, a
   config-supplied stage prefix), this is a behavioural break with no migration.
2. **A query-position interpolation is floored as if it were a path segment.**
   `` api.get(`/search?path=${p}`) `` with `p = "a/b"` is refused for containing a
   `/` — inside a query value, where `/` is ordinary. `resolve_path` walks
   `PathTemplate.parts` with no notion of whether the preceding
   `PathPart::Literal` already emitted a `?`, so it cannot distinguish the two
   positions.

Neither is a security hole — both fail closed — but both are unstated refusals of
legitimate input, which is how a floor gets weakened later by someone who only sees
the false positives.

**Fix:** track whether a `?` has been emitted while walking `path.parts`, and once
it has, floor subsequent dynamic contributions with the query rule set proposed in
WR-01 rather than the path floor:

```rust
let mut in_query = false;
for (index, part) in path.parts.iter().enumerate() {
    match part {
        PathPart::Literal(s) => { in_query |= s.contains('?'); result.push_str(s); },
        PathPart::Variable(var) => {
            let rendered = …;
            if in_query { floor_query_contribution(var, &rendered)? }
            else        { floor_layer_one_contribution(var, &rendered)? }
            result.push_str(&rendered);
        },
        …
    }
}
```

For (1), add an explicit `HttpExecutor` seam for layer-1 rules (mirroring
`placeholder_rules`) or document in the crate CHANGELOG that a layer-1 `${var}`
may not contain `/`, so the break is discoverable before a deploy.

---

### WR-04: the layer-1 refusal echoes a caller-authored variable identifier, and its justifying comment is contradicted 15 lines above it

**File:** `crates/pmcp-code-mode/src/executor.rs:3514-3526`

**Issue:** The `PathPart::Variable` arm interpolates the script's variable
identifier into a client-visible refusal, justified inline by:

```rust
// The refusal names the variable IDENTIFIER, which the part
// carries. A script-chosen identifier is operator-shipped
// content, unlike the value.
floor_layer_one_contribution(var, &rendered)?;
```

The same function's rustdoc, fifteen lines earlier, says the opposite:

> This arm used to push the stringified value straight in … **Code Mode scripts are
> model-authored, so that was the untrusted route.**

Both cannot be true. For a *script tool* the script is operator-shipped
(`[[tools]] script`), and the comment holds. For the generic `execute_code` tool the
script arrives in the `tools/call` arguments, so the identifier is
attacker-chosen — and it is echoed verbatim into `ExecutionError::RuntimeError`'s
message, which reaches the client. Contrast the `PathPart::Expression` arm directly
below, which is careful to use a fixed positional descriptor for exactly this
reason.

The confidentiality impact is low (the client authored the identifier, so it learns
nothing new) and identifiers cannot carry spaces, which is why I am filing this as a
WARNING rather than a BLOCKER. But SC-7 is stated as "must never contain an
attacker-supplied argument KEY", and this is an attacker-supplied name reaching a
refusal on the surface the same function calls untrusted. The `garde`
identifier-shape residual is already documented and accepted; this is a *second*
instance of that class, on a different surface, and it is not recorded.

**Fix:** either use a fixed positional descriptor for the `Variable` arm too
(`format!("path variable #{index}")`, matching the `Expression` arm), or — if the
identifier's diagnostic value is worth keeping — narrow the comment to state the
provenance split honestly and record the `execute_code` case as an accepted
residual next to the `garde` one:

```rust
// The refusal names the variable IDENTIFIER. For a `[[tools]] script` that is
// operator-shipped text. For the generic `execute_code` tool the SCRIPT — and
// therefore this identifier — is caller-supplied; it is echoed anyway because an
// identifier cannot carry whitespace and the caller authored it. Same accepted
// residual as `render_garde_refusal`'s bare-identifier map key. The VALUE is
// never echoed on either surface.
```

---

### WR-05: pass 2 of both placeholder substitutions rewrites a mutating string, defeating the documented "cannot manufacture a placeholder" guarantee

**File:** `crates/pmcp-code-mode/src/executor.rs:3017-3022` (layer 2),
`crates/pmcp-server-toolkit/src/http/client.rs:251-255` (curated)

**Issue:** `resolve_layer_two_placeholders`' rustdoc states:

> Pass one also tests containment against the ORIGINAL template rather than a
> progressively substituted copy, so a value that itself contains `{`/`}` cannot
> manufacture a placeholder for a later key to fill.

Pass one does test against the original template. Pass **two** does not apply
against it:

```rust
let mut resolved = template.to_string();
for (placeholder, rendered) in substitutions {
    resolved = resolved.replace(&placeholder, &rendered);
}
```

With `template = "/x/{a}/{b}"`, `a = "{b}"` and `b = "v"`: pass 1 accepts both
(`{`/`}` are not in the floor's denylist), pass 2 replaces `{a}` → `{b}` giving
`/x/{b}/{b}`, and the *next* iteration replaces both occurrences → `/x/v/v`. `b`'s
value has been spliced into `a`'s position. The guarantee as written does not hold;
what pass 1 buys is only that a value cannot introduce a key that is *not already*
a substitution.

`HttpClient::substitute_path` has the identical shape and additionally makes the
outcome depend on `operation.path_parameters()` iteration order: with `b` before
`a`, the residual `{b}` survives and `check_composed_path` refuses the residual
brace instead — so the same arguments produce a success or a refusal depending on
declaration order.

Impact is bounded — the spliced value was itself floored, and the composed check
still runs — so this is a WARNING, not a BLOCKER. But a security rustdoc that
overstates its guarantee is the class this phase exists to remove.

**Fix:** make pass 2 a single left-to-right scan over the original template rather
than N sequential `replace` calls, so no substituted text is ever re-scanned:

```rust
fn apply_substitutions(template: &str, subs: &[(String, String)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    'outer: while !rest.is_empty() {
        for (placeholder, rendered) in subs {
            if let Some(tail) = rest.strip_prefix(placeholder.as_str()) {
                out.push_str(rendered);
                rest = tail;
                continue 'outer;
            }
        }
        let ch = rest.chars().next().expect("non-empty");
        out.push(ch);
        rest = &rest[ch.len_utf8()..];
    }
    out
}
```

Apply it at both sites, and add the `a = "{b}"` case as a test asserting the
residual `{b}` is preserved and then refused by the composed check, in both
parameter orders.

---

### WR-06: `OutboundRequest::path` carries the author-written query string, contradicting its documented contract

**File:** `crates/pmcp-server-toolkit/src/policy.rs:81-101` (the `path` and `query`
field docs), against `crates/pmcp-server-toolkit/src/http/client.rs:655-664` and
`crates/pmcp-server-toolkit/src/code_mode.rs:1330-1336`

**Issue:** `OutboundRequest::path` is documented as

> The FULLY RESOLVED request target: every path placeholder substituted and the
> configured base URL already joined on, **with no query string appended**. … The
> query pairs are carried separately in `Self::query`.

and `query` as "The query pairs that will be appended to `Self::path`". Both HTTP
surfaces violate this whenever the `?` narrowing fires:

- curated: `run_request_policy(tool, method, &joined, …)` where
  `joined = join_url(base_url, substituted)` and `substituted` may contain an
  operator-written `?…` — that is the whole point of `check_composed_path`'s split;
- Code Mode: `run_request_policy(&upper, &url, &query_params, …)` where
  `url = join_url(base_url, resolved_path)` and `ResolvedPath` explicitly permits
  one author-written `?`.

So a template-embedded query pair appears inside `path` and is **absent** from
`query`. A policy written to the documented contract — e.g. one that scans `query`
for a forbidden parameter such as `include_pii=true`, or one that exact-matches
`path` against an allowlist — has a blind spot the contract told it did not exist.
Caller-supplied data is still fully visible (path values in `path`, query values in
`query`), which is why this is a WARNING rather than a BLOCKER.

**Fix:** split once at the hook and populate the two fields as documented, so the
contract becomes true rather than aspirational:

```rust
let (target, embedded) = joined.split_once('?').map_or((joined.as_str(), ""), |(p, q)| (p, q));
let mut policy_query: Vec<(String, String)> = embedded
    .split('&')
    .filter(|pair| !pair.is_empty())
    .map(|pair| pair.split_once('=').map_or((pair.to_string(), String::new()),
        |(k, v)| (k.to_string(), v.to_string())))
    .collect();
policy_query.extend(query.iter().map(|(k, v)| (k.clone(), v.clone())));
policy_query.sort();
self.run_request_policy(tool, &method, target, &policy_query, body).await?;
```

Do it on both surfaces, and add a `tests/request_policy.rs` row with an
author-written query in the configured `path` asserting the policy sees it in
`req.query` and `req.path` with no `?`.

---

### WR-07: `Makefile`'s `test-server-toolkit` comment states a default feature set this phase changed

**File:** `Makefile:576-593`

**Issue:** The new comment block justifying `--features http,input-validation`
reads:

> `input-validation` is REQUIRED for the same MEASURED reason `http` already is …
> the toolkit's `default` is still `["code-mode"]`, so without naming the feature
> here the whole file … compiles to `running 0 tests` and exits 0.

The same commit changed `crates/pmcp-server-toolkit/Cargo.toml` to
`default = ["code-mode", "input-validation"]`. The stated reason for the flag no
longer holds — `cargo test -p pmcp-server-toolkit --features http` would now build
the acceptance file, because defaults are additive. The flag is still worth keeping
(it makes the leg independent of the default set, and would survive a future
default change), but the comment now documents a manifest state that does not
exist, which invites a later reader either to delete the flag or to mistrust the
whole block.

**Fix:** replace the measured claim with the current one:

```make
# `input-validation` is named EXPLICITLY even though Phase 128 put it in the
# toolkit's `default` set, so this leg does not depend on that default staying
# put: `tests/input_validation_acceptance.rs` is
# `#![cfg(all(feature = "http", feature = "input-validation"))]` and would compile
# to `running 0 tests` — exit 0 — the moment the default changed. The
# REQUIRED_TEST_BINARIES loop below is the paired guard.
```

---

### WR-08: `cargo pmcp validate config` prints a green check for a config with active opt-outs, on a different stream from the findings

**File:** `cargo-pmcp/src/commands/validate.rs:769-776` (`emit_config_lint_findings`),
`cargo-pmcp/src/commands/validate.rs:800-819` (the summary block)

**Issue:** Three small things compose into a summary that can read as a pass:

1. Findings go to **stderr** (`eprintln!`), while the summary line goes to
   **stdout** (`println!`). A CI step that captures only stdout sees
   `✓ Server config valid (3 input-validation findings)` and no findings.
2. The summary is a green `✓` in both branches. With
   `enforce_input_schema = false` the config *is* structurally valid, so the
   `✓` is technically accurate — but the phase's own prohibition is "an
   enforcement that is off must never read as on", and a green check plus a count
   is exactly how a reader concludes nothing is wrong.
3. Under `PMCP_QUIET` (`not_quiet == false`) **nothing at all** is printed — not
   the banner, not the findings — and the command exits 0. So the quiet mode of the
   pre-deploy gate reports an unenforced server as silently fine.

**Fix:** write findings and summary to the same stream, and make the marker reflect
whether any finding is an *opt-out* (as opposed to an advisory `uncapped-string`):

```rust
let opt_outs = parsed.lint().iter()
    .filter(|f| f.rule.starts_with("opt-out-"))
    .count();
let marker = if opt_outs > 0 { style("!").yellow() } else { style("✓").green() };
println!("  {marker} Server config valid — {} finding(s), {opt_outs} ACTIVE enforcement opt-out(s)", findings.len());
```

and emit the opt-out lines even under `PMCP_QUIET` (quiet should suppress
decoration, not a security finding), or document explicitly that `PMCP_QUIET`
suppresses them.

---

### WR-09: a `[[tools]]` path that combines the new `?` narrowing with a placeholder in the query is a hard config error with an unrelated message

**File:** `crates/pmcp-server-toolkit/src/config.rs:764-798`
(`check_path_template_segments` / `is_supported_path_segment`)

**Issue:** The phase advertises operator-written query strings in a `[[tools]]`
`path` as a supported shape (`substitute_path`'s "The one narrowing" section,
`lint_against_spec`'s query-stripping, and the
`curated_path_injection_accepts_an_author_written_query_string_in_the_path` test).
A config author following that will reasonably try to put a placeholder in the
query:

```toml
path = "/content/{version}/CUI?string={query}"
```

`check_path_template_segments` splits on `/` and evaluates the last segment,
`CUI?string={query}`. It contains `{`, does not start with `{`, so
`is_supported_path_segment` returns `false` and `validate()` fails with
`MalformedPathTemplateSegment { segment: "CUI?string={query}" }` — an error about a
malformed *path* template, on a segment the author thinks of as a query. The correct
answer (declare `query` as a query parameter and drop it from `path`) is not in the
message.

Failing closed is right; the diagnosis is not.

**Fix:** detect the case and say so. In `check_path_template_segments`, strip an
author-written query before the segment walk and, if the stripped query contains a
`{`, return a dedicated error:

```rust
let (template, query) = path.split_once('?').map_or((path, ""), |(p, q)| (p, q));
if query.contains('{') {
    return Err(ConfigValidationError::PlaceholderInPathQueryString {
        tool: tool.name.clone(),
        detail: "a `{name}` placeholder in the `path`'s query string is not substituted; \
                 declare it as a [[tools.parameters]] entry instead and remove it from `path`"
            .to_string(),
    });
}
for segment in template.split('/') { … }
```

---

### WR-10: a declared parameter named `body` is sent twice — once in the query string and once as the whole JSON payload

**File:** `crates/pmcp-server-toolkit/src/http/client.rs:337-339` against
`crates/pmcp-server-toolkit/src/tools.rs:1023-1029`

**Issue:** `build_body` short-circuits on `args.get("body")` and returns that value
as the entire request body. `build_operation` has already registered every non-path
declared parameter — including one named `body` — as `ParameterLocation::Query`, so
`build_query` also picks it up and `query_pairs_mut().append_pair` appends it to the
URL. The same value therefore travels twice, in two encodings.

This is pre-existing, but Phase 128 promotes it from a corner case to the *only*
remaining way for a curated `POST` tool to send a body (see CR-02), so it is now on
the happy path rather than off it. If the value is PHI it is now also in the access
log's request line.

**Fix:** exclude `body` from the query build when `has_request_body` is set, and
document the reserved name:

```rust
fn build_query(operation: &Operation, args: &Map<String, Value>) -> Result<…> {
    let mut query = HashMap::new();
    for param in operation.query_parameters() {
        // `body` is the reserved whole-payload key consumed by `build_body`; it must
        // not ALSO be appended to the URL.
        if operation.has_request_body && param.name == "body" { continue; }
        …
    }
}
```

Better still, fold this into CR-02's `ParameterLocation::Body` fix and drop the
magic `"body"` key entirely.

---

### WR-11: the enforcement report picks its severity by comparing a display string to `"ON"`

**File:** `crates/pmcp-server-toolkit/src/policy.rs:589-612`

**Issue:** `render_validation_report` computes a human-readable `&str`
(`"ON"` / `"OFF"` / `"OFF (feature)"`) and then decides the log level by comparing
it back:

```rust
level: if schema_check == "ON" { ReportLevel::Info } else { ReportLevel::Warn },
```

A change to the display text in one place — `"on"`, `"ENABLED"`, `"ON (default)"` —
silently flips every enforced server's startup line from `Info` to `Warn`, or (with
the comparison edited instead) turns an OFF state into `Info`. In a function whose
whole purpose is "an enforcement that is off must never read as on", the severity
should not be derived from presentation.

**Fix:** decide the state once, then render from it:

```rust
enum SchemaCheck { On, OffByConfig, OffByFeature }

let state = if !cfg!(feature = "input-validation") { SchemaCheck::OffByFeature }
            else if report.enforce_input_schema   { SchemaCheck::On }
            else                                  { SchemaCheck::OffByConfig };
let level = match state { SchemaCheck::On => ReportLevel::Info, _ => ReportLevel::Warn };
let rendered = match state {
    SchemaCheck::On => "ON",
    SchemaCheck::OffByConfig => "OFF",
    SchemaCheck::OffByFeature => "OFF (feature)",
};
```

Add a test that asserts the `(state, level)` pairing directly rather than through
the rendered text.

---

_Reviewed: 2026-09-27_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
