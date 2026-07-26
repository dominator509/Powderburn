# Checklist: implementation (run per milestone)

Before:
- [ ] Re-ground: read the milestone block, the node Non-goals, then `sh scripts/ledger.sh tail 15`
- [ ] Confirm the previous milestone checkbox is checked and its sentinel is in the ledger
- [ ] Confirm every path in CHANGE is one you intend to touch, and nothing else is

During:
- [ ] Transcribe CONTENT exactly where CONTENT is given; do not improve it
- [ ] Where composing, write the test first, from the transcribed acceptance criteria
- [ ] Confirm every third-party symbol by reading `vendor/<crate>/` before calling it
- [ ] Names only from SPEC-002 and SPEC-003
- [ ] Comments carry the why and cite invariant numbers

After:
- [ ] Run the RUN commands in order; observe every EXPECT sentinel in real output
- [ ] Kernel crates touched? Run the LF-03 determinism triple-run block
- [ ] `git status --porcelain` and `git diff --name-only HEAD~1`: nothing outside CHANGE
- [ ] Read your own diff; revert any hunk you cannot justify in one sentence
- [ ] Append MILESTONE_PASS with the observed sentinel
- [ ] Commit with `[EP-XXX][Mk] <summary>`
- [ ] Check the Progress checkbox
