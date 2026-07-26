#!/usr/bin/env sh
# Read-only: the signing key exists, is a regular file, is not world readable,
# and is not inside the repository. The key is never read or printed.
set -eu
[ -f "$PB_RELEASE_SIGNING_KEY" ] || exit 1
case "$PB_RELEASE_SIGNING_KEY" in "$PB_HOME"/*) exit 1 ;; esac
perms=$(stat -c '%a' "$PB_RELEASE_SIGNING_KEY" 2>/dev/null || echo 777)
case "$perms" in 600|400) ;; *) exit 1 ;; esac
command -v minisign >/dev/null 2>&1 || exit 1
