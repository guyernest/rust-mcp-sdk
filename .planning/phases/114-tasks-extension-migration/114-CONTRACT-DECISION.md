# Phase 114 — Contract-First Decision Record

**Measured:** 2026-07-28
**Plan:** 114-20 (wave 1, no dependencies, touches no source file)
**Status:** Task 1 complete (measurement + options). `## Decision` awaiting the owner at the Task 2 blocking checkpoint.

This document exists because `CLAUDE.md` § *Contract-First Development* states a **mandatory**,
unqualified repository rule, and a phase plan cannot grant itself an exemption from one. The
question is settled here, at wave 1, **before** the seventeen implementation plans run — rather
than being discovered at the end of them in `114-18`.

This document deliberately does **not** choose. It measures, states both options with their real
costs, and stops.

---

## 1. Measurement

Every command below was run on **2026-07-28** from the repository root
`/Users/guy/Development/mcp/sdk/rust-mcp-sdk` on branch `fix/mcp-publisher-oidc-audience`.
Output is verbatim except where noted (`pmat` output has ANSI colour escapes stripped, and long
reports are excerpted with the elision marked). A future reader must be able to re-run each
command and either get the same answer or see plainly that the ground shifted.

### 1.1 The rule, as written

`CLAUDE.md:353-359` and `AGENTS.md:319-325` are **byte-identical** on this point:

```
$ sed -n '353,359p' CLAUDE.md
## Contract-First Development

All new features and bug fixes must follow provable-contract-first methodology:
1. Write or update the contract YAML in `../provable-contracts/contracts/<crate>/`
2. Run `pmat comply check` to validate compliance
3. Implement the code to satisfy the contract
4. Run `pmat comply check` again to confirm

$ diff <(sed -n '353,359p' CLAUDE.md) <(sed -n '319,325p' AGENTS.md)
(no output; exit status 0 — the two files state the rule identically)
```

### 1.2 Does `../provable-contracts/` exist? — NO, at any of three parent levels

```
$ ls -d ../provable-contracts
ls: ../provable-contracts: No such file or directory      (exit 1)

$ ls -d ../../provable-contracts
ls: ../../provable-contracts: No such file or directory   (exit 1)

$ ls -d ../../../provable-contracts
ls: ../../../provable-contracts: No such file or directory (exit 1)
```

Resolved absolute paths checked, so the negative result is unambiguous:

| Relative | Absolute path checked |
|---|---|
| `..` | `/Users/guy/Development/mcp/sdk/provable-contracts` |
| `../..` | `/Users/guy/Development/mcp/provable-contracts` |
| `../../..` | `/Users/guy/Development/provable-contracts` |

The first row is the sibling directory `CLAUDE.md` names. It is absent, as Phase 113 also recorded.

### 1.3 Is `AGENTS.md` present, and is it tracked? — PRESENT, UNTRACKED, NOT IGNORED

```
$ ls -la AGENTS.md
-rw-r--r--@ 1 guy  staff  13313 Jul 27 04:01 AGENTS.md

$ git ls-files AGENTS.md
(no output; exit status 0, 0 lines — the file is NOT tracked)

$ git status --porcelain AGENTS.md
?? AGENTS.md

$ git check-ignore -v AGENTS.md
(no output; exit status 1 — the file is NOT gitignored either)

$ wc -c AGENTS.md CLAUDE.md
   13313 AGENTS.md
   16978 CLAUDE.md
```

**This is itself part of the answer, as the plan anticipated.** `AGENTS.md` restates the same
mandatory contract-first rule as `CLAUDE.md` (§1.1), was last written **2026-07-27**, is neither
committed nor ignored, and would therefore vanish from any fresh clone, any CI checkout, and any
`git worktree`. A rules file that binds this working tree but no other is a weaker authority than
`CLAUDE.md`, which is tracked. Whichever option is chosen, **the contract-first obligation is
carried by `CLAUDE.md` (tracked), not by `AGENTS.md` (untracked)** — so nothing in this decision
should rest on `AGENTS.md` alone. Committing or removing `AGENTS.md` is out of scope for this plan
and is not proposed here.

