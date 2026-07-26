#!/usr/bin/env sh
# End to end: the real entry points. Headless simulator drives whole scenarios,
# headless capture drives the real renderer through a software adapter.
set -eu
: "${PB_HOME:?PB_HOME must be set}"
: "${PB_CACHE_DIR:?PB_CACHE_DIR must be set}"
: "${PB_HEADLESS_ADAPTER:?PB_HEADLESS_ADAPTER must be set}"
cd "$PB_HOME"
mkdir -p "$PB_CACHE_DIR"
cargo test --offline -p pb-cli --locked --test e2e -- --test-threads=1
cargo test --offline -p pb-render --locked --test capture -- --test-threads=1
echo "test-e2e: ok"
