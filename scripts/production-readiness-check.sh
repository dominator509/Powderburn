#!/usr/bin/env sh
# Evaluate every SPEC-008 ship criterion without weakening or substituting it.
set -eu

: "${PB_HOME:?PB_HOME must be set}"
: "${PB_RELEASE_DIR:?PB_RELEASE_DIR must be set}"
: "${PB_CACHE_DIR:?PB_CACHE_DIR must be set}"
cd "$PB_HOME"
fail() { echo "production-readiness: FAIL - $1" >&2; exit 1; }

mode="${1:-readiness}"
[ "$#" -le 1 ] || fail "usage: $0 [--ship-gate]"
[ "$mode" = readiness ] || [ "$mode" = "--ship-gate" ] ||
  fail "usage: $0 [--ship-gate]"

[ -z "$(git status --porcelain)" ] ||
  fail "working tree is dirty; ship evidence must come from a clean tree"
mkdir -p "$PB_CACHE_DIR"

sh scripts/spec-coverage-check.sh >/dev/null || fail "spec coverage is incomplete"
sh scripts/verify.sh >"$PB_CACHE_DIR/readiness-verify.log" 2>&1 ||
  fail "verify.sh failed; see $PB_CACHE_DIR/readiness-verify.log"
grep -q '^verify: ok$' "$PB_CACHE_DIR/readiness-verify.log" ||
  fail "verify sentinel missing"
sh scripts/coverage-check.sh >"$PB_CACHE_DIR/readiness-coverage.log" 2>&1 ||
  fail "coverage thresholds failed; see $PB_CACHE_DIR/readiness-coverage.log"
if grep -RIn '#\[ignore\]' crates >/dev/null 2>&1; then
  fail "ignored tests remain"
fi

sh scripts/reality-gate.sh >/dev/null || fail "reality gate failed"
sh scripts/security-check.sh >/dev/null || fail "security gate failed"
sh scripts/dependency-audit.sh >/dev/null || fail "dependency audit failed"
git ls-files --error-unmatch .env >/dev/null 2>&1 && fail ".env is tracked"

./target/release/pbcli a11y-report >"$PB_CACHE_DIR/readiness-a11y.log" 2>&1 ||
  fail "accessibility report failed"
grep -qx 'a11y: ok' "$PB_CACHE_DIR/readiness-a11y.log" || fail "a11y sentinel missing"

./target/release/pbcli selftest --emit-metrics >"$PB_CACHE_DIR/readiness-metrics.log" 2>&1 ||
  fail "metrics selftest failed"
for metric in \
  sim.step.ms ai.turn.ms render.frame.ms sim.events.per_turn content.load.ms \
  save.write.ms save.size.bytes smoke.volumes.live rng.draws.per_turn
do
  grep -q "^metric: $metric " "$PB_CACHE_DIR/readiness-metrics.log" ||
    fail "required metric missing: $metric"
done
metric_value() {
  awk -v metric="$1" '$1 == "metric:" && $2 == metric { print $3; exit }' \
    "$PB_CACHE_DIR/readiness-metrics.log"
}
[ "$(metric_value content.load.ms)" -le 900 ] || fail "content.load.ms exceeds 900"
[ "$(metric_value save.write.ms)" -le 250 ] || fail "save.write.ms exceeds 250"
[ "$(metric_value save.size.bytes)" -le 8388608 ] || fail "save.size.bytes exceeds 8 MiB"

sh scripts/golden-campaign.sh >"$PB_CACHE_DIR/readiness-golden-campaign.log" 2>&1 ||
  fail "golden campaign replay failed"
grep -qx 'golden-campaign: ok' "$PB_CACHE_DIR/readiness-golden-campaign.log" ||
  fail "golden campaign sentinel missing"

[ -L "$PB_RELEASE_DIR/current" ] || fail "published current release symlink missing"
release_directory=$(cd "$PB_RELEASE_DIR/current" && pwd -P)
artifact=$(find "$release_directory" -maxdepth 1 -type f -name '*.tar.zst' | head -n 1)
[ -n "$artifact" ] || fail "current release artifact missing"
sh scripts/smoke-test.sh --released "$artifact" >"$PB_CACHE_DIR/readiness-smoke.log" 2>&1 ||
  fail "released smoke failed"
sh scripts/live-fire.sh --released "$release_directory" >"$PB_CACHE_DIR/readiness-live-fire.log" 2>&1 ||
  fail "released live-fire failed"
grep -qx 'live-fire: ok' "$PB_CACHE_DIR/readiness-live-fire.log" ||
  fail "released live-fire sentinel missing"
sh scripts/make-release-index.sh --check >/dev/null || fail "release index failed"
[ -f "$release_directory/REFERENCE_MACHINE.txt" ] ||
  fail "release reference machine metadata missing"
[ -f "$release_directory/NOTES.md" ] || fail "release notes missing"

sh scripts/reproducibility-check.sh >"$PB_CACHE_DIR/readiness-repro.log" 2>&1 ||
  fail "reproducibility failed; see $PB_CACHE_DIR/readiness-repro.log"
grep -qx 'reproducible: identical' "$PB_CACHE_DIR/readiness-repro.log" ||
  fail "reproducibility sentinel missing"

grep -q 'ROLLBACK DRILL COMPLETED' .agent/state/LEDGER.md ||
  fail "rollback drill is not recorded"
for file in \
  OPERATIONS.md ROLLBACK.md DEPLOYMENT.md RELEASE.md OBSERVABILITY.md \
  .agent/checklists/incident-response.md .agent/checklists/release.md
do
  [ -f "$file" ] || fail "operations document missing: $file"
done
grep -q 'MANUAL STEP: itch.io publication' RELEASE.md ||
  fail "manual external-publish boundary missing"
unsafe_butler=$(grep -RIn 'butler push' scripts 2>/dev/null | grep -v 'echo' || true)
[ -z "$unsafe_butler" ] || fail "a script can execute the manual itch.io command"

echo "LBI-01: ok"
echo "LBI-02: ok"
echo "LBI-03: ok"
echo "LBI-04: ok"
echo "LBI-05: ok"
echo "LBI-06: ok"
echo "LBI-07: ok"
echo "LBI-08: ok"
echo "LBI-09: ok"
echo "LBI-10: ok"
echo "LBI-11: ok"
echo "LBI-12: ok"
echo "LBI-13: ok"
echo "production-readiness: ok"

if [ "$mode" = "--ship-gate" ]; then
  tag=$(git describe --tags --exact-match HEAD 2>/dev/null || true)
  case "$tag" in
    v[0-9]*.[0-9]*.[0-9]*) ;;
    *) fail "HEAD has no immutable semantic release tag" ;;
  esac
  version=${tag#v}
  [ "$(basename "$release_directory")" = "$version" ] ||
    fail "published current release does not match $tag"
  grep -q "RUN_COMPLETE.*$tag" .agent/state/LEDGER.md ||
    fail "RUN_COMPLETE for $tag is absent from the ledger"
  echo "ship-gate: pass"
fi
