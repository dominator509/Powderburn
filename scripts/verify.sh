#!/usr/bin/env sh
# The full gate chain. Every gate, in order, in one run, from the current tree.
# This is the command that decides whether the repository is green.
set -eu
: "${PB_HOME:?PB_HOME must be set}"
cd "$PB_HOME"
sh scripts/format-check.sh
sh scripts/lint.sh
sh scripts/typecheck.sh
sh scripts/reality-gate.sh
sh scripts/test-unit.sh
sh scripts/test-integration.sh
sh scripts/build.sh
sh scripts/test-e2e.sh
sh scripts/security-check.sh
sh scripts/dependency-audit.sh
sh scripts/smoke-test.sh
sh scripts/live-fire.sh
echo "verify: ok"
