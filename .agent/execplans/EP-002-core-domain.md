NODE-META-BEGIN
ID: EP-002
DEPS: EP-001
MAX_ATTEMPTS_PER_MILESTONE: 6
VERIFY: sh scripts/test-unit.sh && sh scripts/test-integration.sh
VERIFY_SENTINEL: test-integration: ok
GREEN_TAG: green/EP-002
NODE-META-END

# EP-002 Core Domain: the simulation kernel

## 1. Purpose and Big Picture

This is the node the whole product rests on. It builds the deterministic combat kernel described by
SPEC-001: the addressed random generator, the sequence clock, the action point economy, the ten-stage
shot pipeline, hit locations and criticals, wounds, explosives, the environment systems including
black powder smoke, morale, and the state hash. When it is done, a scenario plus a seed plus a
journal produces a reproducible terminal state hash, and that hash is what every later proof compares
against.

## 2. Scope

pb-core geometry, ids, events, and hashing. pb-rng addressed streams. pb-rules table types and
lookup. pb-sim: `SimState`, `step`, `advance_to_next_actor`, `state_hash`, the clock, the AP economy,
the shot pipeline, hit locations, wounds, explosives, smoke, light, weather, cover, morale. pb-ai
utility scoring. The proving scenarios `prov_called_shot`, `prov_full_battle`, `prov_sixty_actors`
and their journals and golden hashes.

## 3. Non-goals

- No content file format. EP-003 owns RON loading; this node consumes rule tables constructed in
  test code and in the proving scenarios only.
- No save format. EP-003.
- No CLI. EP-004.
- No rendering, no audio. EP-005.
- No balance tuning. Numbers come from SPEC-001 verbatim; changing one requires a spec update.

## 4. Context and Orientation

Entry state is `green/EP-001`. Binding invariants: LBI-01 determinism, LBI-02 fixed point, LBI-03
import law, LBI-04 journal completeness, LBI-10 AP conservation. Every one of them is enforced by
`scripts/lint-determinism.sh` or by a test written in this node.

## 5. Files to Read First

    .agent/specs/SPEC-001-core-domain.md
    .agent/specs/SPEC-002-data-model.md
    .agent/specs/SPEC-003-api-contracts.md
    ARCHITECTURE.md
    TESTING.md

## 6. Expected Changed Files

    crates/pb-core/src/lib.rs
    crates/pb-core/src/fix32.rs
    crates/pb-core/src/ids.rs
    crates/pb-core/src/geom.rs
    crates/pb-core/src/event.rs
    crates/pb-core/src/hash.rs
    crates/pb-rng/src/lib.rs
    crates/pb-rules/src/lib.rs
    crates/pb-rules/src/tables.rs
    crates/pb-sim/src/lib.rs
    crates/pb-sim/src/state.rs
    crates/pb-sim/src/clock.rs
    crates/pb-sim/src/action.rs
    crates/pb-sim/src/shot.rs
    crates/pb-sim/src/wounds.rs
    crates/pb-sim/src/environment.rs
    crates/pb-sim/src/morale.rs
    crates/pb-sim/src/explosive.rs
    crates/pb-sim/src/hash.rs
    crates/pb-sim/tests/ap_economy.rs
    crates/pb-sim/tests/shot_pipeline.rs
    crates/pb-sim/tests/hit_locations.rs
    crates/pb-sim/tests/explosives.rs
    crates/pb-sim/tests/environment.rs
    crates/pb-sim/tests/morale.rs
    crates/pb-sim/tests/determinism.rs
    crates/pb-ai/src/lib.rs
    crates/pb-ai/src/utility.rs
    tests/journals/prov_called_shot.jrnl
    tests/journals/prov_full_battle.jrnl
    tests/golden/prov_full_battle.hash

## 7. Interfaces and Contracts

Transcribe these signatures exactly; they are frozen by SPEC-003 section 4.

    pub fn step(state: &mut SimState, cmd: Command) -> Result<Vec<Event>, SimError>;
    pub fn advance_to_next_actor(state: &mut SimState) -> Option<ActorId>;
    pub fn state_hash(state: &SimState) -> [u8; 32];
    pub fn draw(seed: u64, scenario: u32, tick: u64, actor: u32, stream: StreamTag, lo: i32, hi: i32) -> i32;

`step` is total: it applies the command completely and returns its events, or it applies nothing and
returns an error. There is no partial application. Event names and their key order come from SPEC-002
section 4 and are a byte-level contract because live-fire greps them.

## 8. Milestones

