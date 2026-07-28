#!/usr/bin/env sh
# Enforce SPEC-008 line-coverage floors with the pinned offline workspace.
set -eu

: "${PB_HOME:?PB_HOME must be set}"
: "${PB_CACHE_DIR:?PB_CACHE_DIR must be set}"
cd "$PB_HOME"
command -v cargo-llvm-cov >/dev/null 2>&1 ||
  { echo "coverage: FAIL - cargo-llvm-cov is not installed" >&2; exit 1; }
mkdir -p "$PB_CACHE_DIR"

workspace_log="$PB_CACHE_DIR/coverage-workspace.log"
kernel_log="$PB_CACHE_DIR/coverage-kernel.log"
cargo llvm-cov --offline --workspace --summary-only --quiet >"$workspace_log" 2>&1 ||
  { echo "coverage: FAIL - workspace measurement failed; see $workspace_log" >&2; exit 1; }
cargo llvm-cov --offline \
  --package pb-core --package pb-rng --package pb-rules --package pb-sim \
  --summary-only --quiet >"$kernel_log" 2>&1 ||
  { echo "coverage: FAIL - kernel measurement failed; see $kernel_log" >&2; exit 1; }

line_percent() {
  awk '$1 == "TOTAL" { value=$10; gsub("%", "", value); print value; exit }' "$1"
}
workspace=$(line_percent "$workspace_log")
kernel=$(line_percent "$kernel_log")
[ -n "$workspace" ] && [ -n "$kernel" ] ||
  { echo "coverage: FAIL - could not parse TOTAL line" >&2; exit 1; }
awk -v measured="$workspace" 'BEGIN { exit !(measured >= 70.0) }' ||
  { echo "coverage: FAIL - workspace line coverage ${workspace}% is below 70%" >&2; exit 1; }
awk -v measured="$kernel" 'BEGIN { exit !(measured >= 85.0) }' ||
  { echo "coverage: FAIL - kernel line coverage ${kernel}% is below 85%" >&2; exit 1; }

echo "coverage-workspace-lines: ${workspace}%"
echo "coverage-kernel-lines: ${kernel}%"
echo "coverage: ok"
