# PMCP SDK change request: secure-by-default input validation

> **Provenance.** Verbatim snapshot of Claude doc `9mvbYNAzFwviiwbaF2BkBb` at **rev 12**, taken
> 2026-09-26. Authored by Guy Ernest; reviewed and revised against the tree at commit `3b2d7baf`
> before this snapshot. The doc is the living source; this file is the phase's frozen input.
> Review corrections that are NOT in the doc are recorded in `128-REVIEW-NOTES.md`.

The config-driven server path (`pmcp-server-toolkit` / `pmcp-openapi-server`) should enforce each
tool's declared input schema and check Code Mode requests after path resolution, so every MCP server
team gets safe inputs without hand-writing validation, and keeps a small set of hooks for domain rules.

## The problem

Today the config path only *describes* input limits: it writes them into `inputSchema`, but no layer
enforces them at runtime, so each server team rebuilds validation and misses cases.

**The toolkit already documents this enforcement as existing — that is the sharper defect.** Three
places assert it against named threat IDs: `tools.rs:15-17` says unknown argument keys are rejected by
pmcp's request-validation path at `tools/call` time (T-83-05-02); `tools.rs:556-557` says arg injection
is bounded by the object-envelope schema, enforced upstream (T-90-03-01); `tools.rs:615` says args are
schema-validated before the script runs (T-90-05-03). None of it exists. `src/server/mod.rs:2590` and
`:2820` pass `req.arguments` straight to `handler.handle`, `src/server/core.rs` adds no check, and the
test carrying T-90-05-03 (`tests/script_tool.rs:185`) asserts argument binding, not validation. This
request therefore closes a gap between documented and actual behaviour rather than adding a feature:
the Phase 83 and Phase 90 threat sign-offs recorded mitigations that were never implemented.

Findings, against `pmcp` 2.20.x, `pmcp-server-toolkit` 0.1.3 and `pmcp-code-mode` 0.5.4:

| Area | What exists | Gap |
| --- | --- | --- |
| `TypedTool<T>` (`src/server/typed_tool.rs`) | Schema generated from `T` with `schemars`; arguments deserialized with serde | Field validators are never run. The `validation` feature pulls in `garde` and `jsonschema`, but nothing in `src/` calls `garde`. |
| `ParamDecl` (`pmcp-server-toolkit/src/config.rs`) | `max_length`, `minimum`, `maximum`, `enum`, copied into `inputSchema` by `build_param_property` | No runtime check. `additionalProperties: false` is emitted (`tools.rs:206`) but never honoured. No `pattern` or `min_length`. A string parameter with no `max_length` has no limit at all. |
| Code Mode HTTP (`code_mode.rs`, `resolve_path`) | Read-only methods only, approval tokens, host pinned by `join_url` | A custom `HttpExecutor` wrapper only sees the path *template*. `{key}` placeholders are filled from the body after the wrapper runs, and placeholder values aren't checked against the OpenAPI parameter schemas. |
| Single-call HTTP (`http/client.rs`, `substitute_path`) | `render_scalar` rejects objects and arrays in a path position | Any string substitutes verbatim — `path.replace(&placeholder, &value_str)`, no character check, no percent-encoding. The CR-01 class exists here too, with no Code Mode involved, and this is the default lightest build. |

**Evidence from a production-bound server (UMLS MCP, Triarii project).** The team had to hand-write a
`ValidatingTool` wrapper (a `jsonschema` check before every call) and an `OutboundGuard` around
`HttpCodeExecutor`. A code review then found two blockers, both confirmed by probes against wiremock:

- **Filter length cap missing (CR-02).** Three filter parameters had no `max_length`, so the
  hand-written enforcement didn't cover them. A 5,000-character `sabs` value and the free text
  `"Jane Doe DOB 1970-01-01 MRN 123"` in `semanticGroups` were both forwarded upstream.
- **Code Mode guard bypass (CR-01).**
  `api.get('/search/{a}{b}', {a:'2026AA?string=<180 chars>', b:'<200 chars>'})` passed the guard, whose
  path had no `?` and no long value. After `resolve_path` it became
  `/search/2026AA?string=<380 chars>`. The spec-declared `/content/{version}/CUI/{cui}` with
  `version = 'current/../../search/current?string=…'` reached a different endpoint the same way.

The platform's own Code Mode safety held throughout: a `POST` was refused before any request, and the
API key never left the server or the upstream host. The gap is in rules the platform doesn't know
about: parameter shapes, length caps and no free text in raw query strings. These matter to every
server that handles sensitive text (PHI in this case).

## Proposed built-in defaults

Four changes make the config path safe out of the box: a server that declares its parameters gets them
enforced, with no Rust code.

