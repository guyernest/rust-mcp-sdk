---
phase: 120-config-server-packaging
verified: 2026-08-23T15:13:59Z
status: gaps_found
score: 4/4 roadmap success criteria verified; 2 additional CRITICAL defects found and independently reproduced
behavior_unverified: 0
overrides_applied: 0
gaps:
  - truth: "aggregate() correctly preserves two ConfigSlots that share the same SlotType::key() (kind, name) but declare different config_key values — the field this phase introduced specifically to record where a resolved value is written must never be silently discarded during dedup"
    status: failed
    reason: "CR-02 (from 120-REVIEW.md), independently reproduced by this verifier. aggregate()'s dedup guard at crates/pmcp-package/src/slot/aggregate.rs:34 is `Entry::Occupied(e) if e.get().slot == slot.slot`, which compares only the SlotType field, not the full ConfigSlot (config_key is excluded). Two slots such as a Secret named TFL_APP_KEY filling backend.auth.query_params.app_key and a second Secret of the same name filling backend.auth.headers.app_key collapse into one entry; the second config_key is silently lost. Reproduced directly against the shipped aggregate() function: aggregate([&a, &b]) with distinct config_key values returns 1 entry, not 2 (verifier repro removed after confirming; not left in the tree). The map key (slot.slot.key()) also excludes config_key, compounding the loss. This does not break the single london-tube proving fixture (its three slots have distinct (kind,name) pairs), so roadmap SC3's literal assertion holds for that fixture — but the defect undermines the general correctness of the PKG-03 'aggregate returns the config slots' machinery for any package with two config-value slots sharing a name."
    artifacts:
      - path: "crates/pmcp-package/src/slot/aggregate.rs"
        issue: "Dedup/conflict key omits ConfigSlot.config_key (lines 26-34); comment calls the guard 'byte-equal declaration' but it is not — ConfigSlot derives PartialEq over both fields and that comparison was available and not used."
    missing:
      - "Make config_key part of both the BTreeMap key and the occupied-entry equality check in aggregate(), and add a regression test asserting two same-name slots with different config_key both survive aggregation (per the fix sketched in 120-REVIEW.md CR-02)."
  - truth: "scripts/check-release-coverage.sh (invoked by `make quality-gate`, Makefile:896, and the CI quality-gate job) fails loudly when its cargo-metadata/jq pipeline breaks, rather than silently reporting success over an empty crate list"
    status: failed
    reason: "CR-01 (from 120-REVIEW.md), independently reproduced by this verifier. `mapfile -t PUBLISHABLE < <(cargo metadata ... | jq ...)` does not propagate the process-substitution's exit status even under `set -euo pipefail` (pipefail does not cover process substitution). A failing jq/cargo-metadata pipeline yields an empty PUBLISHABLE array, the for-loop body never runs, `missing` stays empty, and the script prints 'all 0 publishable workspace members have a publish step' and exits 0. Reproduced standalone: `mapfile -t P < <(echo not-json | jq -r '.packages[]' 2>/dev/null | sort); echo count=${#P[@]}` -> count=0, exit 0. This script is chained into `make quality-gate` and the CI quality-gate job, so both can report green while verifying nothing. Git archaeology shows the python3->jq refactor (commit 1ed946e6, 2026-08-22 23:22) landed on this working branch between phase-120 wave-1 and wave-2 commits — i.e. inside this phase's execution window, though not authored by a numbered 120-0X plan and not listed in any 120-0X PLAN's files_modified. It is NOT one of the four roadmap PKG-01/02/03 success criteria for Phase 120, and Phase 124 (PKGR-01)'s success criteria address a DIFFERENT residual gap (workspace-excluded crate coverage), not this exit-code/pipefail defect — so this is not a deferred item, it is a live, currently-unresolved defect in a file this phase's window touched."
    artifacts:
      - path: "scripts/check-release-coverage.sh"
        issue: "Lines 19-26: mapfile over a process-substituted jq pipeline swallows a pipeline failure; no post-hoc check that jq is installed or that the resulting array is non-empty."
    missing:
      - "Fail loudly if jq is absent; capture `cargo metadata` output first and check its own exit code before piping to jq; refuse (exit 1) if PUBLISHABLE ends up empty (per the fix sketched in 120-REVIEW.md CR-01)."
