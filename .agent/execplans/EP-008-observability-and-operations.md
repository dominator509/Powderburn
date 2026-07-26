NODE-META-BEGIN
ID: EP-008
DEPS: EP-007
MAX_ATTEMPTS_PER_MILESTONE: 6
VERIFY: sh scripts/verify.sh
VERIFY_SENTINEL: verify: ok
GREEN_TAG: green/EP-008
NODE-META-END

# EP-008 Observability and Operations

## 1. Purpose and Big Picture

A shipped offline game has no dashboard to watch, so observability means something specific here:
when a player reports that something went wrong, the artifacts on their disk must be enough for a
maintainer to reproduce it exactly. This node builds structured logging with redaction, the metrics
that back the budget claims, the crash bundle, the trace facility, and the runbooks that turn a
report into a reproduction.

## 2. Scope

Structured logging with levels and a stable field vocabulary. The metric set from SPEC-007 emitted by
`pbcli selftest --emit-metrics`. Crash bundles containing the reproduction triple of seed, journal,
and content hash. The trace facility for shots, AI decisions, and rng draws. Log rotation and size
caps. Every runbook in `docs/runbooks/`. The `pbtool repro` command that turns a crash bundle back
into a running scenario.

## 3. Non-goals

- No telemetry, no phoning home, no opt-in analytics. LBI-09 forbids it and ADR-0002 records the
  decision.
- No log shipping and no aggregation service.
- No performance optimization; this node measures and reports, and EP-007 already met the budgets.

## 4. Context and Orientation

Entry is `green/EP-007`. The design constraint that shapes everything here is that the maintainer
will never have the player's machine. Therefore every artifact must be self-contained and every
reproduction must be exact, which is possible only because LBI-01 makes the simulation deterministic.
The reproduction triple is the payoff of every determinism decision made in EP-002.

## 5. Files to Read First

    .agent/specs/SPEC-007-observability.md
    OBSERVABILITY.md
    OPERATIONS.md
    crates/pb-core/src/redact.rs

## 6. Expected Changed Files

    crates/pb-core/src/{log,metrics}.rs
    crates/pb-app/src/{main,crashbundle}.rs
    crates/pb-cli/src/{cmd_selftest,cmd_trace}.rs
    crates/pb-tools/src/repro.rs
    crates/pb-cli/tests/observability.rs
    docs/runbooks/{crash-report,save-will-not-load,desync-or-nondeterminism,performance-regression,content-validation-failure,release-rollback}.md
    OBSERVABILITY.md
    OPERATIONS.md

## 7. Interfaces and Contracts

    log: <level> ts=<iso8601> mod=<module> event=<name> <k>=<v> ...
    metric: <name> <value> <unit>

Both formats are transcribed from SPEC-007 sections 2 and 3 and are greppable contracts. Field names
come from the SPEC-007 section 2 vocabulary and nowhere else; inventing a field name is a defect.

## 8. Milestones

### M1: Structured logging with the field vocabulary
GOAL: Every log line parses, uses only declared field names, and redacts by construction.
READ: SPEC-007 sections 1 and 2
CHANGE: crates/pb-core/src/log.rs, call sites across the workspace
CONTENT: five levels, error through trace, with the default at info and `PB_LOG` overriding. The
logger takes a field enum, not free strings, so an undeclared field is a compile error. Every value
passes through `redact::path` and `redact::user` on the way out. Logs go to
`$PB_HOME/logs/powderburn.log`, rotating at 8 MiB with three files kept, so a long-running install
cannot fill a disk.
RUN:
    cargo run --offline -q -p pb-cli --bin pbcli -- selftest
    awk '!/^log: (error|warn|info|debug|trace) ts=/ {print "BAD: " $0; bad=1} END {exit bad+0}' "$PB_HOME/logs/powderburn.log" && echo "log-format: ok"
