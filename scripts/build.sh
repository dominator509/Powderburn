#!/usr/bin/env sh
# Offline release build and optional byte-reproducible distribution archive.
set -eu

: "${PB_HOME:?PB_HOME must be set}"
[ -d "$PB_HOME" ] || { echo "build: FAIL - PB_HOME is not a directory" >&2; exit 1; }

OUT_DIR=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --release)
      shift
      ;;
    --out)
      [ "$#" -ge 2 ] || { echo "build: FAIL - --out requires a directory" >&2; exit 1; }
      OUT_DIR=$2
      shift 2
      ;;
    *)
      echo "build: FAIL - unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

cd "$PB_HOME"
SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-$(git log -1 --pretty=%ct)}"
export SOURCE_DATE_EPOCH
export RUSTFLAGS="--remap-path-prefix=$PB_HOME=/build --remap-path-prefix=$PB_HOME/vendor=/vendor -C debuginfo=1"

cargo build --offline --locked --release -p pb-app --bin powderburn
cargo build --offline --locked --release -p pb-cli --bin pbcli
cargo build --offline --locked --release -p pb-tools --bin pbtool

TARGET_ROOT="${CARGO_TARGET_DIR:-$PB_HOME/target}"
for binary in powderburn pbcli pbtool; do
  [ -x "$TARGET_ROOT/release/$binary" ] ||
    { echo "build: FAIL - missing $TARGET_ROOT/release/$binary" >&2; exit 1; }
done

if [ -n "$OUT_DIR" ]; then
  command -v tar >/dev/null 2>&1 || { echo "build: FAIL - tar not found" >&2; exit 1; }
  command -v zstd >/dev/null 2>&1 || { echo "build: FAIL - zstd not found" >&2; exit 1; }
  command -v sha256sum >/dev/null 2>&1 ||
    { echo "build: FAIL - sha256sum not found" >&2; exit 1; }

  mkdir -p "$OUT_DIR"
  OUT_DIR=$(cd "$OUT_DIR" && pwd)
  VERSION="${PB_ARTIFACT_VERSION:-$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\([^"]*\)"/\1/p' Cargo.toml)}"
  [ -n "$VERSION" ] || { echo "build: FAIL - workspace version not found" >&2; exit 1; }
  TARGET=$(rustc -vV | sed -n 's/^host: //p')
  [ -n "$TARGET" ] || { echo "build: FAIL - rustc host target not found" >&2; exit 1; }
  PACKAGE="powderburn-$VERSION-$TARGET"
  STAGE=$(mktemp -d "$OUT_DIR/.stage.XXXXXX")
  PACKAGE_ROOT="$STAGE/$PACKAGE"
  mkdir -p "$PACKAGE_ROOT/bin"

  for binary in powderburn pbcli pbtool; do
    cp "$TARGET_ROOT/release/$binary" "$PACKAGE_ROOT/bin/$binary"
  done
  cp -R assets content "$PACKAGE_ROOT/"
  mkdir -p "$PACKAGE_ROOT/tests"
  cp -R tests/fixtures tests/golden tests/journals "$PACKAGE_ROOT/tests/"
  cp DEPLOYMENT.md RELEASE.md LICENSE REFERENCE_MACHINE.txt "$PACKAGE_ROOT/"

  ARTIFACT="$OUT_DIR/$PACKAGE.tar.zst"
  (
    cd "$STAGE"
    tar --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 --numeric-owner \
      --format=gnu -cf - "$PACKAGE"
  ) | zstd -19 --threads=1 --quiet -o "$ARTIFACT"
  (cd "$OUT_DIR" && sha256sum "$(basename "$ARTIFACT")") >"$ARTIFACT.sha256"

  case "$STAGE" in
    "$OUT_DIR"/.stage.*) rm -rf "$STAGE" ;;
    *) echo "build: FAIL - unsafe staging path: $STAGE" >&2; exit 1 ;;
  esac
  echo "artifact: $ARTIFACT"
fi

echo "build: ok"
