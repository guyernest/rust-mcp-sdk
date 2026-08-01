---
phase: 115-json-schema-2020-12-structured-output-caching-hints
verified: 2026-08-01T00:00:00Z
status: gaps_found
score: 3/4 must-haves verified
overrides_applied: 0
gaps:
  - truth: "SCHM-01: Schema validation runs Draft 2020-12 explicitly pinned, no `$schema` auto-detect (jsonschema 0.49)"
    status: failed
    reason: >-
      `normalize_schema_dialect` (src/server/output_validation.rs:146-165) rewrites only the
      document's ROOT `$schema`. Under JSON Schema 2020-12 a `$schema` on an embedded schema
      resource (any subschema that also carries `$id`) is legal and `jsonschema` 0.49.2 honours
      it, so a legacy dialect declaration on such a resource survives the pin unnormalized and
      resolves an empty vocabulary set there — the exact vacuous-validator bypass the pin exists
      to close, just moved one level down. Independently reproduced on this tree (not merely
      accepted from the code review): a schema whose root is draft-07 and whose one `$ref`'d
      `$defs` entry carries `$id` + a draft-07 `$schema` yields `(v1=Violates, v2=Conforms)` on an
      instance that violates `type: integer` — v2 is MEASURABLY WEAKER than v1, which both the
      module's rustdoc ("the pin wins UNCONDITIONALLY") and the phase's own contract invariant
      ("never honoured and never used as a vocabulary source") explicitly claim cannot happen.
      Three independent test/fuzz layers (unit `normalization_cases()`, `arb_schema_document()` in
      the property test, and `is_dialect_neutral`/`is_neutral_subschema` in the fuzz target) were
      also independently confirmed to structurally exclude the triggering shape, so a green gate
      and a 660k-run fuzz campaign never had a chance to catch this.
    artifacts:
      - path: "src/server/output_validation.rs"
        issue: "normalize_schema_dialect (:146-165) is root-only; the rustdoc at :26-28 and :142-145 states a safety property ('UNCONDITIONALLY', 'measured: a nested declaration does not trigger the bypass') that is false for the $id-bearing nested case, which was never measured"
      - path: "contracts/mcp-protocol-sdk-v1.yaml"
        issue: "output_schema_draft_pin invariant 1 ('a legacy declaration is normalized to the 2020-12 URI before compilation, never honoured and never used as a vocabulary source') is false as written for embedded schema resources"
      - path: "fuzz/fuzz_targets/fuzz_schema_draft_pin.rs"
        issue: "is_dialect_neutral/is_neutral_subschema (:182-230) treats ANY nested $schema as non-neutral (excluded from invariant 3) and $ref/$defs/$id are absent from DIALECT_NEUTRAL_KEYWORDS, so every document containing an embedded resource is skipped rather than checked"
      - path: "tests/property_tests.rs"
        issue: "arb_schema_document() (:868-893) injects a $schema at the root only and strips any other occurrence before generation, so the fuzzed space never contains the $id+nested-$schema shape"
    missing:
      - "Recursive normalization in normalize_schema_dialect: walk every object node, not just the root, and rewrite/strip $schema wherever it occurs, before handing the document to draft202012::new"
      - "A behavioural regression test asserting the measured (Violates, Conforms) case above becomes (Violates, Violates) after the fix"
      - "A fixed case in normalization_cases() with $id + nested $schema, asserted expected_owned == true"
      - "Widen arb_schema_document() to inject $id/$schema pairs on non-root nodes, and relax is_dialect_neutral (or add a second invariant) so the property test and fuzzer can reach embedded-resource shapes"
      - "Correct the '$schema pin wins UNCONDITIONALLY' / 'measured: a nested declaration does not trigger the bypass' rustdoc claims and the contract invariant text to state what is actually true post-fix"
---

# Phase 115: JSON Schema 2020-12 + Structured Output + Caching Hints Verification Report

**Phase Goal:** Adopt MCP spec `2026-07-28` era semantics for JSON Schema Draft 2020-12 output
validation (SCHM-01), non-object structured tool output (SCHM-02), and result caching hints
`ttlMs`/`cacheScope` (SCHM-03) — with the v1 (`2025-11-25`) wire behaviourally frozen.

**Verified:** 2026-08-01
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