deferred: []
---

# Phase 120: Config-Server Packaging Verification Report

**Phase Goal:** A server whose entire identity is a `config.toml` plus an OpenAPI spec has a complete package identity — vendor media types carry both as layers, the binary is dual-mode (embedded bootstrap bytes, or a `BinaryRef { digest, media_type }` resolved in the target environment), and the baked-versus-slot split is decided, documented and machine-checkable.
**Verified:** 2026-08-23T15:13:59Z
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (Roadmap Success Criteria)

| # | Truth (roadmap SC) | Status | Evidence |
|---|---|---|---|
| 1 | A `pmcp-openapi-server` package built from `london-tube.toml` + `london-tube-api.yaml` packs to a local OCI layout with no bootstrap layer, under new `application/vnd.pmcp.*` vendor media types, and `unpack_server` restores both files byte-identically (PKG-01) | ✓ VERIFIED | `MT_SERVER_CONFIG`/`MT_SERVER_OPENAPI_SPEC`/`MT_SERVER_BINARY_REF` constants exist in `crates/pmcp-package/src/oci/media_types.rs:73,90,96`; `pack_server`'s doctest (`pack.rs:244`) packs a referenced-binary + config-only package and asserts `unpacked.config.unwrap().bytes == config_toml` and `unpacked.spec == None`; `config_only_package_manifest_carries_no_bootstrap_layer`, `config_only_package_restores_config_bytes_verbatim_under_its_original_name`, `a_packed_spec_restores_its_bytes_verbatim_under_its_original_name`, `the_real_london_tube_fixture_packs_as_a_config_only_package` all pass (`cargo test --manifest-path crates/pmcp-package/Cargo.toml --test config_server`, 29/29 passed, run by this verifier). |
| 2 | Both binary modes round-trip; a referenced package unpacked in an environment without the blob reports the digest to resolve rather than a missing-layer error, and a caller cannot mistake referenced for embedded (PKG-02) | ✓ VERIFIED | `BinaryMode`(pack)/`UnpackedBinary`(unpack) in `oci/pack.rs:104`, `oci/unpack.rs:90` — `Referenced` arm carries `digest`/`media_type`, NO bytes field. `read_binary_mode` (`unpack.rs:167-192`) returns `Ok(UnpackedBinary::Referenced{digest,media_type})` for a referenced-only manifest (not an `Err`). An absent/null wire digest is rejected (`a_binary_ref_layer_with_no_digest_is_rejected_at_unpack` passes). `an_embedded_package_still_round_trips_its_bootstrap_bytes` and `well_formed_0_2_0_packages_of_either_binary_mode_still_unpack` pass. |
| 3 | The baked-vs-slot split is enforced: one byte of `london-tube-api.yaml` changes the manifest digest and `digest::verify` rejects the stale digest, while endpoint/credentials/auth-mode surface as `ConfigSlot`s via `classify`/`aggregate` with no spec-derived slot (PKG-03) | ✓ VERIFIED (fixture path), with a confirmed underlying defect — see Gap 1 | `one_flipped_spec_byte_moves_the_packed_digest_and_the_stale_one_is_rejected` (digest_stability.rs:427) passes and directly proves both halves (digest moves AND stale digest -> `PackageError::DigestMismatch`). `the_real_fixtures_three_slots_classify_aggregate_and_carry_no_spec_derived_slot` (config_server.rs:1158) passes: the 3 real london-tube slots classify as 2 BehaviorRelevant (endpoint, auth_mode) + 1 IdentityBearing (secret), no spec-derived slot. **However**, `aggregate()`'s dedup logic has a CRITICAL, independently-reproduced bug (CR-02, see Gaps) that silently discards `config_key` when two slots share `(kind,name)` — untested and unguarded, even though it doesn't break this specific 3-slot fixture. |
| 4 | A golden fixture pins the config-only package's canonical digest, so a later layer-set/layer-order/media-type change fails `digest_stability.rs` (PKG-01, PKG-02) | ✓ VERIFIED | `EXPECTED_CONFIG_SERVER_PACKED_MANIFEST_DIGEST` pinned at `digest_stability.rs:388`; `config_server_packed_manifest_digest_matches_pinned_constant` passes. Layer-order insensitivity proven by `any_layer_permutation_unpacks_to_an_equal_server` (property test that rewrites the content-addressed manifest, per the Codex-review-incorporated fix noted in 120-02-PLAN.md) — passes. Duplicate-media-type and both/neither-binary-arm rejections proven by `negative.rs`'s `server_layout::*` tests — all pass. |