| # | Change | Where | Default |
| --- | --- | --- | --- |
| D1 | **Enforce the declared `inputSchema` at runtime** for every config-declared tool (HTTP single-call, script, SQL): one `jsonschema` check before any backend call. `additionalProperties: false` unless the config opts out. Errors name the rule and the parameter's JSON pointer, never the value or an undeclared argument name. Treat missing `arguments` (`null`) as `{}`. | `pmcp-server-toolkit` tool synthesizer | On |
| D2 | **Richer `ParamDecl`**: add `pattern`, `min_length`, `format`, and `items` (with `max_items`) for list-style parameters. Config validation rejects a `pattern` that doesn't compile under the runtime engine, and documents that engine's `\s`/`\S` coverage. | `config.rs`, `tools.rs` | n/a |
| D3 | **Default string cap**: a server-level `default_max_length` (suggested 256) applied to every string parameter without its own `max_length`. `ServerConfig::validate` warns on an uncapped string, and fails in a strict mode. | `[server]` config | On, 256 |
| D4 | **Code Mode request check after path resolution**, in `HttpCodeExecutor` — and the same checks on the single-call path in `HttpClient::substitute_path`, which has the identical hole with no Code Mode involved. (a) Refuse placeholder values containing `?`, `#`, `/`, `..`, or percent-encoded forms of these. (b) Validate each placeholder value against the matching path parameter in the OpenAPI spec (`pattern`, `maxLength`, `enum`). (c) Apply the default string cap to placeholders and remaining body values. Refusals use fixed messages with no path or value. | `pmcp-server-toolkit/src/code_mode.rs` + `src/http/client.rs` | On |

D4 closes the CR-01 class for every server, not just the one that found it: the guard sees the URL as
it will actually be sent, where today it sees the template the script wrote. It must cover **both**
HTTP surfaces. `HttpCodeExecutor::execute_request` calls `resolve_path` inside the impl, so a decorator
wrapping the public `HttpExecutor` trait (`pmcp-code-mode/src/executor.rs:2425`) is blind by
construction; `HttpClient::substitute_path` (`http/client.rs:150-163`) substitutes any string straight
into the path with no check. Scoping D4 to `code_mode.rs` alone would close the class for script and
Code Mode tools while leaving curated single-call tools — the default, JS-engine-free build — open to
the same injection.

D1 is the enforcement the UMLS team hand-wrote (`ValidatingTool`), moved upstream. D3 makes the CR-02
class, a forgotten `max_length`, fail safe instead of open.

## Escape hatches

Three extension points cover the domain rules a schema can't express, so no team has to wrap or fork
toolkit internals.

| # | Hook | What it gets | Typical use |
| --- | --- | --- | --- |
| E1 | **`RequestPolicy` trait**, registered on the server builder and run after D1/D4 on every outbound backend request (curated tools, script tools and Code Mode alike) | Tool name, HTTP method, the fully resolved path, the query pairs (before auth is added), the body. Returns allow, or refuse with a fixed message. Async, so it can keep per-session state. | PHI rules; a cap on free text spread across several calls in one session; endpoint allowlists; rate budgets |
| E2 | **Per-tool `ArgumentValidator`**, attachable by tool name from Rust | The parsed arguments, after D1 | Cross-field rules (e.g. `sabs` required when `searchType=exact`), code-list lookups, normalization |
| E3 | **`garde` on `TypedTool<T>`**: when `T: garde::Validate`, run `validate()` after deserializing and map errors the same value-free way as D1 | The typed struct | Teams that leave the config path for hand-written tools. This is the struct-with-field-validators model, applied the same way as config tools. |

**Where each rule belongs:** a rule on one parameter's shape goes in config (D2/D3). A rule on a
combination of values goes in E2. A rule about what may leave the server goes in E1. With E1 and E2,
the UMLS server would drop its hand-written `ValidatingTool` and `OutboundGuard` and keep only a short
PHI policy.

## Compatibility and rollout

Ship the defaults on, with explicit per-server opt-outs: existing servers that already send valid
arguments see no change, and any argument the new checks would reject was never checked before.

1. **Toolkit minor release** (0.2.0): D1–D4 and E1–E2 behind the toolkit's existing
   `input-validation` feature (`Cargo.toml:102`), forwarded from `default`; the Code Mode half of D4
   additionally needs `openapi-code-mode`. Do **not** gate the whole set on `openapi-code-mode` as
   first drafted — that umbrella pulls in the SWC/JS engine, so a curated single-call server would pay
   for a script engine it never uses, and the toolkit deliberately keeps those builds separate.
   `input-validation = ["dep:jsonschema"]` already exists with zero references in `src/`, a
   declared-but-never-implemented feature of the same shape as core's unused `garde` half, so
   implementing D1 there also retires the dead flag. The same PR corrects the three false enforcement
   claims in `tools.rs` (lines 15-17, 556-557, 615).
   - Opt-outs in config: `[server.validation] enforce_input_schema = false`, `default_max_length = 0`
     (off), `additional_properties = true`.
   - Every opt-out is logged once at startup, so it shows in deploy logs.
