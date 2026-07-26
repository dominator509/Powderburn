# TESTING

## The pyramid

| Level | Where | What is real | Command |
| --- | --- | --- | --- |
| Unit | `#[cfg(test)]` inside each crate | The real function under test | `sh scripts/test-unit.sh` |
| Integration | `crates/*/tests/` | Real content files, real save files, the real kernel | `sh scripts/test-integration.sh` |
| Contract | `crates/pb-cli/tests/contract.rs` | The real CLI surface and its exact output lines | included in integration |
| End to end | `crates/pb-cli/tests/e2e.rs`, `crates/pb-render/tests/capture.rs` | The real binaries, the real renderer, a software adapter | `sh scripts/test-e2e.sh` |
| Live fire | `scripts/live-fire.sh` | Everything, through the shipped release binaries | `sh scripts/live-fire.sh` |
| Smoke | `scripts/smoke-test.sh` | The built or released artifact | `sh scripts/smoke-test.sh` |
| Benchmark | `crates/pb-sim/benches/` and `pbcli bench turn` | The real kernel under load | `pbcli bench turn` |

## The test double zone

Mocks, fakes, and fixtures are legal in exactly these places and nowhere else:

    crates/*/tests/
    crates/*/src/**/*.rs inside #[cfg(test)] modules
    tests/fixtures/

Even inside the zone: integration, contract, end to end, and live fire suites use the real simulation
kernel, real content parsing, and real files on disk. There is no in-memory impostor for the kernel,
because the kernel is the thing under test in nearly every meaningful assertion.

`tests/fixtures/` holds deliberately invalid content used to prove the validator rejects it, for
example `violation_alters_history.ron`. These are inputs, not doubles.

## Mocking rules

1. Never mock `pb_sim::step`, `pb_content::load_all`, `pb_save::read`, or `PbRng`. Use a real
   scenario, real content, and a fixed seed.
2. Never assert on a mock of the thing under test.
3. Forced failures use real mechanisms: a truncated save file on disk, a content file with a bad id,
   a journal line that is illegal at its tick, a read-only directory for the write path, an asset
   file with a corrupt header.
4. No production code branches on a test flag. Configuration may differ between environments;
   behavior may not.

## Test data lifecycle

Every test that writes uses a unique directory under `$PB_CACHE_DIR/test/<test_name>/` and removes it
on both success and failure. Golden files live in `$PB_GOLDEN_DIR` and are regenerated only by
`pbtool golden refresh` on a clean tree with an ADR. A test may never write into `content/`,
`assets/`, or `tests/golden/`.

## Required tests per feature

Any change to the shot pipeline, the sequence clock, the environment systems, or the rule tables
requires: a unit test of the changed function; an integration test through `pb_sim::step` asserting
the emitted events; a golden update with an ADR if the state hash moves; and a determinism triple-run
before commit.

Any change to content requires: `pbtool validate content` clean; the representation lint clean; and
`campaign audit --dangling-refs` at zero.

Any change to the CLI surface requires a contract test asserting the exact output line, because LF
proofs grep those lines byte for byte.

## Flaky test policy

A flaky test is a bug. Fix it or delete it with an ADR naming what it was supposed to protect and
what now protects that. Never retry until green. Never add a sleep. Never mark ignored. `grep -RIn
'#\[ignore\]' crates` returning anything is a production readiness failure.

In this project, flakiness in the kernel is almost always a determinism defect and is investigated
with `pbcli sim --trace-actor` before anything is touched.

## Coverage targets

pb-core, pb-rng, pb-rules, pb-sim: 85 percent lines. Workspace: 70 percent. Measured with
`cargo llvm-cov --offline --workspace --summary-only`. Coverage is a floor, not a goal; a covered
line that asserts nothing is worse than an uncovered one.

## Validation matrix

Every externally visible behavior maps to at least one test that exercises the real implementation.

| Spec behavior | Test file | Level |
| --- | --- | --- |
| SPEC-001 s4 sequence clock ordering and tie breaks | `crates/pb-sim/src/clock.rs` tests | Unit |
| SPEC-001 s5 AP costs and LBI-10 conservation | `crates/pb-sim/tests/ap_economy.rs` | Integration |
| SPEC-001 s6 shot pipeline stage order and modifiers | `crates/pb-sim/tests/shot_pipeline.rs` | Integration |
| SPEC-001 s7 hit locations, multipliers, criticals | `crates/pb-sim/tests/hit_locations.rs` | Integration |
| SPEC-001 s8 explosives scatter and cover destruction | `crates/pb-sim/tests/explosives.rs` | Integration |
| SPEC-001 s9 smoke accumulation, decay, drift, LOS blocking | `crates/pb-sim/tests/environment.rs` | Integration |
| SPEC-001 s10 Sand, morale states, rout | `crates/pb-sim/tests/morale.rs` | Integration |
| SPEC-001 s11 weapon records and first year enforcement | `crates/pb-content/tests/anachronism.rs` | Integration |
| SPEC-002 s3 entity schema round trip | `crates/pb-content/tests/schema.rs` | Integration |
| SPEC-002 s8 ruleset and content hashing stability | `crates/pb-content/tests/hashing.rs` | Integration |
| SPEC-003 s1 CLI surface and exact sentinels | `crates/pb-cli/tests/contract.rs` | Contract |
| SPEC-003 s3 journal legality and replay equality | `crates/pb-cli/tests/replay.rs` | Integration |
| SPEC-004 s4 accessibility floor, all seven checks | `crates/pb-app/tests/a11y.rs` | Integration |
| SPEC-004 s8 capture determinism | `crates/pb-render/tests/capture.rs` | End to end |
| SPEC-005 s2 save integrity and refusal codes | `crates/pb-save/tests/integrity.rs` | Integration |
| SPEC-005 s3 mod sandbox rejections | `crates/pb-content/tests/mod_sandbox.rs` | Integration |
| SPEC-006 registry codes are emitted as specified | `crates/pb-cli/tests/errors.rs` | Contract |
| SPEC-007 metrics presence and log fields | `crates/pb-cli/tests/observability.rs` | Integration |
| SPEC-000 s7 representation law | `crates/pb-content/tests/representation.rs` | Integration |
| SPEC-000 s5.5 historical immutability | `crates/pb-content/tests/history.rs` | Integration |
| LBI-07 permadeath propagation | `crates/pb-content/tests/permadeath.rs` | Integration |
| All ten core outcomes | `scripts/live-fire.sh` | Live fire |

## Definition of test done

The behavior has a test at the lowest level that can own it; the test fails if the behavior is
removed, which was verified once by removing it; the test uses the real implementation; the test
cleans up after itself; and the matrix row above exists.
