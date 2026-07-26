#!/usr/bin/env sh
# Read-only: the optional publication key is shaped like a butler key and butler exists.
# No upload is attempted and no request is made.
set -eu
[ -n "${PB_ITCH_API_KEY:-}" ] || exit 1
len=$(printf '%s' "$PB_ITCH_API_KEY" | wc -c | tr -d ' ')
[ "$len" -ge 20 ] || exit 1
command -v butler >/dev/null 2>&1 || exit 1
