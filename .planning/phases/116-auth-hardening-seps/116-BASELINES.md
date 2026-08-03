# Phase 116 — Baselines

The evidence anchor for Phase 116. Every phase-end claim in `116-15` is a DELTA against a number
recorded here, with the command that produced it, so a future reader can re-derive it rather than
trust it.

**Phase-base SHA: `b2bf91571236af2ee9f64c7b14608f3be4edf133` (`b2bf9157`).**
**Measured: 2026-08-03.** Toolchain: `stable-aarch64-apple-darwin`, `pmat 3.15.0`.

Written by plan `116-01`. Nothing under `src/`, `Cargo.toml` or `Cargo.lock` was changed by this
plan; the one `tests/` change it made is recorded and justified in
[Contract-First Finding](#contract-first-finding).

---

## Contract-First Finding

CLAUDE.md § "Contract-First Development" mandates four steps, in order: (1) write or update the
contract YAML, (2) run `pmat comply check`, (3) implement, (4) re-check. Cross-AI review (Codex,
HIGH — "The contract-first plan conflicts with repository policy") flagged that the previous
revision of `116-01` concluded the opposite — that no contract was needed. **This section records
the discharge of step (1) and step (2), AHEAD of any implementation.**

### Where the contracts actually live

| Location | CLAUDE.md names it? | `make comply` resolves it? | Exists on disk? |
|---|---|---|---|
| `../provable-contracts/contracts/pmcp/` | YES (§ Contract-First Development) | no | **NO — the directory `../provable-contracts` does not exist** |
| in-repo `contracts/` | no | YES (`Makefile:842-849`) | **YES** |

Measured:

```
$ [ -d ../provable-contracts ] && echo EXISTS || echo ABSENT
ABSENT
$ [ -d ../provable-contracts/contracts/pmcp ] && echo EXISTS || echo ABSENT
ABSENT
$ [ -d contracts ] && echo EXISTS || echo ABSENT
EXISTS
```

So the path CLAUDE.md names is a sibling checkout this machine does not have, and the in-repo
`contracts/` tree is the one every gate in this repository actually reads. **Phase 116 authors into
the in-repo tree.** This is a documentation gap in CLAUDE.md, not a licence to skip the mandate.

### What existed before this phase: nothing

Per-file, case-insensitive hit counts for the six tokens this phase's surface would use, across
every `contracts/**/*.yaml`, measured BEFORE the edit:

| File | `oauth` | `dcr` | `issuer` | `credential` | `application_type` | `auth_cmd` |
|---|---|---|---|---|---|---|
| `contracts/binding.yaml` | 0 | 0 | 0 | 0 | 0 | 0 |
| `contracts/mcp-protocol-sdk-v1.yaml` | 0 | 0 | 0 | 0 | 0 | 0 |
| `contracts/team-servers-v1.yaml` | 0 | 0 | 0 | 0 | 0 | 0 |
| `contracts/team-servers/binding.yaml` | 0 | 0 | 0 | 0 | 0 | 0 |
| `contracts/team-servers/binding.broken.yaml` | 0 | 0 | 0 | 0 | 0 | 0 |

Command:

```bash
for f in $(find contracts -name '*.yaml' | sort); do
  for tok in oauth dcr issuer credential application_type auth_cmd; do
    printf '%-46s %-18s %s\n' "$f" "$tok" "$(grep -ic "$tok" "$f")"
  done
done
```

**Zero hits in every cell.** The OAuth client surface this phase hardens was entirely uncontracted.
That is exactly the condition the mandate exists for, and it is why the contract had to be written
first rather than declared unnecessary.

### What was authored, ahead of the implementation

**THE CONTRACT WAS AUTHORED AHEAD OF IMPLEMENTATION, per CLAUDE.md § "Contract-First Development":
not one line of this phase's `src/` code exists yet, so every invariant below was derived from the
cited RFC or SEP clause rather than transcribed back out of a shipped function. Plan `116-15` flips
the eight bindings from `status: planned` to `status: implemented`, and only after resolving each
`function:` against a real declaration in `src/`.**

Three new `equations:` entries in `contracts/mcp-protocol-sdk-v1.yaml` (equation count 13 → 16), each
carrying `formula:` (block scalar), `domain:`, `codomain:`, `invariants:`, `preconditions:`,
`postconditions:` and `lean_theorem:` in the shape the existing entries use:

| Equation | Lines | Invariants | Requirement / decision |
|---|---|---|---|
| `oauth_authorization_response_validation` | `contracts/mcp-protocol-sdk-v1.yaml:492-583` | 10 | AUTH-01 / D-12, RFC 9207, RFC 3986 §6.2.2-6.2.3 |
| `oauth_discovery_anchor` | `contracts/mcp-protocol-sdk-v1.yaml:584-651` | 7 | AUTH-01 + AUTH-03 / D-13, SEP-2351, RFC 8414 §2 and §3.3 |
| `oauth_credential_binding` | `contracts/mcp-protocol-sdk-v1.yaml:652-708` | 7 | AUTH-03 / D-116-R1, SEP-2352 |

Eight `status: planned` bindings under a new `# === OAuth Client Hardening (Phase 116) ===` section
at `contracts/binding.yaml:828-952` (record count 64 → 72), each with a `notes:` naming Phase 116 and
its requirement ID. Every `signature:` is the DESIGNED interface copied from the owning plan's
`<interfaces>` block — the point of contract-first is that the signature is committed before the body:

| Function | `module_path:` | Owning plan |
|---|---|---|
| `validate_authorization_response` | `pmcp::shared::oauth_validation` | 116-02 |
| `iss_presence_from` | `pmcp::shared::oauth_validation` | 116-02 |
| `discovery_url_candidates` | `pmcp::shared::oauth_validation` | 116-04 |
| `issuer_matches_metadata` | `pmcp::shared::oauth_validation` | 116-04 |
| `classify_discovery_failure` | `pmcp::shared::oauth_validation` | 116-04 |
| `derive_application_type` | `pmcp::shared::oauth_validation` | 116-04 |
| `parse_credential_snapshot` | `pmcp::shared::credential_store` | 116-05 |
| `CredentialKey::new` | `pmcp::shared::credential_store` | 116-05 |

No pre-existing equation or binding entry was modified:

```
$ git diff --numstat -- contracts/
126	0	contracts/binding.yaml
225	0	contracts/mcp-protocol-sdk-v1.yaml
$ git diff -- contracts/ | grep -c '^-[^-]'
0
```

**Additions only, zero removed lines.**

### The gate the plan did not know about — observed, then fixed

`116-01` predicted that `status: planned` was safe because `comply-bindings-check`
(`Makefile:818-834`) resolves `function:` values only in `contracts/team-servers/binding.yaml` and
`pmat comply check --path .` is informational per D-07. **Both halves of that prediction are correct
and were confirmed. The prediction was also incomplete.**

`tests/phase115_contract_bindings.rs` — added by Phase 115 as "the missing resolver", because before
it *nothing in this repository read `contracts/binding.yaml` at all* — DOES read this file, and its
test `phase115_contract_bindings_planned_entries_are_scoped_to_phase_115` confines `planned` to a
named equation list so it cannot become a universal escape hatch.

The failure was **OBSERVED before it was fixed**, not anticipated:

```
$ cargo nextest run --features full,oauth -E 'binary(phase115_contract_bindings)'
     Summary [   0.936s] 5 tests run: 4 passed, 1 failed, 0 skipped
        FAIL  phase115_contract_bindings_planned_entries_are_scoped_to_phase_115

FAILURE MODE: `status: planned` was used outside Phase 115. ...
  contracts/binding.yaml:861 equation `oauth_authorization_response_validation` function `validate_authorization_response`
  contracts/binding.yaml:873 equation `oauth_authorization_response_validation` function `iss_presence_from`
  contracts/binding.yaml:884 equation `oauth_discovery_anchor` function `discovery_url_candidates`
  contracts/binding.yaml:896 equation `oauth_discovery_anchor` function `issuer_matches_metadata`
  contracts/binding.yaml:906 equation `oauth_discovery_anchor` function `classify_discovery_failure`
  contracts/binding.yaml:918 equation `oauth_discovery_anchor` function `derive_application_type`
  contracts/binding.yaml:929 equation `oauth_credential_binding` function `parse_credential_snapshot`
  contracts/binding.yaml:941 equation `oauth_credential_binding` function `CredentialKey::new`

WHAT TO DO: ... If a future phase genuinely needs contract-first `planned` bindings, extend
PHASE_115_EQUATIONS deliberately in this file — that edit is the conversation this test exists to force.
```

Full log: `target/116-verify/phase115_contract_bindings.OBSERVED-RED.log`.

The fix is the edit that failure message demands, and it is the ONLY `tests/` change plan `116-01`
makes (deviation Rule 3 — a blocking issue the plan did not foresee):

- a SECOND enumeration `PHASE_116_EQUATIONS` naming exactly the three equations above, plus a
  `planned_is_permitted` helper joining the two lists. It is not a widening of the Phase 115 list and
  not a predicate over `contract:` or a filename, so a fourth equation cannot join by accident.
- an anti-vacuity floor `phase_116_records >= 8`, mirroring the existing Phase 115 floor, so deleting
  the whole Phase 116 section cannot make the new permission cover nothing and pass silently.

Both directions were then observed:

| Control | Command | Result |
|---|---|---|
| Positive (the fix works) | `cargo nextest run --features full,oauth -E 'binary(phase115_contract_bindings)'` | `Summary [0.892s] 5 tests run: 5 passed, 0 skipped` |
| **Negative (the permission is not blanket)** | temporarily flip the unrelated `jsonrpc_framing` / `JSONRPCRequest::validate` binding to `status: planned`, re-run | `4 passed, 1 failed` — "`status: planned` was used outside the enumerated contract-first phases" |

The negative control's edit was reverted and verified byte-for-byte: `shasum -a 256 -c` → `OK`,
`status: implemented` restored at `contracts/binding.yaml:10`, and `grep -c '^  status: planned'`
back to exactly `8`.

**Hand-off recorded in the section comment itself** (`contracts/binding.yaml:828-859`): `116-15`
flips all eight to `implemented` after resolving each `function:` against real source, and must then
remove the three equations from `PHASE_116_EQUATIONS` (or leave them with a written reason) so
`planned` cannot outlive the phase that justified it. The comment also warns `116-15` that
`CredentialKey::new` resolves through the shared `fn new` needle, which is not unique to that type,
so that one must be verified by hand.

### Step (2) of the mandate: `pmat comply check`

The exact invocation, so a later plan citing "comply passed" names the same command:

```
$ make comply          # Makefile:841-849 (.PHONY at :841, target at :842)
...
note: pmat comply reported project-level advisories (informational; see CLAUDE.md D-07).
🔗 comply-bindings-check: resolving team-servers binding.yaml functions against source
  ✓ build_team_fs_server
  ✓ build_mem_mcp_server
  ✓ build_approval_mcp_server
  ✓ build_team_mcp_server
✓ every team-servers binding resolves to a real function
exit=0
```

Both files parse:

```
$ python3 -c "import yaml; yaml.safe_load(open('contracts/mcp-protocol-sdk-v1.yaml')); \
              yaml.safe_load(open('contracts/binding.yaml')); print('yaml ok')"
yaml ok        # exit 0
```

`make comply` chains `pmat comply check --path .` for its REPORT only — its holistic project-level
exit is informational per CLAUDE.md D-07 (the repo is intentionally mid-migration at the project
level) — and then enforces team-servers binding drift deterministically. **`make comply` exits 0.**
A plan citing "comply passed" is citing exactly this, and nothing stronger.