A code review (`115-REVIEW.md`, completed 2026-08-01, `status: issues_found`, 1 blocker/10
warning/4 info) landed after the phase's own SUMMARY.md claims, the owner's blocking-gate sign-off
(115-10 Task 3, commit `496da96b`), and the `[x] Complete` booking of all three requirements in
`.planning/REQUIREMENTS.md`. **None of those three artifacts reflect the review's finding** — the
sign-off timestamp precedes the review by construction. This verification does not inherit either
the review's or the SUMMARYs' conclusion; every claim below was re-derived against the working
tree.

**The review's blocker (CR-01) was independently reproduced, not merely accepted.** A standalone
example was written against the crate's own `output_validation::fuzz_support::validate_bytes` seam
(`cargo run --example _verify_schm01_repro --features "fuzzing,validation"`, file deleted after
use — zero net change to the tree) and confirmed the exact three-row measurement the review
reports, including the regression row `root-draft07 + embedded (v1,v2) = Some((Violates,
Conforms))` — v2 accepting an instance v1 correctly rejects, on Draft 2020-12's own sanctioned
mechanism for a `$schema` below the root (an `$id`-bearing embedded schema resource). The three
generators the phase built to fence this exact class of bug (`normalization_cases()`,
`arb_schema_document()`, `is_dialect_neutral`) were also independently read and confirmed to
structurally exclude the triggering shape — this is not a coverage gap the phase could plausibly
have caught with the tests it wrote.

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | SCHM-01: Draft 2020-12 explicitly pinned, no `$schema` auto-detect, wasm-clean, SEP-2106-compliant | ✗ FAILED | Root-only normalization leaves embedded-schema-resource `$schema` declarations honoured; independently reproduced v2-weaker-than-v1 regression (see Gaps). Wasm-clean and SEP-2106 sub-claims hold (orchestrator-confirmed `cargo build --target wasm32-unknown-unknown --no-default-features --features "wasm,validation"` exit 0; `v2_schema_tripwires` manifest/graph tests pass) but do not rescue the pin claim itself |
| 2 | SCHM-02: v2 `structuredContent` accepts any JSON value (scalar/array/null/object); v1 keeps object-shaped behavior | ✓ VERIFIED | `cargo nextest run --features full -E 'binary(structured_tool_output)'` → 20/20 pass (re-run this session), covering scalar/array/null/string payloads across both native dispatchers with an in-band `resultType` era witness; `CallToolResult::structured_value` additive, `structured`'s signature unchanged (D-06) |
| 3 | SCHM-03: list/read results carry additive `ttlMs`/`cacheScope`, ensured on v2 and stripped on v1 at one chokepoint | ✓ VERIFIED | `binary(v2_caching_hints)` 19/19, `binary(v1_lists_golden)` 7/7, `binary(v2_schema_tripwires)` 13/13, `binary(v2_core_schema_facts)` 8/8, `binary(vendored_schema_provenance)` 6/6, `binary(phase115_contract_bindings)` 5/5 — all re-run this session, 78/78 pass, matching the counts REQUIREMENTS.md claims exactly. Latent (not currently firing) hazards noted below |
| 4 | v1 (`2025-11-25`) wire is behaviourally frozen | ✓ VERIFIED | `v1_lists_golden` byte-identical goldens pass with a leak guard proven to fire; `compile_for_era`'s `Era::V1` arm is `jsonschema::validator_for` verbatim (D-01), untouched by the CR-01 defect since the v1 code path never normalizes anything |

