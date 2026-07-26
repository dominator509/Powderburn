#!/usr/bin/env sh
# Whole-workspace type resolution including tests and benches, offline and locked.
set -eu
: "${PB_HOME:?PB_HOME must be set}"
cd "$PB_HOME"
cargo check --offline --workspace --all-targets --locked
echo "typecheck: ok"
