#!/usr/bin/env sh
#
# release.sh — Build, sign, checksum, and publish a POWDERBURN release.
#
# Usage: scripts/release.sh --version <semver>
#
#   --version  Release version string (e.g. 0.3.0).
#
# Idempotent: refuses to overwrite an existing release directory.
# Prerequisites: cargo, minisign, sha256sum.
#
# Environment:
#   PB_RELEASE_DIR  Target directory for published releases.
#                   Default: /var/www/powderburn/releases
#   PB_SIGN_KEY     Minisign secret key path (required for signing).
#                   Default: ~/.config/powderburn/release.key
#   CARGO_NET_OFFLINE  Set to false to allow network fetches.

set -eu

# ---- Parse arguments ----------------------------------------------------------

VERSION=""
while [ $# -gt 0 ]; do
    case "$1" in
        --version)
            VERSION="${2:-}"
            shift 2
            ;;
        *)
            echo "error: unknown argument: $1" >&2
            echo "usage: $0 --version <semver>" >&2
            exit 1
            ;;
    esac
done

if [ -z "$VERSION" ]; then
    echo "error: --version is required" >&2
    echo "usage: $0 --version <semver>" >&2
    exit 1
fi

# Validate semver (basic — X.Y.Z or X.Y.Z-prerelease)
if ! echo "$VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$'; then
    echo "error: version \"$VERSION\" does not look like a valid semver" >&2
    exit 1
fi

# ---- Resolve paths ------------------------------------------------------------

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
RELEASE_DIR="${PB_RELEASE_DIR:-/var/www/powderburn/releases}"
TARGET_DIR="$ROOT_DIR/target/release"
OUT_DIR="$RELEASE_DIR/v$VERSION"
BINARY_NAME="powderburn"
BINARY_PATH="$TARGET_DIR/$BINARY_NAME"
SIGN_KEY="${PB_SIGN_KEY:-$HOME/.config/powderburn/release.key}"

# ---- Idempotency check --------------------------------------------------------

if [ -d "$OUT_DIR" ]; then
    echo "error: release directory already exists: $OUT_DIR" >&2
    echo "Refusing to overwrite an existing release." >&2
    echo "Remove it manually if you intend to rebuild: rm -rf $OUT_DIR" >&2
    exit 1
fi

# ---- Prerequisite check -------------------------------------------------------

command -v cargo >/dev/null 2>&1 || { echo "error: cargo not found" >&2; exit 1; }
command -v minisign >/dev/null 2>&1 || { echo "error: minisign not found" >&2; exit 1; }
command -v sha256sum >/dev/null 2>&1 || { echo "error: sha256sum not found" >&2; exit 1; }

if [ ! -f "$SIGN_KEY" ]; then
    echo "error: signing key not found: $SIGN_KEY" >&2
    echo "Generate one with: minisign -G -p $SIGN_KEY.pub -s $SIGN_KEY" >&2
    exit 1
fi

# ---- Step 1: Build with --release ---------------------------------------------

echo "=== Building $BINARY_NAME v$VERSION (release) ==="
cd "$ROOT_DIR"

CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}" \
    cargo build --release --package pb-core --package powderburn 2>&1

if [ ! -f "$BINARY_PATH" ]; then
    echo "error: build produced no binary at $BINARY_PATH" >&2
    exit 1
fi

echo "Build complete: $BINARY_PATH"

# ---- Step 2: Create output directory ------------------------------------------

mkdir -p "$OUT_DIR"

# ---- Step 3: Copy binary ------------------------------------------------------

cp "$BINARY_PATH" "$OUT_DIR/$BINARY_NAME"
echo "Copied binary to $OUT_DIR/$BINARY_NAME"

# ---- Step 4: Create checksums -------------------------------------------------

cd "$OUT_DIR"

echo "=== Creating checksums ==="
sha256sum "$BINARY_NAME" > "$BINARY_NAME.sha256"
sha256sum --check "$BINARY_NAME.sha256"

# Also create a combined checksums file with the version as context
{
    echo "# POWDERBURN v$VERSION checksums"
    echo "# Generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    sha256sum "$BINARY_NAME"
} > checksums.txt

echo "Checksums created."

# ---- Step 5: Sign with minisign -----------------------------------------------

echo "=== Signing binary ==="
minisign -Sm "$BINARY_NAME" -s "$SIGN_KEY" -t "POWDERBURN v$VERSION"

if [ ! -f "$BINARY_NAME.minisig" ]; then
    echo "error: minisign signature was not created" >&2
    exit 1
fi

echo "Signature created: $BINARY_NAME.minisig"

# Also sign the checksums file
minisign -Sm checksums.txt -s "$SIGN_KEY" -t "POWDERBURN v$VERSION checksums"

# ---- Step 6: Verify signature -------------------------------------------------

echo "=== Verifying signature ==="
PUB_KEY="${SIGN_KEY}.pub"
if [ -f "$PUB_KEY" ]; then
    minisign -Vm "$BINARY_NAME" -p "$PUB_KEY"
else
    echo "warning: public key not found at $PUB_KEY — manual verification required" >&2
fi

# ---- Step 7: Update release index ---------------------------------------------

echo "=== Updating release index ==="
cd "$ROOT_DIR"
scripts/make-release-index.sh "$RELEASE_DIR"

echo ""
echo "=== Release v$VERSION published successfully ==="
echo "  Directory: $OUT_DIR"
echo "  Binary:    $OUT_DIR/$BINARY_NAME"
echo "  Checksum:  $OUT_DIR/$BINARY_NAME.sha256"
echo "  Signature: $OUT_DIR/$BINARY_NAME.minisig"
echo ""
echo "To activate this release:"
echo "  ln -sfn $OUT_DIR $RELEASE_DIR/current"
