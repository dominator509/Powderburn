#!/usr/bin/env sh
# Regenerate the static release index from what is actually on disk.
# Never deletes an artifact. The newest version directory becomes current.
set -eu
DIR="${1:?release dir}"
[ -d "$DIR" ] || { echo "make-release-index: FAIL - no such directory: $DIR" >&2; exit 1; }
IDX="$DIR/index.html"
{
  echo '<!doctype html><meta charset="utf-8"><title>POWDERBURN releases</title>'
  echo '<h1>POWDERBURN releases</h1>'
  echo '<p>Verify with: minisign -Vm FILE -p powderburn.pub</p><ul>'
  for d in $(ls -1 "$DIR" | grep -E '^[0-9]+\.[0-9]+\.[0-9]+$' | sort -Vr); do
    for f in "$DIR/$d"/*.tar.zst; do
      [ -f "$f" ] || continue
      b=$(basename "$f")
      s=$(sha256sum "$f" | awk '{print $1}')
      printf '<li>%s <a href="%s/%s">%s</a> sha256 %s <a href="%s/%s.minisig">sig</a></li>\n' \
        "$d" "$d" "$b" "$b" "$s" "$d" "$b"
    done
  done
  echo '</ul>'
} > "$IDX"
echo "make-release-index: ok"
