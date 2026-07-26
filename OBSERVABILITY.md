# OBSERVABILITY

This file instantiates SPEC-007. It is the operator-facing version; the spec is the authority.

## Logging strategy

Structured, line oriented, to stderr and `$PB_CONFIG_DIR/log/powderburn.log`, rotated at 8 MB with
three files kept.

    <ISO8601-UTC> <LEVEL> <target> <key>=<value> ... msg="<text>"

Every line carries `build`, `ruleset`, and `content`. Battle lines add `scenario`, `tick`, `seed`.
Default filter INFO; override with `PB_LOG`, for example `PB_LOG=pb_sim=debug,pb_render=warn`.

## Redaction

Never logged: environment values, `.env` contents, absolute paths outside the config and install
directories, player-chosen names, Ledger entry text. Paths appear relative to the install root or as
`<config>/...`. Enforced by `scripts/security-check.sh`.

## Metrics

Emitted on demand by `pbcli selftest --emit-metrics` and by the in-game debug overlay, one per line
as `metric: <name> <value>`. The set and budgets are in SPEC-007 section 3: `sim.step.ms` at or under
16, `ai.turn.ms` at or under 120, `render.frame.ms` p95 at or under 16, `content.load.ms` under 900,
`save.write.ms` under 250, `save.size.bytes` under 8 MB, `smoke.volumes.live` under 4096, plus
`sim.events.per_turn` and `rng.draws.per_turn`.

`rng.draws.per_turn` is the canary. A change in draw count for a golden scenario means the shot
pipeline changed shape even if the terminal hash happens to survive. Investigate before shipping.

## Traces

`pbcli sim --trace-actor <id>` emits every AI candidate with its utility score, every modifier in the
shot assembly with its signed value, and every RNG draw with its stream tag. This is the first tool
to reach for at ladder rung 2 on any determinism or balance question.

## Health checks

`pbcli selftest`. See OPERATIONS.md.

## Dashboards

None, deliberately. The equivalent is the after-action report inside the game, which renders the
event log as prose, and the metrics dump.

## Alerts

Build time only. `scripts/verify.sh` fails on any budget regression. There is no runtime alerting
because there is no runtime service, and pretending otherwise would be theatre.

## Production debugging

A player reports a bug. Ask for three files: the crash artifact from `$PB_CONFIG_DIR/crash/`, the log
file, and the save. The crash artifact carries the build, ruleset, and content hashes, which pin the
exact code and data. The save carries the seed and the campaign flags. Together they reproduce the
run: `pbcli campaign audit --save <f>` then `pbcli sim --resume <f> --emit-events`.

## Acceptance criteria (verified in EP-008)

1. Every SPEC-007 metric emitted by `pbcli selftest --emit-metrics`.
2. Every log line carries build, ruleset, and content.
3. `PB_LOG=pb_sim=trace` measurably increases output volume.
4. A forced panic in a debug build writes a crash artifact containing exactly the SPEC-006 section 4
   fields and nothing forbidden, proven by a scripted grep.
5. Rotation proven by writing 9 MB and asserting two files exist.
