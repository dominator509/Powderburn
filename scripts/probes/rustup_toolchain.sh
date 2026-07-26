#!/usr/bin/env sh
# Read-only: confirm the pinned toolchain is installed and is the active compiler.
set -eu
[ "${RUSTUP_TOOLCHAIN:-}" = "host" ] && exit 0
rustup toolchain list | grep -q "^${RUSTUP_TOOLCHAIN}" || exit 1
v=$(rustc --version | awk '{print $2}')
[ "$v" = "1.85.0" ] || exit 1
