#!/usr/bin/env bash
# Every publishable workspace member MUST have a publish step in release.yml.
#
# Why this exists: a crate can be a workspace member, build in CI, and pass every
# quality gate while having no publish step at all — so it silently never ships.
# That went unnoticed twice: `pmcp-openapi-server` (fixed 2026-07-27) and
# `pmcp-tasks` (fixed 2026-08-21, discovered only because a downstream consumer
# could not pin it). Both ledgers are hand-maintained prose; this makes the
# workflow half machine-checked.
#
# A crate that should NOT publish declares `publish = false` in its Cargo.toml.
# That is the single opt-out — there is no allowlist here on purpose.
#
# KNOWN BLIND SPOT: workspace-EXCLUDED crates (pmcp-package) carry their own
# [workspace] table, so root `cargo metadata --no-deps` cannot see them and this
# check does not cover them. Phase 124 (PKGR-01) extends the gate to close that.
#
# Failure discipline (a gate that cannot see must say so, never pass):
# - `cargo metadata` / `jq` failures are EXPLICIT failures — the pipeline is not
#   run inside a process substitution, whose exit status `set -euo pipefail`
#   cannot observe (that shape made this gate print "all 0 publishable workspace
#   members have a publish step." and exit 0 when cargo was broken).
# - An EMPTY crate list is a failure: this workspace has ~20 publishable
#   members, so zero means the data source lied, not that coverage holds.
# - Comment lines in the workflow are stripped before matching, so a
#   commented-out publish step (or a prose comment naming a crate) never counts
#   as coverage.
# - No bash-4-isms (`mapfile`, empty-array `"${a[@]}"` under `set -u`): this is
#   chained into the local `make quality-gate`, and stock macOS bash is 3.2.
set -euo pipefail

WORKFLOW="${1:-.github/workflows/release.yml}"
[ -f "$WORKFLOW" ] || { echo "::error::$WORKFLOW not found"; exit 1; }

METADATA_JSON="$(cargo metadata --no-deps --format-version 1)" || {
  echo "::error::cargo metadata failed — release-ledger coverage was NOT checked"
  exit 1
}

# `publish` is null for publishable crates and [] for `publish = false`.
PUBLISHABLE="$(printf '%s' "$METADATA_JSON" \
  | jq -r '.packages[] | select(.publish == null) | .name' | sort)" || {
  echo "::error::jq failed over cargo metadata — release-ledger coverage was NOT checked"
  exit 1
}

if [ -z "$PUBLISHABLE" ]; then
  echo "::error::cargo metadata reported ZERO publishable workspace members —"
  echo "::error::this workspace has ~20, so the data source is broken; refusing to pass a check that verified nothing"
  exit 1
fi

# Strip full-line comments so a commented-out publish step never counts as
# coverage (release.yml prose comments already name crates next to the literal
# `cargo publish -p ...`).
PUBLISH_LINES="$(grep -vE '^[[:space:]]*#' "$WORKFLOW" || true)"

total=0
missing_count=0
missing_list=""
while IFS= read -r crate; do
  [ -n "$crate" ] || continue
  total=$((total + 1))
  # Match the publish step, not a longer crate name that shares the prefix.
  if ! printf '%s\n' "$PUBLISH_LINES" | grep -qE "cargo publish -p ${crate}( |\$)"; then
    missing_count=$((missing_count + 1))
    missing_list="${missing_list}  - ${crate}
"
  fi
done <<<"$PUBLISHABLE"

if [ "$missing_count" -gt 0 ]; then
  echo "::error::${missing_count} publishable workspace member(s) have no publish step in $WORKFLOW:"
  printf '%s' "$missing_list"
  echo ""
  echo "Fix by EITHER adding a publish step to $WORKFLOW (and an entry to CLAUDE.md's"
  echo "publish order), OR setting 'publish = false' if the crate is not meant to ship."
  exit 1
fi

echo "release-coverage: all ${total} publishable workspace members have a publish step."
