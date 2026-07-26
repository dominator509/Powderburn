#!/usr/bin/env sh
# Read-only: the self-hosted release directory exists and is writable by this account.
set -eu
case "$PB_RELEASE_DIR" in /*) ;; *) exit 1 ;; esac
[ -d "$PB_RELEASE_DIR" ] || exit 1
[ -w "$PB_RELEASE_DIR" ] || exit 1
