# GRAPH - POWDERBURN build graph

## Narrative of the build arc

POWDERBURN is built inside out, because the thing that must be true first is determinism. EP-000
proves the toolchain and the command surface against an empty skeleton. EP-001 lays the Rust
workspace, the offline vendor tree, the lint and format law, and a verify.sh that is green on
nothing. EP-002 builds the simulation kernel: fixed-point math, the seeded generator, the
continuous-turn-based sequence clock, action points, hit locations, wounds, morale, and the state
hash that every later proof depends on. EP-003 makes content and saves real: the RON content schema,
the validator that enforces historical immutability and representation law, the append-only
hash-chained in-game Ledger, and save round-trip integrity. EP-004 exposes the command surface the
proofs drive: the headless simulator, the replay engine, and the campaign runner. From there the
graph splits: EP-005 builds the renderer, the input map, and the screens through to a headless frame
capture, while EP-006 hardens the trust boundaries, the no-network guarantee, and the mod sandbox.
They rejoin at EP-007, which hardens the test suite until every core outcome has a regression and
every failure mode has a forced-failure test. EP-008 adds structured logging, crash reporting that
carries no personal data, frame and turn budget metrics, and the runbooks. EP-009 produces signed,
reproducible artifacts and drills the rollback. EP-010 runs the whole thing from a clean tree, fires
all ten live-fire proofs, and ships.

## Node inventory

| Node | Title | Purpose | Exit evidence |
| --- | --- | --- | --- |
| EP-000 | Discovery and toolchain | Prove rustc 1.85.0, the offline vendor path, and every COMMANDS.md sentinel against an empty skeleton | `preflight: ok` and every stub script sentinel |
| EP-001 | Foundation | Workspace, lockfile, vendor tree, rustfmt, clippy deny set, one real passing test, green verify.sh | `verify: ok` |
| EP-002 | Core domain | Deterministic simulation kernel: Fix32, PbRng, sequence clock, AP economy, called shots, wounds, Sand, state hash | `test-unit: ok` plus LF-03 determinism triple-run |
| EP-003 | Data and persistence | Content schema and validator, campaign graph, the hash-chained Ledger, save and load | `test-integration: ok` plus `pbtool validate: ok` |
| EP-004 | Service layer | pbcli sim, replay, capture, campaign subcommands; error contract; journal format | `test-integration: ok` plus `replay: match` |
| EP-005 | Client | wgpu isometric renderer, input map, screens, accessibility floor, headless capture | `test-e2e: ok` plus `capture: ok` |
| EP-006 | Security baseline | Untrusted-input parsing, mod sandbox, no-network symbol audit, save integrity | `security-check: ok` |
| EP-007 | Testing hardening | Coverage targets, regression per core outcome, forced-failure tests, flaky purge | `verify: ok` from clean tree |
| EP-008 | Observability and operations | Structured logs, redaction, turn and frame budgets, crash artifacts, runbooks | `smoke: ok` |
| EP-009 | Deployment and release | Reproducible signed artifacts, release index, rollback drill | `build: ok` plus signature verification |
| EP-010 | Production readiness and ship | Clean-tree verify, ten live-fire proofs, reviews, ship gate | `production-readiness: ok` |

GRAPH-TABLE-BEGIN
NODE EP-000 DEPS -
NODE EP-001 DEPS EP-000
NODE EP-002 DEPS EP-001
NODE EP-003 DEPS EP-002
NODE EP-004 DEPS EP-003
NODE EP-005 DEPS EP-004
NODE EP-006 DEPS EP-004
NODE EP-007 DEPS EP-005,EP-006
NODE EP-008 DEPS EP-007
NODE EP-009 DEPS EP-008
NODE EP-010 DEPS EP-009
GRAPH-TABLE-END

## Graph rules (L1, immutable during a run)

One node equals one ExecPlan equals one bounded unit of work with entry evidence, exit evidence, and
a green tag. Cycles between nodes are forbidden. The only cycles anywhere are the bounded
intra-milestone loops of .agent/LOOPS.md.

SINGLE WRITER: at most one node is IN_PROGRESS repository-wide, ever. The holder is recorded by a
LEASE event in the ledger. Node status is derived from the ledger by `sh scripts/ledger.sh status`;
there is no separate status file, so there is nothing to fall out of sync.

A node is DONE only when all five hold: every milestone passed with evidence; the node VERIFY
command printed VERIFY_SENTINEL in this session; the Expected Changed Files audit passed; NODE_DONE
was appended; `green/EP-XXX` was tagged.

A run never rewires its own graph. If the graph is wrong, that is a generation defect and the correct
response is NODE_BLOCKED with a report, not an edit.

## Dispatch table

| graph-next.sh output | Action |
| --- | --- |
| `NEXT <id>` | Append LEASE for that node, then execute its ExecPlan from milestone one. |
| `RESUME <id>` | If the lease is yours, re-verify the last checked milestone sentinel and continue at the first unchecked milestone. If it is another agent's and its newest ledger event is older than 90 minutes, append LEASE_TAKEOVER and continue from ledger and plan state. Otherwise do nothing. |
| `BLOCKED <id>` | Terminal. Read the NODE_BLOCKED report in that plan's Progress section. Do not restart, do not work around. |
| `STALL <id>` | Graph defect. Append NODE_BLOCKED for that id with detail GRAPH_STALL. Treat as BLOCKED. |
| `ALL_DONE` | Run the ship gate in AGENTS.md section 15, then append RUN_COMPLETE. |

## Checkpoint, rollback, and cohesion

Commit after every milestone as `[EP-XXX][M<k>] <summary>`. Tag `green/EP-XXX` at NODE_DONE.
Rollback only from ladder rung 4: `git reset --hard <last green tag or last milestone commit>`,
append ROLLBACK with the target ref, re-enter on the declared FALLBACK. Rollback never crosses a
green tag.

Git plus the ledger are the entire coordination bus. Run graph-next.sh fresh before every lease.
HEARTBEAT at least every fifteen minutes of activity and after every milestone. LEASE_RELEASE if you
stop for any reason other than NODE_DONE or NODE_BLOCKED.
