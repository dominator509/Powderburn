# DECISIONS

Every material assumption and every pre-decided fork is an ADR here. Add new ones with the template
in `.agent/templates/adr-template.md`. Never change an accepted ADR; supersede it.

## Decision table

| ID | Decision | Status | Date |
| --- | --- | --- | --- |
| ADR-0001 | Continuous turn based sequence clock rather than fixed initiative rounds | Accepted | 2026-07-25 |
| ADR-0002 | Fixed-point Fix32 arithmetic; no floats in the kernel | Accepted | 2026-07-25 |
| ADR-0003 | Counter-based addressed RNG streams rather than a sequential generator | Accepted | 2026-07-25 |
| ADR-0004 | RON for content, not JSON or a binary format | Accepted | 2026-07-25 |
| ADR-0005 | Historical immutability as a validated structural law | Accepted | 2026-07-25 |
| ADR-0006 | The in-game Ledger is hash-chained and is the save integrity root | Accepted | 2026-07-25 |
| ADR-0007 | ExecPlans transcribe schemas, signatures, and tests verbatim; implementation bodies are composed test-first | Accepted | 2026-07-25 |
| ADR-0008 | wgpu with a software adapter for headless proof; no display required for any gate | Accepted | 2026-07-25 |
| ADR-0009 | Data-only mods, no scripting engine | Accepted | 2026-07-25 |
| ADR-0010 | Linux x86_64 only for v1 | Accepted | 2026-07-25 |
| ADR-0011 | minisign for artifact signing | Accepted | 2026-07-25 |
| ADR-0012 | Auto-deploy authorized to the self-hosted release directory only; external publication is MANUAL | Accepted | 2026-07-25 |

## ADR-0001 Continuous turn based sequence clock

Context. The Fallout Tactics lineage used a per-round initiative order. A sequence clock, where each
actor holds a `next_act_at` tick, expresses speed as frequency rather than order.
Decision. Use a monotonic tick with `turn_length = 100 - Sequence * 4` clamped to 50..96.
Consequences. A fast actor genuinely acts more often. Ties are broken by Sequence then actor id,
which is total, so scheduling is deterministic. Smoke decay, bleeding, and fuses all share one clock.
Alternatives rejected. Fixed rounds, because they make speed a coin flip; real-time with pause,
because it destroys reproducibility.

## ADR-0002 Fixed point

Context. Determinism across builds is the product's load-bearing property. IEEE floats are
deterministic in principle and a minefield in practice once compiler flags, math libraries, and
target features vary.
Decision. `Fix32`, i32 with 10 fractional bits, in every determinism-critical crate. Floats permitted
only in pb-render and pb-audio.
Consequences. A lint gate can enforce it mechanically. Rules must be expressed in integers, which
they already are.

## ADR-0003 Addressed RNG streams

Context. A sequential generator makes save-resume fragile: drawing one extra number anywhere shifts
every later roll.
Decision. `draw(seed, scenario, tick, actor, stream, lo, hi)`, a counter-based hash. Streams are
named in SPEC-001 section 1.
Consequences. Resume is exact without serializing generator state. A change in draw count is
detectable by the `rng.draws.per_turn` metric even when the final hash survives.

## ADR-0004 RON for content

Context. The game is meant to be modded and diffed by humans.
Decision. RON, validated by `pbtool`, with a documented schema in SPEC-002.
Consequences. Slower parse than a binary format; budget is 900ms and is measured. If exceeded, the
pre-decided fallback is a build-time binary cache keyed by content hash, not a format change.

## ADR-0005 Historical immutability

Context. The campaign intersects real events involving real nations and real dead people.
Decision. Nodes tagged HISTORICAL_FIXED carry a date, an outcome, and citations, and have no outgoing
edge that alters the outcome. Enforced structurally, rejected with E-HIST-001.
Consequences. The player can be present, be hurt, and save one fictional person inside an event, and
can never win it. This is the design, not a constraint on it.

## ADR-0006 The Ledger as integrity root

Context. The game needed a save integrity mechanism and a narrative spine, and they can be the same
object.
Decision. Every named death appends an entry carrying the previous entry hash. The chain head is
stored in the save and recomputed on load. Mismatch yields E-SAVE-TAMPERED and an Unverified mode
that disables the Ledger ending.
Consequences. Tampering is detectable without punishing a player whose disk failed. The credits read
the player's own book back to them.

## ADR-0007 Transcription versus composition in the ExecPlans

Context. Directive 5 of the master prompt prefers full transcription of load-bearing files. A game of
this size cannot have every source line embedded in a plan.
Decision. ExecPlans embed verbatim: the vocabulary tables, the rule tables, every public signature
from SPEC-003, every acceptance test, and every content schema. Implementation bodies are composed by
the EXECUTOR against tests written first in the same milestone.
Consequences. The hallucination surface is confined to implementation bodies whose correctness is
decided by transcribed tests. Every composing milestone carries a verification grep.
Alternatives rejected. Embedding the whole game, which no context can hold; describing the game
loosely, which is what this pack exists to prevent.

## ADR-0008 wgpu plus software adapter

Context. Every ship criterion must be provable with no human and no display.
Decision. Render through wgpu; prove through `pbcli capture --adapter gl` on llvmpipe or lavapipe;
assert non-blank by unique color count and correctness by frame hash against a golden.
Consequences. A software adapter is slow; capture proofs use a single frame, not a benchmark. Frame
budget is measured separately on real hardware.

## ADR-0009 Data-only mods

Context. Modders want power; LBI-09 and the sandbox model forbid running foreign code.
Decision. Data only, path-confined, validated by the shipped validator, rejected on executables.
Consequences. Deep systemic modding via rule tables; no new mechanics without a fork. Acceptable, and
stated plainly in the documentation so nobody is surprised.

## ADR-0010 Linux only for v1

Context. Reaching a real ship gate matters more than platform breadth.
Decision. x86_64 Linux only. Windows cross-build deferred to v1.1.
Consequences. Recorded as a non-goal in PROJECT_BRIEF.md and in RELEASE.md.

## ADR-0011 minisign

Context. Artifacts must be signed; the toolchain must be offline and tiny.
Decision. minisign detached signatures; the public key ships in the release directory.
Alternatives rejected. gpg, for complexity; unsigned artifacts, for obvious reasons.

## ADR-0012 Auto-deploy scope

Context. Hands-off deployment is desirable; publishing to a third party is an irreversible external
side effect.
Decision. AUTO_DEPLOY is authorized for `PB_RELEASE_DIR` on the operator's own host. The butler
command for itch.io is printed as MANUAL and never executed by the run.
Consequences. The run reaches RUN_COMPLETE without a human, and no external publication happens
without one.

## ADR-0013 wgpu renderer dependency expansion

Context. EP-005 added a hardware-accelerated isometric renderer via wgpu. wgpu itself and its
transitive dependency tree (naga, raw window handles, Apple block/framework crates, etc.) raised
the crate count from ~60 to ~165. Some transitive deps are Apple-only (block, cocoa, objc) and
never compiled on our Linux build target. These vendored Apple-only crates lack license files
because they come from the crates.io tarball, not a repo with a LICENSE file in their root.

Decision. Accept ~180 dependencies as the new budget, covering the graphics stack. Apple-only
crates (block, cocoa, objc, core-graphics-types, etc.) are added to .agent/dep-waivers since
they are never linked into the Linux binary and do not affect the shipped artifact.

Consequences. Larger vendor tree (~280 MB). CI wall clock for `cargo check --offline` increases
correspondingly. The dependency audit now checks for 180 total, with waivers for Apple-only deps.
