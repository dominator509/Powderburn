#!/usr/bin/env sh
# 6LAYER deterministic scheduler. Reads GRAPH-TABLE and the ledger.
# Prints exactly one line:
#   NEXT <id>    first PENDING node whose deps are all DONE
#   RESUME <id>  a node holds an unreleased lease
#   BLOCKED <id> a node is terminally blocked
#   STALL <id>   no eligible node but work remains (graph defect; treat as BLOCKED)
#   ALL_DONE     every node is DONE
set -eu
GRAPH=".agent/GRAPH.md"
[ -f "$GRAPH" ] || { echo "graph-next.sh: missing $GRAPH" >&2; exit 1; }
tmp=$(mktemp)
trap 'rm -f "$tmp" "$tmp.status"' EXIT
awk '
  /^GRAPH-TABLE-BEGIN$/ { t=1; next }
  /^GRAPH-TABLE-END$/   { t=0 }
  t && $1=="NODE"       { print $2, $4 }
' "$GRAPH" > "$tmp"
[ -s "$tmp" ] || { echo "graph-next.sh: GRAPH-TABLE empty or missing" >&2; exit 1; }
: > "$tmp.status"
while read -r id deps; do
  st=$(sh scripts/ledger.sh status "$id")
  printf '%s %s %s\n' "$id" "$st" "$deps" >> "$tmp.status"
done < "$tmp"
blocked=$(awk '$2=="BLOCKED"{print $1; exit}' "$tmp.status")
if [ -n "$blocked" ]; then echo "BLOCKED $blocked"; exit 0; fi
resume=$(awk '$2=="IN_PROGRESS"{print $1; exit}' "$tmp.status")
if [ -n "$resume" ]; then echo "RESUME $resume"; exit 0; fi
next=$(awk '
  { st[$1]=$2; ord[NR]=$1; dep[$1]=$3; n=NR }
  END {
    for (i=1; i<=n; i++) {
      id=ord[i]
      if (st[id]=="PENDING") {
        ok=1
        m=split(dep[id], a, ",")
        for (j=1; j<=m; j++) { d=a[j]; if (d!="-" && st[d]!="DONE") { ok=0; break } }
        if (ok) { print id; exit }
      }
    }
  }
' "$tmp.status")
if [ -n "$next" ]; then
  echo "NEXT $next"
else
  undone=$(awk '$2!="DONE"{print $1; exit}' "$tmp.status")
  if [ -z "$undone" ]; then echo "ALL_DONE"; else echo "STALL $undone"; fi
fi
