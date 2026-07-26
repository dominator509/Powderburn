# SPEC-007 Observability

Layer L2. There is no server, no dashboard vendor, and no telemetry. Observability here means: when
something goes wrong on a player's machine or in a build, the artifact on disk is enough to explain
it.

## 1. Logging

Structured, line oriented, written to stderr and to `$PB_CONFIG_DIR/log/powderburn.log` with rotation
at 8 MB and three files kept. Format:

    <ISO8601-UTC> <LEVEL> <target> <key>=<value> ... msg="<text>"

Levels: ERROR, WARN, INFO, DEBUG, TRACE. Default filter is `INFO`, overridable by `PB_LOG` using the
standard directive syntax, for example `PB_LOG=pb_sim=debug,pb_render=warn`.

Required fields on every line: `build` (short git hash), `ruleset` (first 8 hex of ruleset hash),
`content` (first 8 hex of content hash). Battle lines additionally carry `scenario`, `tick`, `seed`.

## 2. Redaction rules

Never logged, at any level: the contents of `.env`, any value of any environment variable, any
absolute path outside `$PB_CONFIG_DIR` and the installation directory, the player's chosen character
names, and the full text of Ledger entries. Paths are logged relative to the installation root or as
`<config>/...`. `scripts/security-check.sh` fails on any log macro that interpolates an environment
value.

## 3. Metrics

Metrics are counters and histograms held in process and dumped on demand, not scraped. `pbcli
selftest --emit-metrics` and the in-game debug overlay both render the same set, one per line as
`metric: <name> <value>`.

| Metric | Type | Purpose |
| --- | --- | --- |
| `sim.step.ms` | histogram | Simulation step cost; budget 16ms at p100 |
| `ai.turn.ms` | histogram | Full AI turn for one actor; budget 120ms at p100 |
| `render.frame.ms` | histogram | Frame time; budget 16ms at p95 on the reference machine |
| `sim.events.per_turn` | histogram | Event volume, catches runaway loops |
| `content.load.ms` | gauge | Startup content parse time; budget 900ms |
| `save.write.ms` | gauge | Budget 250ms |
| `save.size.bytes` | gauge | Budget 8 MB, alarm at 32 MB |
| `smoke.volumes.live` | gauge | Catches smoke leak, budget 4096 |
| `rng.draws.per_turn` | counter | A change here across builds means a determinism break |

`rng.draws.per_turn` is the canary: any change in draw count for a golden scenario means the shot
pipeline changed shape, even if the final hash happens to survive.

## 4. Health and self test

`pbcli selftest` loads content, runs a fixed twelve-tick scenario, verifies the state hash against a
compiled-in constant, verifies the Ledger chain, and prints `selftest: ok`. It is the health check
used by `scripts/smoke-test.sh` and by the released artifact smoke.

## 5. Traces

Optional, off by default, enabled by `--trace-actor <id>` on `pbcli sim`. Emits every decision the
utility AI considered for that actor with its score, every modifier in the shot assembly, and every
RNG draw with its stream tag. This is the primary debugging instrument for LF-03 failures and is the
first thing rung 2 of the ladder reaches for.

## 6. Alerting

There is no runtime alerting because there is no runtime service. The equivalent is build-time:
`scripts/verify.sh` fails on any budget regression, and `bench turn` is run in EP-007 and EP-010
against the golden scenarios. A budget regression is a failing gate, not a warning.

## 7. Acceptance criteria (wired into EP-008)

1. Every metric in section 3 is emitted by `pbcli selftest --emit-metrics`.
2. Every log line carries `build`, `ruleset`, and `content`.
3. `PB_LOG=pb_sim=trace` changes output volume, proven by a line count comparison.
4. A forced panic in a debug build produces a crash artifact containing exactly the fields in
   SPEC-006 section 4 and no others, proven by a scripted grep for forbidden fields.
5. Log rotation is proven by writing 9 MB and asserting two files exist.
