NODE-META-BEGIN
ID: EP-004
DEPS: EP-003
MAX_ATTEMPTS_PER_MILESTONE: 6
VERIFY: sh scripts/test-integration.sh
VERIFY_SENTINEL: test-integration: ok
GREEN_TAG: green/EP-004
NODE-META-END

# EP-004 Service Layer: the command surface every proof drives

## 1. Purpose and Big Picture

The game has no HTTP API, so the service layer is the command line. This node builds `pbcli` and
`pbtool` exactly as SPEC-003 specifies, including the journal format that makes replay possible and
the exact output lines that live-fire greps byte for byte. When it is done, every ship criterion
except rendering is scriptable, and the ten live-fire proofs can begin to run for real.

## 2. Scope

`pbcli`: sim, replay, campaign new, campaign play, campaign audit, bench turn, selftest, a11y-report
stub surface (the checks land in EP-005), serve-replay behind a feature. `pbtool`: validate content,
validate representation, validate provenance, golden refresh, image stats, atlas pack. The journal
parser and writer. The error contract: every SPEC-006 code surfaced with its exact string.

## 3. Non-goals

- No `capture` implementation; the flag is declared here and implemented in EP-005 where the renderer
  exists.
- No accessibility checks; EP-005.
- No UI.
- No new simulation rules.

## 4. Context and Orientation

Entry is `green/EP-003`. The contract risk in this node is output drift: LF proofs match exact lines,
so a stray capitalization or a reordered key breaks proofs that look unrelated. The contract tests in
M5 exist for exactly that reason.

## 5. Files to Read First

    .agent/specs/SPEC-003-api-contracts.md
    .agent/specs/SPEC-006-error-handling.md
    COMMANDS.md
    scripts/live-fire.sh

## 6. Expected Changed Files

    crates/pb-cli/src/{main,args,cmd_sim,cmd_replay,cmd_campaign,cmd_bench,cmd_selftest,journal,output}.rs
    crates/pb-cli/src/replay_server.rs
    crates/pb-cli/Cargo.toml
    crates/pb-cli/tests/{contract,replay,errors,e2e}.rs
    crates/pb-tools/src/{main,validate,golden,image,atlas}.rs
    crates/pb-tools/tests/validate.rs
    tests/journals/{m01_elk_creek_victory,m04_whitehorse_dies,prov_sixty_actors}.jrnl
    tests/journals/{branch_spare_teague,branch_kill_teague}.script

## 7. Interfaces and Contracts

The subcommand table, flags, and sentinels in SPEC-003 sections 1 and 2 are transcribed exactly. The
journal grammar in SPEC-003 section 3 is transcribed exactly:

    <tick> <actor_id> <Command> <arg>=<value> ...

Commands: Move, Face, Stance, Snap, Aimed, Called, Fan, Reload, ClearJam, DrawBead, Bandage, Rally,
Throw, Melee, Loot, UseItem, Hold, EndTurn. An illegal line at its tick is `E-JOURNAL-ILLEGAL`, a hard
error, never a skip.

## 8. Milestones

### M1: Argument parsing and the output module
GOAL: Every flag in SPEC-003 exists, and every sentinel is emitted from one place.
READ: SPEC-003 sections 1 and 2
CHANGE: crates/pb-cli/src/{main,args,output}.rs, crates/pb-tools/src/main.rs
CONTENT: parse arguments by hand or with a vendored minimal parser; no dependency is added for this.
Every sentinel string is a `const` in `output.rs` so a change is a one-line diff that the contract
test catches. Unknown flags exit 2 with `E-CLI-001` and a usage line on stderr.
RUN: cargo run --offline -q -p pb-cli --bin pbcli -- --help >/dev/null && echo "cli: parses"
EXPECT: `cli: parses`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-004 MILESTONE_PASS "M1 cli: parses"
FALLBACK: none needed.
COMMIT: git add -A && git commit -m "[EP-004][M1] cli argument surface and sentinel constants"

