# Checklist: preflight

- [ ] Run from the repository root: `[ -f AGENTS.md ] && [ -d .agent ]`
- [ ] `.env` exists and every REQUIRED variable in PREFLIGHT.md has a value
- [ ] `rustc --version` prints exactly 1.85.0
- [ ] `git --version` is 2.30 or newer
- [ ] `zstd --version` is 1.5 or newer
- [ ] `python3 --version` is 3.10 or newer
- [ ] `minisign -v` runs
- [ ] Software graphics adapter present: `sh scripts/probes/pb_headless_adapter.sh`
- [ ] Signing key outside the repository at mode 600: `sh scripts/probes/pb_release_signing_key.sh`
- [ ] Release directory exists and is writable: `sh scripts/probes/pb_release_dir.sh`
- [ ] `sh scripts/preflight.sh` prints `preflight: ok`
- [ ] `git status --porcelain` is empty
- [ ] `sh scripts/ledger.sh tail 10` shows no unreleased LEASE from another agent
- [ ] Known blockers: read the last NODE_BLOCKED in the ledger, if any, and confirm it is resolved
