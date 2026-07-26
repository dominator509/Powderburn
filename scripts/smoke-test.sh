#!/usr/bin/env sh
# Smoke: the shortest path that proves a built artifact is alive and correct.
# Default target is target/release. With --released, the artifact under $PB_RELEASE_DIR.
set -eu
: "${PB_HOME:?PB_HOME must be set}"
: "${PB_CACHE_DIR:?PB_CACHE_DIR must be set}"
cd "$PB_HOME"
mkdir -p "$PB_CACHE_DIR/smoke"
BIN_DIR="target/release"
if [ "${1:-}" = "--released" ]; then
  : "${PB_RELEASE_DIR:?PB_RELEASE_DIR must be set}"
  ver=$(sed -n 's/^version *= *"\(.*\)"/\1/p' crates/pb-app/Cargo.toml | head -n 1)
  art="$PB_RELEASE_DIR/powderburn-$ver-x86_64-linux.tar.zst"
  [ -f "$art" ] || { echo "smoke: FAIL - released artifact not found: $art" >&2; exit 1; }
  minisign -Vm "$art" -p "$PB_RELEASE_DIR/powderburn.pub" >/dev/null 2>&1 \
    || { echo "smoke: FAIL - signature verification failed for $art" >&2; exit 1; }
  rm -rf "$PB_CACHE_DIR/smoke/unpack"
  mkdir -p "$PB_CACHE_DIR/smoke/unpack"
  tar --use-compress-program=unzstd -xf "$art" -C "$PB_CACHE_DIR/smoke/unpack"
  BIN_DIR="$PB_CACHE_DIR/smoke/unpack/powderburn-$ver/bin"
fi
"$BIN_DIR/powderburn" --version >/dev/null || { echo "smoke: FAIL - app will not report version" >&2; exit 1; }
"$BIN_DIR/pbcli" selftest --emit-hash > "$PB_CACHE_DIR/smoke/selftest.log" 2>&1 \
  || { echo "smoke: FAIL - pbcli selftest failed" >&2; exit 1; }
grep -qx 'selftest: ok' "$PB_CACHE_DIR/smoke/selftest.log" \
  || { echo "smoke: FAIL - selftest sentinel missing" >&2; exit 1; }
grep -q '^state-hash: ' "$PB_CACHE_DIR/smoke/selftest.log" \
  || { echo "smoke: FAIL - selftest emitted no state hash" >&2; exit 1; }
echo "smoke: ok"
