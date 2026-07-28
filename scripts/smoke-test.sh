#!/usr/bin/env sh
# Smoke the built tree or, with --released, only an unpacked release archive.
set -eu

: "${PB_HOME:?PB_HOME must be set}"
: "${PB_CACHE_DIR:?PB_CACHE_DIR must be set}"
mkdir -p "$PB_CACHE_DIR"
CACHE_ROOT=$PB_CACHE_DIR

BIN_DIR="$PB_HOME/target/release"
RUN_ROOT="$PB_HOME"
SCRATCH=""

cleanup() {
  if [ -n "$SCRATCH" ]; then
    case "$SCRATCH" in
      "$CACHE_ROOT"/smoke.*) rm -rf "$SCRATCH" ;;
      *) echo "smoke-test: unsafe scratch path: $SCRATCH" >&2 ;;
    esac
  fi
}
trap cleanup EXIT HUP INT TERM

if [ "${1:-}" = "--released" ]; then
  [ "$#" -eq 2 ] ||
    { echo "smoke-test: FAIL - usage: $0 --released <artifact.tar.zst>" >&2; exit 1; }
  ARTIFACT=$2
  [ -f "$ARTIFACT" ] ||
    { echo "smoke-test: FAIL - artifact not found: $ARTIFACT" >&2; exit 1; }
  ARTIFACT_DIR=$(cd "$(dirname "$ARTIFACT")" && pwd)
  ARTIFACT_NAME=$(basename "$ARTIFACT")
  if [ -f "$ARTIFACT.sha256" ]; then
    (cd "$ARTIFACT_DIR" && sha256sum -c "$ARTIFACT_NAME.sha256" >/dev/null) ||
      { echo "smoke-test: FAIL - checksum mismatch" >&2; exit 1; }
  fi
  if [ -f "$ARTIFACT.sig" ]; then
    PUBKEY="${PB_RELEASE_SIGNING_KEY:-}.pub"
    [ -f "$PUBKEY" ] ||
      PUBKEY="$ARTIFACT_DIR/powderburn.pub"
    [ -f "$PUBKEY" ] ||
      { echo "smoke-test: FAIL - signature exists but public key is missing" >&2; exit 1; }
    minisign -Vm "$ARTIFACT" -x "$ARTIFACT.sig" -p "$PUBKEY" >/dev/null 2>&1 ||
      { echo "smoke-test: FAIL - signature verification failed" >&2; exit 1; }
  fi

  SCRATCH=$(mktemp -d "$CACHE_ROOT/smoke.XXXXXX")
  tar --use-compress-program=unzstd -xf "$ARTIFACT" -C "$SCRATCH"
  RUN_ROOT=$(find "$SCRATCH" -mindepth 1 -maxdepth 1 -type d | head -n 1)
  [ -n "$RUN_ROOT" ] ||
    { echo "smoke-test: FAIL - archive has no package root" >&2; exit 1; }
  BIN_DIR="$RUN_ROOT/bin"
fi

for binary in powderburn pbcli pbtool; do
  [ -x "$BIN_DIR/$binary" ] ||
    { echo "smoke-test: FAIL - missing released binary: $BIN_DIR/$binary" >&2; exit 1; }
done

RUNTIME_HOME="${SCRATCH:-$PB_CACHE_DIR}/runtime-home"
mkdir -p "$RUNTIME_HOME/config" "$RUNTIME_HOME/saves" "$RUNTIME_HOME/cache"
export PB_HOME="$RUNTIME_HOME"
export PB_CONFIG_DIR="$RUNTIME_HOME/config"
export PB_SAVE_DIR="$RUNTIME_HOME/saves"
export PB_CACHE_DIR="$RUNTIME_HOME/cache"

cd "$RUN_ROOT"
"$BIN_DIR/powderburn" --version >/dev/null ||
  { echo "smoke-test: FAIL - app version command failed" >&2; exit 1; }
"$BIN_DIR/pbcli" selftest --emit-hash >"$PB_CACHE_DIR/selftest.log" 2>&1 ||
  { echo "smoke-test: FAIL - selftest failed" >&2; exit 1; }
grep -qx 'selftest: ok' "$PB_CACHE_DIR/selftest.log" ||
  { echo "smoke-test: FAIL - selftest sentinel missing" >&2; exit 1; }

"$BIN_DIR/pbcli" capture --scenario content/scenarios/prov_full_battle.ron --seed 1867 \
  --out "$PB_CACHE_DIR/frame.png" >/dev/null 2>&1 ||
  { echo "smoke-test: FAIL - headless capture failed" >&2; exit 1; }
[ -s "$PB_CACHE_DIR/frame.png" ] ||
  { echo "smoke-test: FAIL - capture is empty" >&2; exit 1; }

"$BIN_DIR/pbcli" campaign new --save "$PB_SAVE_DIR/smoke.pbsave" --company-seed 90210 \
  >/dev/null 2>&1 ||
  { echo "smoke-test: FAIL - campaign creation failed" >&2; exit 1; }
"$BIN_DIR/pbcli" campaign play --save "$PB_SAVE_DIR/smoke.pbsave" \
  --mission m01_elk_creek --autoplay --emit-outcome >"$PB_CACHE_DIR/mission.log" 2>&1 ||
  { echo "smoke-test: FAIL - released mission play failed" >&2; exit 1; }
grep -qx 'outcome: VICTORY' "$PB_CACHE_DIR/mission.log" ||
  { echo "smoke-test: FAIL - released mission did not reach victory" >&2; exit 1; }

"$BIN_DIR/pbcli" sim --scenario content/scenarios/prov_full_battle.ron --seed 1867 \
  --journal tests/journals/prov_full_battle.jrnl --suspend-at-tick 144 \
  --save "$PB_SAVE_DIR/mid-combat.pbsave" --emit-hash >"$PB_CACHE_DIR/suspend.log" 2>&1 ||
  { echo "smoke-test: FAIL - mid-combat save failed" >&2; exit 1; }
"$BIN_DIR/pbcli" sim --resume "$PB_SAVE_DIR/mid-combat.pbsave" --emit-hash \
  >"$PB_CACHE_DIR/resume.log" 2>&1 ||
  { echo "smoke-test: FAIL - mid-combat resume failed" >&2; exit 1; }
grep -qx 'chain: intact' "$PB_CACHE_DIR/resume.log" ||
  { echo "smoke-test: FAIL - resumed Ledger chain is not intact" >&2; exit 1; }

echo "smoke-test: ok"
echo "smoke: ok"
