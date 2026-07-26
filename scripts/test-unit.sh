#!/usr/bin/env sh
# Unit tests only: library targets across the workspace.
set -eu
: "${PB_HOME:?PB_HOME must be set}"
cd "$PB_HOME"
cargo test --offline --workspace --locked --lib
echo "test-unit: ok"