2. **`pmcp` minor release**: E3 (`garde` on `TypedTool`), gated on the existing `validation` feature.
   It's additive, since types that don't implement `Validate` behave as today.
3. **`cargo pmcp validate deploy`**: warn on uncapped string parameters and on any opt-out, so
   reviewers see it before a pmcp.run deploy.
4. **Docs**: one page on the layers (config shape, then E2, then E1), with the UMLS CR-01 and CR-02
   cases as worked examples of what each layer catches.

Risk: a server whose `enum` or `pattern` in config was loose and never enforced will start refusing
some calls. The startup log lists each enforced rule per tool, so such a regression is easy to trace.

## Acceptance tests

Each case runs against a wiremock upstream and asserts **zero upstream requests** on refusal, plus a
refusal message containing neither the value nor the path.

| Case | Input | Expected |
| --- | --- | --- |
| Uncapped filter (CR-02) | string param without `max_length`, value of 5,000 chars | Refused by D3 (cap 256) |
| Free text in a code-list param | `pattern` `^[A-Z0-9_]+(,[A-Z0-9_]+)*$`, value `Jane Doe DOB 1970-01-01` | Refused by D1/D2 |
| Query via placeholder (CR-01) | `api.get('/search/{v}', {v:'2026AA?string=x'})` | Refused by D4(a) |
| Length via two placeholders (CR-01) | `api.get('/search/{a}{b}', {a:'<180>', b:'<200>'})` where the result goes over the cap | Refused by D4(a)/(c) |
| Traversal through a spec path (CR-01) | `/content/{version}/CUI/{cui}`, `version='current/../../search/current?string=…'` | Refused by D4(a)/(b) |
| Query via a curated single-call param (CR-01, no Code Mode) | curated `[[tools]]` HTTP tool on `/content/{version}/CUI/{cui}`, `version='current?string=x'` | Refused by D4(a) inside `substitute_path` |
| Traversal via a curated single-call param (CR-01, no Code Mode) | same tool, `version='current/../../search/current'` | Refused by D4(a)/(b) inside `substitute_path` |
| Missing `arguments` | tools/call with no `arguments` on a zero-parameter tool | Accepted as `{}` |
| Missing `arguments` on a tool with required params | tools/call with no `arguments`, tool declares `required` | Refused by D1, not passed as `{}` to the backend |
| Undeclared argument | extra key `apiKey` | Refused by D1; error names the declared parameters, never the rejected key |
| Compliant call | valid args, placeholder matching the spec pattern | Exactly one upstream request, auth added server-side |
| Policy hook (E1) | policy refuses path prefix `/search` | Refused with the policy's fixed message, zero requests |
| `garde` (E3) | `#[garde(length(max = 10))]` field, 11 chars | `Error::Validation`, value not echoed |

The three Code Mode CR-01 probes come from the UMLS review; the two curated single-call probes are
their twins on the path that needs no JS engine. The team can share its probe crate, which links a
real config and spec, as a starting point.

## Open questions

- [ ] **Default cap value.** Is 256 right as the D3 default, or should it be required in strict mode
  with no default?
- [ ] **Placeholders.** Should D4(a) refuse `/` in placeholder values outright, or allow it where the
  OpenAPI spec marks a parameter `allowReserved`?
- [ ] **Schema engine version.** Should the runtime `jsonschema` version be pinned in the toolkit? Its
  `\s`/`\S` handling differs by version: 0.57 misses U+3000.
- [ ] **E1 timing.** Should E1 run before or after outgoing auth is applied? Before keeps credentials
  out of policy code, and is what's proposed here.
- [ ] **Platform.** Should pmcp.run's built-in `openapi-api` server adopt the same defaults, so
  platform and SDK servers behave alike?
- [ ] **Timeout budget.** Should D1–D4 also add a per-request total-time budget (retries × timeout)?
  Measured in-tree the defaults are worse than the review's ~47 s estimate: `timeout_seconds: 30`,
  `retries: 3`, `retry_backoff_ms: 1000` (`http/client.rs:43-51`) with
  `for attempt in 0..=max_retries` (`:290`) is four attempts plus 1+2+4 s of backoff, about **127 s**
  — 4.4× an API Gateway's 29 s limit, and a Code Mode script may make 50 such calls. It may belong in
  a separate request, but scope that request against 127 s.
