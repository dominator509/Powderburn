#!/usr/bin/env sh
# Read-only: a software graphics adapter of the named backend is present.
set -eu
case "$PB_HEADLESS_ADAPTER" in
  gl)
    command -v glxinfo >/dev/null 2>&1 || exit 1
    LIBGL_ALWAYS_SOFTWARE=1 glxinfo -B 2>/dev/null | grep -qi 'llvmpipe\|softpipe' || exit 1
    ;;
  vulkan)
    command -v vulkaninfo >/dev/null 2>&1 || exit 1
    vulkaninfo --summary 2>/dev/null | grep -qi 'lavapipe\|llvmpipe' || exit 1
    ;;
  *) exit 1 ;;
esac
