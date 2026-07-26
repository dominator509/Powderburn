# execute-active-execplan.md - run one named node under the same laws

Inputs the operator substitutes before use:
- EXECPLAN_PATH: the exact path, for example `.agent/execplans/EP-004-api-or-service-layer.md`
- OPTIONAL_USER_REQUEST: any additional constraint, or the word none

Procedure:
1. Read AGENTS.md, COMMANDS.md, .agent/GRAPH.md, .agent/LOOPS.md, .agent/PLANS.md.
2. Export the non-interactive environment block from COMMANDS.md.
3. Run `sh scripts/preflight.sh`. It must print `preflight: ok`. If it does not, report the exact
   missing items from PREFLIGHT.md and stop.
4. Run `sh scripts/graph-next.sh`. If it does not name the node at EXECPLAN_PATH, stop and report the
   mismatch; do not work a node the scheduler did not dispatch.
5. Append LEASE for that node.
6. Execute every milestone in order. Before each one, re-ground per .agent/LOOPS.md 5.6: the
   milestone block, the node Non-goals, then `sh scripts/ledger.sh tail 15`.
7. Commit after every milestone. Append MILESTONE_PASS with the observed sentinel.
8. At the end: node VERIFY command, Expected Changed Files audit, NODE_DONE, `git tag green/EP-XXX`,
   LEASE_RELEASE.
9. Stop. Do not continue to the next node in this mode.

If OPTIONAL_USER_REQUEST is not none, it applies only within the node's Scope. It never overrides
Non-goals, never adds files outside a CHANGE list, and never weakens a gate. If it conflicts with the
plan, record the conflict in the Decision Log and follow the plan.

Final response per AGENTS.md section 16.
