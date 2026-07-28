#!/usr/bin/env sh
# Validate the append-only release index and every published artifact.
set -eu

MODE="${1:---check}"
[ "$MODE" = "--check" ] ||
  { echo "index: FAIL - usage: $0 --check" >&2; exit 1; }
: "${PB_RELEASE_DIR:?PB_RELEASE_DIR must be set}"
[ -d "$PB_RELEASE_DIR" ] ||
  { echo "index: FAIL - release directory missing: $PB_RELEASE_DIR" >&2; exit 1; }

INDEX="$PB_RELEASE_DIR/index.txt"
PUBKEY="$PB_RELEASE_DIR/powderburn.pub"

for artifact in "$PB_RELEASE_DIR"/*/*.tar.zst; do
  [ -f "$artifact" ] || continue
  directory=$(dirname "$artifact")
  canonical_directory=$(readlink -f "$directory")
  name=$(basename "$artifact")
  [ -f "$artifact.sha256" ] ||
    { echo "index: FAIL - checksum missing for $name" >&2; exit 1; }
  [ -f "$artifact.sig" ] ||
    { echo "index: FAIL - signature missing for $name" >&2; exit 1; }
  [ -f "$PUBKEY" ] ||
    { echo "index: FAIL - public key missing" >&2; exit 1; }
  (cd "$directory" && sha256sum -c "$name.sha256" >/dev/null) ||
    { echo "index: FAIL - checksum mismatch for $name" >&2; exit 1; }
  minisign -V -q -p "$PUBKEY" -m "$artifact" -x "$artifact.sig" ||
    { echo "index: FAIL - signature mismatch for $name" >&2; exit 1; }
  hash=$(sha256sum "$artifact" | awk '{print $1}')
  bytes=$(wc -c <"$artifact" | tr -d ' ')
  version=$(basename "$canonical_directory")
  [ -f "$INDEX" ] && grep -Eq "^$version [^ ]+ $hash $bytes [0-9]{4}-[0-9]{2}-[0-9]{2}T[^ ]+ artifact=$name signature=$name.sig$" "$INDEX" ||
    { echo "index: FAIL - artifact not recorded: $name" >&2; exit 1; }
done

echo "index: ok"
