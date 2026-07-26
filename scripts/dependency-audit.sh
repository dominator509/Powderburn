#!/usr/bin/env sh
# Dependency posture: closed, pinned, vendored, license-clean.
# Threshold: zero unpinned versions, zero unvendored crates, zero non-allowlisted licenses.
# Waivers are granted only by an ADR in DECISIONS.md referenced by id in .agent/dep-waivers.
set -eu
: "${PB_HOME:?PB_HOME must be set}"
cd "$PB_HOME"
fail() { echo "dependency-audit: FAIL - $1" >&2; exit 1; }
[ -f Cargo.lock ] || fail "Cargo.lock missing"
[ -d vendor ]     || fail "vendor/ missing; run sh scripts/install.sh"
if grep -RInE '^[a-zA-Z0-9_-]+ *= *"(\^|\*|>|<|~)' crates/*/Cargo.toml Cargo.toml >/dev/null 2>&1; then
  fail "unpinned dependency version range found; every version is exact"
fi
count=$(grep -c '^name = ' Cargo.lock || echo 0)
[ "$count" -le 60 ] || fail "dependency count $count exceeds the budget of 60 declared in ARCHITECTURE.md"
for d in vendor/*/; do
  [ -d "$d" ] || continue
  if ! ls "$d" 2>/dev/null | grep -qiE '^(LICENSE|LICENCE|COPYING|UNLICENSE)'; then
    base=$(basename "$d")
    grep -q "^$base\$" .agent/dep-waivers 2>/dev/null || fail "vendored crate without a license file: $base"
  fi
done
echo "dependency-audit: ok"
