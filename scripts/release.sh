#!/usr/bin/env sh
# Build, checksum, sign, verify, and optionally publish one immutable release.
set -eu

: "${PB_HOME:?PB_HOME must be set}"
: "${PB_CACHE_DIR:?PB_CACHE_DIR must be set}"
: "${PB_RELEASE_DIR:?PB_RELEASE_DIR must be set}"
: "${PB_RELEASE_SIGNING_KEY:?PB_RELEASE_SIGNING_KEY must be set}"

VERSION=""
MODE=""
INJECT_FAILURE=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)
      [ "$#" -ge 2 ] || { echo "release: FAIL - --version requires a value" >&2; exit 1; }
      VERSION=$2
      shift 2
      ;;
    --dry-run)
      MODE=dry-run
      shift
      ;;
    --publish)
      MODE=publish
      shift
      ;;
    --inject-failure)
      INJECT_FAILURE=1
      shift
      ;;
    *)
      echo "release: FAIL - unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

[ -n "$VERSION" ] || { echo "release: FAIL - --version is required" >&2; exit 1; }
[ -n "$MODE" ] || { echo "release: FAIL - choose --dry-run or --publish" >&2; exit 1; }
echo "$VERSION" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.]+)?$' ||
  { echo "release: FAIL - invalid semantic version: $VERSION" >&2; exit 1; }
WORKSPACE_VERSION=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\([^"]*\)"/\1/p' "$PB_HOME/Cargo.toml")
[ -n "$WORKSPACE_VERSION" ] ||
  { echo "release: FAIL - workspace version is unreadable" >&2; exit 1; }
case "$VERSION" in
  *-*) ;;
  "$WORKSPACE_VERSION") ;;
  *)
    echo "release: FAIL - production version $VERSION does not match workspace $WORKSPACE_VERSION" >&2
    exit 1
    ;;
esac
[ -f "$PB_RELEASE_SIGNING_KEY" ] ||
  { echo "release: FAIL - signing key not found: $PB_RELEASE_SIGNING_KEY" >&2; exit 1; }
[ -f "$PB_RELEASE_SIGNING_KEY.pub" ] ||
  { echo "release: FAIL - public key not found: $PB_RELEASE_SIGNING_KEY.pub" >&2; exit 1; }

DEST="$PB_RELEASE_DIR/$VERSION"
if [ "$MODE" = publish ] && [ -d "$DEST" ]; then
  ARTIFACT=$(find "$DEST" -maxdepth 1 -type f -name '*.tar.zst' | head -n 1)
  [ -n "$ARTIFACT" ] ||
    { echo "release: FAIL - existing release has no archive: $DEST" >&2; exit 1; }
  sh "$PB_HOME/scripts/smoke-test.sh" --released "$ARTIFACT" >/dev/null ||
    { echo "release: FAIL - existing release failed verification: $DEST" >&2; exit 1; }
  echo "publish: already present"
  echo "MANUAL STEP: itch.io publication"
  echo "butler push \"$DEST\" USER/GAME:linux --userversion \"$VERSION\""
  exit 0
fi

mkdir -p "$PB_CACHE_DIR" "$PB_RELEASE_DIR"
WORK=$(mktemp -d "$PB_CACHE_DIR/release-$VERSION.XXXXXX")
cleanup() {
  case "$WORK" in
    "$PB_CACHE_DIR"/release-"$VERSION".*) rm -rf "$WORK" ;;
    *) echo "release: unsafe work path: $WORK" >&2 ;;
  esac
}
trap cleanup EXIT HUP INT TERM

env PB_ARTIFACT_VERSION="$VERSION" sh "$PB_HOME/scripts/build.sh" \
  --release --out "$WORK" >/dev/null
ARTIFACT=$(find "$WORK" -maxdepth 1 -type f -name '*.tar.zst' | head -n 1)
[ -n "$ARTIFACT" ] || { echo "release: FAIL - build produced no archive" >&2; exit 1; }

