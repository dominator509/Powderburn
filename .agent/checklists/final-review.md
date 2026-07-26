# Checklist: final review

- [ ] `git status --porcelain` empty
- [ ] `sh scripts/verify.sh` run from scratch, `verify: ok` observed this session
- [ ] `sh scripts/live-fire.sh` all ten proofs observed
- [ ] Expected Changed Files audit per node: `git diff --name-only green/EP-<prev>..green/EP-<id>`
- [ ] Adapter parity: `for f in AGENTS.md CLAUDE.md GEMINI.md .github/copilot-instructions.md .cursor/rules/6layer.mdc .clinerules/6layer.md .hermes/instructions.md .openclaw/instructions.md; do awk '/PRIME-BLOCK-BEGIN/,/PRIME-BLOCK-END/' "$f" | cksum; done` all identical
- [ ] Every ExecPlan acceptance criterion walked and marked with its proving command
- [ ] PRODUCTION_READINESS.md walked line by line
- [ ] `sh scripts/production-readiness-check.sh` prints `production-readiness: ok`
- [ ] Every ExecPlan has Outcomes and Retrospective written
- [ ] DECISIONS.md contains an ADR for every decision made during the run
- [ ] ASSUMPTIONS.md rows all marked confirmed or changed
- [ ] Final report written per AGENTS.md section 16
