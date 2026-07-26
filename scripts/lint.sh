#!/usr/bin/env sh
# Clippy with the workspace deny set. Warnings are errors.
set -eu
: "${PB_HOME:?PB_HOME must be set}"
cd "$PB_HOME"
cargo clippy --offline --workspace --all-targets --locked -- -D warnings
sh scripts/lint-determinism.sh
echo "lint: ok"