**Score:** 4/4 roadmap success criteria hold on their own terms; 2 additional CRITICAL defects independently confirmed in the surrounding machinery (see Gaps).

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/pmcp-package/src/oci/media_types.rs` | `MT_SERVER_CONFIG`, `MT_SERVER_OPENAPI_SPEC`, `MT_SERVER_BINARY_REF` | ✓ VERIFIED | Present, documented, used by `pack_server`/`unpack_server`. |
| `crates/pmcp-package/src/oci/pack.rs` | `BinaryMode`, `ConfigFile`, `OpenApiSpecFile`, new `pack_server` signature | ✓ VERIFIED | Present; `binary_ref` field removed from `ServerPackage` (`grep -n 'pub binary_ref' crates/pmcp-package/src/package/server.rs` returns nothing). |
| `crates/pmcp-package/src/oci/unpack.rs` | `UnpackedBinary`, `UnpackedServer`, `index_layers`, `read_binary_mode`, `detect_legacy_shape` | ✓ VERIFIED | Present; `index_layers` rejects duplicate media types; `detect_legacy_shape` refuses a 0.1.x envelope by name (`server_layout::an_envelope_carrying_the_legacy_binary_ref_shape_is_refused_by_name` passes). |
| `crates/pmcp-package/src/oci/config_validation.rs` | `validate_config_slot_placeholders`, `parse_declared_config_slots`, `validate_config_slot_agreement`, `is_env_reference` | ✓ VERIFIED | Present; agreement + placeholder rejection tests pass (`pack_server_refuses_a_declaration_the_package_does_not_carry`, `pack_server_refuses_a_config_that_bakes_a_slot_declared_credential`, etc). |
| `crates/pmcp-package/src/slot/types.rs` | `SlotType::Endpoint`, `SlotType::AuthMode`, `ConfigSlot.config_key` | ✓ VERIFIED | Present; snake_case discriminators confirmed by round-trip tests; `config_key` is `#[serde(default, skip_serializing_if)]` (additive wire, breaking source) as documented. |
| `crates/pmcp-package/src/slot/required.rs` | `required_slots`, `RequiredSlot` | ✓ VERIFIED (present/wired) — minor: `RequiredSlot` lacks `#[non_exhaustive]` unlike its sibling `ConfigSlot` (info-level, IN-02) | `required_slots` preserves duplicates (does not dedupe) per its documented contract; `ordering_is_stable_under_permutation` passes. |
| `crates/pmcp-package/tests/golden_fixtures/config_server_london_tube_v1/` | Byte-identical copies of the real london-tube fixture | ✓ VERIFIED | `the_vendored_london_tube_fixtures_have_not_drifted_from_their_sources` passes. |
| `crates/pmcp-package/tests/golden_fixtures/env_ref_grammar_v1.tsv` + `crates/pmcp-server-toolkit/tests/env_ref_grammar_parity.rs` | Shared cross-crate accept/reject table | ✓ VERIFIED | Both sides' parity tests pass (`is_env_reference_agrees_with_the_shared_grammar_table_on_every_row`, `parse_env_ref_agrees_with_the_shared_grammar_table_on_every_row`). |
| `crates/pmcp-server-toolkit/src/config.rs`, `env_ref.rs` | `ConfigSlotDecl`, `ConfigSlotKind`, `ServerConfig.config_slots`, `resolved_base_url`, relocated `parse_env_ref` chokepoint | ✓ VERIFIED | 7/7 `base_url_expansion.rs` tests pass under `--features http` (0 tests without the feature flag — file is correctly `#![cfg(feature = "http")]` gated, not a false green). |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `pack_server`'s binary-ref layer | `unpack_server`'s `read_binary_mode` | media-type-keyed lookup | ✓ WIRED | Confirmed by source read + `server_layout::*` tests. |
| `pack_server`'s `ANNOTATION_TITLE` | unpack's restored file name | `org.opencontainers.image.title` annotation | ✓ WIRED | `config_only_package_restores_config_bytes_verbatim_under_its_original_name` passes. |
| `pmcp-package` 0.2.0 | 4 workspace version requirements + `pmcp_package_pin.rs` tripwire + `cargo-pmcp` scaffold template | caret-pin propagation | ✓ WIRED | `pmcp_package_pin_is_the_expected_caret_line` passes; `cargo build`/`cargo test` succeed for cargo-pmcp, pmcp-agent, pmcp-team-servers, pmcp-cfn-renderer against pmcp-package 0.2.0. |
| `[[config_slots]]` in config bytes | `parse_declared_config_slots` -> `validate_config_slot_agreement` against `package.config_slots` | pack-time re-parse (D-01) | ✓ WIRED | `pack_server_refuses_a_declaration_the_package_does_not_carry`, `..._refuses_a_kind_disagreement...`, `..._refuses_a_name_disagreement...`, `..._refuses_a_tested_value_disagreement...` all pass. |
| `ConfigSlot.config_key` | pack-time placeholder validation dotted-path lookup | `resolve_dotted_key` | ✓ WIRED | `pack_server_refuses_a_config_that_bakes_a_slot_declared_credential` passes; `resolve_dotted_key_never_panics_on_arbitrary_dotted_keys` fuzz-style property test passes. |
| `crate::env_ref::parse_env_ref` | `http::auth`, `resolved_base_url`, pmcp-package's `is_env_reference` (documented parity) | shared chokepoint | ✓ WIRED | Confirmed by source read + both parity tests passing. |
| `SlotType::key()` dedup | `aggregate()`'s `BTreeMap` | dedup/conflict guard | ⚠ WIRED BUT DEFECTIVE | See Gap 1 (CR-02) — the guard silently drops `config_key` on a `(kind,name)` collision instead of preserving both entries or erroring. |

