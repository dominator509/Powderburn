# run-graph.md - THE hands-off prompt. Give an agent the contents of this file and nothing else.

PRIME-BLOCK-BEGIN
This repository is governed by a 6LAYER blueprint pack. AGENTS.md is the authoritative control plane; if anything here conflicts with AGENTS.md, AGENTS.md wins.
On every session start, execute THE BOOT SEQUENCE:
1. Read AGENTS.md fully. 2. Read COMMANDS.md. 3. Read .agent/GRAPH.md and .agent/LOOPS.md. 4. Run: sh scripts/ledger.sh tail 30. 5. Run: sh scripts/preflight.sh -- it MUST print "preflight: ok"; if it fails, report the exact missing items from PREFLIGHT.md and stop (this is the only legitimate pre-run stop). 6. Run: sh scripts/graph-next.sh and dispatch on its one-line output exactly as .agent/GRAPH.md specifies. 7. Repeat step 6 after every completed node until ALL_DONE, then run the ship gate in AGENTS.md.
Hard rules: do not ask the user questions; choose the smallest reversible option, record it, continue. Use only commands from COMMANDS.md. Never invent an API, route, table, flag, or env var -- verify in-repo or transcribe from the pack. One node at a time; milestones in order; commit after every milestone; append ledger events as .agent/LOOPS.md requires. Bounded retries per .agent/LOOPS.md -- never repeat a failed fix. No mocks, stubs, demo modes, or placeholder code in production paths; scripts/reality-gate.sh and scripts/live-fire.sh must genuinely pass. Never weaken a gate, skip a test, or claim an unrun result. Stop only at NODE_BLOCKED (with the full evidence report) or ALL_DONE.
PRIME-BLOCK-END

Run the boot sequence now and continue dispatching until ALL_DONE or NODE_BLOCKED. Your session ends
only at RUN_COMPLETE or a blocked report.

Concretely:
1. Export the non-interactive environment block from COMMANDS.md.
2. Run the boot sequence steps 1 through 7 above, in order, without skipping.
3. On NEXT, append LEASE, open the named ExecPlan, and execute its milestones in order under the
   grammar in .agent/PLANS.md and the ladder in .agent/LOOPS.md.
4. Commit after every milestone. Append MILESTONE_PASS with the sentinel you actually observed.
5. At the end of a node: run its VERIFY command, run the Expected Changed Files audit, append
   NODE_DONE, tag green/EP-XXX, append LEASE_RELEASE, then run scripts/graph-next.sh again.
6. On ALL_DONE, run the ship gate in AGENTS.md section 15, then append RUN_COMPLETE with the tag.
7. On failure, climb the ladder. On a terminal failure, write the blocked report from
   .agent/LOOPS.md 5.7 into the ExecPlan Progress section and append NODE_BLOCKED.

Do not ask any question. Do not summarize and wait. Do not stop to confirm. The repository is the
only channel and the ledger is the only status.
