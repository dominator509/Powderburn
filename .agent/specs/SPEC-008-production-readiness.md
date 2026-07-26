# SPEC-008 Production Readiness

Layer L2. The ship standard, instantiated for POWDERBURN. Every line has a verifying command.
PRODUCTION_READINESS.md carries the same list as an operator checklist.

## 1. Functional

- Every core outcome LF-01 through LF-10 passes: `sh scripts/live-fire.sh` prints `live-fire: ok`.
- All SPEC-001 behaviors implemented: the validation matrix in TESTING.md has no unmapped row.
- All non-goals still excluded: `grep -RIn 'multiplayer\|telemetry\|analytics' crates` returns
  nothing outside comments in SPEC files.
- No known critical bugs, or each one accepted by an ADR in DECISIONS.md.

## 2. Testing

- One fresh run of `sh scripts/verify.sh` prints every sentinel in order and ends `verify: ok`.
- Coverage: pb-core, pb-rng, pb-rules, pb-sim at 85 percent line coverage or better; the workspace at
  70 percent. Measured by `cargo llvm-cov --offline --workspace --summary-only`.
- Every core outcome has a regression test file named in the TESTING.md matrix.
- Zero ignored tests: `grep -RIn '#\[ignore\]' crates` returns nothing.

## 3. Reality

- `sh scripts/reality-gate.sh` prints `reality gate: ok`.
- Zero test doubles in production paths: the reality gate covers `crates`, `content`, `assets`.
- No demo mode: `grep -RIn 'demo_mode\|sandbox_mode' crates` returns nothing.

## 4. Security

- `sh scripts/security-check.sh` prints `security-check: ok`.
- `.env` untracked; no key material in tracked files.
- LBI-09 proven: `nm -uC target/release/powderburn` contains no socket, connect, getaddrinfo, or TLS
  symbol.
- Untrusted input: save, journal, content, mod, and asset parsers each have an explicit limit
  constant, asserted by the security check.
- `sh scripts/dependency-audit.sh` prints `dependency-audit: ok`: every version exact, everything
  vendored, every vendored crate licensed, dependency count at or under 60.

## 5. Privacy and data

- The game collects nothing. No identifier is generated, stored, or transmitted.
- Save location documented in OPERATIONS.md; saves are the player's files and are never touched
  outside their own directory.
- Crash artifacts contain only the SPEC-006 section 4 fields, proven by the EP-008 scripted grep.
- Save compatibility policy documented in RELEASE.md; a breaking change bumps `format_version` and
  ships a refusal message naming the version, never a silent migration.

## 6. Performance

- `worst-ai-turn-ms` at or under 120 and `worst-sim-step-ms` at or under 16 in LF-10.
- `render.frame.ms` p95 at or under 16 on the reference machine, measured by
  `pbcli capture --bench`.
- Full golden campaign replay completes in under 540 seconds: `time pbcli replay --journal
  tests/journals/golden_campaign.jrnl`.
- Content load under 900ms; save write under 250ms; save size under 8 MB.
- Reference machine is defined in ENVIRONMENT.md and is the machine the release is cut on.

## 7. Accessibility

- `pbcli a11y-report` prints `a11y: ok`, covering all seven checks in SPEC-004 section 4.

## 8. Observability

- Every metric in SPEC-007 section 3 emitted by `pbcli selftest --emit-metrics`.
- Every log line carries build, ruleset, and content.
- Log rotation proven; crash artifact contents proven.

## 9. Deployment

- `sh scripts/build.sh` prints `build: ok` and produces all three binaries.
- Artifacts are reproducible: two builds from the same commit produce identical sha256 sums, asserted
  by EP-009.
- Every artifact is signed and its signature verifies: `minisign -Vm <artifact> -p <pub>`.
- The release index in `$PB_RELEASE_DIR` lists version, date, sha256, and signature path.
- Post deploy smoke: `sh scripts/smoke-test.sh --released` prints `smoke: ok`.

## 10. Rollback

- ROLLBACK.md names triggers, owner, and steps.
- A rollback drill was performed and recorded in the ledger with the string
  `ROLLBACK DRILL COMPLETED` during EP-009.
- Rolling back means republishing the previous signed artifact and updating the index; there is no
  server state to unwind, which is stated explicitly so nobody looks for one during an incident.

## 11. Operations

- OPERATIONS.md, the incident-response checklist, and the release checklist all exist and are
  concrete.
- Known risks recorded in DECISIONS.md and in the EP-010 retrospective.

## 12. The ship gate

Clean tree, `verify: ok`, `production-readiness: ok`, release tagged, hands-off publish to
`PB_RELEASE_DIR`, released smoke green, the butler command printed as MANUAL, `RUN_COMPLETE` appended
with the tag. Exactly as AGENTS.md section 15 states.