### M1: pb-core and pb-rng
GOAL: Fixed point, ids, geometry, events, hashing, and the addressed generator exist and are tested.
READ: SPEC-001 section 1, SPEC-002 section 2
CHANGE: crates/pb-core/src/{lib,fix32,ids,geom,event,hash}.rs, crates/pb-rng/src/lib.rs
CONTENT: `Fix32` moves from lib.rs into fix32.rs unchanged from EP-001. Ids are newtypes:
`ActorId(u32)`, `Tick(u64)`, `Ap(i16)`, `ScenarioId(u32)`. Geometry is `TileXY { x: i16, y: i16 }`
with eight facings numbered 0 north through 7 northwest, Chebyshev distance, and a Bresenham line
walker. Events are the enum named in SPEC-002 section 4, deriving a stable `Display` that emits
exactly `event: <Name> <k>=<v>` with keys in declaration order. Hashing is blake3 over a canonical
byte encoding with fields in declaration order. `draw` is a counter-based hash of the five inputs
reduced to `lo..=hi` by rejection, never by modulo, so the distribution is exact.
RUN:
    cargo test --offline -p pb-core -p pb-rng --locked
    sh scripts/lint-determinism.sh
EXPECT: test output ends `test result: ok`, then `lint-determinism: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-002 MILESTONE_PASS "M1 core and rng ok"
FALLBACK: if blake3 cannot be vendored, use SHA-256 from a vendored pure-Rust implementation and
record the swap in the Decision Log; the hash function is an implementation detail behind
`pb_core::hash`, which is why it is behind a function.
COMMIT: git add -A && git commit -m "[EP-002][M1] core types and addressed rng"

### M2: The sequence clock and the AP economy
GOAL: Actors act in a deterministic order at frequencies determined by Sequence, and AP can never go
negative.
READ: SPEC-001 sections 3, 4, 5
CHANGE: crates/pb-sim/src/{lib,state,clock,action}.rs, crates/pb-sim/tests/ap_economy.rs
CONTENT: `turn_length = clamp(100 - Sequence * 4, 50, 96)`. `advance_to_next_actor` selects the
smallest `next_act_at`, breaking ties by Sequence descending then ActorId ascending. Every action
cost comes from the SPEC-001 section 5 table, transcribed as a `const` table in `action.rs` with the
spec section cited in a comment. `step` checks cost against remaining AP before doing anything and
returns `SimError::InsufficientAp` otherwise. After every applied action, `debug_assert!(ap >= 0)`
and a release-mode check that panics with `E-SIM-001`, because LBI-10 says a negative AP means the
state is already wrong.
Tests to transcribe: three actors with Sequence 2, 5, and 9 over 600 ticks act in the exact counts
the formula predicts; a tie between equal Sequence resolves by ActorId; an action costing 5 with 4 AP
remaining returns `InsufficientAp` and changes nothing, proven by comparing `state_hash` before and
after.
RUN: cargo test --offline -p pb-sim --locked --test ap_economy
EXPECT: `test result: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-002 MILESTONE_PASS "M2 clock and ap economy ok"
FALLBACK: none needed; the formulas are given.
COMMIT: git add -A && git commit -m "[EP-002][M2] sequence clock and AP economy"

### M3: The shot pipeline, hit locations, wounds
GOAL: A shot resolves through exactly the ten stages of SPEC-001 section 6, emitting the specified
events in order.
READ: SPEC-001 sections 6, 7
CHANGE: crates/pb-sim/src/{shot,wounds}.rs, crates/pb-rules/src/tables.rs,
crates/pb-sim/tests/{shot_pipeline,hit_locations}.rs
CONTENT: transcribe the hit chance assembly exactly as SPEC-001 section 6 stage 3 lists it, each term
as a named local so a trace can print it. Transcribe the hit location table, the damage multipliers,
the critical effects, and the called shot modifiers from section 7 as const tables. Clamp the
assembled chance to 5..=95 after summation, never before. A called shot that misses by 10 or less
strikes an adjacent location, where adjacency is: Head to Eyes or Torso; Eyes to Head; Torso to
Vitals or either arm; Vitals to Torso; GunArm to Torso; OffArm to Torso; Legs to Torso.
Tests: a called shot to GunArm that beats its chance emits `HitLocation`, `DamageApplied`,
`WoundApplied wound=Broken`, and `WeaponDropped`, in that order; the same shot at a range band shift
produces the modifier delta the table predicts; a Full cover target makes the shot illegal rather
than impossible-to-hit.
RUN: cargo test --offline -p pb-sim --locked --test shot_pipeline --test hit_locations
EXPECT: `test result: ok` for both
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-002 MILESTONE_PASS "M3 shot pipeline ok"
FALLBACK: if the ten-stage pipeline proves unwieldy as one function, split it into ten named
functions called in order by a driver that cannot reorder them, which is a structural improvement,
not a simplification. Never collapse stages.
COMMIT: git add -A && git commit -m "[EP-002][M3] shot pipeline, hit locations, wounds"

