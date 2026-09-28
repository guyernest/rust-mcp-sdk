---
phase: "128"
slug: "secure-by-default-input-validation-for-config-driven-servers"
status: verified
# threats_open = count of OPEN threats at or above workflow.security_block_on severity (the blocking gate)
threats_open: 0
asvs_level: 1
created: "2026-09-28"
---

# Phase 128 — Security

> Per-phase security contract: threat register, accepted risks, and audit trail.

Register origin: `register_authored_at_plan_time: true` — all eleven PLAN.md files carry a
parseable `<threat_model>` block. The audit therefore VERIFIED declared mitigations; it did not
scan for new threats.

**Register size correction.** An earlier pass of this audit counted **74** rows. The register is
**81**: `128-11-PLAN.md` contributes `T-128-50` … `T-128-56` (release mechanics), which a
truncated extraction had cut. Recorded because an undercounted register is the same
documented-but-absent class this phase exists to close, one layer up.

---

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| MCP client → `tools/call` arguments | Untrusted JSON. Before this phase it reached `handler.handle` unchecked (`src/server/mod.rs:2590`, `:2820`). | Caller-supplied arguments; may carry PHI |
| toolkit handler → upstream REST backend | A refused call must not cross this boundary at all (zero upstream requests). | Outbound request path, query, body, credential |
| refusal message → MCP client | Data leaving the server; may carry PHI if it echoes the input. | Refusal text |
| MCP client → `execute_code` script | `execute_code` is a registered generic tool; its script is **caller-supplied** (`crates/pmcp-code-mode/src/handler.rs` `build_execute_tool`). | Caller-authored JS, incl. identifiers |
| operator `config.toml` → server boot | Operator-supplied declarations parsed at boot; a bad `pattern` must fail here, not at call time. | Tool/param declarations |
| HTTP response redirect → outbound retry | A followed redirect would carry a governed request past `RequestPolicy`. | Redirect target |

---

## Threat Register

81 rows. Disposition: 74 `mitigate`, 7 `accept`. Severity: 61 high, 15 medium, 5 low; no `critical`.

Per-row `file:line` evidence is recorded in the audit transcript. Summarized by status:

| Status | Count | Threat IDs |
|--------|-------|------------|
| closed | 80 | T-128-01 … T-128-49, T-128-49a, T-128-49b, T-128-50 … T-128-56 (everything except T-128-49c) |
| **open (blocking)** | **0** | — (T-128-49a closed 2026-09-28, see below) |
| open — below `high` threshold (non-blocking) | 1 | T-128-49c |

### CLOSED 2026-09-28 — was the sole blocking threat

**T-128-49a is now CLOSED, measured on a clean GitHub runner.** The measurement the phase
said could only be taken by CI has been taken.

| Evidence | Value |
|---|---|
| Run | `36483102030` @ `44c5eebd` — **completed/success** |
| Provisioning (step 8) | **success** — `nightly-x86_64-unknown-linux-gnu … rustc 1.101.0-nightly (d080e7dff 2026-09-27)`; `cargo-fuzz` installed |
| `Run quality gate` (step 12) | **success** — `✅ ALL TOYOTA WAY QUALITY CHECKS PASSED` |
| Strict leg executed | `fuzzing fuzz_input_schema_enforcement for 5s (failure PROPAGATES)` and `fuzzing fuzz_placeholder_pattern_redos for 5s (failure PROPAGATES)` |
| Execs | **36 615** and **11 943** runs; rss 438 MB / 485 MB |
| Crashes / artifacts | `ERROR: libFuzzer` 0 · `deadly signal` 0 · `SUMMARY: libFuzzer` 0 · `Test unit written to` 0 |

The feared outcome did not occur: `rustup toolchain install nightly` and
`cargo install cargo-fuzz` both succeed on the runner image, so the leg does not redden
every PR and there is no pressure to delete it. Broken-window row 84 (the RELEASE BLOCKER
carried from 128-10) is marked `fixed` in `.planning/WINDOWS.md`.

Verified from the runner log rather than from the green checkmark, deliberately: a passing
gate that never ran the leg is the exact false-green class this phase exists to close, and
rows 22 and 58 record that `make test-fuzz` does precisely that on stable.

### Historical — the open finding, retained for its reasoning


| Threat ID | Category | Component | Severity | Disposition | Mitigation | Status |
|-----------|----------|-----------|----------|-------------|------------|--------|
| T-128-49a | Repudiation | `.github/workflows/ci.yml` `test-fuzz-strict` leg | high | mitigate | "Decide and implement the CI home … **with a clean-runner smoke test as the measurement**." Half 1 present (`ci.yml:295-301` installs nightly + `cargo-fuzz`; `quality-gate` in `gate.needs` at `:859`; `Makefile:2274` chains the leg). **Half 2 does not exist.** | open |