### Behavioral Spot-Checks / Test Execution (run directly by this verifier, not taken from SUMMARY claims)

| Behavior | Command | Result | Status |
|---|---|---|---|
| Full `pmcp-package` test suite (unit + 5 integration files + 8 doctests) | `cargo test --manifest-path crates/pmcp-package/Cargo.toml` | 160 lib + 20 digest_stability + 14 negative + 4 roundtrip + 29 config_server + 8 doctests, all passed | ✓ PASS |
| `pmcp-server-toolkit` base_url/env-ref tests | `cargo test -p pmcp-server-toolkit --test env_ref_grammar_parity --test base_url_expansion --features http` | 1 + 7 passed | ✓ PASS |
| `cargo-pmcp` package-inspect + version-pin tripwire | `cargo test --manifest-path cargo-pmcp/Cargo.toml --test package_inspect --test pmcp_package_pin` | 3 + 1 passed | ✓ PASS |
| CR-02 reproduction: `aggregate()` on two same-`(kind,name)`, different-`config_key` slots | inline `#[test]` added to `aggregate.rs`, run, then reverted (`git checkout --`) | `assertion left==right failed: left: 1, right: 2` — bug confirmed live | ✗ FAIL (confirms the gap) |
| CR-01 reproduction: `mapfile` over a failing `jq` pipeline under `set -euo pipefail` | `mapfile -t P < <(echo not-json \| jq -r '.packages[]' 2>/dev/null \| sort); echo count=${#P[@]}` | `count=0`, script's own logic would print success and exit 0 | ✗ FAIL (confirms the gap) |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| PKG-01 | 120-01, 120-02 | No-bespoke-binary config-only server packing via vendor media types | ✓ SATISFIED | Roadmap SC1 verified above. |
| PKG-02 | 120-01, 120-02, 120-05 | Dual-mode binary (embedded / referenced) | ✓ SATISFIED | Roadmap SC2 verified above. |
| PKG-03 | 120-03, 120-04, 120-05 | Baked-vs-slot split, decided and machine-checked | ✓ SATISFIED for the shipped proving fixture; underlying `aggregate()` machinery has a confirmed unaddressed defect (Gap 1) | Roadmap SC3 verified above with caveat. |

