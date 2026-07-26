# continue-execplan.md - resume a node that is already in progress

1. Read AGENTS.md, COMMANDS.md, .agent/LOOPS.md.
2. Export the non-interactive environment block.
3. `sh scripts/preflight.sh` must print `preflight: ok`.
4. `sh scripts/graph-next.sh` must print `RESUME <id>`. If it prints anything else, follow the
   dispatch table instead; do not force a resume.
5. Determine lease ownership from `sh scripts/ledger.sh tail 40`. If the lease is another agent's and
   its newest event is under 90 minutes old, stop; another agent is live. If it is older, append
   LEASE_TAKEOVER.
6. Read, in this order: the ExecPlan Progress section, Surprises and Discoveries, Decision Log, then
   `sh scripts/ledger.sh tail 30`.
7. RE-VERIFY the last checked milestone: run its RUN commands again and confirm its EXPECT sentinel
   still appears. A checkbox is a claim; the sentinel is the evidence. If it does not reproduce,
   treat the milestone as unchecked and re-enter it at ladder rung 1.
8. Resume at the first unchecked milestone. Re-ground first.
9. Continue exactly as in execute-active-execplan.md from step 6.

Never restart a node from milestone one because resuming looks confusing. If the state is genuinely
ambiguous, use the node Idempotence and Recovery section, which names the exact reset command.