EXPECT: `selftest: ok` then `log-format: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-008 MILESTONE_PASS "M1 log-format: ok"
FALLBACK: none needed.
COMMIT: git add -A && git commit -m "[EP-008][M1] structured logging with a closed field vocabulary"

### M2: The metric set
GOAL: Every claim the project makes about its budgets is backed by an emitted number.
READ: SPEC-007 section 3, SPEC-008 section 2
CHANGE: crates/pb-core/src/metrics.rs, crates/pb-cli/src/cmd_selftest.rs
CONTENT: emit exactly the SPEC-007 section 3 set: `sim.step.ms`, `ai.turn.ms`, `render.frame.ms`,
`content.load.ms`, `save.write.ms`, `save.load.ms`, `rng.draws.per_turn`, `smoke.tiles.active`,
`actors.alive`, `ledger.entries`, `mem.rss.mb`. Each is emitted as `metric: <name> <value> <unit>`
with the worst case, not the mean, for the timing metrics, because a worst case is what a player
feels.
RUN:
    cargo run --offline -q -p pb-cli --bin pbcli -- selftest --emit-metrics
EXPECT: eleven `metric:` lines, one per name above
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-008 MILESTONE_PASS "M2 metrics emitted"
FALLBACK: if a metric cannot be measured cheaply, emit it with the value `unavailable` and record why
in OBSERVABILITY.md, rather than omitting the line and leaving a silent gap.
COMMIT: git add -A && git commit -m "[EP-008][M2] the metric set"

### M3: The crash bundle and the reproduction triple
GOAL: A crash produces a bundle that reproduces the crash on another machine.
READ: SPEC-007 section 4, crates/pb-core/src/redact.rs
CHANGE: crates/pb-app/src/crashbundle.rs, crates/pb-tools/src/repro.rs
CONTENT: the bundle is a directory under `$PB_HOME/crash/<timestamp>/` containing `report.txt` with
the redacted report from EP-006 M5, `journal.jrnl` with every command of the current mission,
`meta.toml` with the seed, scenario id, content hash, ruleset hash, version, and commit, and
`state.hash`. `pbtool repro <dir>` loads the meta, replays the journal, and asserts the terminal hash
matches, printing `repro: match` or `repro: differ at tick <n>`.
RUN:
    cargo run --offline -q -p pb-app --bin powderburn -- --headless --scenario prov_full_battle --force-panic-at-tick 200 || true
    cargo run --offline -q -p pb-tools --bin pbtool -- repro "$(ls -d "$PB_HOME"/crash/* | tail -1)"
EXPECT: `repro: match`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-008 MILESTONE_PASS "M3 repro: match"
FALLBACK: none. If the bundle does not reproduce, determinism is broken and the fix belongs in
EP-002's code with a regression test, not in a workaround here.
COMMIT: git add -A && git commit -m "[EP-008][M3] crash bundle and pbtool repro"

### M4: The trace facility
GOAL: A maintainer can see every input to a decision without a debugger.
READ: SPEC-007 section 5
CHANGE: crates/pb-cli/src/cmd_trace.rs
CONTENT: `--trace-shot` prints every one of the ten pipeline stages with every named term of the hit
chance assembly and every rng draw with its stream tag and address. `--trace-actor` prints every AI
candidate with its component scores and the chosen candidate index. `--trace-rng` prints every draw as
`draw seed=<..> scenario=<..> tick=<..> actor=<..> stream=<..> lo=<..> hi=<..> value=<..>`, which is
the addressing scheme from LBI-01 made visible.
RUN:
    cargo run --offline -q -p pb-cli --bin pbcli -- sim --scenario prov_called_shot --journal tests/journals/prov_called_shot.jrnl --trace-shot | grep -c "^stage: " 
EXPECT: a count of at least 10
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-008 MILESTONE_PASS "M4 trace facility ok"
FALLBACK: none needed.
COMMIT: git add -A && git commit -m "[EP-008][M4] shot, actor, and rng tracing"

### M5: The runbooks
GOAL: Six named failures each have a runbook a maintainer can follow without prior context.
READ: OPERATIONS.md, .agent/templates/runbook-template.md
CHANGE: docs/runbooks/*.md, OPERATIONS.md
CONTENT: each runbook follows the template: symptom, immediate check, diagnosis steps with exact
commands, resolution, and prevention. The desync runbook walks from a player report to
`pbtool repro`. The save-will-not-load runbook maps each SPEC-006 save error to its cause and its
remedy, including the case where the remedy is that the save is genuinely unrecoverable and the
Ledger is the only thing that can be salvaged, which is a real outcome the runbook must state
plainly.
RUN:
    for f in crash-report save-will-not-load desync-or-nondeterminism performance-regression content-validation-failure release-rollback; do test -s "docs/runbooks/$f.md" || { echo "MISSING $f"; exit 1; }; done; echo "runbooks: complete"
EXPECT: `runbooks: complete`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-008 MILESTONE_PASS "M5 runbooks: complete"
FALLBACK: none needed.
COMMIT: git add -A && git commit -m "[EP-008][M5] six operational runbooks"

### M6: Prove the observability chain end to end
GOAL: An induced failure flows from crash to bundle to reproduction to a named runbook step.
READ: docs/runbooks/desync-or-nondeterminism.md
CHANGE: crates/pb-cli/tests/observability.rs
CONTENT: the test induces a panic in a child process, locates the newest crash bundle, asserts it
contains all four files, runs `pbtool repro` against it, asserts `repro: match`, and asserts the
report contains the state hash and contains neither the home path literal nor the user name.
RUN:
    cargo test --offline -p pb-cli --locked --test observability
    sh scripts/verify.sh
EXPECT: `test result: ok` then `verify: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-008 MILESTONE_PASS "M6 verify: ok"
FALLBACK: none needed.
COMMIT: git add -A && git commit -m "[EP-008][M6] end-to-end observability proof"

## 9. Validation and Acceptance

| Criterion | Command | Sentinel |
| --- | --- | --- |
| Every log line parses | M1 awk check | `log-format: ok` |
| Eleven metrics emitted | `pbcli selftest --emit-metrics` | eleven `metric:` lines |
| Crash bundles reproduce | `pbtool repro` | `repro: match` |
| Trace shows all ten stages | M4 grep | count at least 10 |
| Six runbooks exist | M5 loop | `runbooks: complete` |
| Chain proven end to end | `--test observability` | `test result: ok` |
| Whole node | `sh scripts/verify.sh` | `verify: ok` |

## 10. Idempotence and Recovery

`git reset --hard green/EP-007`. Delete `$PB_HOME/logs` and `$PB_HOME/crash` freely; both are
regenerated and neither is authoritative.

## 11. Progress
- [ ] M1 Structured logging with the field vocabulary
- [ ] M2 The metric set
- [ ] M3 The crash bundle and the reproduction triple
- [ ] M4 The trace facility
- [ ] M5 The runbooks
- [ ] M6 Prove the observability chain end to end

## 12. Surprises and Discoveries

## 13. Decision Log

## 14. Outcomes and Retrospective