No orphaned requirements: REQUIREMENTS.md maps exactly PKG-01/PKG-02/PKG-03 to Phase 120, and all three appear in at least one plan's `requirements:` frontmatter (120-01/02: PKG-01,PKG-02; 120-03/04: PKG-03; 120-05: PKG-01,PKG-02,PKG-03).

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---|---|---|---|
| `crates/pmcp-package/src/slot/aggregate.rs` | 26-34 | Dedup guard compares `SlotType` only, silently drops `config_key` on a `(kind,name)` collision (CR-02) | 🛑 Blocker | Silently loses "where a resolved credential is written" for any package with two config-value slots sharing a name — the exact hazard `config_key` was added to prevent. Independently reproduced by this verifier. |
| `scripts/check-release-coverage.sh` | 19-26 | `mapfile` over a process-substituted `jq` pipeline swallows pipeline failure; empty result set reports success (CR-01) | 🛑 Blocker | Chained into `make quality-gate` and CI's `quality-gate` job — both can report green while verifying nothing. Independently reproduced by this verifier. Landed on this branch inside phase 120's execution window (between wave-1 and wave-2 commits) but not authored by a numbered 120-0X plan; not addressed by Phase 124's stated success criteria (which target workspace-excluded-crate coverage, a different gap). |
| `README.md` / `crates/pmcp-openapi-server/examples/london_tube_min.rs` | 64 / 17 | Two in-repo invocation snippets for `london-tube.toml` were not updated after this phase made `base_url` a required `${TFL_BASE_URL}` slot (WR-02) | ⚠ Warning | Confirmed: `examples/london-tube.toml` requires `TFL_BASE_URL`/`TFL_APP_KEY`; both snippets omit them and would fail at dispatch with `DispatchError::UnresolvedBaseUrl`. |
| `crates/pmcp-agent/src/config/resolver.rs` | 158-165 | `SlotType::Endpoint` falls back silently to the package's `tested_value` on an unset env var, and `detect_deviation` suppresses the warning in exactly that fallback case (WR-04) | ⚠ Warning | Contradicts the sibling `resolved_base_url` in `pmcp-server-toolkit`, which deliberately errors on an unset endpoint. Confirmed by source read; zero test coverage of either new `SlotType` arm in this resolver (WR-05, confirmed: `grep -c "SlotType::Endpoint" crates/pmcp-agent/tests/*.rs` → 0). |
| `crates/pmcp-package/src/oci/unpack.rs` | 396-406 | `unpack_single_layer` (agent/team/workflow) lacks the media-type + duplicate-layer hardening `unpack_server`'s `index_layers` added (WR-06) | ⚠ Warning | The stated threat model ("shadow the real layer with an attacker's") applies identically to the unprotected sibling entry points. |
| `scripts/check-release-coverage.sh` | 31 | Regex narrowed to `cargo publish -p ${crate}`, dropping the `--manifest-path` form `release.yml` uses for `pmcp-package` (WR-07) | ⚠ Warning | Currently masked because `pmcp-package` is workspace-excluded and invisible to `cargo metadata --no-deps`; becomes a false-missing report once Phase 124 closes that gap. |
| `crates/pmcp-package/src/slot/types.rs` | 80-102 | `Endpoint`/`AuthMode.tested_value` is unvalidated free text serialized into a package layer (WR-08) | ⚠ Warning | A mis-declared `kind = "endpoint"` slot can carry a resolved credential in `tested_value`, outside the placeholder rule's scope. |
| `cargo-pmcp/fuzz/corpus/fuzz_package_kind/` | — | New fuzz seed corpus is never exercised by any Makefile target or workflow (WR-09) | ⚠ Warning | `make test-fuzz` only walks the root `fuzz/` directory. |
| `cargo-pmcp/src/commands/package/inspect.rs` | 118-123, 164-169 | `render_server` drops `binary`/`config`/`spec` from `UnpackedServer`; no test inspects a config-only package (WR-10) | ⚠ Warning | Confirmed: `render_server(pkg: &ServerPackage)` takes only the inner package, and `package_inspect.rs`'s 3 tests cover a non-layout-path rejection, a zero-manifest rejection, and an agent fixture — none exercise a config-only server package. |
| `crates/pmcp-package/README.md` | "0.1 -> 0.2 break" section | Blanket "no 0.1.x reader in 0.2.x" claim; break is actually confined to `mcp-server` packages (WR-11) | ℹ Info | `EXPECTED_WORKFLOW_DIGEST`/`EXPECTED_AGENT_DIGEST`/`EXPECTED_TEAM_DIGEST` are unchanged, confirming agent/team/workflow 0.1.x packages still unpack under 0.2.0. |
| `crates/pmcp-server-toolkit/src/config.rs`, `crates/pmcp-package/src/slot/required.rs`, `cargo-pmcp/fuzz/.../.gitignore`, `cargo-pmcp/src/commands/package/kind.rs`, `crates/pmcp-package/src/oci/config_validation.rs` + `env_ref.rs` | various | IN-01 through IN-05 from 120-REVIEW.md | ℹ Info | Not independently re-verified beyond the review's own evidence; none affect PKG-01/02/03. |

