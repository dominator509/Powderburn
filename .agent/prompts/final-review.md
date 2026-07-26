# final-review.md - the full review, from scratch

1. Confirm a clean tree: `git status --porcelain` prints nothing. If it does not, stop and report.
2. Export the non-interactive environment block from COMMANDS.md.
3. Run `sh scripts/verify.sh` from scratch. Cached green is not green. Record every sentinel you
   actually observe, in order.
4. Run `sh scripts/reality-gate.sh` and record `reality gate: ok`.
5. Run `sh scripts/live-fire.sh` and record every `live-fire: LF-xx` line plus `live-fire: ok`.
6. Expected Changed Files audit: for every completed node, compare `git diff --name-only
   green/EP-<prev>..green/EP-<id>` against that node's section 6 list. Report any difference.
7. Adapter parity: run the check from COMMANDS.md. All cksum lines must match.
8. Walk the acceptance criteria of every ExecPlan and mark each met or not met with the command that
   proved it.
9. Walk PRODUCTION_READINESS.md line by line and run every command it names.
10. Run `sh scripts/production-readiness-check.sh` and record `production-readiness: ok`.
11. Write Outcomes and Retrospective in EP-010: what was built, what surprised you, what gate should
    have caught something earlier and now does, what risks remain.
12. Final report per AGENTS.md section 16.

Do not fix anything during a final review without leaving the review, entering the ladder in the
owning node, and coming back to step 1. A review that fixes as it goes is not a review.
