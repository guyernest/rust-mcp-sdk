---
phase: 72
slug: investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-04-19
mode: decision-validation
---

# Phase 72 — Validation Strategy

> Phase 72 is a **research/decision phase**, not a code/implementation phase.
> Canonical Nyquist (test files → code requirements) is replaced by
> **decision-quality validation**: did the phase produce a recommendation
> that is traceable, falsifiable, and evidence-backed?

See 72-RESEARCH.md §"Validation Architecture" for full framing.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Markdown audit (no test framework) |
| **Config file** | `.planning/phases/72-.../72-DECISION-RUBRIC.md` |
| **Quick run command** | `grep -c` checks on deliverable structure (per REQ-ID row below) |
| **Full suite command** | Plan-checker agent review against the per-REQ criteria below |
| **Estimated runtime** | Manual audit, ~5 min per deliverable |

---

## Sampling Rate

- **After every plan wave:** Plan-checker agent reads produced deliverables against their REQ-ID criterion (PASS/FAIL).
- **Before `/gsd-verify-work`:** All 5 deliverables present and cross-document consistency confirmed (inventory rows ↔ strategy matrix ↔ PoC proposal ↔ rubric thresholds ↔ recommendation).
- **Max feedback latency:** Per-plan review (no continuous watch — decision work is discrete).

---

## Per-Task Verification Map (REQ-ID → Deliverable)

| Req ID | Plan | Deliverable | Validation Criterion | Automated Command | File Exists | Status |
|---------|------|-------------|----------------------|-------------------|-------------|--------|
| RMCP-EVAL-01 | 01 | `72-INVENTORY.md` | ≥15 pmcp module families, each with pmcp evidence (file:line) AND rmcp evidence (docs.rs anchor or GitHub blob URL+line) | `grep -cE '\| [a-zA-Z_/.]+:[0-9]+ \|' 72-INVENTORY.md` ≥ 15 | ❌ W0 | ⬜ pending |
| RMCP-EVAL-02 | 01 | `72-STRATEGY-MATRIX.md` | 5 options × 5 criteria = 25 cells, no `TBD`, each Adopt/Stay/Hybrid/Selective/Fork row scored with rationale | `grep -E '^\| (A\. Full adopt\|B\. Hybrid\|C\. Selective\|D\. Status quo\|E\. Fork)' ≥ 5`; `! grep -i 'TBD' 72-STRATEGY-MATRIX.md` | ❌ W0 | ⬜ pending |
| RMCP-EVAL-03 | 02 | `72-POC-PROPOSAL.md` | ≥2 PoC slices, each with `LOC: <N>` (<500), `Files:`, `Pass:`, `Fail:` sections, and at least one slice executable in ≤3 days | `grep -c '^### PoC Slice' 72-POC-PROPOSAL.md` ≥ 2; every slice block contains `LOC:`, `Files:`, `Pass:`, `Fail:` | ❌ W0 | ⬜ pending |
| RMCP-EVAL-04 | 02 | `72-DECISION-RUBRIC.md` | ≥5 falsifiable thresholds (numeric or boolean), each citing a named data source | `grep -c '^- \*\*Threshold:\*\*' 72-DECISION-RUBRIC.md` ≥ 5; every threshold line precedes a `Data source:` line | ❌ W0 | ⬜ pending |
| RMCP-EVAL-05 | 03 | `72-RECOMMENDATION.md` | Starts with `**Recommendation:** <A\|B\|C\|D\|E>` and has a subsection per rubric criterion cited from `72-DECISION-RUBRIC.md` | `head -5 72-RECOMMENDATION.md \| grep -E '^\*\*Recommendation:\*\* [A-E]'`; `grep -c '^### Criterion' 72-RECOMMENDATION.md` ≥ 5 | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

All 5 deliverables are created fresh in this phase — nothing pre-exists.

- [ ] `72-INVENTORY.md` — Plan 01 Wave 1
- [ ] `72-STRATEGY-MATRIX.md` — Plan 01 Wave 1
- [ ] `72-POC-PROPOSAL.md` — Plan 02 Wave 2
- [ ] `72-DECISION-RUBRIC.md` — Plan 02 Wave 2
- [ ] `72-RECOMMENDATION.md` — Plan 03 Wave 3

*No test framework install — decision validation is markdown-audit only.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Cross-document consistency: inventory ↔ strategy matrix ↔ PoC ↔ rubric ↔ recommendation | RMCP-EVAL-01..05 | Requires semantic judgment — e.g., "does PoC slice 2 actually exercise the extension points flagged risky in row 7 of the inventory?" | Reviewer reads all 5 docs and fills the Falsifiability Checklist in 72-RESEARCH.md §"Validation Architecture" |
| Recommendation justification quality | RMCP-EVAL-05 | Requires judging whether the chosen option's rationale meaningfully engages all 5 rubric criteria (not just name-drops them) | Reviewer confirms each `### Criterion` subsection in `72-RECOMMENDATION.md` cites a specific threshold outcome and supporting inventory/matrix row |

---

## Falsifiability Checklist (phase-gate — from 72-RESEARCH.md)

- [ ] Every row in `72-INVENTORY.md` cites a pmcp `file:line` AND an rmcp docs.rs anchor or GitHub blob URL with line number
- [ ] `72-STRATEGY-MATRIX.md` has all 25 cells filled (5 options × 5 criteria), no `TBD`
- [ ] `72-POC-PROPOSAL.md` names ≥1 slice that is ≤500 LOC touched AND ≥1 slice executable in ≤3 days (can be same slice)
- [ ] `72-DECISION-RUBRIC.md` has ≥5 falsifiable thresholds, each citing its data source
- [ ] `72-RECOMMENDATION.md` picks one of {A, B, C, D, E} and the justification engages every rubric criterion

---

## Validation Sign-Off

- [ ] All 5 deliverables produced in their target Wave
- [ ] Falsifiability Checklist above fully checked
- [ ] Cross-document consistency confirmed by plan-checker
- [ ] `nyquist_compliant: true` set in frontmatter (already set — decision-validation framing satisfies Nyquist intent)

**Approval:** pending
