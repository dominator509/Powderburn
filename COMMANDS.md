# COMMANDS.md - the only legal command source for POWDERBURN

Coding agents must not invent commands. If a command is missing or stale, update this file first,
citing repository evidence, with a Decision Log entry in the active ExecPlan and a ledger append.

## Working directory rule

Every command below runs from the repository root, the directory containing AGENTS.md. Never `cd`
into a crate to run a cargo command; use `-p <crate>`. Scripts resolve paths from `$PB_HOME`, so a
stray `cd` is a defect, not a shortcut.

## Non-interactive environment block

Export this at session start, before anything else. Verbatim:

    export CI=true
    export GIT_TERMINAL_PROMPT=0
    export GIT_PAGER=cat
    export PAGER=cat
    export DEBIAN_FRONTEND=noninteractive
    export CARGO_TERM_COLOR=never
    export CARGO_NET_OFFLINE=true
    export RUST_BACKTRACE=1
    set -a; . ./.env; set +a

## The command table

| Purpose | Exact command | Sentinel |
| --- | --- | --- |
| install / vendor | `sh scripts/install.sh` | `install: ok` |
| preflight | `sh scripts/preflight.sh` | `preflight: ok` |
| format check | `sh scripts/format-check.sh` | `format-check: ok` |
| lint | `sh scripts/lint.sh` | `lint: ok` |
| typecheck | `sh scripts/typecheck.sh` | `typecheck: ok` |
| unit tests | `sh scripts/test-unit.sh` | `test-unit: ok` |
| integration tests | `sh scripts/test-integration.sh` | `test-integration: ok` |
| end to end | `sh scripts/test-e2e.sh` | `test-e2e: ok` |
| build | `sh scripts/build.sh` | `build: ok` |
| reality gate | `sh scripts/reality-gate.sh` | `reality gate: ok` |
| live fire | `sh scripts/live-fire.sh` | `live-fire: ok` |
| security check | `sh scripts/security-check.sh` | `security-check: ok` |
| dependency audit | `sh scripts/dependency-audit.sh` | `dependency-audit: ok` |
| smoke | `sh scripts/smoke-test.sh` | `smoke: ok` |
| smoke against release | `sh scripts/smoke-test.sh --released` | `smoke: ok` |
| full verify | `sh scripts/verify.sh` | `verify: ok` |
| production readiness | `sh scripts/production-readiness-check.sh` | `production-readiness: ok` |
| content validate | `cargo run --offline -q -p pb-tools --bin pbtool -- validate content` | `pbtool validate: ok` |
| golden refresh | `cargo run --offline -q -p pb-tools --bin pbtool -- golden refresh` | `pbtool golden: ok` |
| scheduler | `sh scripts/graph-next.sh` | one of NEXT, RESUME, BLOCKED, STALL, ALL_DONE |
| ledger append | `sh scripts/ledger.sh append <AGENT_ID> <NODE> <EVENT> <detail>` | none |
| ledger status | `sh scripts/ledger.sh status EP-XXX` | DONE, BLOCKED, IN_PROGRESS, PENDING |
| ledger tail | `sh scripts/ledger.sh tail 30` | none |

## Headless simulation commands (the spine of every proof)

Run one scripted scenario deterministically and print the terminal state hash:

    cargo run --offline -q -p pb-cli --bin pbcli -- sim \
      --scenario content/scenarios/<id>.ron \
      --seed <u64> \
      --journal tests/journals/<id>.jrnl \
      --emit-hash

Sentinel: a single line `state-hash: <64 hex chars>`.

Replay an existing journal and compare to the golden hash:

    cargo run --offline -q -p pb-cli --bin pbcli -- replay \
      --journal tests/journals/<id>.jrnl \
      --expect "$(cat "$PB_GOLDEN_DIR/<id>.hash")"

Sentinel: `replay: match`.

Capture one deterministic frame with no display attached:

    cargo run --offline -q -p pb-cli --bin pbcli -- capture \
      --scenario content/scenarios/<id>.ron \
      --seed <u64> \
      --adapter "$PB_HEADLESS_ADAPTER" \
      --out "$PB_CACHE_DIR/<id>.png"

Sentinel: `capture: ok <sha256>`.

## Local run (backgrounded, with readiness probe and kill path)

POWDERBURN is a desktop binary, not a server, so the only backgrounded process in this repository is
the headless simulator used by end-to-end proofs. Start, probe, kill:

    cargo run --offline -q -p pb-cli --bin pbcli -- serve-replay --port 8791 & echo $! > "$PB_CACHE_DIR/pbcli.pid"
    i=0; while [ "$i" -lt 30 ]; do if curl -sf http://127.0.0.1:8791/healthz >/dev/null 2>&1; then break; fi; i=$((i+1)); sleep 2; done
    [ "$i" -lt 30 ] || { echo "READINESS_TIMEOUT_pbcli" >&2; exit 1; }
    kill "$(cat "$PB_CACHE_DIR/pbcli.pid")" 2>/dev/null || true

`serve-replay` binds 127.0.0.1 only, exists solely for the replay differ used in EP-007, is behind
the `replay-server` cargo feature, and is never compiled into a release artifact. This is the single
exception to LBI-09 and it is enforced by `scripts/security-check.sh`, which fails if the release
binary contains any socket symbol.

## Adapter parity check

Run verbatim. All cksum lines must be identical:

    for f in AGENTS.md CLAUDE.md GEMINI.md .github/copilot-instructions.md .cursor/rules/6layer.mdc .clinerules/6layer.md .hermes/instructions.md .openclaw/instructions.md; do awk '/PRIME-BLOCK-BEGIN/,/PRIME-BLOCK-END/' "$f" | cksum; done

## Forbidden commands

Never run any of these. There is no situation in this repository that requires one.

- Interactive REPLs or editors: `vi`, `vim`, `nano`, `emacs`, `less`, `more`, `man`, `cargo add -i`.
- Watch or foreground-blocking modes: `cargo watch`, `bacon`, any `--watch` flag, `tail -f` inside a
  milestone (the operator may use it outside the run as telemetry).
- Network-touching cargo: any cargo invocation without `--offline`, `cargo update`, `cargo install`
  from a registry, `cargo publish`.
- History and remote damage: `git push --force`, `git rebase`, `git filter-branch`, `git reset --hard`
  outside the rollback protocol, `git tag -d` on a green tag, `git clean -fdx` at repository root.
- Destructive filesystem: `rm -rf` on any path outside `$PB_CACHE_DIR`, `target/`, or `dist/`.
- Anything that opens a pager, an editor, a browser, or a credential prompt.

## Recovery pointers

A command that fails takes you to .agent/LOOPS.md section 5.3, the verify-fix ladder. A service that
never becomes ready takes you to section 5.4 with signature `READINESS_TIMEOUT_<service>`. A milestone
over budget takes you to section 5.3 rung 3 with signature `BUDGET_EXCEEDED`. A terminal failure takes
you to section 5.7, the blocked report format.
