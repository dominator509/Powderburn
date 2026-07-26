# Checklist: rollback

- [ ] Trigger confirmed against the ROLLBACK.md list, with the evidence written down first
- [ ] Previous version artifact present in `$PB_RELEASE_DIR/<prev>/`
- [ ] `minisign -Vm` on the previous artifact verifies
- [ ] `printf '%s\n' "<prev>" > "$PB_RELEASE_DIR/CURRENT"`
- [ ] `sh scripts/make-release-index.sh "$PB_RELEASE_DIR"`
- [ ] `sh scripts/smoke-test.sh --released` prints `smoke: ok`
- [ ] Withdrawn version marked withdrawn in the index and in its NOTES.md
- [ ] Nothing was deleted from the release directory
- [ ] Fresh download, unpack, and `pbcli selftest` on a machine that is not the build machine
- [ ] Ledger append: ROLLBACK with the version and the trigger
- [ ] Postmortem ADR opened in DECISIONS.md within a week, ending in a new gate or matrix row
