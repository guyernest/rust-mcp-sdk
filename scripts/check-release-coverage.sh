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
set -euo pipefail

WORKFLOW="${1:-.github/workflows/release.yml}"
[ -f "$WORKFLOW" ] || { echo "::error::$WORKFLOW not found"; exit 1; }

# `publish` is null for publishable crates and [] for `publish = false`.
mapfile -t PUBLISHABLE < <(
  cargo metadata --no-deps --format-version 1 \
    | python3 -c 'import json,sys; print("\n".join(sorted(p["name"] for p in json.load(sys.stdin)["packages"] if p.get("publish") is None)))'
)

missing=()
for crate in "${PUBLISHABLE[@]}"; do
  # Match the publish step, not a comment or a longer crate name that shares the prefix.
  if ! grep -qE "cargo publish (-p ${crate}( |\$)|--manifest-path [^ ]*/${crate}/Cargo\.toml)" "$WORKFLOW"; then
    missing+=("$crate")
  fi
done

if [ ${#missing[@]} -gt 0 ]; then
  echo "::error::${#missing[@]} publishable workspace member(s) have no publish step in $WORKFLOW:"
  for m in "${missing[@]}"; do echo "  - $m"; done
  echo ""
  echo "Fix by EITHER adding a publish step to $WORKFLOW (and an entry to CLAUDE.md's"
  echo "publish order), OR setting 'publish = false' if the crate is not meant to ship."
  exit 1
fi

echo "release-coverage: all ${#PUBLISHABLE[@]} publishable workspace members have a publish step."
