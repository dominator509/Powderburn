#!/usr/bin/env sh
# Vendor every dependency and prove the workspace resolves with no network.
set -eu
: "${PB_HOME:?PB_HOME must be set; see PREFLIGHT.md}"
cd "$PB_HOME"
rustup toolchain list | grep -q '1.85.0' || { echo "install: FAIL - toolchain 1.85.0 not installed" >&2; exit 1; }
rustup component add rustfmt clippy --toolchain 1.85.0 >/dev/null 2>&1 || true
if [ ! -d vendor ]; then
  cargo vendor --versioned-dirs vendor > .cargo/config.toml.new
  mkdir -p .cargo
  mv .cargo/config.toml.new .cargo/config.toml
fi
[ -f Cargo.lock ] || { echo "install: FAIL - Cargo.lock missing; lockfile must be committed" >&2; exit 1; }
cargo metadata --offline --format-version 1 >/dev/null
echo "install: ok"
