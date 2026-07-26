# PLANS - the ExecPlan standard

An ExecPlan is a self-contained implementation document for one node. A new agent with no prior
conversation must be able to complete it from the plan, the laws in .agent/, and the ledger alone.
If a plan requires knowledge that is not in those three places, the plan is defective.

## Machine header

Every ExecPlan opens with:

    NODE-META-BEGIN
    ID: EP-XXX
    DEPS: <csv or ->
    MAX_ATTEMPTS_PER_MILESTONE: <n>
    VERIFY: <exact node-level verify command>
    VERIFY_SENTINEL: <exact expected line>
    GREEN_TAG: green/EP-XXX
    NODE-META-END

## Required sections, in order

1. Purpose and Big Picture. Why this node exists and what the repository looks like after it.
2. Scope. What is in this node.
3. Non-goals. What is explicitly not in this node, including work that belongs to a later node.
4. Context and Orientation. The state of the repository at entry and the invariants in play.
5. Files to Read First. Exact paths, in reading order.
6. Expected Changed Files. Exact paths. This is the audit list checked at node end.
7. Interfaces and Contracts. Types, signatures, content keys, and command surfaces taken from the
   vocabulary-locked specs. No new names may be introduced here.
8. Milestones. Each obeying the grammar below.
9. Validation and Acceptance. Node-level criteria, each with a command and a sentinel.
10. Idempotence and Recovery. Exactly how to re-enter this node cold, including what to reset.
11. Progress. One checkbox per milestone. The only mutable region besides 12, 13, 14.
12. Surprises and Discoveries. Empty scaffold at generation.
13. Decision Log. Empty scaffold at generation.
14. Outcomes and Retrospective. Empty scaffold at generation.

## Milestone grammar (no exceptions)

    ### M<k>: <name>
    GOAL: one sentence, observable.
    READ: exact paths to re-read before acting.
    CHANGE: exact paths created or modified. Nothing else may change.
    CONTENT: complete file bodies to transcribe, or anchored edits given as exact old text and exact
             new text with a verification grep, or the exact discovery commands whose output fills a
             named template blank.
    RUN: exact commands, in order.
    EXPECT: the exact sentinel line or lines RUN must produce.
    EVIDENCE: the exact ledger append.
    FALLBACK: the pre-decided alternative path for ladder rung 3. Never a mock. "none needed" is
              legal only for trivially safe milestones and must carry a one clause justification.
    COMMIT: git add -A && git commit -m "[EP-XXX][M<k>] <summary>"

## Execution rules

Milestones run strictly in order. Re-ground before each one per .agent/LOOPS.md 5.6. Commit after
each one. Never batch two milestones into one commit. Never start milestone k+1 while k is unchecked.

## Validation rules

A milestone passes when its EXPECT sentinel appears in real output from its RUN commands in this
session. A node passes when its VERIFY_SENTINEL appears and the Expected Changed Files audit is
clean.

## Idempotence rule

Every node states how to re-enter cold. The general form in this repository: `git reset --hard
green/EP-<prev>` restores the entry state exactly, because every node begins from a green tag and
commits only inside its own milestones.

## Completion rule

Append NODE_DONE, tag green, append LEASE_RELEASE, then run `sh scripts/graph-next.sh` again. Do not
announce completion without the tag.