if [ "$INJECT_FAILURE" -eq 1 ]; then
  SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-$(git -C "$PB_HOME" log -1 --pretty=%ct)}"
  FAIL_ROOT=$(mktemp -d "$WORK/inject.XXXXXX")
  tar --use-compress-program=unzstd -xf "$ARTIFACT" -C "$FAIL_ROOT"
  PACKAGE_ROOT=$(find "$FAIL_ROOT" -mindepth 1 -maxdepth 1 -type d | head -n 1)
  [ -n "$PACKAGE_ROOT" ] || { echo "release: FAIL - cannot inject failure" >&2; exit 1; }
  printf '#!/usr/bin/env sh\nexit 70\n' >"$PACKAGE_ROOT/bin/pbcli"
  chmod +x "$PACKAGE_ROOT/bin/pbcli"
  PACKAGE=$(basename "$PACKAGE_ROOT")
  (
    cd "$FAIL_ROOT"
    tar --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 --numeric-owner \
      --format=gnu -cf - "$PACKAGE"
  ) | zstd -19 --threads=1 --quiet -f -o "$ARTIFACT"
fi

(cd "$WORK" && sha256sum "$(basename "$ARTIFACT")") >"$ARTIFACT.sha256"
minisign -S -s "$PB_RELEASE_SIGNING_KEY" -m "$ARTIFACT" -x "$ARTIFACT.sig" \
  -t "POWDERBURN $VERSION" >/dev/null
minisign -V -q -p "$PB_RELEASE_SIGNING_KEY.pub" -m "$ARTIFACT" -x "$ARTIFACT.sig" ||
  { echo "release: FAIL - signature self-verification failed" >&2; exit 1; }

if [ "$MODE" = dry-run ]; then
  echo "release: dry-run ok"
else
  PUBLISH_TMP="$PB_RELEASE_DIR/.publishing-$VERSION-$$"
  [ ! -e "$PUBLISH_TMP" ] ||
    { echo "release: FAIL - temporary publish path exists: $PUBLISH_TMP" >&2; exit 1; }
  mkdir "$PUBLISH_TMP"
  cp "$ARTIFACT" "$ARTIFACT.sha256" "$ARTIFACT.sig" "$PUBLISH_TMP/"
  cp "$PB_HOME/REFERENCE_MACHINE.txt" "$PUBLISH_TMP/"
  cp "$PB_RELEASE_SIGNING_KEY.pub" "$PB_RELEASE_DIR/powderburn.pub"
  HASH=$(sha256sum "$PUBLISH_TMP/$(basename "$ARTIFACT")" | awk '{print $1}')
  BYTES=$(wc -c <"$PUBLISH_TMP/$(basename "$ARTIFACT")" | tr -d ' ')
  TARGET=$(rustc -vV | sed -n 's/^host: //p')
  TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  KEY_FINGERPRINT=$(sha256sum "$PB_RELEASE_SIGNING_KEY.pub" | awk '{print $1}')
  awk -v version="$VERSION" '
    $0 ~ "^## \\[" version "\\]([[:space:]]|$)" { capture=1 }
    capture && /^## \[/ && $0 !~ "^## \\[" version "\\]([[:space:]]|$)" { exit }
    capture { print }
  ' "$PB_HOME/CHANGELOG.md" >"$PUBLISH_TMP/changelog-section.md"
  [ -s "$PUBLISH_TMP/changelog-section.md" ] ||
    { echo "release: FAIL - CHANGELOG has no section for $VERSION" >&2; exit 1; }
  {
    printf '# POWDERBURN %s\n\n' "$VERSION"
    printf -- '- Target: `%s`\n' "$TARGET"
    printf -- '- Artifact sha256: `%s`\n' "$HASH"
    printf -- '- Minisign public-key sha256: `%s`\n' "$KEY_FINGERPRINT"
    printf -- '- Save compatibility: compatible with saves whose format, ruleset, and content hashes match; otherwise loading is refused without migration.\n\n'
    cat "$PUBLISH_TMP/changelog-section.md"
  } >"$PUBLISH_TMP/NOTES.md"
  rm "$PUBLISH_TMP/changelog-section.md"
  mv "$PUBLISH_TMP" "$DEST"
  printf '%s %s %s %s %s artifact=%s signature=%s\n' \
    "$VERSION" "$TARGET" "$HASH" "$BYTES" "$TIMESTAMP" \
    "$(basename "$ARTIFACT")" "$(basename "$ARTIFACT").sig" >>"$PB_RELEASE_DIR/index.txt"
  ln -sfn "$VERSION" "$PB_RELEASE_DIR/current"
  echo "publish: ok"
fi

echo "MANUAL STEP: itch.io publication"
echo "butler push \"$DEST\" USER/GAME:linux --userversion \"$VERSION\""
