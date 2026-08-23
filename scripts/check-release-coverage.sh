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
set -euo pipefail

WORKFLOW="${1:-.github/workflows/release.yml}"
[ -f "$WORKFLOW" ] || { echo "::error::$WORKFLOW not found"; exit 1; }

# `publish` is null for publishable crates and [] for `publish = false`.
mapfile -t PUBLISHABLE < <(
  cargo metadata --no-deps --format-version 1 \
    | jq -r '.packages[] | select(.publish == null) | .name' | sort
)

missing=()
for crate in "${PUBLISHABLE[@]}"; do
  # Match the publish step, not a comment or a longer crate name that shares the prefix.
  if ! grep -qE "cargo publish -p ${crate}( |\$)" "$WORKFLOW"; then
    missing+=("$crate")
  fi
done

if [ ${#missing[@]} -gt 0 ]; then
  echo "::error::${#missing[@]} publishable workspace member(s) have no publish step in $WORKFLOW:"
  printf '  - %s\n' "${missing[@]}"
  echo ""
  echo "Fix by EITHER adding a publish step to $WORKFLOW (and an entry to CLAUDE.md's"
  echo "publish order), OR setting 'publish = false' if the crate is not meant to ship."
  exit 1
fi

echo "release-coverage: all ${#PUBLISHABLE[@]} publishable workspace members have a publish step."
