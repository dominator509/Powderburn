#!/usr/bin/env sh
# Build the current tree twice in isolated target/output roots and compare bytes.
set -eu

: "${PB_HOME:?PB_HOME must be set}"
: "${PB_CACHE_DIR:?PB_CACHE_DIR must be set}"
cd "$PB_HOME"

root="$PB_CACHE_DIR/reproducibility"
a="$root/a"
b="$root/b"
mkdir -p "$a" "$b"
rm -f "$a"/*.tar.zst "$a"/*.sha256 "$b"/*.tar.zst "$b"/*.sha256

CARGO_TARGET_DIR="$root/target-a" sh scripts/build.sh --release --out "$a" >/dev/null
CARGO_TARGET_DIR="$root/target-b" sh scripts/build.sh --release --out "$b" >/dev/null
artifact_a=$(find "$a" -maxdepth 1 -type f -name '*.tar.zst' | head -n 1)
artifact_b=$(find "$b" -maxdepth 1 -type f -name '*.tar.zst' | head -n 1)
[ -n "$artifact_a" ] && [ -n "$artifact_b" ] ||
  { echo "reproducible: FAIL - build artifact missing" >&2; exit 1; }
hash_a=$(sha256sum "$artifact_a" | awk '{print $1}')
hash_b=$(sha256sum "$artifact_b" | awk '{print $1}')
[ "$hash_a" = "$hash_b" ] ||
  { echo "reproducible: FAIL - $hash_a differs from $hash_b" >&2; exit 1; }
echo "reproducible-sha256: $hash_a"
echo "reproducible: identical"