### 1.4 What does compliance actually enforce today?

```
$ sed -n '686,690p' Makefile
	@$(MAKE) check-todos
	@$(MAKE) check-unwraps
	@$(MAKE) validate-always
	@$(MAKE) purity-check
	@$(MAKE) comply

$ sed -n '841,849p' Makefile
.PHONY: comply
comply:
	@if command -v pmat &> /dev/null; then \
	  echo "$(BLUE)Running pmat comply check --path . (report; project-level advisories are informational — D-07)$(NC)"; \
	  pmat comply check --path . || echo "$(YELLOW)note: pmat comply reported project-level advisories (informational; see CLAUDE.md D-07). team-servers binding drift is enforced below.$(NC)"; \
	else \
	  echo "$(YELLOW)⚠ warn: pmat absent, skipping pmat comply (team-servers binding drift still enforced below)$(NC)"; \
	fi
	@$(MAKE) --no-print-directory comply-bindings-check
```

`comply-bindings-check` is at `Makefile:818-834` (the plan estimated ~851; that line number is
inside the `comply-ci` comment block). It is the fail-closed half: every `function:` in
`contracts/team-servers/binding.yaml` must resolve to a real `fn` in
`crates/pmcp-team-servers/src`, or the gate exits 1.

Tooling present:

```
$ command -v pmat
/Users/guy/.cargo/bin/pmat

$ pmat --version
pmat 3.15.0

$ command -v pdmt
(no output; exit status 1 — ABSENT)

$ command -v pv
(no output; exit status 1 — ABSENT)
```

`pmat` is present at exactly the 3.15.0 that `CLAUDE.md` pins for CI, so `pmat comply check` is
genuinely executable here and cannot be skipped on tool-absence grounds.

### 1.5 CORRECTION to this plan's own stated premise — the rule's step 1 DOES have a destination

The `114-20-PLAN.md` objective asserts that `make comply` runs advisory `pmat comply check` plus
`comply-bindings-check` and that *"neither reads a per-crate contract YAML. So the rule's step 1
currently has no destination."*

**That assertion is FALSE as measured, and the correction is material to the decision.** This
repository carries an **in-repo, git-tracked `contracts/` tree**, and `pmat comply check --path .`
reads and grades it:

```
$ find contracts -type f | wc -l
38
$ git ls-files contracts/ | wc -l
38          (all 38 are tracked)

$ ls contracts/*.yaml
contracts/binding.yaml
contracts/mcp-protocol-sdk-v1.yaml
contracts/team-servers-v1.yaml
```

```
$ pmat comply check --path .            # exit status 1 (project-level, informational per D-07)
... 259 lines; contract-relevant checks excerpted verbatim ...
  ⚠ CB-1200: Provable Contracts: Found 2 contract file(s) but `pv` CLI not installed. Install: cargo install --path ../provable-contracts/crates/provable-contracts-cli
  ✓ CB-1202: Contract Coverage: 2/2 critical keywords covered (100%)
  ✓ CB-1203: Contract Annotations: 1/1 contract-bound fns have macros
  ✗ CB-1204: Build.rs Pipeline: Contracts have preconditions but no build.rs to emit assertion env vars
  ✓ CB-1205: Provability Invariant: 1 kernel contract(s) satisfy provability invariant
  - CB-1206: Verification Levels: No proof-status.json in ../provable-contracts/
  ⚠ CB-1207: Contract Drift: 1/2 contract(s) stale (>90 days since last commit), 1 fresh
  ✓ CB-1305: Contract Surface Classification: 2/2 classified (kernel=2)
  ⚠ CB-1354: Contract Query Readiness: Partial (2/4): have [contracts/YAML, binding.yaml], missing [binding-index.json, pv CLI]
  ⚠ CB-1409: No L0 Autonomous Code: 5/9 AI-authored commit(s) lack work contracts: docs(114-01): state the requirement booking precisely; docs(114-01): complete vendored ext-tasks schema & D-18 h...; test(114-01): provenance tripwire over the vendored ext-t...
```

So the accurate statement of the ground truth is a **split**:

| Piece | Where the rule says it lives | Where it actually is |
|---|---|---|
| The contract **YAML** | `../provable-contracts/contracts/<crate>/` | **In-repo at `contracts/`**, tracked, and read by `pmat comply` (CB-1200/1202/1205/1305) |
| The `pv` **verifier CLI** | — | **ABSENT**; CB-1200 names its install source as the absent `../provable-contracts/crates/provable-contracts-cli` |
| `proof-status.json` | `../provable-contracts/` | **ABSENT** (CB-1206) |

The absent sibling repo is where the *verifier* and *proof status* live. The *authoring
destination* — the thing step 1 of the rule asks for — exists in this repo today and is already
being graded. **This cuts against both options and is recorded here rather than folded into
either one:** it makes Option A materially cheaper than the plan estimated (no sibling repo to
locate or recreate), and it removes Option B's "there is nowhere to write it" rationale, leaving
Option B resting solely on the D-18 provisional-values argument.

Note also **CB-1409 already names Phase 114's own `114-01` commits** as lacking work contracts, so
this repository's compliance surface is grading this phase right now.

### 1.6 Does the existing SDK contract cover this phase's surface? — NO

```
$ grep -c -i "task" contracts/mcp-protocol-sdk-v1.yaml
0
$ grep -n -i "extension" contracts/mcp-protocol-sdk-v1.yaml
(no output; exit status 1)
```

`contracts/mcp-protocol-sdk-v1.yaml` (413 lines, `target_crate: pmcp` via `contracts/binding.yaml`)
declares **ten** equations — `jsonrpc_framing`, `protocol_version_negotiation`, `session_lifecycle`,
`tool_dispatch_integrity`, `transport_abstraction`, `error_code_mapping`, `payload_limits`,
`cancellation_safety`, `batch_request_ordering`, `sampling_dispatch` — plus 7 falsification rules
and 5 Kani obligations. **None mentions tasks or extensions.** Its metadata reads
`version: 1.0.0`, `created: '2026-04-03'`, describing *"PAIML MCP Protocol SDK v2.1"* (the crate is
now at 2.17).

Staleness, measured:

```
$ git log -1 --format='%ad %h' --date=short -- contracts/mcp-protocol-sdk-v1.yaml
2026-04-03 3cf37e04
$ git log -1 --format='%ad %h' --date=short -- contracts/team-servers-v1.yaml
2026-07-18 8aca2bf0
$ git log -1 --format='%ad %h' --date=short -- contracts/binding.yaml
2026-05-30 fffa999e
$ date -u +%Y-%m-%d
2026-07-28
```

116 days since the SDK contract was last touched — that is CB-1207's *"1/2 contract(s) stale
(>90 days), 1 fresh"*, and the stale one is the SDK contract this phase would extend.

```
$ git log --since=2026-07-20 --oneline -- contracts/
(no output — contracts/ has NOT been touched at any point in the v2.5 milestone)
```

---

## 2. Precedent — what Phase 113 did

**Phase 113 was not silent, and it did not author a contract.** It recorded an explicit,
named deviation. This is a finding in both directions and is stated plainly here.

`113-SPEC-RECHECK.md` § *Contract-First Environment (Section C)* (lines 912-1013) records the same
`ls -d ../provable-contracts` → `No such file or directory` measurement, then, under
`### Deviation from CLAUDE.md MANDATORY directives`, lists three consciously-deviated MANDATORY
directives with substitutes and residual risk. The third is verbatim:

> **Deviation 3 — contract-first ran in-repo only.**
>
> - **Directive:** "Write or update the contract YAML in `../provable-contracts/contracts/<crate>/`
>   … Run `pmat comply check`."
> - **Why not:** `../provable-contracts` does not exist in this workspace (C.1). The external
>   contract YAML for `pmcp` cannot be read or updated from here.
> - **Substitute:** `pmat comply check --path .`, i.e. the in-repo compliance surface, run via
>   `make quality-gate`'s `comply` stage, plus the repo's own deterministic
>   `comply-bindings-check` source-resolution gate (Makefile:819-835).
> - **Residual risk:** MEDIUM. Phase 113's wire-level behavior is not being graded against an
>   external, versioned contract before implementation; drift between the shipped SDK and the
>   canonical `provable-contracts` YAML would go undetected in this phase. Note also that Codex
>   blocking finding 8 asks for contract updates to *precede* implementation; with the checkout
>   absent, that ordering cannot be honored here and remains deferred to plan 12.

