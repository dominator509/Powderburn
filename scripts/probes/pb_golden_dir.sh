#!/usr/bin/env sh
# Read-only: the golden corpus root is absolute and its parent is writable.
set -eu
case "$PB_GOLDEN_DIR" in /*) ;; *) exit 1 ;; esac
d=$(dirname "$PB_GOLDEN_DIR")
[ -d "$d" ] && [ -w "$d" ] || exit 1