No `TBD`/`FIXME`/`XXX` debt markers found in the phase's modified files.

### Human Verification Required

None. All findings above are either confirmed via direct test execution/source read (this verifier) or carried forward from the code review with source citations; no item requires subjective/visual/runtime judgment beyond what was already exercised.

### Gaps Summary

Phase 120's four roadmap success criteria (PKG-01, PKG-02, PKG-03) all hold on their own terms, proven by 190+ passing tests this verifier ran directly (not taken from SUMMARY claims), and the config-only packaging / dual-mode-binary / baked-vs-slot mechanics are real, well-tested, and correctly wired for the shipped london-tube proving fixture.

However, two CRITICAL defects flagged by the phase's own code review (120-REVIEW.md) were independently reproduced by this verifier and remain unfixed in the tree:

1. **CR-02** — `aggregate()` silently discards a `ConfigSlot`'s `config_key` when two slots share `(kind,name)` — the exact scenario `config_key` (a field this phase introduced) exists to distinguish. This is a genuine correctness gap in a PKG-03 core artifact; it does not break the single shipped fixture, but it is untested and will silently misroute a resolved value's target config path the first time a real package needs two same-named slots at different config paths.
2. **CR-01** — `scripts/check-release-coverage.sh`, chained into `make quality-gate` and CI, can report success while verifying zero crates if its `jq`/`cargo metadata` pipeline fails. This is not one of Phase 120's PKG-01/02/03 criteria and landed on this branch without being authored by a numbered 120-0X plan, but it is a live, reproducible defect in a file touched during this phase's execution window, and it is not addressed by Phase 124's stated success criteria (which target a different residual gap — workspace-excluded crate coverage).

Recommendation: land the CR-02 fix (make `config_key` part of `aggregate()`'s key/equality check, plus a regression test) before this phase closes, since it is squarely inside PKG-03's scope. CR-01 should be tracked and fixed promptly (it undermines every future quality-gate run) but can reasonably be picked up as a fast-follow rather than blocking Phase 120's own closure, since it predates this phase's plans and Phase 120 did not introduce it.

---

_Verified: 2026-08-23T15:13:59Z_
_Verifier: Claude (gsd-verifier)_