**Measured 2026-09-28, three independent instruments, no push and no CI triggered:**

1. The commit adding the provisioning is `9fec4900`. `git branch -r --contains 9fec4900` → **empty**.
2. `gh api repos/paiml/rust-mcp-sdk/commits/9fec4900/check-runs` → **HTTP 422, "No commit found for SHA"**. GitHub has never seen the commit.
3. The branch `fix/oauth-discovery-optional-fields` is **88 commits ahead of `origin`**; the newest CI run on it is at `3b2d7baf` (2026-09-19), the pre-phase tree.

**Why this is not a technicality.** `Makefile:1090-1093` — `if [ -n "$CI" ]; then … exit 1; fi` — makes
a missing nightly or `cargo-fuzz` a HARD failure, inside `make quality-gate`, inside a `gate.needs`
job, under the org-required `gate` check. If either install fails on the runner image, the first
push turns **every** PR red. That is precisely the outcome T-128-49a was written to prevent, and
`ci.yml:268-269` predicts its end state: *"The realistic outcome of a gate that is red on every run
is not a fixed toolchain — it is a deleted gate."* The risk is unmeasurable locally by
construction: a developer machine that already has nightly takes the other branch of the guard.

The phase's own artifacts say so unprompted and are contradicted by nothing later:
`128-10-SUMMARY.md` — *"the clean-runner smoke test is the one open half"*; `128-11-SUMMARY.md` —
`threat_flag: unrun_ci_provisioning`, **"RELEASE BLOCKER"**. The four CR fix commits
(`5c37285c`…`39a80181`) touch no CI file.

**What closes it:** push the branch (or open the PR) and confirm, on a clean runner at a commit
containing `9fec4900`'s `ci.yml`, that (a) the "Install nightly + cargo-fuzz" step succeeds and
(b) `make quality-gate` → `test-fuzz-strict` runs both targets and exits 0. Record the run id and
sha here, then re-run `/gsd-secure-phase 128`. This is the one item in the phase that cannot be
discharged read-only.

### Open — below threshold (non-blocking)

| Threat ID | Category | Component | Severity | Disposition | Mitigation | Status |
|-----------|----------|-----------|----------|-------------|------------|--------|
| T-128-49c | Denial of Service | `fuzz/fuzz_targets/fuzz_input_schema_enforcement.rs` validator cache | medium | mitigate | Declared: "a `fuzzing`-gated uncached seam, mirroring the split `src/server/output_validation.rs:598-605`". **Absent** — `grep -n fuzzing src/server/schema_validation.rs` → **0 hits** (the precedent file has 7). | open — below `high` threshold (non-blocking) |

A different mitigation shipped: a bounded schema projection capping distinct cache entries at
4×8×5×9×4 = 5760, with arbitrary-schema coverage routed through the uncached
`check_input_schema_compiles`. It is **declared, not concealed** —
`fuzz_input_schema_enforcement.rs:92-113` states *"`schema_validation` has no equivalent
`fuzzing`-gated seam"*, and `128-10-SUMMARY.md` Deviation 1 records the outcome (rss 544 MB over
108k runs) and the coverage residual. The register row nonetheless still names a mechanism that
does not exist.

**What closes it:** either add the `fuzzing`-gated seam in the shape of
`output_validation::fuzz_support`, or amend the T-128-49c row to describe the projection that
actually shipped.

### Closed — verification notes on rows worth calling out

| Threat ID | Note |
|-----------|------|
| T-128-01 | `ValidatingToolHandler::wrap` (`tools.rs:283`); `handle`/`handle_output` both call `self.check(&args)?` before `inner` (`:414`, `:435`), covering all **8** public synthesizer entry points. |
| T-128-04b | Measured exactly as declared: `grep -c 'feature = "schema-validation"' src/server/output_validation.rs` = **18**; `feature = "validation"` = **0**. |
| T-128-07a | `validate_resolved_path` wired on BOTH surfaces — `http/client.rs:255` and `pmcp-code-mode/src/executor.rs:2516-2521`. SC-4's "both surfaces" holds. |
| T-128-22a | **Stronger than planned.** `ResolvedPath::from_checked` (`executor.rs:2513`) is the ONLY constructor and it RUNS `validate_resolved_path`; no `new`/`new_unchecked` exists. |
| T-128-24 | Measured zero second implementations of `denied_byte` / `placeholder_floor` / `decode_once` outside core; only test *names* match. |
| T-128-29 | Measured: `cargo tree -p pmcp-server-toolkit --no-default-features --features http -e normal -i pmcp-code-mode` → not in graph. The curated build stays JS-engine-free. |
| T-128-33 | Measured: `cargo tree -p cargo-pmcp -e normal -i swc_common` → not in graph. `default-features = false` on the CLI's toolkit edge is holding. |
| T-128-36c | `narrowed_executor(...)` sits in **production** `build_server` (`assemble.rs:355`), before both fan-outs (`:381`, `:396`) — not a `#[cfg(test)]` site. |
| T-128-46 | The 36-phrase sweep found a **third** stale claim the plan did not name; 125-hit disposition ledger in `128-10-SUMMARY.md:205-302`. |
| T-128-50 | `./scripts/check-release-coverage.sh` → exit 0, "all 25 publishable workspace members have a publish step". |

