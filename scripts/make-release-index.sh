#!/usr/bin/env sh
#
# make-release-index.sh — Create or update the release index.txt.
#
# Usage: scripts/make-release-index.sh [release-dir]
#
# Scans the release directory for versioned subdirectories (v*), verifies
# checksums for each, and rebuilds index.txt.
#
# If release-dir is omitted, defaults to $PB_RELEASE_DIR or
# /var/www/powderburn/releases.
#
# Environment:
#   PB_RELEASE_DIR  Target directory for published releases.
#                   Default: /var/www/powderburn/releases

set -eu

# ---- Resolve paths ------------------------------------------------------------

RELEASE_DIR="${1:-}"
if [ -z "$RELEASE_DIR" ]; then
    RELEASE_DIR="${PB_RELEASE_DIR:-/var/www/powderburn/releases}"
fi

INDEX_FILE="$RELEASE_DIR/index.txt"
CURRENT_LINK="$RELEASE_DIR/current"

# ---- Prerequisite checks ------------------------------------------------------

if [ ! -d "$RELEASE_DIR" ]; then
    echo "error: release directory does not exist: $RELEASE_DIR" >&2
    exit 1
fi

command -v sha256sum >/dev/null 2>&1 || { echo "error: sha256sum not found" >&2; exit 1; }

# ---- Scan versions ------------------------------------------------------------

echo "=== Scanning releases in $RELEASE_DIR ==="

VERSIONS=""
for dir in "$RELEASE_DIR"/v*/; do
    if [ -d "$dir" ]; then
        VERSIONS="$VERSIONS $(basename "$dir")"
    fi
done

if [ -z "$VERSIONS" ]; then
    echo "warning: no version directories found in $RELEASE_DIR" >&2
fi

# Determine the current version from the symlink
CURRENT=""
if [ -L "$CURRENT_LINK" ]; then
    CURRENT="$(basename "$(readlink "$CURRENT_LINK")")"
fi

# ---- Build index --------------------------------------------------------------

{
    echo "# POWDERBURN Release Index"
    echo "# Generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "# Format: VERIFY|PUBLISH|ROLLBACK <timestamp> <details>"
    echo ""

    for ver in $VERSIONS; do
        VER_DIR="$RELEASE_DIR/$ver"
        BINARY="$VER_DIR/powderburn"
        CHECKSUM_FILE="$VER_DIR/powderburn.sha256"

        # Verify checksum if a .sha256 file exists
        VERIFY_STATUS="not_verified"
        if [ -f "$CHECKSUM_FILE" ] && [ -f "$BINARY" ]; then
            if sha256sum -c "$CHECKSUM_FILE" >/dev/null 2>&1; then
                VERIFY_STATUS="ok"
            else
                VERIFY_STATUS="FAILED"
            fi
        fi

        TIMESTAMP=""
        if [ -f "$BINARY" ]; then
            TIMESTAMP="$(date -u -r "$BINARY" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || echo "unknown")"
        fi

        # Mark current
        CURRENT_MARKER=""
        if [ "$ver" = "$CURRENT" ]; then
            CURRENT_MARKER=" [current]"
        fi

        echo "VERIFY $TIMESTAMP dir=$ver checksum=$VERIFY_STATUS$CURRENT_MARKER"
    done

    echo ""
    echo "# Existing publish / rollback history from previous index:"
} > "$INDEX_FILE.tmp"

# Preserve any PUBLISH/ROLLBACK lines already in the existing index
if [ -f "$INDEX_FILE" ]; then
    grep -E '^(PUBLISH|ROLLBACK) ' "$INDEX_FILE" >> "$INDEX_FILE.tmp" || true
fi

# Move temp index into place
mv "$INDEX_FILE.tmp" "$INDEX_FILE"

echo "=== Index updated: $INDEX_FILE ==="
echo ""
echo "Current releases:"
for ver in $VERSIONS; do
    MARKER=""
    if [ "$ver" = "$CURRENT" ]; then
        MARKER="  ← CURRENT"
    fi
    echo "  $ver$MARKER"
done
