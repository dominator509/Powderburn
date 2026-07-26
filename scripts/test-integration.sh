#!/usr/bin/env sh
# Integration tests: real content files, real save files, real simulation kernel.
set -eu
: "${PB_HOME:?PB_HOME must be set}"
: "${PB_GOLDEN_DIR:?PB_GOLDEN_DIR must be set}"
cd "$PB_HOME"
cargo test --offline --workspace --locked --test '*'
echo "test-integration: ok"
