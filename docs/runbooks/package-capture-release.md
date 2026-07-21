# Release Runbook: `cargo pmcp package capture` (+ `show` / `import` / `approve`)

**Scope.** `cargo pmcp package capture` is a thin GraphQL client of pmcp.run's
remote capture API — the SDK does not implement capture logic, it only calls it.
This runbook covers the execution logistics to ship the verb: rebasing the
long-lived feature branch, running the not-yet-executed end-to-end test against
the dev endpoint, and the coordinated `cargo-pmcp` release. These steps are
intentionally kept out of the design spec
(`docs/superpowers/specs/2026-07-20-package-capture-contract-seam-design.md`,
§8) because they go stale — treat every count and version number below as a
snapshot that must be re-verified at execution time, not copied blind.

All four verbs — `capture`, `show`, `import`, `approve` — live on one branch
(`feat/package-remote-capture-show`) and **ship together in a single
`cargo-pmcp` release**. There is no way to release `capture` alone.

## Pre-flight checklist

- [ ] `/opt/homebrew/bin/git status` on `feat/package-remote-capture-show` is clean (no uncommitted work).
- [ ] Confirm PR [#303](https://github.com/paiml/rust-mcp-sdk/pull/303) (`ci(release): publish pmcp-package/agent/team-servers before cargo-pmcp`) is **merged**. It fixes a `release.yml` crate-ordering bug where `cargo-pmcp` published before its `pmcp-agent` / `pmcp-package` dependencies. As of this writing it is **still open** — do not tag a release relying on the automated publish order until it merges; if it's still open when you get here, either land it first or manually verify/adjust the publish order in `release.yml`.
- [ ] You have either interactive pmcp.run credentials (for `cargo pmcp login`) or M2M env credentials for the dev environment, for the E2E phase.
- [ ] You know which AgentTeam **slug id** (not UUID) you'll capture against for the E2E run (e.g. `day-trip-planner-team`).
- [ ] **`import` E2E is green before you tag.** The release ships `capture`/`show`/`import`/`approve` together, but `import`'s happy path (`completed_dry_run` → the rendered pre-flight disposition table) had **never** run live as of 2026-07-21 — it was blocked server-side by the per-component pull-by-payload-digest bug (pmcp.run Phase-171 handoff `171-HANDOFF-import-component-pull-digest.md`; FIX #1 is platform-only, in their import Lambda `pull.rs`, no SDK change). The CLI verb is a correct thin client, but do not tag a release shipping a verb whose success path is unproven. Once the platform deploys their fix, run `cargo pmcp package import day-trip-planner-team@1.0.0` and confirm it returns `completed_dry_run` with all-`reuse` dispositions and a clean report. (`capture`/`approve` are already verified; this gate is `import`-specific.)
- [ ] Use the **absolute git binary** (`/opt/homebrew/bin/git`) for every rebase/diff/status command in this runbook. The `rtk` shell proxy in this environment corrupts `git diff` / `git status` / `gh` output — a prior session lost time to this before identifying it. Prefer `/opt/homebrew/bin/gh` for the same reason if `gh` output looks garbled.

---

## Phase 1 — Rebase onto `origin/main`

The branch is long-lived and has drifted from `origin/main` in both directions.
Verify the current divergence yourself; do not trust a stale count:

```bash
/opt/homebrew/bin/git fetch origin main
/opt/homebrew/bin/git rev-list --count HEAD..origin/main   # commits behind
/opt/homebrew/bin/git rev-list --count origin/main..HEAD   # commits ahead
```

As of 2026-07-20 this was **111 commits behind, 316 ahead**. The "ahead" side
is this branch's own history (capture/show/import/approve + the GSD planning
trail) and is expected to stay large until merge — it is not a rebase risk by
itself.

**On the "behind" side:** a prior session's spot check found the large
majority of behind-commits are automated `docs: update quality badges and
report` bot commits that touch only the README quality-badge lines — those
are trivially low-conflict. **Do not assume all of them are**, though:
re-verify the split before you start, since real work also lands on
`origin/main` between snapshots (e.g. unrelated feature phases merging):

```bash
/opt/homebrew/bin/git log --oneline HEAD..origin/main | grep -c "quality badges"
/opt/homebrew/bin/git log --oneline HEAD..origin/main | grep -vc "quality badges"
```

At the 2026-07-20 snapshot this was ~87 badge-bot commits vs. ~24 non-badge
commits (real upstream work, e.g. an unrelated secrets-deployment phase). The
non-badge commits are not automatically a problem — most touch files this
branch never does — but they mean "low-conflict," not "zero-conflict."

Rebase:

```bash
/opt/homebrew/bin/git checkout feat/package-remote-capture-show
/opt/homebrew/bin/git rebase origin/main
```

**Expected conflicts:** at most trivial README quality-badge line conflicts
from the bot commits. Resolve those by taking `origin/main`'s version of the
badge line(s):

