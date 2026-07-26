#!/usr/bin/env sh
#
# rollback.sh — Roll a POWDERBURN release back to a previous version.
#
# Usage: scripts/rollback.sh --to <version>
#
#   --to  Target version directory name (e.g. v0.3.0).
#
# Effect:
#   1. Repoints the $PB_RELEASE_DIR/current symlink to the target version.
#   2. Appends a ROLLBACK line to $PB_RELEASE_DIR/index.txt.
#   3. Re-runs the smoke test on the rollback target.
#
# Environment:
#   PB_RELEASE_DIR  Target directory for published releases.
#                   Default: /var/www/powderburn/releases

set -eu

# ---- Parse arguments ----------------------------------------------------------

TO_VERSION=""
while [ $# -gt 0 ]; do
    case "$1" in
        --to)
            TO_VERSION="${2:-}"
            shift 2
            ;;
        *)
            echo "error: unknown argument: $1" >&2
            echo "usage: $0 --to <version>" >&2
            exit 1
            ;;
    esac
done

if [ -z "$TO_VERSION" ]; then
    echo "error: --to is required" >&2
    echo "usage: $0 --to <version>" >&2
    exit 1
fi

# Normalise — accept "0.3.0" or "v0.3.0"
case "$TO_VERSION" in
    v*) ;;
    *) TO_VERSION="v$TO_VERSION" ;;
esac

# ---- Resolve paths ------------------------------------------------------------

RELEASE_DIR="${PB_RELEASE_DIR:-/var/www/powderburn/releases}"
CURRENT_LINK="$RELEASE_DIR/current"
TARGET_DIR="$RELEASE_DIR/$TO_VERSION"
INDEX_FILE="$RELEASE_DIR/index.txt"

# ---- Prerequisite checks ------------------------------------------------------

if [ ! -d "$TARGET_DIR" ]; then
    echo "error: target release directory not found: $TARGET_DIR" >&2
    echo "Available releases:" >&2
    ls -1d "$RELEASE_DIR"/v*/ 2>/dev/null || echo "  (none)" >&2
    exit 1
fi

if [ ! -f "$TARGET_DIR/powderburn" ]; then
    echo "error: target release $TARGET_DIR has no powderburn binary" >&2
    exit 1
fi

if [ ! -x "$TARGET_DIR/powderburn" ]; then
    echo "error: powderburn binary in $TARGET_DIR is not executable" >&2
    exit 1
fi

# ---- Determine current version before rollback --------------------------------

CURRENT_VERSION=""
if [ -L "$CURRENT_LINK" ]; then
    CURRENT_VERSION="$(readlink "$CURRENT_LINK")"
elif [ -f "$CURRENT_LINK" ]; then
    # In case current is a regular file (unlikely)
    CURRENT_VERSION="$(cat "$CURRENT_LINK" 2>/dev/null || echo "unknown")"
else
    CURRENT_VERSION="<none>"
fi

# Normalise the current version for the index entry
case "$CURRENT_VERSION" in
    *v*) FROM_VERSION="$CURRENT_VERSION" ;;
    *)   FROM_VERSION="$CURRENT_VERSION" ;;
esac

if [ "$TARGET_DIR" = "$CURRENT_VERSION" ] || [ "$TARGET_DIR" = "$(readlink -f "$CURRENT_LINK" 2>/dev/null)" ]; then
    echo "error: target version $TO_VERSION is already the current release" >&2
    exit 1
fi

# ---- Step 1: Repoint the symlink ----------------------------------------------

echo "=== Rolling back $CURRENT_VERSION → $TO_VERSION ==="
ln -sfn "$TARGET_DIR" "$CURRENT_LINK"
echo "Symlink updated: $CURRENT_LINK → $TARGET_DIR"

# ---- Step 2: Append rollback line to index.txt --------------------------------

TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
ROLLBACK_LINE="ROLLBACK $TIMESTAMP from=$CURRENT_VERSION to=$TO_VERSION"

mkdir -p "$(dirname "$INDEX_FILE")"
echo "$ROLLBACK_LINE" >> "$INDEX_FILE"
echo "Index updated: $ROLLBACK_LINE"

# ---- Step 3: Re-run smoke test ------------------------------------------------

echo "=== Running smoke test on $TO_VERSION ==="
if "$TARGET_DIR/powderburn" --smoke-test 2>&1; then
    echo "=== Rollback complete: $TO_VERSION is now current ==="
else
    echo "error: smoke test FAILED on rollback target $TO_VERSION" >&2
    echo "The symlink has been updated but the release may not function correctly." >&2
    echo "Manual investigation required." >&2
    exit 1
fi
