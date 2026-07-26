#!/usr/bin/env sh
# LIVE FIRE: the definition of "the software actually works".
# One scripted, non-interactive proof per core user outcome, executed against the
# real simulation kernel, the real content tree, the real renderer, and real files
# on disk. No mocks. No fixtures standing in for the thing under test.
# Prints "live-fire: ok" only if every proof passes.
set -eu
: "${PB_HOME:?PB_HOME must be set}"
: "${PB_CACHE_DIR:?PB_CACHE_DIR must be set}"
: "${PB_GOLDEN_DIR:?PB_GOLDEN_DIR must be set}"
: "${PB_ASSET_ROOT:?PB_ASSET_ROOT must be set}"
: "${PB_HEADLESS_ADAPTER:?PB_HEADLESS_ADAPTER must be set}"
cd "$PB_HOME"
mkdir -p "$PB_CACHE_DIR/live-fire"
LF="$PB_CACHE_DIR/live-fire"
CLI="./target/release/pbcli"
TOOL="./target/release/pbtool"
[ -x "$CLI" ]  || { echo "live-fire: FAIL - $CLI missing; run sh scripts/build.sh first" >&2; exit 1; }
[ -x "$TOOL" ] || { echo "live-fire: FAIL - $TOOL missing; run sh scripts/build.sh first" >&2; exit 1; }
fail() { echo "live-fire: FAIL - $1" >&2; exit 1; }
note() { printf 'live-fire: %s\n' "$1"; }

# LF-01 A player can start a new campaign and win the opening mission end to end.
note "LF-01 opening mission Elk Creek"
"$CLI" campaign new --save "$LF/lf01.pbsave" --company-seed 90210 >"$LF/lf01.new.log" 2>&1 \
  || fail "LF-01 campaign new failed, see $LF/lf01.new.log"
"$CLI" campaign play \
  --save "$LF/lf01.pbsave" \
  --mission m01_elk_creek \
  --journal tests/journals/m01_elk_creek_victory.jrnl \
  --emit-outcome >"$LF/lf01.play.log" 2>&1 \
  || fail "LF-01 mission play failed, see $LF/lf01.play.log"
grep -qx 'outcome: VICTORY' "$LF/lf01.play.log" || fail "LF-01 did not reach VICTORY"
grep -qx 'ledger-entries: 4' "$LF/lf01.play.log" || fail "LF-01 in-game Ledger did not record the four scripted deaths"

# LF-02 Called shots produce real, located, mechanical consequences.
note "LF-02 called shot to the gun arm"
"$CLI" sim \
  --scenario content/scenarios/prov_called_shot.ron \
  --seed 7 \
  --journal tests/journals/prov_called_shot.jrnl \
  --emit-events >"$LF/lf02.log" 2>&1 \
  || fail "LF-02 sim failed, see $LF/lf02.log"
grep -qx 'event: HitLocation actor=e_bandit_02 location=GunArm' "$LF/lf02.log" || fail "LF-02 no GunArm hit recorded"
grep -qx 'event: WoundApplied actor=e_bandit_02 wound=Broken' "$LF/lf02.log"  || fail "LF-02 Broken wound not applied"
grep -qx 'event: WeaponDropped actor=e_bandit_02 item=colt_army_1860' "$LF/lf02.log" || fail "LF-02 weapon was not dropped"

# LF-03 Determinism. Same ruleset, same content, same seed, same journal, same hash. Three runs.
note "LF-03 determinism triple run"
h1=$("$CLI" sim --scenario content/scenarios/prov_full_battle.ron --seed 1867 --journal tests/journals/prov_full_battle.jrnl --emit-hash | sed -n 's/^state-hash: //p')
h2=$("$CLI" sim --scenario content/scenarios/prov_full_battle.ron --seed 1867 --journal tests/journals/prov_full_battle.jrnl --emit-hash | sed -n 's/^state-hash: //p')
h3=$("$CLI" sim --scenario content/scenarios/prov_full_battle.ron --seed 1867 --journal tests/journals/prov_full_battle.jrnl --emit-hash | sed -n 's/^state-hash: //p')
[ -n "$h1" ] || fail "LF-03 no state hash emitted"
[ "$h1" = "$h2" ] && [ "$h2" = "$h3" ] || fail "LF-03 determinism drift: $h1 $h2 $h3"
golden=$(cat "$PB_GOLDEN_DIR/prov_full_battle.hash")
[ "$h1" = "$golden" ] || fail "LF-03 hash differs from golden corpus: got $h1 want $golden"

# LF-04 Save and load in the middle of a firefight preserves exact simulation state.
note "LF-04 mid-combat save round trip"
"$CLI" sim \
  --scenario content/scenarios/prov_full_battle.ron --seed 1867 \
  --journal tests/journals/prov_full_battle.jrnl \
  --suspend-at-tick 340 --save "$LF/lf04.pbsave" --emit-hash >"$LF/lf04.a" 2>&1 \
  || fail "LF-04 suspend failed"
ha=$(sed -n 's/^state-hash: //p' "$LF/lf04.a")
"$CLI" sim --resume "$LF/lf04.pbsave" --emit-hash >"$LF/lf04.b" 2>&1 || fail "LF-04 resume failed"
hb=$(sed -n 's/^resumed-hash: //p' "$LF/lf04.b")
[ "$ha" = "$hb" ] || fail "LF-04 save round trip changed state: $ha vs $hb"
grep -qx 'chain: intact' "$LF/lf04.b" || fail "LF-04 in-game Ledger hash chain not intact after load"

