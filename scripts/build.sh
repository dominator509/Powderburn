#!/usr/bin/env sh
# Release build of every shipped binary, offline and locked, reproducible flags on.
set -eu
: "${PB_HOME:?PB_HOME must be set}"
cd "$PB_HOME"
SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-$(git log -1 --pretty=%ct)}"
export SOURCE_DATE_EPOCH
export RUSTFLAGS="--remap-path-prefix=$PB_HOME=/build -C debuginfo=1"
cargo build --offline --locked --release -p pb-app --bin powderburn
cargo build --offline --locked --release -p pb-cli --bin pbcli
cargo build --offline --locked --release -p pb-tools --bin pbtool
for b in powderburn pbcli pbtool; do
  [ -x "target/release/$b" ] || { echo "build: FAIL - missing target/release/$b" >&2; exit 1; }
done
echo "build: ok"
