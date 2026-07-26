NODE-META-BEGIN
ID: EP-010
DEPS: EP-009
MAX_ATTEMPTS_PER_MILESTONE: 6
VERIFY: sh scripts/production-readiness-check.sh
VERIFY_SENTINEL: production-readiness: ok
GREEN_TAG: green/EP-010
NODE-META-END

# EP-010 Production Readiness and Ship

## 1. Purpose and Big Picture

The last node proves the promise. Every load-bearing invariant is checked, all ten live-fire proofs
run against the released artifact rather than the build tree, the ship gate from SPEC-008 is
evaluated honestly, and the run is closed in the ledger. Nothing is built here. If something is
missing at this node, the answer is to go back to the node that owns it, not to add it here.

## 2. Scope

Clean-tree verification. The full live-fire suite against the released binary. The thirteen invariant
checks. The ship gate. The version tag. The hands-off publication. `RUN_COMPLETE` in the ledger and
the honest limitations list.

## 3. Non-goals

- No new code, no new content, no new tests. This node only evaluates.
- No itch.io publication; that is the documented manual step from EP-009 M6.
- No lowering of a criterion to make the gate pass. A gate that will not pass is a blocked report.

## 4. Context and Orientation

Entry is `green/EP-009`, with a published, signed, smoke-tested `0.1.0-rc1` in the release directory
and a rehearsed rollback. The single most important rule for this node is in section 8 M4: the ship
gate is evaluated, not negotiated.

## 5. Files to Read First

    .agent/specs/SPEC-008-production-readiness.md
    PRODUCTION_READINESS.md
    .agent/checklists/production-readiness.md
    scripts/live-fire.sh
    ARCHITECTURE.md invariant table

## 6. Expected Changed Files

    PRODUCTION_READINESS.md
    .agent/state/LEDGER.md
    ROADMAP.md

## 7. Interfaces and Contracts

None change. If an interface changes at this node, the run has failed its own rules and the change
belongs in the node that owns that interface, with that node's tests rerun and its tag remade.

## 8. Milestones

### M1: Clean tree, full verify
GOAL: The gate chain passes on a tree with nothing uncommitted and nothing ignored that matters.
READ: scripts/verify.sh, .agent/checklists/final-review.md
CHANGE: none
CONTENT: confirm `git status --porcelain` is empty. Confirm no tracked file contains a placeholder by
running the reality gate. Then run the full chain.
RUN:
    test -z "$(git status --porcelain)" && echo "tree: clean"
    sh scripts/reality-gate.sh
    sh scripts/verify.sh
EXPECT: `tree: clean`, `reality gate: ok`, `verify: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-010 MILESTONE_PASS "M1 tree: clean, verify: ok"
FALLBACK: if the tree is dirty, commit the changes to the node that owns them and rerun that node's
VERIFY before returning here. Never commit stray work under EP-010.
COMMIT: none

### M2: All ten live-fire proofs against the released artifact
GOAL: Every core user outcome is demonstrated on the thing a player would download.
READ: scripts/live-fire.sh, SPEC-000 section 4
CHANGE: none
CONTENT: unpack `0.1.0-rc1` into a scratch directory with a fresh `PB_HOME`, point `live-fire.sh` at
the unpacked binary, and run all ten proofs: LF-01 the opening mission is playable to victory; LF-02 a
called shot produces its mechanical consequence; LF-03 the same seed and journal reproduce the same
hash three times; LF-04 a save round trips and resumes to the same hash with an intact chain; LF-05 a
companion death propagates and leaves no dangling reference; LF-06 two branch scripts produce
different mission manifests; LF-07 content that alters a fixed historical outcome is rejected with
E-HIST-001; LF-08 a headless capture produces a real frame; LF-09 representation and provenance
validate; LF-10 the AI turn and sim step budgets are met.
RUN:
    sh scripts/live-fire.sh --released "$PB_RELEASE_DIR/current"
EXPECT: ten `LF-0n: pass` lines then `live-fire: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-010 MILESTONE_PASS "M2 live-fire: ok, 10/10"
FALLBACK: none. A failing proof sends the run back to the node that owns the failing behavior. Record
a blocked report naming the proof, the node, and the observed output.
COMMIT: none

### M3: The thirteen invariants, checked one at a time
GOAL: Every load-bearing invariant has a named, passing check, and no invariant is merely asserted.
READ: ARCHITECTURE.md invariant table
CHANGE: PRODUCTION_READINESS.md
CONTENT: fill the invariant table in PRODUCTION_READINESS.md with, for each of LBI-01 through LBI-13,
the exact command that checks it and the exact observed sentinel from this run. LBI-01 determinism by
LF-03; LBI-02 fixed point by the determinism lint; LBI-03 import law by the same lint; LBI-04 journal
completeness by replay match; LBI-05 historical immutability by LF-07; LBI-06 representation by
LF-09; LBI-07 permadeath by LF-05; LBI-08 save compatibility by LF-04; LBI-09 no network by the
symbol check; LBI-10 AP conservation by the property test; LBI-11 asset provenance by LF-09; LBI-12
accessibility by `a11y-report`; LBI-13 ledger chain by LF-04. Any invariant without an observed
sentinel is a blocked report, not a checkbox.
RUN:
    sh scripts/production-readiness-check.sh