**Score:** 3/4 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/server/output_validation.rs` | Era-branched schema validation, v2 pinned to Draft 2020-12 | ✗ **FUNCTIONAL DEFECT** | Exists, substantive, wired into both native dispatchers — but `normalize_schema_dialect` does not implement the pin it documents; see CR-01 above |
| `src/types/caching.rs` (`project_caching_hints`, `CacheScope`, `DEFAULT_TTL_MS`) | Single projector for `ttlMs`/`cacheScope` | ✓ VERIFIED | `v2_schema_tripwires_caching_hints_are_written_in_exactly_one_place` passes; `object.remove(...)` on the v1 strip path is order-preserving today only because the two fields are declared last on all six structs — WARNING (WR-01), not currently firing |
| `src/server/core.rs`, `mod.rs`, `streamable_http_server.rs` (`inject_v2_result_envelope` call sites) | Every cacheable serialization site routed through one chokepoint | ✓ VERIFIED (native dispatchers) | `v2_schema_tripwires_every_envelope_call_site_names_its_cacheability`, `..._every_cacheable_serialization_site_routes_through_the_projector` pass. `WasmServerCore::handle_request`'s hand-built `tools/list` literal never calls the projector (WR-10) — no leak today (the value is a literal and carries no hint), but the "every dispatcher" claim in the wasm module's rustdoc overstates what is enforced there |
| `src/types/tools.rs` (`CallToolResult::structured_value`) | Additive non-object structured-content constructor | ✓ VERIFIED | 20/20 `structured_tool_output` tests pass; `structured_value` and `structured` are byte-identical bodies with no shared implementation (WR-08, cosmetic — a future edit to one is a silent divergence the D-06 freeze forbids, but not a current defect) |
| `contracts/mcp-protocol-sdk-v1.yaml` + `contracts/binding.yaml` + `tests/phase115_contract_bindings.rs` | Three contract equations, bindings, deterministic resolver | ✓ VERIFIED | `phase115_contract_bindings_every_implemented_binding_resolves_to_real_source` passes (re-run); `output_schema_draft_pin` invariant 1 is, however, a **false claim about the shipped code** (see CR-01) — the contract's existence is verified, its truth is not |
| `schema/vendored/core-2026-07-28/` + `PROVENANCE.md` | Digest-fenced published-core vendoring | ✓ VERIFIED | `vendored_schema_provenance` 6/6 pass. PROVENANCE.md's "nothing in the build reads them" claim is false — `src/types/caching.rs:474-475` `include_str!`s `schema.json` into the crate's own unit-test build (WR-07, documentation-accuracy only) |
| `examples/s52_v2_caching_hints.rs` | Runnable v2 demonstration | ⚠️ WIRED but non-conformant | Runs and exits 0 (orchestrator-confirmed), demonstrates `ttlMs`/`cacheScope` and scalar `structuredContent` correctly. Its `read()` handler emits `{"text": "..."}` with **no `uri`**, which the vendored `2026-07-28` schema's own `TextResourceContents` requires (WR-03) — the phase's headline copy-paste artifact is itself non-conformant on one call |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `warn_on_schema_mismatch` (dispatchers) | `schema_mismatch` → `cached_validator` → `compile_for_era` | era-threaded call | WIRED (mechanically) / **DEFECT (semantically)** | The call chain is correctly threaded and era-resolved; the function it calls implements the wrong normalization scope (root-only), so the wiring is sound but what it computes is not what SCHM-01 claims |
| `request_is_cacheable` | `inject_v2_result_envelope` | `Cacheable` claim per call site | WIRED | 5 production call sites verified by name in the review and confirmed passing here via `v2_schema_tripwires` |
| `inject_v2_result_envelope` | `project_caching_hints` | single chokepoint | WIRED | `..._caching_hints_are_written_in_exactly_one_place` passes |
| `CallToolResult::structured_value` | dispatcher serialization | `with_structured_content` | WIRED | `structured_tool_output` era-aware tests pass on both native dispatchers |
| `WasmServerCore::handle_request` (`tools/list`) | `project_caching_hints` | — | **NOT WIRED** | Hand-built `json!({"tools": tools})`, never reaches the projector — no leak today (literal value), but not "every dispatcher" as documented (WR-10) |

### Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| SCHM-01 | 115-01, 03, 08, 09, 10, 11 | Draft 2020-12 pin, no `$schema` auto-detect, wasm-clean, SEP-2106 | ✗ **BLOCKED** | Independently reproduced vacuous-validator bypass on embedded schema resources; contradicts requirement text as written |
| SCHM-02 | 115-01, 03, 04, 09, 10, 11 | v2 non-object `structuredContent`, v1 frozen | ✓ SATISFIED | 20/20 tests, live example output confirmed |
| SCHM-03 | 115-01, 02, 05, 06, 07, 08, 09, 10, 11 | Additive `ttlMs`/`cacheScope` on six cacheable results | ✓ SATISFIED | 47/47 tests across four dedicated binaries; two latent warnings (WR-01 order hazard, WR-10 wasm gap) not currently firing |

No orphaned requirements — `.planning/REQUIREMENTS.md`'s traceability table maps only SCHM-01/02/03
to Phase 115, and all eleven plans declare a subset of exactly those three IDs.

**Process finding, already owned (D-115-G in `deferred-items.md`, not re-reported as a gap):** all
three requirements were flipped to `[x] Complete` in wave 1 (`115-11`, contract-only, before any
runtime behaviour existed), then re-derived with measured evidence by `115-10`. That process defect
is closed by the phase's own ledger. It is noted here only because it is the same booking that
CR-01 shows to be substantively wrong for SCHM-01 — the re-derivation added evidence but did not
re-derive the one invariant that turned out to be false.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `src/server/output_validation.rs` | 26-28, 142-145 | Rustdoc asserts a safety property ("wins UNCONDITIONALLY", "measured: ... does not trigger the bypass") that is false for the un-measured `$id` case | 🛑 Blocker (documentation actively misleads, and the property it claims is what SCHM-01 requires) | See CR-01 |
| `src/types/caching.rs` | 248-251 | `Map::remove` is `swap_remove` under this repo's `preserve_order` feature; order-preserving only by accident of field-declaration order | ⚠️ Warning | Latent D-11 byte-identity hazard; not firing today |
| `src/types/resources.rs`, `prompts.rs`, `tools.rs`, `protocol/mod.rs` (6 sites) | various | `Option<u64>` `ttlMs` with no lenient/range-checked (de)serialization | ⚠️ Warning | A malformed peer `ttlMs` hard-fails the entire client-side result parse; not a phase-goal blocker (server-emission side is correct) |
| `examples/s52_v2_caching_hints.rs` | 134-137 | Non-conformant `resources/read` body (`{"text":...}`, missing required `uri`) | ⚠️ Warning | Copy-paste risk for server authors; example still runs and demonstrates the caching-hint behavior correctly |
| `src/server/traits.rs`, `wasm_server_tests.rs` | — | Never-compiled orphan files received blind edits (`ttl_ms: None, cache_scope: None`) with zero verification signal | ⚠️ Warning | Pre-existing orphan status (per project MEMORY.md); no debt markers found; not a new defect but absorbs future edits silently |
| (phase-touched production files) | — | `TBD`/`FIXME`/`XXX` scan | ℹ️ Info | Zero matches across all 15 phase-touched `src/`+`examples/` files — no debt-marker gate violation |

### Human Verification Required

None required to determine phase status — the blocking finding (CR-01) is deterministically
reproducible and was independently reproduced in this session, not left uncertain. Recommended for
the owner regardless, since the existing sign-off predates the review:

**Re-affirm or override the 115-10 sign-off in light of CR-01**

**Test:** Review the reproduction in `115-REVIEW.md` CR-01 and this report, then decide whether to
(a) accept a closure plan implementing the recursive-normalization fix, or (b) explicitly override
SCHM-01 with a documented rationale (e.g., accepting the residual risk given `outputSchema` is
warn-only and author-declared, not attacker-supplied).
**Expected:** A recorded decision, either a new plan or a `VERIFICATION.md` override entry.
**Why human:** The blocking-gate `checkpoint:human-verify` answered by the owner on 2026-08-01
(commit `496da96b`) was given before `115-REVIEW.md` existed, so it cannot be read as covering this
finding.

### Gaps Summary

One BLOCKER: **SCHM-01 is not actually achieved.** The phase's central claim — "Draft 2020-12
explicitly pinned ... no `$schema` auto-detect" — holds only for the document root. JSON Schema
2020-12 legally permits (and `jsonschema` 0.49.2 honours) a `$schema` declaration on any embedded
schema resource (a subschema with its own `$id`), and `normalize_schema_dialect` does not reach
those. This is not a hypothetical: the exact case the phase's own `115-RESEARCH.md` measured as
safe ("a nested `$schema` ... does NOT trigger it") was measured only for the `$id`-less shape,
which is indeed inert; the `$id`-bearing shape — the one 2020-12 actually sanctions and the one that
matters — was never measured, and it does trigger the original bypass. Independently reproduced this
session: `root-draft07 + embedded (v1,v2) = (Violates, Conforms)` — v2 is measurably weaker than v1
on the exact regression direction SCHM-01 was written to close.

SCHM-02 and SCHM-03 are genuinely achieved: both have dedicated, re-run, passing test suites whose
counts match what `.planning/REQUIREMENTS.md` claims exactly (20 + 78 tests across five binaries),
and their core chokepoints (single-projection, dispatcher coverage, era witnesses) were independently
confirmed, not merely trusted from the SUMMARYs. Several WARNING-level residuals exist on the
SCHM-03/caching-hints side (byte-order hazard, hard-parse surface, wasm dispatcher gap, a
non-conformant reference example) — all correctly classified as non-blocking by the code review and
independently confirmed as such here (none currently fires in the shipped test suite).

The fix for CR-01 is well-scoped (recursive normalization instead of root-only, plus widening the
three test/fuzz generators that structurally cannot reach the bug today) and does not require
re-opening SCHM-02/03. A closure plan for this single blocker should re-run the full
`make quality-gate` afterward since the fix touches `src/server/output_validation.rs` production
code.

---

_Verified: 2026-08-01_
_Verifier: Claude (gsd-verifier)_
