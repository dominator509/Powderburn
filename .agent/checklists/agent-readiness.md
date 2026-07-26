# Checklist: agent readiness (plan self-containment audit)

Run before executing any ExecPlan. Every line names a thing to open or a command to run.

- [ ] Open the plan. Does it have the NODE-META header with ID, DEPS, MAX_ATTEMPTS, VERIFY,
      VERIFY_SENTINEL, GREEN_TAG?
- [ ] Does it have all fourteen sections in order?
- [ ] Section 3 Non-goals: is it non-empty and does it name the node that owns each excluded item?
- [ ] Section 5 Files to Read First: do all paths exist? `for f in <paths>; do [ -f "$f" ] || echo MISSING "$f"; done`
- [ ] Section 6 Expected Changed Files: is every path exact, with no globs and no directories?
- [ ] Section 7: does every name appear in `.agent/specs/SPEC-002-data-model.md` or
      `SPEC-003-api-contracts.md`? `grep -n '<name>' .agent/specs/SPEC-00{2,3}*.md`
- [ ] Every milestone: does it carry GOAL, READ, CHANGE, CONTENT, RUN, EXPECT, EVIDENCE, FALLBACK,
      COMMIT? Nine fields, no exceptions.
- [ ] Every RUN command: does it appear in COMMANDS.md? `grep -n '<command>' COMMANDS.md`
- [ ] Every EXPECT sentinel: is it an exact line, not a description?
- [ ] Section 10 Idempotence: does it name an exact reset command?
- [ ] Section 11 Progress: one checkbox per milestone, all unchecked at entry?
- [ ] `sh scripts/preflight.sh` prints `preflight: ok`
- [ ] `sh scripts/graph-next.sh` dispatches this node
- [ ] `git status --porcelain` is empty
