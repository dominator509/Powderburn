# Checklist: release

- [x] Clean tree; `verify: ok`; `production-readiness: ok` all observed this session
- [x] CHANGELOG.md has an entry with a Save compatibility line
- [x] Version in `crates/pb-app/Cargo.toml` is correct and unused
- [x] `git tag -a v<version> -m "POWDERBURN v<version>"`
- [x] `sh scripts/build.sh` prints `build: ok`
- [x] Build twice; sha256 of both artifacts identical
- [x] Package, sign, publish per DEPLOYMENT.md exact deploy steps
- [x] `minisign -Vm <artifact> -p "$PB_RELEASE_DIR/powderburn.pub"` verifies
- [x] `sh scripts/make-release-index.sh "$PB_RELEASE_DIR"` prints `make-release-index: ok`
- [x] `sh scripts/smoke-test.sh --released <artifact.tar.zst>` prints `smoke-test: ok`
- [x] REFERENCE_MACHINE.txt and NOTES.md present in the version directory
- [x] The butler command printed as MANUAL, not executed
- [x] `RUN_COMPLETE` appended with the tag
