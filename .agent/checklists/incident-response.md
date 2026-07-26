# Checklist: incident response

An incident here is a bad release. There is no service to page anyone about.

Detect
- [ ] Player report, failed smoke, or a failing gate on a rebuild of a released tag
- [ ] Reproduce on the released artifact, not on the working tree

Triage
- [ ] Which ROLLBACK.md trigger does this match? If any, roll back before debugging further
- [ ] Collect the three files: crash artifact, log, save
- [ ] `pbcli campaign audit --save <f>` and `pbcli sim --resume <f> --emit-events`
- [ ] Record the build, ruleset, and content hashes from the crash artifact

Mitigate
- [ ] Roll back per `.agent/checklists/rollback.md` if triggered
- [ ] Otherwise state plainly why not, in writing, with the evidence

Communicate
- [ ] Update NOTES.md for the affected version in plain language, no euphemism
- [ ] State what an affected player should do

Resolve
- [ ] Fix forward on trunk with a test that fails without the fix
- [ ] Full `verify.sh` and the ten live-fire proofs before republishing

Verify
- [ ] `sh scripts/smoke-test.sh --released` on the new version
- [ ] The original reproduction no longer reproduces

Document
- [ ] Postmortem ADR: what shipped, which gate should have caught it, which gate now does

Follow up
- [ ] The new gate or TESTING.md matrix row exists and is in `scripts/verify.sh`
