#!/usr/bin/env sh
# LBI-01, LBI-02, LBI-03: the determinism lints that clippy cannot express.
# Fails if a determinism-critical crate uses floats, wall-clock, OS randomness,
# unordered maps in state, or imports a presentation crate.
set -eu
: "${PB_HOME:?PB_HOME must be set}"
cd "$PB_HOME"
CRITICAL="crates/pb-core crates/pb-rng crates/pb-rules crates/pb-sim crates/pb-ai crates/pb-content"
bad=0
report() { printf '%s\n' "$1"; bad=1; }
for d in $CRITICAL; do
  [ -d "$d/src" ] || continue
  out=$(grep -RInE '\b(f32|f64)\b' "$d/src" || true)
  [ -z "$out" ] || report "LBI-02 float in determinism-critical crate:
$out"
  out=$(grep -RInE 'SystemTime|Instant::now|std::time' "$d/src" || true)
  [ -z "$out" ] || report "LBI-01 clock read in determinism-critical crate:
$out"
  out=$(grep -RInE 'rand::|thread_rng|getrandom|OsRng' "$d/src" || true)
  [ -z "$out" ] || report "LBI-01 nondeterministic randomness in determinism-critical crate:
$out"
  out=$(grep -RInE '\bHashMap\b|\bHashSet\b' "$d/src" || true)
  [ -z "$out" ] || report "LBI-01 unordered collection in determinism-critical crate; use BTreeMap or IndexVec:
$out"
  out=$(grep -RInE 'use +pb_(render|audio|app|cli|tools)' "$d/src" || true)
  [ -z "$out" ] || report "LBI-03 import law violation, simulation importing presentation:
$out"
done
if [ "$bad" -ne 0 ]; then
  echo "lint-determinism: FAIL" >&2
  exit 1
fi
echo "lint-determinism: ok"
