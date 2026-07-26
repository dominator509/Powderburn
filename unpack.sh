#!/bin/sh
# unpack.sh -- split BLUEPRINT_PACK.md back into a real file tree.
# Usage: sh unpack.sh BLUEPRINT_PACK.md [dest_dir]
set -eu

SRC="${1:-BLUEPRINT_PACK.md}"
DEST="${2:-.}"

[ -f "$SRC" ] || { echo "unpack: source not found: $SRC" >&2; exit 1; }
mkdir -p "$DEST"

awk -v dest="$DEST" '
  /^=== FILE: / {
    path = substr($0, 11)
    sub(/ ===$/, "", path)
    sub(/[ \t]+$/, "", path)
    full = dest "/" path
    n = split(full, parts, "/")
    dir = parts[1]
    for (i = 2; i < n; i++) dir = dir "/" parts[i]
    if (n > 1) system("mkdir -p \"" dir "\"")
    out = full
    printf "" > out
    count++
    next
  }
  /^=== END FILE ===$/ { close(out); out = ""; next }
  out != "" { print >> out }
  END { printf "unpack: wrote %d files\n", count }
' "$SRC"

chmod +x "$DEST"/scripts/*.sh 2>/dev/null || true
chmod +x "$DEST"/scripts/probes/*.sh 2>/dev/null || true
chmod +x "$DEST"/unpack.sh 2>/dev/null || true

echo "unpack: ok"
