#!/usr/bin/env sh
# Mechanical coverage for promises that ordinary compile/test gates cannot see.
set -eu
: "${PB_HOME:?PB_HOME must be set}"
cd "$PB_HOME"
fail() { echo "spec-coverage: FAIL - $1" >&2; exit 1; }

missions=$(grep -c 'kind: "Mission"' content/campaign/nodes.ron || true)
camps=$(grep -c 'kind: "Camp"' content/campaign/nodes.ron || true)
[ "$missions" -eq 24 ] || fail "SPEC-000 requires 24 missions; found $missions"
[ "$camps" -eq 12 ] || fail "SPEC-000 requires 12 camp interludes; found $camps"

weapons=$(grep -c '^[[:space:]]*id: "' content/rules/weapons.ron || true)
[ "$weapons" -eq 26 ] || fail "SPEC-001 requires exactly 26 period-correct weapons; found $weapons"
if grep -Eqi 'alien weapon|god weapon|mythic weapon|legendary weapon|f_alien' \
  content/rules/weapons.ron content/rules/factions.ron; then
  fail "out-of-scope fantasy or alien records remain in historical rules data"
fi

[ -d content/dialogue ] || fail "content/dialogue is missing"
dialogue_files=$(find content/dialogue -maxdepth 1 -type f -name '*.ron' | wc -l)
[ "$dialogue_files" -ge 13 ] ||
  fail "expected companion and act dialogue corpus; found $dialogue_files files"

for screen in \
  Title NewCompany Camp MapTravel Briefing Battle AfterAction LedgerView Settings Bibliography \
  SaveSlot LoadSlot
do
  grep -Eq "^[[:space:]]+$screen," crates/pb-render/src/ui_contract.rs ||
    fail "runtime GameScreen is missing $screen"
done

for event in \
  TurnBegin TurnEnd Moved StanceChanged Fired Misfire Jammed Missed HitLocation \
  DamageApplied WoundApplied Critical WeaponDropped SmokeDeposited SmokeDecayed \
  SandLost MoraleStateChanged Routed ActorKilled CompanionKilled ObjectiveComplete \
  LedgerEntryWritten ScenarioEnded
do
  grep -Eq "^[[:space:]]+$event([[:space:]]|\\{|,)" crates/pb-core/src/event.rs ||
    fail "event vocabulary is missing $event"
done

for test_path in \
  crates/pb-sim/tests/ap_economy.rs \
  crates/pb-sim/tests/shot_pipeline.rs \
  crates/pb-sim/tests/hit_locations.rs \
  crates/pb-sim/tests/explosives.rs \
  crates/pb-sim/tests/environment.rs \
  crates/pb-sim/tests/morale.rs \
  crates/pb-content/tests/anachronism.rs \
  crates/pb-content/tests/schema.rs \
  crates/pb-content/tests/hashing.rs \
  crates/pb-cli/tests/contract.rs \
  crates/pb-cli/tests/replay.rs \
  crates/pb-app/tests/a11y.rs \
  crates/pb-render/tests/capture.rs \
  crates/pb-save/tests/integrity.rs \
  crates/pb-content/tests/mod_sandbox.rs \
  crates/pb-cli/tests/errors.rs \
  crates/pb-cli/tests/observability.rs \
  crates/pb-content/tests/representation.rs \
  crates/pb-content/tests/history.rs \
  crates/pb-content/tests/permadeath.rs
do
  [ -f "$test_path" ] || fail "TESTING.md maps behavior to missing $test_path"
done

if grep -q 'Full state reconstruction.*left for' crates/pb-app/src/saveload.rs; then
  fail "application save/load still declares state reconstruction unfinished"
fi

if grep -RIn '#\[ignore\]' crates >/dev/null 2>&1; then
  fail "SPEC-008 requires zero ignored tests"
fi

echo "spec-coverage: ok"