EXPECT: thirteen `LBI-nn: ok` lines then `production-readiness: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-010 MILESTONE_PASS "M3 production-readiness: ok"
FALLBACK: none.
COMMIT: git add -A && git commit -m "[EP-010][M3] invariant evidence table"

### M4: The ship gate, evaluated not negotiated
GOAL: An honest yes or an honest blocked report.
READ: SPEC-008 section 6, .agent/checklists/production-readiness.md
CHANGE: PRODUCTION_READINESS.md
CONTENT: walk the ship gate criteria in order and record for each the command run and the output
observed. Where a criterion is not met, the run stops and produces a blocked report in the
EXECUTION_RULES.md format. Lowering a criterion to pass the gate is forbidden; changing a criterion
requires an ADR, a spec edit, and a rerun of the affected node, in that order.
Write the honest limitations section: keyboard and mouse only with no controller support; three
desktop targets with no console or handheld; English only with the localization hooks present but
unused; a single save slot per company with no cloud sync; software-rendered captures in CI which are
slower than a real adapter and therefore not a performance measurement.
RUN:
    sh scripts/production-readiness-check.sh --ship-gate
EXPECT: `ship-gate: pass` with each criterion listed
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-010 MILESTONE_PASS "M4 ship-gate: pass"
FALLBACK: none. A blocked report is a successful outcome of this milestone when the gate genuinely
does not pass.
COMMIT: git add -A && git commit -m "[EP-010][M4] ship gate evaluation and honest limitations"

### M5: Tag and publish 1.0.0 hands off
GOAL: The release exists, is signed, is indexed, and was produced without a human touching a keyboard
mid-run.
READ: RELEASE.md, scripts/release.sh
CHANGE: none
CONTENT: tag `v1.0.0` on the commit that passed M4. Run the release for all three targets and publish
to `PB_RELEASE_DIR`. Then run the released smoke test and the released live-fire suite again against
`1.0.0`, because the artifact just built is not the artifact previously proven. Print, without
executing, the manual butler command from EP-009 M6.
RUN:
    git tag -a v1.0.0 -m "POWDERBURN 1.0.0"
    sh scripts/release.sh --version 1.0.0 --publish
    sh scripts/smoke-test.sh --released "$PB_RELEASE_DIR/1.0.0/powderburn-1.0.0-x86_64-unknown-linux-gnu.tar.zst"
    sh scripts/live-fire.sh --released "$PB_RELEASE_DIR/1.0.0"
EXPECT: `publish: ok`, `smoke-test: ok`, `live-fire: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-010 MILESTONE_PASS "M5 1.0.0 published"
FALLBACK: if publication fails partway, run `rollback.sh --to 0.1.0-rc1`, which was rehearsed in
EP-009 M5, then diagnose. Never leave a half-published version as `current`.
COMMIT: none

### M6: Close the run
GOAL: The ledger tells the whole story and the roadmap tells the next one.
READ: .agent/state/LEDGER.md, ROADMAP.md
CHANGE: .agent/state/LEDGER.md, ROADMAP.md
CONTENT: append `RUN_COMPLETE` with the version, the commit, and the ten live-fire results. Update
ROADMAP.md with what shipped and what was deliberately deferred, each deferral naming the reason.
Confirm every node has a `NODE_DONE` entry and every ExecPlan has its sections 12, 13, and 14 filled,
because an empty retrospective means the run learned nothing.
RUN:
    grep -c "NODE_DONE" .agent/state/LEDGER.md
    sh scripts/ledger.sh append <AGENT_ID> EP-010 RUN_COMPLETE "v1.0.0 published, live-fire 10/10"
EXPECT: a count of 11 then the ledger append succeeding
EVIDENCE: the `RUN_COMPLETE` line itself
FALLBACK: if a node is missing its `NODE_DONE`, that node did not finish. Return to it.
COMMIT: git add -A && git commit -m "[EP-010][M6] close the run"

## 9. Validation and Acceptance

| Criterion | Command | Sentinel |
| --- | --- | --- |
| Clean tree and full verify | `sh scripts/verify.sh` | `verify: ok` |
| Ten live-fire proofs on the artifact | `sh scripts/live-fire.sh --released` | `live-fire: ok` |
| Thirteen invariants with evidence | `sh scripts/production-readiness-check.sh` | thirteen `LBI-nn: ok` |
| Ship gate | `--ship-gate` | `ship-gate: pass` |
| 1.0.0 published and re-proven | M5 commands | `publish: ok`, `live-fire: ok` |
| Run closed | M6 | `RUN_COMPLETE` in the ledger |

## 10. Idempotence and Recovery

This node creates one irreversible artifact, the `v1.0.0` tag. Re-entry after a failure at M5 or M6
requires deleting the tag and the published version directory first, and recording both deletions in
the ledger, because the release index is append-only and its history must stay truthful.

## 11. Progress
- [ ] M1 Clean tree, full verify
- [ ] M2 All ten live-fire proofs against the released artifact
- [ ] M3 The thirteen invariants, checked one at a time
- [ ] M4 The ship gate, evaluated not negotiated
- [ ] M5 Tag and publish 1.0.0 hands off
- [ ] M6 Close the run

## 12. Surprises and Discoveries

## 13. Decision Log

## 14. Outcomes and Retrospective
