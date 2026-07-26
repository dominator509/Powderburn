# Checklist: release

- [ ] Clean tree; `verify: ok`; `production-readiness: ok` all observed this session
- [ ] CHANGELOG.md has an entry with a Save compatibility line
- [ ] Version in `crates/pb-app/Cargo.toml` is correct and unused
- [ ] `git tag -a v<version> -m "POWDERBURN v<version>"`
- [ ] `sh scripts/build.sh` prints `build: ok`
- [ ] Build twice; sha256 of both artifacts identical
- [ ] Package, sign, publish per DEPLOYMENT.md exact deploy steps
- [ ] `minisign -Vm <artifact> -p "$PB_RELEASE_DIR/powderburn.pub"` verifies
- [ ] `sh scripts/make-release-index.sh "$PB_RELEASE_DIR"` prints `make-release-index: ok`
- [ ] `sh scripts/smoke-test.sh --released` prints `smoke: ok`
- [ ] REFERENCE_MACHINE.txt and NOTES.md present in the version directory
- [ ] The butler command printed as MANUAL, not executed
- [ ] `RUN_COMPLETE` appended with the tag