`113-12-SUMMARY.md` § *Contract-First — Recorded Honestly* (lines 262-281) re-ran every command
rather than copying the earlier result, confirmed the identical state, and stated:
*"No contract was updated in a checkout that does not exist, and none is claimed."* The
per-plan verification tables of `113-17`, `113-18`, `113-19` and `113-20` each carry the same
`ls ../provable-contracts/contracts/` → `No such file or directory` row, *"recorded rather than
skipped silently."*

Four properties of that precedent bear directly on the choice now, and all four are measured, not
inferred:

1. **113 shipped without a contract YAML for its surface.** `contracts/` was untouched for the
   entire phase (§1.6, no commits since 2026-07-20).
2. **113's waiver was executor-authored, not owner-decided.** It appears in a spec-recheck
   document and in plan SUMMARYs. There is no `Chosen:` / `Decided by:` / `Date:` record of an
   owner ruling on it. That asymmetry is exactly the gap the cross-AI review raised against
   Phase 114 and is why this document exists.
3. **113's deferral terminated in a record, not in a gated obligation.** Deviation 3 defers the
   contract-precedes-implementation ordering "to plan 12"; plan 12 re-measured and re-recorded it
   under *"Recorded Honestly"* and created no follow-up gate.
   `grep -i "contract\|provable" .planning/phases/113-.../deferred-items.md` returns three hits,
   **none** of which is the contract-first obligation. This is threat **T-114-107** ("a waiver that
   quietly becomes permanent") observed in the wild rather than hypothesised.
4. **113's stated substitute was the in-repo compliance surface** — the same surface §1.5 shows is
   already reading two contract YAMLs. 113 therefore already treated in-repo as the fallback
   destination, while never adding an equation to it.

Phase 114 inherits this precedent whichever way it goes: Option B continues it with the two
defects above corrected (owner-decided, and gated); Option A departs from it.

---

## 3. The two options

Both are defensible. They are presented at equal weight and in the plan's order. The §1.5
correction changes the cost of each — it lowers Option A's setup cost and removes one of
Option B's two stated rationales — and that is stated in both entries rather than in only one.

### Option A — author the contract now

**What it means concretely, as corrected by §1.5:** *not* locating or recreating the absent
`../provable-contracts/` sibling. The authoring destination is the in-repo, tracked, already-graded
`contracts/` tree. Concretely: add equations to `contracts/mcp-protocol-sdk-v1.yaml` (or a new
sibling YAML) covering this phase's surface, and add corresponding `contracts/binding.yaml` rows
binding each equation to a real `fn`, matching the `target_crate: pmcp` shape already in that file:

| Requirement | Surface the contract would pin |
|---|---|
| TASK-01 | the `io.modelcontextprotocol/tasks` extension-negotiation capability and its era projection (v1 `initialize` byte-identical; v2 `server/discover` carries the entry) |
| TASK-02 | `tasks/update` — the `{taskId, inputs}` shape and the atomic `InputRequired`→`Working` CAS transition |
| TASK-03 | `tasks/get` result inlining, and `tasks/result` / `tasks/list` era-gated to `-32601` on v2 |
| TASK-04 | the flat `CreateTaskResult{taskId,status,ttlMs,pollIntervalMs}` and the `resultType:"task"` discriminator |
| TASK-05 | the three-row owner-binding identity table and its fail-closed `-32003` refusal |

**Pros:** satisfies `CLAUDE.md` § Contract-First Development literally; the compliance step has a
real, already-graded destination; the contract becomes the machine-checkable statement of
TASK-01..06; it also refreshes an SDK contract that CB-1207 currently reports stale at 116 days and
that covers none of this phase's surface.

**Cost:** the contract must be written against a schema this phase is **explicitly holding as
provisional** under D-18. `114-SPEC-RECHECK.md` records `## Verdict: PENDING` and a 39-row
Wire-Value Inventory of values the next seventeen plans will write; a contract authored now pins
values the final-schema gate is expected to move, and would need re-authoring at that gate. It
therefore encodes a claim stronger than the current evidence supports — the trust-boundary concern
named as T-114-108. Secondary costs: `pv` and `proof-status.json` are absent (§1.4/§1.5), so the
contract can be graded by `pmat comply` but not verified by `pv` here; CB-1204 (`Build.rs Pipeline`)
already fails on the existing two contracts and more preconditions do not improve it; and the
authoring lands on the critical path of a wave-1 plan.

**Follow-up obligation:** a `114-SPEC-RECHECK.md` row, worded as a **condition** matching the Third
Outcome Policy shape that file already uses — *when a versioned (non-`draft`) schema directory
exists in both `modelcontextprotocol/modelcontextprotocol` and `modelcontextprotocol/ext-tasks`,
re-author the Phase-114 equations in `contracts/mcp-protocol-sdk-v1.yaml` and their
`contracts/binding.yaml` rows against the published values, and re-run `pmat comply check
--path .`* — with the vendored-schema SHA-256 tripwire from `114-01` as the change detector.

### Option B — record an explicit owner waiver for Phase 114

**What it means concretely:** the contract-first step is waived for Phase 114 by owner decision,
recorded in this document with `Chosen:` / `Decided by:` / `Date:`, and re-entered as a gated
obligation. `114-18` then **cites** this waiver rather than declining the contract on its own
authority.

**Pros:** honest about the provisional-schema hold — D-18 books the whole phase at `[~]` pending a
versioned schema directory, and no throwaway contract is authored against values the phase itself
expects to move. The phase's compliance surface is already covered by three independent
mechanisms landed or planned in-phase: the vendored-schema provenance tripwire (`114-01`, five
tests, SHA-256 + git-blob cross-check), the golden byte fixtures (`114-02`), and the source
tripwires (`114-16`). It continues Phase 113's recorded precedent (§2) with that precedent's two
measured defects corrected — this waiver is owner-decided rather than executor-authored, and it
carries a gate rather than terminating in a record.

**Cost:** a documented deviation from a rule `CLAUDE.md` states without qualification, which
lowers the bar for any later phase that cites it as precedent — the trust-boundary concern named
as T-114-106. The §1.5 correction sharpens this cost: the "there is nowhere to write it" rationale
does **not** hold, so Option B rests solely on the provisional-values argument, and it leaves
`contracts/mcp-protocol-sdk-v1.yaml` stale at 116 days with zero coverage of the tasks surface for
another phase. Additionally, the phase's surface stays ungraded against a versioned contract
before implementation (113's own residual-risk assessment: MEDIUM), and the obligation must be
re-entered at the D-18 gate rather than forgotten — which is precisely what did not happen in
Phase 113.

**Follow-up obligation:** a `114-SPEC-RECHECK.md` row, worded as a **condition** and matching the
Third Outcome Policy shape that file already uses — *when a versioned (non-`draft`) schema
directory exists in both `modelcontextprotocol/modelcontextprotocol` and
`modelcontextprotocol/ext-tasks`, the contract-first question re-enters: author the Phase-114
equations in `contracts/mcp-protocol-sdk-v1.yaml` and their `contracts/binding.yaml` rows against
the published values and run `pmat comply check --path .`, or record a further explicit owner
waiver.* Partial publication lands in `STILL-ABSENT`, not a fourth state, per that file's existing
policy. `"revisit later"` is explicitly disallowed.

---

## 4. Decision

*To be filled at the Task 2 blocking checkpoint by the owner. `Chosen:` MUST be exactly
`option-a` or `option-b` (or a recorded third path). `Follow-up obligation:` MUST name a concrete
artifact and gate — never "revisit later".*

Chosen:
Decided by:
Date:
Follow-up obligation:

### Rationale

*(To be filled with the owner's stated reasoning, if any is given. If none is given, this section
records that plainly rather than inventing one — the Phase 113 precedent for that discipline is
the 113-28 checkpoint, where no conditions, review date or scope narrowing were stated and none
were invented.)*
