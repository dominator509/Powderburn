# PRODUCTION READINESS

The Section 13 ship standard instantiated for POWDERBURN. Every line has a verifying command or file
path. `scripts/production-readiness-check.sh` enforces every line that can be enforced mechanically.

## Functional

- [ ] LF-01 opening mission completes: `sh scripts/live-fire.sh` (LF-01 block)
- [ ] LF-02 called shot consequence: same, LF-02 block
- [ ] LF-03 determinism triple run matches golden: LF-03 block
- [ ] LF-04 mid-combat save round trip, chain intact: LF-04 block
- [ ] LF-05 permadeath propagation, zero dangling refs: LF-05 block
- [ ] LF-06 branch divergence produces different missions: LF-06 block
- [ ] LF-07 historical immutability rejected with E-HIST-001: LF-07 block
- [ ] LF-08 headless frame non-blank and hash matched: LF-08 block
- [ ] LF-09 representation and provenance clean: LF-09 block
- [ ] LF-10 turn and step budgets met: LF-10 block
- [ ] Every SPEC behavior mapped: TESTING.md validation matrix has no unmapped row
- [ ] Non-goals still excluded: `grep -RIn 'multiplayer\|telemetry\|analytics' crates`
- [ ] Known critical bugs: none, or each with an ADR in DECISIONS.md

## Testing

- [ ] One fresh run: `sh scripts/verify.sh` prints `verify: ok`
- [ ] Coverage: `cargo llvm-cov --offline --workspace --summary-only`, kernel 85 percent, workspace 70
- [ ] Regression per core outcome: TESTING.md matrix
- [ ] Zero ignored tests: `grep -RIn '#\[ignore\]' crates` empty

## Reality

- [ ] `sh scripts/reality-gate.sh` prints `reality gate: ok`
- [ ] No demo mode: `grep -RIn 'demo_mode\|sandbox_mode' crates` empty
- [ ] Test doubles confined to the zone in TESTING.md

## Security

- [ ] `sh scripts/security-check.sh` prints `security-check: ok`
- [ ] `.env` untracked: `git ls-files | grep -x .env` empty
- [ ] LBI-09: `nm -uC target/release/powderburn` has no socket, connect, getaddrinfo, or TLS symbol
- [ ] Every untrusted parser has a `MAX_` limit constant
- [ ] `sh scripts/dependency-audit.sh` prints `dependency-audit: ok`

## Privacy and data

- [ ] Nothing collected, nothing transmitted: SECURITY.md threat table
- [ ] Save location documented: OPERATIONS.md
- [ ] Crash artifact fields exactly as SPEC-006 section 4: EP-008 scripted grep
- [ ] Save compatibility policy stated: RELEASE.md

## Performance

- [ ] `worst-ai-turn-ms` at or under 120 and `worst-sim-step-ms` at or under 16: LF-10
- [ ] `render.frame.ms` p95 at or under 16 on the reference machine
- [ ] Golden campaign replay under 540 seconds
- [ ] `content.load.ms` under 900, `save.write.ms` under 250, `save.size.bytes` under 8 MB
- [ ] Reference machine recorded: `$PB_RELEASE_DIR/<version>/REFERENCE_MACHINE.txt`

## Accessibility

- [ ] `pbcli a11y-report` prints `a11y: ok`, all seven SPEC-004 section 4 checks

## Observability

- [ ] Every SPEC-007 metric emitted: `pbcli selftest --emit-metrics`
- [ ] Log lines carry build, ruleset, content
- [ ] Rotation proven; crash artifact contents proven

## Deployment

- [ ] `sh scripts/build.sh` prints `build: ok`, all three binaries present
- [ ] Reproducible: two builds of the same commit, identical sha256
- [ ] Every artifact signed and verified: `minisign -Vm <artifact> -p <pub>`
- [ ] Release index correct: `sh scripts/make-release-index.sh "$PB_RELEASE_DIR"`
- [ ] Post-deploy smoke: `sh scripts/smoke-test.sh --released` prints `smoke: ok`

## Rollback

- [ ] ROLLBACK.md names triggers, owner, and steps
- [ ] Drill performed and recorded: `grep 'ROLLBACK DRILL COMPLETED' .agent/state/LEDGER.md`

## Operations

- [ ] OPERATIONS.md, `.agent/checklists/incident-response.md`, `.agent/checklists/release.md` exist
- [ ] Known risks recorded in DECISIONS.md and the EP-010 retrospective

## The gate

- [ ] Clean tree: `git status --porcelain` empty
- [ ] `verify: ok` observed this session
- [ ] `production-readiness: ok` observed this session
- [ ] Release tagged
- [ ] Published hands-off to `PB_RELEASE_DIR`; released smoke green
- [ ] The butler command printed as MANUAL
- [ ] `RUN_COMPLETE` appended with the tag