Sanity: `cargo check -p pmcp-server-toolkit --features http,input-validation` → exit 0
(pmcp 2.21.0 / pmcp-code-mode 0.6.0 / pmcp-server-toolkit 0.2.0), so every anchor cited is live
compiled code rather than dead text.

---

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-128-01 | T-128-05 (low, DoS) | Config-supplied `pattern` compiled by `jsonschema`. Operator-supplied, not caller-supplied. Residual instrumented by `fuzz_placeholder_pattern_redos.rs` (registered `fuzz/Cargo.toml:417`, `fuzz.yml:66`). | Phase 128 plan author | 2026-09-28 |
| AR-128-02 | T-128-10 (low, DoS) | Declared `pattern` compiled inside `validate_path_placeholder` routes through `cached_input_validator`, not `compile_input_2020_12`; `BacktrackLimitExceeded` → `tracing::warn!` naming `schema_path` only (`schema_validation.rs:324-330`). | Phase 128 plan author | 2026-09-28 |
| AR-128-03 | T-128-15 (low, DoS) | `additional_properties = true` is an explicit operator opt-out, and every opt-out is reported by `lint_opt_outs` (`config.rs:603-616`) and `validation_report()` (`:434`). | Phase 128 plan author | 2026-09-28 |
| AR-128-04 | T-128-23 (medium, InfoDisclosure) | `ApiCallLog { path, body }` retains the resolved path in the internal execution log. Mitigated by a doc warning (`executor.rs:2900-2907`) that any surface exposing `api_calls` inherits the disclosure and must redact. Nothing mechanical enforces that. | Phase 128 plan author | 2026-09-28 |
| AR-128-05 | T-128-40 (medium, InfoDisclosure) | A policy- or validator-supplied refusal message is third-party text the SDK cannot redact. Documented on both refusal types: `policy.rs:142` (`ArgumentRefusal`), `:261` (`PolicyRefusal`). | Phase 128 plan author | 2026-09-28 |
| AR-128-06 | T-128-44 (low, DoS) | A `RequestPolicy` implementation can block the request path. Documented on the trait at `policy.rs:218`; the SDK cannot bound third-party code. | Phase 128 plan author | 2026-09-28 |
| AR-128-07 | T-128-48 (low, DoS) | Config-supplied `pattern` as a ReDoS vector. Instrumented rather than guarded: `fuzz_placeholder_pattern_redos.rs:33` records that a timeout is "a FINDING TO FILE, not a licence to hand-roll a regex guard". A2 did not reproduce over 60 751 runs seeded with four nested-quantifier shapes. | Phase 128 plan author | 2026-09-28 |
| AR-128-08 | **T-128-21b** (low, InfoDisclosure) — NEW, added by this audit | A layer-1 path refusal names the caller-chosen JS variable identifier (`executor.rs`, `PathPart::Variable`). Accepted: the identifier is the caller's OWN, so echoing it discloses nothing they do not already know; the JS identifier grammar admits no whitespace, newline or punctuation beyond `$`/`_`, so it cannot carry a log-injection payload; and the VALUE is always redacted. The same identifier class already reaches the client at three other sites, so fixing only this arm would be a partial fix that reads as complete. See the Findings section for why the original *justification* was a defect even though the residual is not. | Operator, 2026-09-28 (`/gsd-secure-phase 128`) | 2026-09-28 |

---

## Findings from this audit (outside the register)

### F1 — `attacker_influenced_name_in_a_refusal` was mis-bounded; its trigger was already met — **CORRECTED**

`128-05-SUMMARY.md` bounded this residual with *"becomes LIVE if a later phase ever makes scripts
caller-supplied."* That condition was **already true on a shipped surface**:
`crates/pmcp-code-mode/src/handler.rs` `build_execute_tool`'s own rustdoc says the tool *"runs
caller-supplied code against the app's op surface"*, and `code_mode.rs:271`/`:348` register
`execute_code` as a generic MCP tool. So the in-code justification *"A script-chosen identifier is
operator-shipped content, unlike the value"* was false on that surface — and was contradicted 25
lines above by the same function's own doc (*"Code Mode scripts are model-authored, so that was the
untrusted route"*).

