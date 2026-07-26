#!/usr/bin/env sh
# Formatting is law, not taste. Fails on any deviation from rustfmt.toml.
set -eu
: "${PB_HOME:?PB_HOME must be set}"
cd "$PB_HOME"
cargo fmt --all -- --check
echo "format-check: ok"