### M2: The journal, sim, and replay
GOAL: A journal drives the kernel and a replay reproduces a hash exactly.
READ: SPEC-003 section 3, crates/pb-sim/tests/determinism.rs
CHANGE: crates/pb-cli/src/{journal,cmd_sim,cmd_replay}.rs, crates/pb-cli/tests/replay.rs,
tests/journals/prov_sixty_actors.jrnl
CONTENT: the parser rejects a malformed line with `E-JOURNAL-PARSE` naming the line number and an
illegal command with `E-JOURNAL-ILLEGAL` naming tick, actor, and command. `sim --emit-hash` prints
exactly `state-hash: <64 hex>`. `replay --expect` prints `replay: match` or
`replay: differ at tick <n>`.
RUN:
    cargo test --offline -p pb-cli --locked --test replay
    cargo run --offline -q -p pb-cli --bin pbcli -- replay --journal tests/journals/prov_full_battle.jrnl --expect "$(cat "$PB_GOLDEN_DIR/prov_full_battle.hash")"
EXPECT: `test result: ok` then `replay: match`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-004 MILESTONE_PASS "M2 replay: match"
FALLBACK: if a journal proves unwieldy to author by hand for the sixty-actor scenario, add
`pbcli sim --record <path>` which writes the AI's own decisions as a journal, then commit the
recording. That is a real feature the project wants anyway, not a shortcut.
COMMIT: git add -A && git commit -m "[EP-004][M2] journal format, sim, and replay"

### M3: Campaign subcommands
GOAL: A campaign can be created, played from a journal or a script, and audited.
READ: SPEC-002 section 5, SPEC-003 section 1
CHANGE: crates/pb-cli/src/cmd_campaign.rs, tests/journals/m01_elk_creek_victory.jrnl,
tests/journals/m04_whitehorse_dies.jrnl, tests/journals/branch_*.script
CONTENT: `campaign play --emit-outcome` prints `outcome: VICTORY|DEFEAT|WITHDRAWN` and
`ledger-entries: <n>`. `--emit-manifest` prints one `available: <mission_id>` line per reachable
mission, sorted, which is what LF-06 diffs. `campaign audit --dangling-refs` prints
`dangling-refs: <n>` and one `ledger-entry: <id> present` line per dead companion. A `.script` file
is a sequence of `mission <id>` and `choose <flag>` lines that drives multiple missions in one run.
RUN:
    cargo run --offline -q -p pb-cli --bin pbcli -- campaign new --save "$PB_CACHE_DIR/t.pbsave" --company-seed 90210
    cargo run --offline -q -p pb-cli --bin pbcli -- campaign play --save "$PB_CACHE_DIR/t.pbsave" --mission m01_elk_creek --journal tests/journals/m01_elk_creek_victory.jrnl --emit-outcome
EXPECT: `campaign: created` then `outcome: VICTORY` and `ledger-entries: 4`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-004 MILESTONE_PASS "M3 outcome: VICTORY"
FALLBACK: if authoring a winning journal for m01 by hand is intractable, use `--record` from M2's
fallback with the AI playing both sides, then trim and commit the recording.
COMMIT: git add -A && git commit -m "[EP-004][M3] campaign subcommands"

### M4: pbtool validate, golden, image, atlas
GOAL: The content tools exist and refuse to do dangerous things.
READ: SPEC-003 section 2, ARCHITECTURE.md forbidden moves
CHANGE: crates/pb-tools/src/{validate,golden,image,atlas}.rs, crates/pb-tools/tests/validate.rs
CONTENT: `golden refresh` checks `git status --porcelain` is empty and refuses otherwise with a named
error, so a golden can never be silently regenerated over uncommitted work. `image stats` prints
`unique-colors: <n>` and `dimensions: <w>x<h>`. `validate content --with-fixture` loads the real tree
plus one fixture and is expected to fail; its exit code is non-zero and its output names the code.
RUN:
    cargo run --offline -q -p pb-tools --bin pbtool -- validate content
    cargo run --offline -q -p pb-tools --bin pbtool -- validate representation
