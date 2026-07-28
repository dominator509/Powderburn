#!/usr/bin/env sh
# Repoint current to one immutable published version and smoke the artifact.
set -eu

: "${PB_HOME:?PB_HOME must be set}"
: "${PB_CACHE_DIR:?PB_CACHE_DIR must be set}"
: "${PB_RELEASE_DIR:?PB_RELEASE_DIR must be set}"

TO=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --to)
      [ "$#" -ge 2 ] || { echo "rollback: FAIL - --to requires a version" >&2; exit 1; }
      TO=$2
      shift 2
      ;;
    *)
      echo "rollback: FAIL - unknown argument: $1" >&2
      exit 1
      ;;
  esac
done
[ -n "$TO" ] || { echo "rollback: FAIL - --to is required" >&2; exit 1; }

TARGET="$PB_RELEASE_DIR/$TO"
[ -d "$TARGET" ] ||
  { echo "rollback: FAIL - release does not exist: $TARGET" >&2; exit 1; }
ARTIFACT=$(find "$TARGET" -maxdepth 1 -type f -name '*.tar.zst' | head -n 1)
[ -n "$ARTIFACT" ] ||
  { echo "rollback: FAIL - release has no archive: $TARGET" >&2; exit 1; }

PREVIOUS=""
if [ -L "$PB_RELEASE_DIR/current" ]; then
  PREVIOUS=$(readlink "$PB_RELEASE_DIR/current")
fi
ln -sfn "$TO" "$PB_RELEASE_DIR/current"

if ! sh "$PB_HOME/scripts/smoke-test.sh" --released "$ARTIFACT" >/dev/null; then
  if [ -n "$PREVIOUS" ]; then
    ln -sfn "$PREVIOUS" "$PB_RELEASE_DIR/current"
  else
    rm -f "$PB_RELEASE_DIR/current"
  fi
  echo "rollback: FAIL - target smoke test failed; current restored" >&2
  exit 1
fi

TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)
printf 'ROLLBACK %s from=%s to=%s\n' "$TIMESTAMP" "${PREVIOUS:-none}" "$TO" \
  >>"$PB_RELEASE_DIR/index.txt"
echo "rollback: ok"
