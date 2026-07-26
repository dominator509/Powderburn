# Checklist: production readiness

This mirrors PRODUCTION_READINESS.md. Open that file and work it line by line; every line names a
command. Then:

- [ ] `sh scripts/production-readiness-check.sh` prints `production-readiness: ok`
- [ ] Ten live-fire proofs green
- [ ] Coverage at target: `cargo llvm-cov --offline --workspace --summary-only`
- [ ] Zero ignored tests: `grep -RIn '#\[ignore\]' crates` empty
- [ ] LBI-09 proven: `nm -uC target/release/powderburn` has no network symbol
- [ ] Reproducible: two builds, identical sha256
- [ ] Every artifact signed and verified
- [ ] `pbcli a11y-report` prints `a11y: ok`
- [ ] Rollback drill in the ledger: `grep 'ROLLBACK DRILL COMPLETED' .agent/state/LEDGER.md`
- [ ] Reference machine recorded in the release directory