**Adjudication.** T-128-21's `mitigate` disposition **is** satisfied and stays CLOSED: its declared
mitigation is narrow and specific — the two error wraps stop formatting the *resolved path* — and
both do (`executor.rs:3174-3175`, `:3360-3361`). The residual itself is genuinely low: what reaches
the client is the caller's own identifier. Review finding WR-04's low-impact classification is
correct on the merits.

**But the justification was a defect under this project's own standard.** A comment asserting a
property the code does not have is exactly the class Phase 128 exists to close — here applied to
the phase's own comment. **Fixed in this audit's commit:** `executor.rs`'s `PathPart::Variable`
comment now states the real reason (caller-chosen, grammar-bounded, value always redacted), records
that the previous justification was false and why, and notes the three sibling sites so a later
reader does not mistake a partial fix for a complete one. Booked as **T-128-21b / AR-128-08**.

### F2 — WR-02: a public, un-hardened redirect constructor — **FIXED**

`crates/pmcp-server-toolkit/src/http/client.rs` `HttpClient::from_config` built a
`reqwest::Client` with **no `.redirect(...)`**, so reqwest's follow-up-to-10-hops default applied —
the exact bypass `pmcp-openapi-server`'s `dispatch.rs:147` hardens against and that `policy.rs:205-211`
warns about by name. It was not counted against T-128-39a because it has **zero in-repo callers**,
but it is `pub`, so a downstream consumer reaching for the constructor that honours `[backend.http]`
got the un-hardened client. **Fixed in this audit's commit:**
`.redirect(reqwest::redirect::Policy::none())` added, matching `dispatch.rs`.

### F3 — ~14 new public items with no `cargo public-api` gate (advisory, unfixed)

Flagged by `128-01`, `-02`, `-06`, `-11`: `PLACEHOLDER_MAX_LENGTH`, `PlaceholderRules` + 3
builders, `PlaceholderRefusal`, `validate_path_placeholder`, `validate_resolved_path`,
`check_input_schema_compiles`, `ResolvedPath`, three `Parameter` fields + `with_rules` +
`placeholder_rules`, `pmcp-server-toolkit::VERSION`. Nothing mechanical will catch a later
over-wide `pub` — notably a re-widening of `ResolvedPath`'s constructor, whose narrowness is
load-bearing for T-128-22a. Consider booking a `cargo public-api` gate.

### F4 — `{other}` self-substitution residual (advisory, low, no register row)

`http/client.rs` — a placeholder value that is itself `{other}`, where `other` is also a supplied
path parameter, can duplicate that already-floored value into its own slot. Non-escalating: the
caller could place the value there directly and it is floored either way, and an unsupplied `other`
is refused by the composed check's residual-brace rule.

### F5 — `128-08-SUMMARY.md` declares no `## Threat Flags` section (process note)

Zero case-insensitive matches for "threat flag" in that file. Nine of eleven plans declare one;
plans 09 and 10 use `## Threat flags` / `# Threat Flags`. "No new flags" is a *claim*, not the
absence of one, so plan 08's new surface (`with_schema` narrowing wiring, `placeholder_rules`
override) rested on an undeclared assertion. Its seven rows were verified independently and no gap
was found. The heading-case drift also means scripted flag extraction under-reads this phase.

### F6 — Pre-existing, out of scope (recorded for completeness)

`128-11`'s `accepted_residual`: three unguarded `pmcp` version emitters
(`cargo-pmcp/src/templates/{oauth/proxy,oauth/authorizer,mcp_app}.rs`) stale by a major or more
(`WINDOWS.md` #85). Also `deferred-items.md` D1–D4 — 5 rustdoc warnings and 9 clippy findings in
non-CI-gated crates, each proven pre-existing with git evidence by the executor. Not phase defects.

---

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-09-28 | 81 | 80 | 1 blocking (+1 non-blocking) | gsd-security-auditor (ASVS L1, `block_on: high`) |
| 2026-09-28 | 81 | 80 | 0 blocking (1 non-blocking: T-128-49c) | CI run 36483102030 @ 44c5eebd closed T-128-49a |

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log (8 rows, incl. the new T-128-21b)
- [x] `threats_open: 0` confirmed — T-128-49a closed by CI run 36483102030 @ 44c5eebd
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-09-28

*One non-blocking threat remains open below the `high` threshold: T-128-49c (medium) — the
declared `fuzzing`-gated uncached seam does not exist; a bounded projection shipped instead,
declared in `128-10-SUMMARY.md` Deviation 1. It does not gate advancement, but the register
row still names a mechanism that is not there and should be amended or implemented.*
