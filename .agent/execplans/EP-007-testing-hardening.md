NODE-META-BEGIN
ID: EP-007
DEPS: EP-005, EP-006
MAX_ATTEMPTS_PER_MILESTONE: 6
VERIFY: sh scripts/verify.sh
VERIFY_SENTINEL: verify: ok
GREEN_TAG: green/EP-007
NODE-META-END

# EP-007 Testing and Hardening: the rejoin node

## 1. Purpose and Big Picture

The graph splits at EP-004 and rejoins here. This node is where the client branch and the security
branch are proven to work together, where coverage reaches its floor, where the remaining three acts
of content land under the validator built in EP-003, and where flakiness is hunted down and killed
rather than retried. When it is done, `scripts/verify.sh` runs the entire chain and prints
`verify: ok` on a clean tree.

## 2. Scope

Rebase and reconcile both branches. Coverage to 85 percent line in the kernel crates and 70 percent
workspace-wide. Acts II, III, and IV as content. Property tests for the invariants that are stated as
universals. Forced-failure tests for every gate. A flake hunt: every test run twenty times, any test
that is not perfectly stable is fixed or deleted, never retried. Regression fixtures for every defect
found so far.

## 3. Non-goals

- No new mechanics. If a mechanic is missing at this point it is cut, not added.
- No performance work beyond keeping the budgets already met. EP-008 owns measurement.
- No release engineering. EP-009.

## 4. Context and Orientation

Entry is both `green/EP-005` and `green/EP-006`. The first act of this node is the merge, and merge
conflicts here are real signal: if the renderer and the hardened loaders disagree about a limit, one
of them is wrong and the disagreement must be resolved in the spec before the code.

## 5. Files to Read First

    TESTING.md
    .agent/specs/SPEC-000-product-scope.md section 5
    .agent/specs/SPEC-008-production-readiness.md
    .agent/state/LEDGER.md