```bash
/opt/homebrew/bin/git checkout --theirs README.md   # only if the ONLY conflict is the badge block
/opt/homebrew/bin/git add README.md
/opt/homebrew/bin/git rebase --continue
```

**Fallback — non-badge conflicts.** If the rebase surfaces a conflict outside
the README badge block (or a badge-block conflict that also touches
surrounding content you don't recognize), **stop and treat it as a signal of
real divergence, not something to auto-resolve.** Inspect the conflicting
commit on `origin/main` with `/opt/homebrew/bin/git log -1 <sha>` and
`/opt/homebrew/bin/git show <sha>`, understand what changed, and resolve it
deliberately (or pull in a teammate who owns that area). Do not blanket
`--theirs`/`--ours` outside the known-trivial badge case.

After a clean rebase, re-run the counts to confirm `origin/main..HEAD` is back
to (approximately) 316 and `HEAD..origin/main` is 0, then re-run the full test
suite before moving on (`make quality-gate` in Phase 4 will do this, but a
quick `cargo test -p cargo-pmcp` here catches rebase breakage early).

---

## Phase 2 — End-to-end test against dev

**Status: run and passing (2026-07-20).** A locally-built CLI ran
`cargo pmcp package capture day-trip-planner-team --version 1.0.0` against the
dev platform to a terminal `✅ Capture complete`, manifest digest
`sha256:af0ae208cceb706a492c03ff0d79e970c76eaf54b9596d54b2229c7e2de1f249`. A
re-run produced the **identical** digest, confirming the platform capture is
deterministic (same team + version → byte-identical manifest). Re-run this
before each release to confirm the deployed platform still answers.

**Endpoints (from the discovered `~/.pmcp/pmcp-run-config.json`).** Two
different URLs — do not conflate them:

- **Environment / discovery base** (`source_api_url` / `mcp_url`) —
  `https://6vlog1csj4.execute-api.us-east-1.amazonaws.com`. This is what
  `PMCP_API_URL` wants (see Auth below); it is the API-Gateway base, **not** the
  GraphQL URL.
- **GraphQL endpoint** the capture ops POST to (`graphql_url`) — **the CLI
  resolves this dynamically from your discovery cache; do not hardcode it.** In
  one 2026-07 dev config it was an `…appsync-api.us-east-1.amazonaws.com/graphql`
  merged-API URL, but the platform team has flagged specific merged-API ids as
  rotating/dead (e.g. `nieihn7yhjbzldmrm6b74ndcha`, `pn5dorma2bdhzcdhascvc4xzka`
  — the latter also being a wrong SDL-introspection source; the authoritative
  source data API is `amplifyData` apiId `q3gd4hrbabeytc2o2zazld6igy`). So rely
  on discovery: **no `PMCP_RUN_GRAPHQL_URL` override is needed**, and don't paste
  a specific merged-API id anywhere as if it were stable.

**Auth — the `PMCP_API_URL` token-refresh gotcha (real snag).** A fresh
`cargo pmcp login` works, but when the cached **access token expires**, the
refresh path runs discovery against the prod default
`https://api.pmcp.run/.well-known/pmcp-config` (which does not resolve for dev)
**unless** you point it at the environment base. It does not reuse the cached
config's endpoint for refresh. So for any dev run, set:

```bash
export PMCP_API_URL="https://6vlog1csj4.execute-api.us-east-1.amazonaws.com"
```

or persist it once so plain `cargo pmcp package capture …` just works:

```bash
cargo pmcp configure add dev --type pmcp-run \
  --api-url https://6vlog1csj4.execute-api.us-east-1.amazonaws.com
cargo pmcp configure use dev
```

Then authenticate (only needed if you have no valid refresh token):

```bash
cargo pmcp login          # interactive PKCE flow
# — or —
# set M2M env credentials per the CLI's documented env vars, no interactive step
```

**Run the capture.** The positional argument is the AgentTeam's **slug id**
(e.g. `day-trip-planner-team`), **not a UUID**:

```bash
cargo pmcp package capture <team-slug-id> --version <x.y.z>
```

> **Known doc bug (tracked separately, Task 6):** `capture.rs`'s current
> `long_help` for this argument says `AgentTeam ID (UUID) — the team's id, not
> its display name.`, which is wrong — the CLI takes the slug id, not a UUID.
> Fix this in the CLI help text as part of (or before) this release; don't let
> the E2E run mask it — verify with a real slug id, not something that merely
> looks like one.

**Success criteria.** The command must reach `completed` status and print a
real manifest digest — not a timeout, not an error status, not a stub value.
Watch the poll loop: `MAX_POLL_WAIT` is 20 minutes, which exceeds the
platform's capture Lambda's 15-minute timeout, so a genuine backend hang will
surface as a normal poller timeout rather than hanging indefinitely. If you
hit the 20-minute bound, that's a real signal (backend stuck or erroring),
not a runbook problem — investigate on the platform side before retrying.

Record the slug id used, the version tag, and the resulting manifest digest
for the release notes / follow-up ticket trail.

---

## Phase 3 — Contract/drift invariant (must be green before release)

The CLI's two hand-written GraphQL operations are checked offline against a
vendored SDL contract:

```bash
cargo test -p cargo-pmcp --test package_capture_contract
```

This must show all **3 tests passing** before you proceed. It validates the
CLI's `SUBMIT_PACKAGE_CAPTURE_QUERY` / `GET_PACKAGE_CAPTURE_STATUS_QUERY`
operations and their response structs against
`contracts/pmcp-run/capture-v1.graphql`.

**Ownership model — read before "fixing" a failure here.** The vendored SDL
is **platform-exported**: the platform owns its contents and PRs updates to
this repo whenever their capture schema changes (see design spec §5b, §6).
The SDK does **not** introspect the source GraphQL API to regenerate this
file — that API is IAM-only, reachable only from the platform's own backend,
and is not client-reachable (no SDK-held credential, user or M2M, can reach
it directly). There is no SDK-side scheduled drift job; the "online" half of
the drift check is entirely platform-owned — they detect schema drift on
their side and open a PR against `capture-v1.graphql` here, which then trips
this same blocking test to force the CLI into lockstep.

**What this means for release:** you do **not** regenerate or re-export the
SDL as part of this release. You only need the offline test green against
whatever `capture-v1.graphql` is currently vendored in the branch. If it's
red, that means the CLI's queries/structs have drifted from the vendored
contract — fix the CLI code (or pull the platform's latest contract-update PR
if one is pending) before continuing; do not weaken or skip the test.

---

## Phase 4 — Coordinated `cargo-pmcp` release

`capture` / `show` / `import` / `approve` ship together in **one**
`cargo-pmcp` release, since they're all on this one branch.

1. **Bump the version.** Update `cargo-pmcp/Cargo.toml`'s `version` field to
   `<x.y.z>` (choose per semver: new verbs → at least a minor bump). If this
   branch also touched `pmcp` itself, bump it too and update the `pmcp = {
   version = "..." }` line(s) that pin it — per the repo's standard
   cross-crate version-bump rule (any downstream crate pinning a bumped dep
   must bump its own pin and version). `cargo-pmcp` currently depends on
   `pmcp`, `mcp-tester`, and `mcp-preview` — confirm none of those also need a
   coordinated bump before tagging.

2. **Run the mandatory quality gate** — this is the same gate CI runs, and is
   stricter than individual `cargo` commands (fmt --all, clippy
   pedantic+nursery, build, test, audit):

   ```bash
   rustup update stable   # local/CI toolchain mismatch is the #1 cause of CI-only failures
   make quality-gate
   ```

   Do not substitute bare `cargo clippy`/`cargo test` calls for this — they
   are weaker than what CI enforces and will miss lints.

3. **Commit the version bump(s), push, PR to upstream `main`** per the
   standard release flow:

   ```bash
   git add cargo-pmcp/Cargo.toml   # + any other bumped Cargo.toml files
   git commit -m "chore: bump cargo-pmcp v<x.y.z>"
   git push -u origin feat/package-remote-capture-show
   gh pr create --repo paiml/rust-mcp-sdk --base main
   ```

4. **After the PR merges and CI is green, tag and push** the tag to
   `upstream` (not your fork):

   ```bash
   git checkout main && git pull upstream main
   git tag -a v<x.y.z> -m "cargo-pmcp v<x.y.z> - package capture/show/import/approve"
   git push upstream v<x.y.z>
   ```

   Pushing a `v*` tag to upstream triggers `.github/workflows/release.yml`,
   which publishes to crates.io in dependency order (with the PR #303
   crate-ordering fix applied — re-confirm it's merged if you skipped the
   pre-flight check above), publishes to the MCP Registry via OIDC-authenticated
   `mcp-publisher`, and attaches cross-platform `mcp-tester`/`cargo-pmcp`
   binaries to the GitHub Release.

5. **Verify the publish.** Check crates.io for the new `cargo-pmcp` version,
   and spot-check the release workflow run in GitHub Actions for the
   crates.io + MCP Registry publish steps succeeding (not just the build/test
   jobs).

---

## Summary — order of operations

1. Rebase (`/opt/homebrew/bin/git rebase origin/main`) — expect low-conflict,
   badge-line-only; escalate anything else.
2. E2E against dev — set `PMCP_API_URL=https://6vlog1csj4.execute-api.us-east-1.amazonaws.com`
   (token-refresh base; the GraphQL URL resolves from discovery), run capture to
   `completed` + a real manifest digest; use a slug id, not a UUID.
3. Offline contract test green (`cargo test -p cargo-pmcp --test
   package_capture_contract`, 3 tests) — no SDL regeneration needed, that's
   platform-owned.
4. Coordinated `cargo-pmcp` release: bump → `make quality-gate` → PR → merge
   → tag `v<x.y.z>` → automated publish via `release.yml` (confirm PR #303 is
   merged first).
