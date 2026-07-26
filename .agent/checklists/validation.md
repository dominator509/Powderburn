# Checklist: validation (every gate, in order, with its sentinel)

- [ ] `sh scripts/format-check.sh` prints `format-check: ok`
- [ ] `sh scripts/lint.sh` prints `lint-determinism: ok` then `lint: ok`
- [ ] `sh scripts/typecheck.sh` prints `typecheck: ok`
- [ ] `sh scripts/reality-gate.sh` prints `reality gate: ok`
- [ ] `sh scripts/test-unit.sh` prints `test-unit: ok`
- [ ] `sh scripts/test-integration.sh` prints `test-integration: ok`
- [ ] `sh scripts/build.sh` prints `build: ok`
- [ ] `sh scripts/test-e2e.sh` prints `test-e2e: ok`
- [ ] `sh scripts/security-check.sh` prints `security-check: ok`
- [ ] `sh scripts/dependency-audit.sh` prints `dependency-audit: ok`
- [ ] `sh scripts/smoke-test.sh` prints `smoke: ok`
- [ ] `sh scripts/live-fire.sh` prints ten `live-fire: LF-xx` lines then `live-fire: ok`
- [ ] `sh scripts/verify.sh` prints `verify: ok`

Rules: a gate that did not run in this session did not pass. Never edit a gate to make it pass.
The only legal gate edit is adding a justified line to `.agent/reality-allow` with a Decision Log
entry naming the file, the pattern, and why the match is legitimate.