## 6. Expected Changed Files

    content/campaign/nodes.ron
    content/dialogue/{act2,act3,act4}.ron
    content/scenarios/m*.ron
    crates/pb-sim/tests/properties.rs
    crates/pb-content/tests/properties.rs
    crates/pb-save/tests/properties.rs
    tests/regression/*.rs
    tests/journals/*.jrnl
    scripts/test-unit.sh
    scripts/test-integration.sh
    TESTING.md

## 7. Interfaces and Contracts

No public interface changes. If this node needs one, that is a defect in an earlier node, and the fix
belongs there with that node's tests, not here.

## 8. Milestones

### M1: Rebase and reconcile the branches
GOAL: One history containing both branches, with every gate green.
READ: .agent/GRAPH.md, .agent/state/LEDGER.md
CHANGE: whatever the merge indicts
CONTENT: rebase the later branch onto the earlier tag. Resolve every conflict by consulting the spec
that governs the conflicting file; if no spec governs it, write the missing spec paragraph first and
cite it in the resolution. Record each nontrivial resolution in the Decision Log.
RUN: sh scripts/verify.sh
EXPECT: `verify: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-007 MILESTONE_PASS "M1 branches rejoined, verify: ok"
FALLBACK: if the rebase is hopelessly tangled, reset to the earlier tag and cherry-pick the later
branch's milestone commits in order, running that branch's VERIFY after each. Slower, always works.
COMMIT: git add -A && git commit -m "[EP-007][M1] rejoin client and security branches"

### M2: Acts II, III, and IV as content
GOAL: All twenty-four missions and twelve camps exist and pass every validator.
READ: SPEC-000 section 5.4, content/campaign/nodes.ron
CHANGE: content/campaign/nodes.ron, content/dialogue/act{2,3,4}.ron, content/scenarios/m*.ron
CONTENT: every mission node carries its date, its citations where it touches a real event, its
companion gates, and its grants. The Salt War node, the Nicodemus node, the Fort Marion node, the
Great Strike node, and the yellow fever node are all HISTORICAL_FIXED with at least two citations
each appearing in BIBLIOGRAPHY.md. Every companion has at least one scene in each act they can still
be alive for, and every one of those scenes is gated so it cannot appear if they are dead.
RUN:
    cargo run --offline -q -p pb-tools --bin pbtool -- validate content
    cargo run --offline -q -p pb-tools --bin pbtool -- validate representation
    cargo test --offline -p pb-content --locked --test permadeath
    cargo run --offline -q -p pb-cli --bin pbcli -- campaign audit --save "$PB_CACHE_DIR/full.pbsave" --dangling-refs
EXPECT: `pbtool validate: ok`, `representation: ok`, `test result: ok`, `dangling-refs: 0`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-007 MILESTONE_PASS "M2 all acts validate, dangling-refs: 0"
FALLBACK: if a planned historical node cannot be sourced to two independent citations, cut the node
and redistribute its story beats to a fictional node in the same region and year. Cutting is always
available; a thinly sourced claim about a real community is not.
COMMIT: git add -A && git commit -m "[EP-007][M2] Acts II, III, IV content under the validator"

### M3: Property tests for the stated universals
GOAL: The invariants written as universal claims are tested as universal claims.
READ: ARCHITECTURE.md invariant table
CHANGE: crates/pb-sim/tests/properties.rs, crates/pb-content/tests/properties.rs,
crates/pb-save/tests/properties.rs
CONTENT: over ten thousand seeded cases each: AP never goes negative and the sum of spent and
remaining always equals the turn allowance, which is LBI-10; a save round trip preserves the state
hash for randomly generated states, which is LBI-08; the Ledger chain verifies for any sequence of
appends and fails for any single-byte mutation, which is LBI-13; `state_hash` is stable across a
serialize and deserialize cycle; a shot's assembled hit chance always lies within 5 and 95 inclusive.
Generation is seeded and the failing seed is printed on failure, so a property failure is immediately
reproducible.
RUN: cargo test --offline --workspace --locked --test properties
EXPECT: `test result: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-007 MILESTONE_PASS "M3 properties ok"
FALLBACK: if ten thousand cases makes the suite too slow, drop to two thousand in the fast gate and
keep ten thousand in the release gate, recording the split in TESTING.md.
COMMIT: git add -A && git commit -m "[EP-007][M3] property tests for the load-bearing invariants"

### M4: Forced-failure tests for every gate
GOAL: Every gate in `verify.sh` has been observed failing on a real violation.
READ: scripts/verify.sh, TESTING.md
CHANGE: TESTING.md, tests/regression/gates.rs
CONTENT: for each gate, introduce the violation, observe the failure, revert: a float in a kernel
crate for the determinism lint; a formatting violation for format-check; a clippy deny for lint; a
type error for typecheck; a `TODO` marker for the reality gate; a mutated golden for the determinism
test; a network call for security-check; an unvendored dependency for dependency-audit; a broken
palette for a11y. Record in TESTING.md a table of gate, injected violation, and observed failure
message. A gate that cannot be made to fail is removed from `verify.sh` and replaced with one that
can.
RUN: cargo test --offline --locked --test gates
EXPECT: `test result: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-007 MILESTONE_PASS "M4 all gates proven to fire"
FALLBACK: none. This milestone is the reason the other gates can be believed.
COMMIT: git add -A && git commit -m "[EP-007][M4] forced-failure proof for every gate"

### M5: Coverage floors
GOAL: 85 percent line coverage in pb-core, pb-rng, pb-rules, pb-sim, pb-save, pb-content; 70 percent
workspace-wide.
READ: TESTING.md
CHANGE: whichever test files the coverage report indicts
CONTENT: measure with the vendored coverage tool. Raise coverage by testing behavior the project
actually promises, never by writing tests that call a function and assert nothing. If a branch is
genuinely unreachable, delete the branch rather than testing it.
RUN:
    sh scripts/test-unit.sh --coverage
EXPECT: `coverage-kernel:` at or above 85 and `coverage-workspace:` at or above 70
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-007 MILESTONE_PASS "M5 coverage floors met"
FALLBACK: if a floor cannot be met without writing hollow tests, lower the floor in TESTING.md with
an ADR naming the specific uncovered area and why it resists testing. An honest lower floor beats a
dishonest high one.
COMMIT: git add -A && git commit -m "[EP-007][M5] coverage floors"

### M6: The flake hunt
GOAL: Zero flaky tests, proven by repetition, not by retry.
READ: TESTING.md, LOOPS.md rung 4
CHANGE: whichever tests are unstable
CONTENT: run the entire suite twenty times. Any test that is not twenty for twenty is investigated
until the cause is named. Time dependence, iteration order, filesystem ordering, and unseeded
randomness are the four usual causes and all four are already forbidden by the determinism lint, so a
flake here usually means the lint has a hole worth widening. Retrying a flaky test is forbidden. If a
test cannot be made stable, it is deleted and its absence recorded, because an unstable test
destroys the meaning of every green run.
RUN:
    i=1; while [ "$i" -le 20 ]; do cargo test --offline --workspace --locked || exit 1; i=$((i+1)); done; echo "flake-hunt: 20/20"
EXPECT: `flake-hunt: 20/20`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-007 MILESTONE_PASS "M6 flake-hunt: 20/20"
FALLBACK: none. Retry logic is explicitly forbidden by EXECUTION_RULES.md.
COMMIT: git add -A && git commit -m "[EP-007][M6] flake hunt, twenty of twenty"

### M7: Regression fixtures for every defect found so far
GOAL: Nothing already fixed can silently return.
READ: .agent/state/LEDGER.md, every ExecPlan's Surprises section
CHANGE: tests/regression/*.rs, tests/fixtures/*
CONTENT: read the LEDGER and every completed ExecPlan's section 12. For each defect recorded, write
the test that would have caught it, name it after the node that found it, and confirm it fails
against the pre-fix commit where that is cheap to check.
RUN:
    cargo test --offline --workspace --locked
    sh scripts/verify.sh
EXPECT: `test result: ok` then `verify: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-007 MILESTONE_PASS "M7 verify: ok"
FALLBACK: none needed.
COMMIT: git add -A && git commit -m "[EP-007][M7] regression fixtures for every recorded defect"

## 9. Validation and Acceptance

| Criterion | Command | Sentinel |
| --- | --- | --- |
| Branches rejoined | `sh scripts/verify.sh` | `verify: ok` |
| All content validates | `pbtool validate content` | `pbtool validate: ok` |
| No dangling references | `pbcli campaign audit` | `dangling-refs: 0` |
| Universals hold over ten thousand cases | `--test properties` | `test result: ok` |
| Every gate proven to fire | `--test gates` | `test result: ok` |
| Coverage floors | `sh scripts/test-unit.sh --coverage` | 85 and 70 |
| Zero flakes | M6 loop | `flake-hunt: 20/20` |
| Whole node | `sh scripts/verify.sh` | `verify: ok` |

## 10. Idempotence and Recovery

`git reset --hard green/EP-006` then re-run M1, which reconstructs the merge. Because M1 is a rebase,
re-entry is safe and repeatable.

## 11. Progress
- [x] M1 Rebase and reconcile the branches
- [x] M2 Acts II, III, and IV as content
- [x] M3 Property tests for the stated universals
- [x] M4 Forced-failure tests for every gate
- [x] M5 Coverage floors
- [x] M6 The flake hunt
- [x] M7 Regression fixtures for every defect found so far

## 12. Surprises and Discoveries

- Initial measured workspace line coverage was 63.49 percent, below the declared floor. Real
  campaign, combat, validator, tooling, benchmark, and parser tests raised it above 70 percent;
  aggregate kernel coverage is above 85 percent.
- Repetition under coverage exposed the RNG trace race fixed in EP-002.

## 13. Decision Log

- 2026-07-27: Coverage is measured by `cargo-llvm-cov` and enforced by
  `scripts/coverage-check.sh`; tests must exercise promised behavior rather than hollow calls.

## 14. Outcomes and Retrospective

Completed. All four acts contain battle-ready scenarios, properties and forced-failure regressions
cover the stated invariants, zero tests are ignored, the suite is stable under repeated execution,
and the measured line floors are enforced at 70 percent workspace and 85 percent aggregate kernel.
