# POWDERBURN production readiness

Candidate: 1.0.1
Audit date: 2026-07-27

This is an evidence index, not a waiver. `scripts/production-readiness-check.sh --ship-gate` is the
final authority and fails unless the worktree is clean, HEAD has an exact semantic tag, the
published current version matches that tag, and the append-only Ledger records `RUN_COMPLETE`.

## Candidate evidence

- [x] SPEC-000 through SPEC-008 mapped without an unimplemented row:
  `SPEC-RECONCILIATION.md`, `scripts/spec-coverage-check.sh`
- [x] 24 missions, 12 camp interludes, nine companions, and all reachable branches validated:
  `crates/pb-content/tests/{campaign,playability}.rs`
- [x] 26 sourced, period-correct weapons; no fantasy/alien faction or weapon data:
  `scripts/spec-coverage-check.sh`
- [x] Offline workspace tests green, zero failures and zero ignored tests:
  `cargo test --offline --workspace`
- [x] Default and replay-server-feature clippy green with warnings denied:
  `cargo clippy --offline --workspace --all-targets -- -D warnings`;
  `cargo clippy --offline -p pb-cli --all-targets --features replay-server -- -D warnings`
- [x] Workspace line coverage at least 70 percent and aggregate kernel at least 85 percent:
  `scripts/coverage-check.sh` (measured 70.19 and 85.58)
- [x] Format, reality, determinism, security, and vendored-dependency gates:
  `scripts/verify.sh`
- [x] Determinism triple run matches
  `250fe4b8d9cc08e901906cc2ed1f01ca6282e60f640ade6a1b82012a64122fbe`
- [x] Historical mutation refused with `E-HIST-001`; representation/provenance clean; dead
  companions leave zero reachable dangling references: LF-05, LF-07, LF-09
- [x] Save tamper/hash/version mismatch refused; mid-combat suspend/resume hash and Ledger chain
  exact: `pb-save` tests and LF-04
- [x] Default release binary contains no socket/connect/DNS/TLS symbols:
  `scripts/security-check.sh`
- [x] All seven accessibility checks pass, including dynamic 200 percent text, keyboard traversal,
  three palettes, non-color cues, subtitles, and slow presentation clock: `pbcli a11y-report`
- [x] Exact nine metrics, structured build/rules/content logging, 8 MiB rotation, redaction, trace,
  and four-file crash reproduction are proven: `pb-cli` and `pb-app` observability tests
- [x] Authored 60-actor AI/simulation budgets and render p95 are below 120/16/16 ms respectively:
  LF-10 and `REFERENCE_MACHINE.txt`
- [x] Complete reachable campaign branch finishes with intact Ledger and zero dangling references
  in under 540 seconds: `scripts/golden-campaign.sh`
- [x] Content load under 900 ms, save write under 250 ms, save under 8 MiB:
  `pbcli selftest --emit-metrics`
- [x] Signed deterministic archive, checksum, release index, and reference/notes metadata:
  `scripts/release.sh`, `scripts/make-release-index.sh --check`
- [x] Released-artifact smoke and LF-01 through LF-10 use only unpacked shipped files:
  RC3 rehearsal evidence
- [x] Two isolated builds are byte-identical: `scripts/reproducibility-check.sh`
- [x] Idempotent publish and deliberately broken-release rollback rehearsed:
  `ROLLBACK DRILL COMPLETED` in `.agent/state/LEDGER.md`
- [x] Operations, rollback, incident response, release, save-recovery, performance, validation, and
  desync runbooks exist
- [x] External itch.io publication is manual; no script executes `butler push`

## Final ship closure

- [x] Clean reconciled tree committed
- [x] Exact immutable `v1.0.1` tag on the reconciled commit
- [x] `1.0.1` published to `PB_RELEASE_DIR` and selected by `current`
- [x] Released 1.0.1 smoke and LF-01 through LF-10 green
- [x] Two-build reproducibility re-proven from the final commit
- [x] `RUN_COMPLETE v1.0.1` appended to the Ledger
- [x] `scripts/production-readiness-check.sh --ship-gate` prints
  `production-readiness: ok` and `ship-gate: pass`

All closure items were evaluated by the command above. The self-hosted Linux x86_64 v1.0.1 artifact
is shippable within the supported boundaries below.

## Supported boundaries

The v1.0.1 artifact supports Linux x86_64, English, keyboard/mouse, local saves, and no networked
gameplay or data collection. Controller support, localization content, Windows/macOS packages,
consoles/handhelds, cloud sync, an editor, and human-controlled itch.io publication are explicit
deferred scope. CI software rendering proves correctness but is not substituted for the recorded
reference performance measurement.