# LF-05 A dead companion stays dead and every reference to them is gated, everywhere, forever.
note "LF-05 permadeath propagation"
"$CLI" campaign play --save "$LF/lf01.pbsave" --mission m04_medicine_lodge \
  --journal tests/journals/m04_whitehorse_dies.jrnl --emit-outcome >"$LF/lf05.log" 2>&1 \
  || fail "LF-05 mission play failed"
grep -qx 'event: CompanionKilled id=c_whitehorse' "$LF/lf05.log" || fail "LF-05 companion did not die"
"$CLI" campaign audit --save "$LF/lf01.pbsave" --dangling-refs >"$LF/lf05.audit" 2>&1 \
  || fail "LF-05 audit command failed"
grep -qx 'dangling-refs: 0' "$LF/lf05.audit" || fail "LF-05 dead companion still referenced by reachable content"
grep -qx 'ledger-entry: c_whitehorse present' "$LF/lf05.audit" || fail "LF-05 death not written into the in-game Ledger"

# LF-06 A moral choice made in Act II really changes what exists in Act III.
note "LF-06 branch divergence"
"$CLI" campaign play --save "$LF/lf06a.pbsave" --from-new --company-seed 90210 \
  --script tests/journals/branch_spare_teague.script --emit-manifest >"$LF/lf06a.log" 2>&1 \
  || fail "LF-06 branch A failed"
"$CLI" campaign play --save "$LF/lf06b.pbsave" --from-new --company-seed 90210 \
  --script tests/journals/branch_kill_teague.script --emit-manifest >"$LF/lf06b.log" 2>&1 \
  || fail "LF-06 branch B failed"
if diff -q "$LF/lf06a.log" "$LF/lf06b.log" >/dev/null 2>&1; then
  fail "LF-06 the two branches produced identical content manifests; the choice is cosmetic"
fi
grep -qx 'available: m12_adobe_walls_relief' "$LF/lf06a.log" || fail "LF-06 branch A missing its exclusive mission"
grep -qx 'available: m12_hide_yard_reckoning' "$LF/lf06b.log" || fail "LF-06 branch B missing its exclusive mission"

# LF-07 History is weather. The campaign may not rewrite a recorded outcome.
note "LF-07 historical immutability"
if "$TOOL" validate content --with-fixture tests/fixtures/violation_alters_history.ron >"$LF/lf07.log" 2>&1; then
  fail "LF-07 validator accepted content that alters a HISTORICAL_FIXED outcome"
fi
grep -q 'E-HIST-001' "$LF/lf07.log" || fail "LF-07 validator rejected for the wrong reason; expected E-HIST-001"
"$TOOL" validate content >"$LF/lf07.clean" 2>&1 || fail "LF-07 real content tree failed validation"
grep -qx 'pbtool validate: ok' "$LF/lf07.clean" || fail "LF-07 clean validation sentinel missing"

# LF-08 The renderer really draws the game, provably, with no display attached.
note "LF-08 headless frame capture"
"$CLI" capture --scenario content/scenarios/prov_full_battle.ron --seed 1867 \
  --adapter "$PB_HEADLESS_ADAPTER" --out "$LF/lf08.png" >"$LF/lf08.log" 2>&1 \
  || fail "LF-08 capture failed, see $LF/lf08.log"
[ -s "$LF/lf08.png" ] || fail "LF-08 capture produced an empty file"
"$TOOL" image stats "$LF/lf08.png" >"$LF/lf08.stats" 2>&1 || fail "LF-08 image stats failed"
uniq_colors=$(sed -n 's/^unique-colors: //p' "$LF/lf08.stats")
[ "${uniq_colors:-0}" -ge 256 ] || fail "LF-08 frame is blank or flat: only ${uniq_colors:-0} unique colors"
cap=$(sed -n 's/^capture: ok //p' "$LF/lf08.log")
goldcap=$(cat "$PB_GOLDEN_DIR/prov_full_battle.frame.sha256")
[ "$cap" = "$goldcap" ] || fail "LF-08 frame hash drift: got $cap want $goldcap"

# LF-09 Every depiction carries its nation, its community, and its sources; no asset is unlicensed.
note "LF-09 representation and provenance"
"$TOOL" validate representation >"$LF/lf09.log" 2>&1 || fail "LF-09 representation lint failed, see $LF/lf09.log"
grep -qx 'representation: ok' "$LF/lf09.log" || fail "LF-09 representation sentinel missing"
"$TOOL" validate provenance --asset-root "$PB_ASSET_ROOT" >"$LF/lf09.prov" 2>&1 \
  || fail "LF-09 asset provenance failed, see $LF/lf09.prov"
grep -qx 'provenance: ok' "$LF/lf09.prov" || fail "LF-09 provenance sentinel missing"

# LF-10 A crowded battle stays inside its turn budget on the reference machine.
note "LF-10 turn budget under load"
"$CLI" bench turn --scenario content/scenarios/prov_sixty_actors.ron --seed 4 --iterations 20 \
  --emit-budget >"$LF/lf10.log" 2>&1 || fail "LF-10 bench failed"
worst=$(sed -n 's/^worst-ai-turn-ms: //p' "$LF/lf10.log")
[ -n "$worst" ] || fail "LF-10 no budget line emitted"
[ "$worst" -le 120 ] || fail "LF-10 worst AI turn ${worst}ms exceeds the 120ms budget in SPEC-008"
frame=$(sed -n 's/^worst-sim-step-ms: //p' "$LF/lf10.log")
[ "${frame:-999}" -le 16 ] || fail "LF-10 worst simulation step ${frame}ms exceeds the 16ms budget in SPEC-008"

echo "live-fire: ok"
