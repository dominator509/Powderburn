# EXECUTION_RULES - the condensed law, one page

1. ONE ACTIVE NODE. At most one node is IN_PROGRESS repository-wide. Lease it in the ledger before
   you touch a file. Release it when you stop.
2. NO HIDDEN CONTEXT. Everything you need is in the repository. Chat scrollback, platform memory,
   and your own recollection are not authoritative and are not evidence.
3. NO ROADMAP IMPLEMENTATION. ROADMAP.md is narrative. Implementation happens only through the graph.
   Run `sh scripts/graph-next.sh`.
4. CONTINUE BY DEFAULT. Finish the node. Never ask for next steps, preferences, or confirmation.
   At a fork the spec does not settle, choose the smallest reversible option, write it in the
   Decision Log, continue.
5. STOP LIST ONLY. Stop only for the five conditions in AGENTS.md section 5.
6. ANTI-DRIFT. Only the paths in the milestone CHANGE list may change. Audit after every milestone.
   Revert anything else.
7. ANTI-HALLUCINATION. Names come from a file you just read, a vocabulary table in SPEC-002 or
   SPEC-003, or verbatim ExecPlan content. Commands come only from COMMANDS.md. Third-party APIs are
   confirmed by reading the vendored source before use.
8. ANTI-FIXATION. Climb the ladder in .agent/LOOPS.md by error signature. Never the same fix twice.
9. EVIDENCE BEFORE EDITS. Read the file before you change it. Confirm the symbol exists before you
   call it.
10. EVIDENCE BEFORE DONE. A gate passes only if you ran it in this session and saw the sentinel. Put
    the sentinel in the MILESTONE_PASS ledger detail.
11. DIFF REVIEW. Read your own diff before every commit. If it contains a change you cannot explain
    in one sentence tied to the milestone GOAL, revert that hunk.
12. BOOT SEQUENCE. Every session starts with it. No exceptions, including resumes.
13. LEDGER DUTIES. LEASE, HEARTBEAT, MILESTONE_PASS, ATTEMPT_FAIL, SIG, FALLBACK_TAKEN, ROLLBACK,
    NODE_DONE, NODE_BLOCKED, LEASE_RELEASE, LEASE_TAKEOVER, RUN_COMPLETE. Append-only. Details never
    contain the sequence space-pipe-space.
14. DETERMINISM DUTY, project specific. Any change under crates/pb-core, pb-rng, pb-rules, pb-sim,
    pb-ai, or pb-content ends with the triple-run determinism proof LF-03 before the commit.
15. FINAL RESPONSE. Nodes completed; changed files versus expected; commands run with observed
    sentinels; acceptance status per criterion; decisions; assumptions; risks; ship-gate status.
