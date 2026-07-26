#!/usr/bin/env sh
# The Section 13 ship standard, instantiated and machine checked.
# Every line of PRODUCTION_READINESS.md that carries a command is enforced here.
set -eu
: "${PB_HOME:?PB_HOME must be set}"
: "${PB_RELEASE_DIR:?PB_RELEASE_DIR must be set}"
cd "$PB_HOME"
fail() { echo "production-readiness: FAIL - $1" >&2; exit 1; }

# Functional: every core outcome has a live-fire proof wired in.
for id in LF-01 LF-02 LF-03 LF-04 LF-05 LF-06 LF-07 LF-08 LF-09 LF-10; do
  grep -q "$id" scripts/live-fire.sh || fail "core outcome $id has no live-fire proof"
done

# Testing: the whole chain is green from this tree.
[ -z "$(git status --porcelain)" ] || fail "working tree is dirty; ship gate requires a clean tree"
sh scripts/verify.sh >/dev/null || fail "verify.sh did not pass"

# Reality: no test doubles in production paths.
sh scripts/reality-gate.sh >/dev/null || fail "reality gate did not pass"

# Security and privacy.
sh scripts/security-check.sh >/dev/null || fail "security-check did not pass"
grep -q 'zero network calls' SECURITY.md || fail "SECURITY.md missing the no-network guarantee"
[ -f content/PROVENANCE.toml ] || fail "content/PROVENANCE.toml missing (LBI-11)"

# Accessibility floor.
./target/release/pbcli a11y-report > .pbcache/a11y.log 2>&1 || fail "a11y report failed"
grep -qx 'a11y: ok' .pbcache/a11y.log || fail "accessibility floor not met; see .pbcache/a11y.log"

# Observability.
grep -q 'PB_LOG' OBSERVABILITY.md || fail "OBSERVABILITY.md does not document the log control variable"
./target/release/pbcli selftest --emit-metrics > .pbcache/metrics.log 2>&1 || fail "metrics selftest failed"
grep -q '^metric: ' .pbcache/metrics.log || fail "no metrics emitted"

# Deployment and rollback.
[ -f DEPLOYMENT.md ] || fail "DEPLOYMENT.md missing"
[ -f ROLLBACK.md ]   || fail "ROLLBACK.md missing"
grep -q 'ROLLBACK DRILL COMPLETED' .agent/state/LEDGER.md || fail "no rollback drill recorded in the ledger (EP-009)"
[ -d "$PB_RELEASE_DIR" ] || fail "PB_RELEASE_DIR does not exist: $PB_RELEASE_DIR"

# Operations.
for f in OPERATIONS.md .agent/checklists/incident-response.md .agent/checklists/release.md; do
  [ -f "$f" ] || fail "missing operations document: $f"
done

echo "production-readiness: ok"