EXPECT: `pbtool validate: ok` then `representation: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-004 MILESTONE_PASS "M4 pbtool validate: ok"
FALLBACK: none needed.
COMMIT: git add -A && git commit -m "[EP-004][M4] pbtool content, golden, image, atlas commands"

### M5: The contract tests and bench and selftest
GOAL: Every output line the proofs depend on is asserted by a test, and the budget instrument exists.
READ: scripts/live-fire.sh, SPEC-007 section 3
CHANGE: crates/pb-cli/src/{cmd_bench,cmd_selftest}.rs, crates/pb-cli/tests/{contract,errors,e2e}.rs
CONTENT: the contract test asserts, byte for byte, every sentinel that `scripts/live-fire.sh` greps:
`outcome: VICTORY`, `ledger-entries: 4`, `event: HitLocation actor=... location=GunArm`,
`event: WoundApplied actor=... wound=Broken`, `event: WeaponDropped actor=... item=...`,
`state-hash: `, `resumed-hash: `, `chain: intact`, `event: CompanionKilled id=`, `dangling-refs: 0`,
`ledger-entry: ... present`, `available: `, `replay: match`, `worst-ai-turn-ms: `,
`worst-sim-step-ms: `, `selftest: ok`, `metric: `. The errors test asserts each SPEC-006 code appears
exactly as written.
RUN:
    cargo test --offline -p pb-cli --locked --test contract --test errors
    sh scripts/test-integration.sh
EXPECT: `test result: ok` then `test-integration: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-004 MILESTONE_PASS "M5 test-integration: ok"
FALLBACK: none needed. If a sentinel in `live-fire.sh` has no producer, that is a defect in this node,
not in the script; implement the producer.
COMMIT: git add -A && git commit -m "[EP-004][M5] contract tests, bench, selftest"

### M6: The feature-gated replay server
GOAL: The only network code in the workspace exists, is confined, and is proven unreachable from the
shipped app.
READ: SECURITY.md, scripts/security-check.sh
CHANGE: crates/pb-cli/src/replay_server.rs, crates/pb-cli/Cargo.toml
CONTENT: `serve-replay` binds 127.0.0.1 only, is behind the `replay-server` cargo feature which is not
in the default feature set, and is used only by the EP-007 replay differ. `pb-app` must not name the
feature anywhere.
RUN:
    sh scripts/build.sh
    sh scripts/security-check.sh
EXPECT: `build: ok` then `security-check: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-004 MILESTONE_PASS "M6 security-check: ok"
FALLBACK: if the security check cannot distinguish the feature-gated path, drop `serve-replay`
entirely and have the EP-007 differ compare two journal files offline. Losing the server costs
nothing; weakening LBI-09 costs everything.
COMMIT: git add -A && git commit -m "[EP-004][M6] feature-gated local replay server"

## 9. Validation and Acceptance

| Criterion | Command | Sentinel |
| --- | --- | --- |
| Journal replay reproduces the golden | `pbcli replay --expect` | `replay: match` |
| Campaign play reaches victory | M3 command | `outcome: VICTORY` |
| Content tools work | `pbtool validate content` | `pbtool validate: ok` |
| Every live-fire sentinel has a producer | `--test contract` | `test result: ok` |
| Every SPEC-006 code surfaces exactly | `--test errors` | `test result: ok` |
| No network outside the gated server | `sh scripts/security-check.sh` | `security-check: ok` |
| Whole node | `sh scripts/test-integration.sh` | `test-integration: ok` |

## 10. Idempotence and Recovery

`git reset --hard green/EP-003`. Delete `$PB_CACHE_DIR` freely; nothing there is authoritative.

## 11. Progress
- [ ] M1 Argument parsing and the output module
- [ ] M2 The journal, sim, and replay
- [ ] M3 Campaign subcommands
- [ ] M4 pbtool validate, golden, image, atlas
- [ ] M5 The contract tests and bench and selftest
- [ ] M6 The feature-gated replay server

## 12. Surprises and Discoveries

## 13. Decision Log

## 14. Outcomes and Retrospective
