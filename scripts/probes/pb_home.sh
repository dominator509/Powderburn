#!/usr/bin/env sh
# Read-only: PB_HOME is an absolute path that is this repository root.
set -eu
case "$PB_HOME" in /*) ;; *) exit 1 ;; esac
[ -f "$PB_HOME/AGENTS.md" ] || exit 1
[ -d "$PB_HOME/.agent" ] || exit 1
