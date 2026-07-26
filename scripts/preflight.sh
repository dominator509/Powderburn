#!/usr/bin/env sh
# 6LAYER preflight: files, tools, environment, credential probes.
# Must print "preflight: ok" before any graph node may start.
# The ONLY legitimate pre-run stop is a failure here.
set -eu
fail() { echo "preflight: FAIL - $1" >&2; exit 1; }
[ -f AGENTS.md ] && [ -d .agent ] || fail "run from repository root"
for f in AGENTS.md COMMANDS.md PREFLIGHT.md .env.example \
         .agent/GRAPH.md .agent/LOOPS.md .agent/state/LEDGER.md \
         .agent/reality-patterns .agent/reality-allow; do
  [ -f "$f" ] || fail "missing required file: $f"
done
for t in git awk grep sed tar zstd sha256sum python3 cargo rustc rustup minisign; do
  command -v "$t" >/dev/null 2>&1 || fail "missing required tool: $t"
done
rv=$(rustc --version 2>/dev/null | awk '{print $2}')
[ "$rv" = "1.85.0" ] || fail "rustc must be exactly 1.85.0, found ${rv:-none} (see PREFLIGHT.md section 1)"
gv=$(git --version 2>/dev/null | awk '{print $3}')
gmaj=$(printf '%s' "$gv" | cut -d. -f1); gmin=$(printf '%s' "$gv" | cut -d. -f2)
[ "$gmaj" -gt 2 ] || { [ "$gmaj" -eq 2 ] && [ "$gmin" -ge 30 ]; } || fail "git must be 2.30 or newer, found ${gv:-none}"
zv=$(zstd --version 2>/dev/null | sed -n 's/.*v\([0-9][0-9]*\.[0-9][0-9]*\).*/\1/p')
zmaj=$(printf '%s' "$zv" | cut -d. -f1); zmin=$(printf '%s' "$zv" | cut -d. -f2)
[ "${zmaj:-0}" -gt 1 ] || { [ "${zmaj:-0}" -eq 1 ] && [ "${zmin:-0}" -ge 5 ]; } || fail "zstd must be 1.5 or newer, found ${zv:-none}"
pv=$(python3 -c 'import sys; print("%d.%d" % sys.version_info[:2])' 2>/dev/null || echo 0.0)
pmaj=$(printf '%s' "$pv" | cut -d. -f1); pmin=$(printf '%s' "$pv" | cut -d. -f2)
[ "$pmaj" -gt 3 ] || { [ "$pmaj" -eq 3 ] && [ "$pmin" -ge 10 ]; } || fail "python3 must be 3.10 or newer, found ${pv:-none}"
[ -f .env ] || fail "missing .env (copy .env.example, fill every REQUIRED value, rerun)"
set -a
. ./.env
set +a
TMP=$(mktemp)
trap 'rm -f "$TMP"' EXIT
awk '/^PREFLIGHT-TABLE-BEGIN$/{t=1;next} /^PREFLIGHT-TABLE-END$/{t=0} t && NF' PREFLIGHT.md > "$TMP"
[ -s "$TMP" ] || fail "PREFLIGHT-TABLE missing or empty in PREFLIGHT.md"
if command -v timeout >/dev/null 2>&1; then TCMD="timeout 30"; else TCMD=""; fi
while IFS='|' read -r var req probe; do
  var=$(printf '%s' "$var" | tr -d ' ')
  req=$(printf '%s' "$req" | tr -d ' ')
  probe=$(printf '%s' "$probe" | tr -d ' ')
  [ -n "$var" ] || continue
  eval "val=\${$var:-}"
  if [ -z "$val" ]; then
    if [ "$req" = "REQUIRED" ]; then fail "env var not set: $var (see PREFLIGHT.md)"; fi
    echo "preflight: optional $var not set; dependent features disabled"
    continue
  fi
  if [ "$probe" != "-" ]; then
    [ -f "$probe" ] || fail "missing probe script: $probe"
    if ! $TCMD sh "$probe" >/dev/null 2>&1; then
      fail "credential probe failed: $var ($probe). Fix the credential, rerun preflight."
    fi
  fi
done < "$TMP"
echo "preflight: ok"
