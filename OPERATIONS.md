# OPERATIONS

## What operations means here

There is no service, no uptime, and no on-call. Operations for POWDERBURN means three things: keeping
the build reproducible, keeping the release directory correct, and being able to answer a player's
bug report from the artifacts on their disk.

## Local operations

| Task | Command |
| --- | --- |
| Full gate chain | `sh scripts/verify.sh` |
| Just the kernel proofs | `sh scripts/test-unit.sh && sh scripts/live-fire.sh` |
| Determinism check after a kernel change | run the LF-03 triple-run block from `scripts/live-fire.sh` |
| Trace one actor's decisions | `pbcli sim --scenario <s> --seed <n> --trace-actor <id>` |
| Compare two replays | `pbcli replay --journal a.jrnl --diff b.jrnl` |
| Regenerate goldens (clean tree only, needs an ADR) | `pbtool golden refresh` |
| Validate all content | `pbtool validate content` |

## Release directory operations

| Task | Command |
| --- | --- |
| Rebuild the index | `sh scripts/make-release-index.sh "$PB_RELEASE_DIR"` |
| Verify a published artifact | `minisign -Vm <artifact> -p "$PB_RELEASE_DIR/powderburn.pub"` |
| Verify contents | `sha256sum -c SHA256SUMS` inside the unpacked directory |
| Roll back | see ROLLBACK.md |

Never delete a published artifact. Rollback depends on the old one still being there, and so does
anyone who already downloaded a link.

## Health checks

`pbcli selftest` is the health check. It loads content, runs a fixed twelve-tick scenario, checks the
state hash against a compiled-in constant, verifies the Ledger chain, and prints `selftest: ok`. If
it fails, the build is wrong; nothing else is worth investigating first.

## Common failure modes and exact diagnostics

| Symptom | First diagnostic | Likely cause |
| --- | --- | --- |
| `state-hash` differs between two runs of the same seed | `pbcli sim --trace-actor <id>` on both, diff the draw log | A float, a clock read, a hash map iteration, or an unordered fold entered a determinism-critical crate |
| `replay: differ at tick N` | `pbcli replay --diff` then read the event log around N | A rule change without a golden update, or a journal recorded against a different ruleset |
| Save refuses to load with E-SAVE-INCOMPAT | Compare the hashes printed in the message | A mod was enabled or disabled, or the build changed content |
| Save refuses with E-SAVE-TAMPERED | `pbcli campaign audit --save <f>` | Ledger chain broken; offer Unverified mode |
| Capture produces a blank frame | `pbtool image stats` unique-colors near 1 | Wrong adapter, missing software driver, or the scene failed to load assets |
| Startup slower than 900ms | `pbcli selftest --emit-metrics` reading `content.load.ms` | Content growth; the pre-decided fallback is the build-time binary cache from ADR-0004 |
| AI turn over budget | `pbcli bench turn --emit-budget` | Utility scoring is evaluating too many candidate tiles; cap the candidate set, do not parallelize |
| Smoke volumes growing without bound | `selftest --emit-metrics` reading `smoke.volumes.live` | Decay not running on the shared clock |

## Backup and restore

The repository is the backup: everything, including `vendor/` and the golden corpus, is committed.
The signing key is not in the repository and is the operator's responsibility; losing it means
future releases are signed with a new key and the new public key must be published alongside the old.
Player saves are the player's own files and are never touched by any operational procedure.

## Scheduled jobs

None. There is nothing to cron.

## Incident triage

An incident here is a bad release: a broken artifact, a determinism regression that shipped, or a
save-corrupting bug. Follow `.agent/checklists/incident-response.md`. The mitigation is almost always
the same: roll the index back to the previous version, then fix forward.

## Escalation

Single operator project. Escalation is: stop, write the blocked report, and decide with the evidence
in hand. Do not ship past a red gate to meet a date; there is no date.

## Maintenance

Dependency updates are a deliberate act with an ADR, a re-vendor, a golden re-verify, and a full
`verify.sh`. Never routine, never automatic, never bulk.

## Operational safety rules

Never regenerate a golden to make a red test green. Never publish an unsigned artifact. Never delete
from the release directory. Never edit a save file by hand and hand it back to a player. Never run a
build from a dirty tree for a release.
