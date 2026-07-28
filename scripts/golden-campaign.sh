#!/usr/bin/env sh
# Deterministic end-to-end replay of one complete authored campaign path.
set -eu

: "${PB_HOME:?PB_HOME must be set}"
: "${PB_CACHE_DIR:?PB_CACHE_DIR must be set}"
cd "$PB_HOME"

CLI="./target/release/pbcli"
[ -x "$CLI" ] ||
  { echo "golden-campaign: FAIL - release pbcli missing" >&2; exit 1; }
RUN="$PB_CACHE_DIR/golden-campaign"
mkdir -p "$RUN"
SAVE="$RUN/company.pbsave"
LOG="$RUN/replay.log"
rm -f "$SAVE" "$LOG"

started=$(date +%s)
"$CLI" campaign new --save "$SAVE" --company-seed 90210 >>"$LOG" 2>&1

play() {
  mission=$1
  mission_log="$RUN/$mission.log"
  "$CLI" campaign play --save "$SAVE" --mission "$mission" --autoplay --emit-outcome \
    >"$mission_log" 2>&1 ||
    { echo "golden-campaign: FAIL - $mission failed; see $mission_log" >&2; exit 1; }
  grep -qx 'outcome: VICTORY' "$mission_log" ||
    { echo "golden-campaign: FAIL - $mission did not reach VICTORY" >&2; exit 1; }
  printf 'mission: %s VICTORY\n' "$mission" >>"$LOG"
}

for mission in \
  m01_elk_creek m002_pawnee_fork m06_smoky_hill_station m04_medicine_lodge \
  m07_washita_winter m08_washita_aftermath m02_promontory m09_rail_grade \
  m10_denver_extension m11_los_angeles_telegram m13_divide_crossing m14_panic_of_1873
do
  play "$mission"
done

"$CLI" campaign play --save "$SAVE" --choice warn_adobe_walls --emit-manifest \
  >"$RUN/choice.log" 2>&1
grep -qx 'available: m12_adobe_walls_relief' "$RUN/choice.log" ||
  { echo "golden-campaign: FAIL - Act II choice did not unlock relief path" >&2; exit 1; }

for mission in \
  m12_adobe_walls_relief m003_adobe_walls m05_hide_yard m15_red_river_supply \
  m16_black_hills_dispatch m18_great_strike m17_nicodemus m20_salt_war \
  m19_fort_marion m21_yellow_fever m24_ledger_reckoning
do
  play "$mission"
done

"$CLI" campaign audit --save "$SAVE" --dangling-refs >"$RUN/audit.log" 2>&1
grep -qx 'chain: intact' "$RUN/audit.log" ||
  { echo "golden-campaign: FAIL - final Ledger chain broken" >&2; exit 1; }
grep -qx 'dangling-refs: 0' "$RUN/audit.log" ||
  { echo "golden-campaign: FAIL - final campaign has dangling references" >&2; exit 1; }

elapsed=$(( $(date +%s) - started ))
[ "$elapsed" -le 540 ] ||
  { echo "golden-campaign: FAIL - ${elapsed}s exceeds 540s budget" >&2; exit 1; }
echo "golden-campaign-seconds: $elapsed"
echo "golden-campaign: ok"
