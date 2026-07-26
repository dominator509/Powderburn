#!/usr/bin/env sh
# 6LAYER reality gate: lexical layer of the no-mock law.
# Fails if forbidden implementation markers exist in production source paths.
# Patterns: .agent/reality-patterns (one ERE per line).
# Allowlist: .agent/reality-allow (EREs matching whole grep output lines to excuse).
set -eu
PAT=".agent/reality-patterns"
ALLOW=".agent/reality-allow"
[ -f "$PAT" ]   || { echo "reality gate: missing $PAT" >&2; exit 1; }
[ -f "$ALLOW" ] || { echo "reality gate: missing $ALLOW" >&2; exit 1; }
SRC_DIRS="crates content assets"
hits=0
for d in $SRC_DIRS; do
  [ -d "$d" ] || continue
  out=$(grep -RInE -f "$PAT" "$d" 2>/dev/null | grep -vE -f "$ALLOW" || true)
  if [ -n "$out" ]; then
    printf '%s\n' "$out"
    hits=1
  fi
done
if [ "$hits" -ne 0 ]; then
  echo "reality gate: FAIL (forbidden implementation markers listed above)" >&2
  exit 1
fi
echo "reality gate: ok"