### M4: Environment, explosives, morale
GOAL: Smoke, light, weather, cover, dynamite, and Sand behave exactly as SPEC-001 sections 8, 9, 10.
READ: SPEC-001 sections 8, 9, 10
CHANGE: crates/pb-sim/src/{environment,explosive,morale}.rs,
crates/pb-sim/tests/{environment,explosives,morale}.rs
CONTENT: smoke deposits density 3 at the muzzle tile and 1 at the two tiles ahead; decays 1 per 40
ticks; drifts one tile per 120 ticks along the scenario wind; line of sight accumulates density with
20 accuracy penalty at 3 and illegality at 6. Cover lives on tile edges, so it is directional, and
degrades after three Soft hits. Dynamite: fuse in ticks, scatter up to 3 tiles from the `Scatter`
stream, radius 3 with damage falling by a third per tile, converting Hard cover to Soft to None.
Sand costs and morale states exactly as section 10, with per-faction Sand multipliers read from the
rules table.
Tests: two actors exchanging fire for 400 ticks in a corridor accumulate smoke to the point where a
shot becomes illegal, proving the signature mechanic works; a stick of dynamite scatters within its
bound over 1000 seeded throws and never outside it; an actor at 20 percent Sand spends its first 2 AP
moving away from the nearest visible enemy.
RUN: cargo test --offline -p pb-sim --locked --test environment --test explosives --test morale
EXPECT: `test result: ok` for all three
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-002 MILESTONE_PASS "M4 environment ok"
FALLBACK: if smoke as a per-tile scalar field is too costly at sixty actors, switch to a sparse
volume list keyed by tile with the same semantics, which is a data structure change and not a rules
change. Record it in the Decision Log.
COMMIT: git add -A && git commit -m "[EP-002][M4] environment, explosives, morale"

### M5: The utility AI
GOAL: Enemies make deterministic, explicable decisions inside the turn budget.
READ: SPEC-001 sections 5, 10, SPEC-007 section 5
CHANGE: crates/pb-ai/src/{lib,utility}.rs
CONTENT: candidate generation is bounded: at most 24 movement candidates chosen by a fixed ring
sampling around the actor, at most 6 targets by nearest-then-lowest-HP, and the action set from
SPEC-001 section 5. Scoring is integer and is a fixed weighted sum of: expected damage, cover gained,
flanking gained, smoke exposure avoided, distance to the objective, and Sand risk. Ties break by
candidate index, never by iteration order of a map. Every candidate and score is emitted when
`--trace-actor` is on.
RUN:
    cargo test --offline -p pb-ai --locked
    sh scripts/lint-determinism.sh
EXPECT: `test result: ok` then `lint-determinism: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-002 MILESTONE_PASS "M5 ai ok"
FALLBACK: if the budget is missed, reduce the movement candidate ring from 24 to 12 before doing
anything else; never introduce nondeterministic parallelism to make a budget.
COMMIT: git add -A && git commit -m "[EP-002][M5] utility AI with bounded candidates"

### M6: Determinism proof and the first golden
GOAL: The same seed and journal produce the same terminal state hash, three times, and that hash is
recorded as the golden.
READ: SPEC-000 section 4 LF-03, TESTING.md
CHANGE: crates/pb-sim/tests/determinism.rs, tests/journals/prov_full_battle.jrnl,
tests/journals/prov_called_shot.jrnl, tests/golden/prov_full_battle.hash
CONTENT: the proving scenarios are constructed in test code at this node because content loading does
not exist until EP-003; EP-003 M6 replaces them with real RON files and asserts the hash is unchanged,
which is the proof that the loader is faithful. The determinism test runs the same journal three
times in one process and once in a fresh process, compares all four hashes, and additionally asserts
the `rng.draws.per_turn` count is stable.
RUN:
    cargo test --offline -p pb-sim --locked --test determinism
    sh scripts/test-unit.sh
    sh scripts/test-integration.sh
EXPECT: `test result: ok`, `test-unit: ok`, `test-integration: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-002 MILESTONE_PASS "M6 test-integration: ok"
FALLBACK: none. A determinism failure here is never worked around; it is diagnosed with the trace
facility at ladder rung 2 and fixed, because every later proof depends on it.
COMMIT: git add -A && git commit -m "[EP-002][M6] determinism proof and golden hash"

## 9. Validation and Acceptance

| Criterion | Command | Sentinel |
| --- | --- | --- |
| AP conservation, LBI-10 | `cargo test --offline -p pb-sim --test ap_economy` | `test result: ok` |
| Ten-stage shot pipeline and events | `--test shot_pipeline` | `test result: ok` |
| Hit locations and criticals | `--test hit_locations` | `test result: ok` |
| Smoke, light, weather, cover | `--test environment` | `test result: ok` |
| Explosives | `--test explosives` | `test result: ok` |
| Morale and rout | `--test morale` | `test result: ok` |
| Determinism, LBI-01 | `--test determinism` | `test result: ok` |
| No float, clock, rng, or map in kernel | `sh scripts/lint-determinism.sh` | `lint-determinism: ok` |
| Whole node | `sh scripts/test-integration.sh` | `test-integration: ok` |

## 10. Idempotence and Recovery

`git reset --hard green/EP-001` restores the entry state exactly. The golden hash file is created by
this node and is deleted by that reset, so no stale golden can survive a re-entry.

## 11. Progress
- [ ] M1 pb-core and pb-rng
- [ ] M2 The sequence clock and the AP economy
- [ ] M3 The shot pipeline, hit locations, wounds
- [ ] M4 Environment, explosives, morale
- [ ] M5 The utility AI
- [ ] M6 Determinism proof and the first golden

## 12. Surprises and Discoveries

## 13. Decision Log

## 14. Outcomes and Retrospective
