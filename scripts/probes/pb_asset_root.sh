#!/usr/bin/env sh
# Read-only: the asset root is absolute and readable, and its parent is writable.
set -eu
case "$PB_ASSET_ROOT" in /*) ;; *) exit 1 ;; esac
d=$(dirname "$PB_ASSET_ROOT")
[ -d "$d" ] && [ -w "$d" ] || exit 1
