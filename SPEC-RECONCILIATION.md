# POWDERBURN specification reconciliation

Date: 2026-07-27
Candidate: 1.0.1
Authority: SPEC-000 through SPEC-008 as instantiated by the ExecPlans, TESTING.md,
SECURITY.md, and the load-bearing invariants.

This report replaces the stale discovery-era inventory. A `PASS` below means the requirement has
executable code and a named proof; it does not mean that a comment or placeholder exists.

| Spec | Status | Implemented contract and proof |
| --- | --- | --- |
| SPEC-000 Product | PASS | Four acts, 24 missions, 12 camp interludes, nine recruitable companions, historical-fixed outcomes, representation law, and LF-01 through LF-10. `scripts/spec-coverage-check.sh`, `crates/pb-content/tests/{campaign,history,representation,permadeath,playability}.rs`, and released-artifact live fire cover the claims. |
| SPEC-001 Rules | PASS | Q22.10 `Fix32`; deterministic sequence/AP economy; complete action and event vocabulary; ten-stage shot resolution including fouling; hit locations, wounds, explosives, smoke, light, weather, cover, Sand/morale, utility AI, derived statistics, XP, skills, Marks, Ways, and 26 period-correct weapons as RON data. Proofs live in `pb-core`, `pb-rules`, `pb-sim`, `pb-ai`, and their integration/property tests. |
| SPEC-002 Data | PASS | Twelve-crate workspace, typed identifiers and entities, bounded RON loading, stable content/ruleset hashes, connected campaign graph, append-only Ledger chain, strict save refusal on hash/tamper/version mismatch, mod confinement, and asset provenance. |
| SPEC-003 Commands | PASS | `pbcli` implements sim, replay, campaign, capture, bench, selftest, accessibility reporting, and an opt-in localhost-only `serve-replay`. `pbtool` implements content/representation/provenance validation, golden handling, real PNG statistics, deterministic real atlas packing, fuzzing, and crash reproduction. Contract/error/replay tests lock the surface. |
| SPEC-004 Client | PASS | Title-to-campaign-to-battle-to-after-action flow, save/load and explicit unverified-load path, isometric wgpu renderer, terrain/character/prop atlases, portraits, music/SFX/subtitles, keyboard/mouse input, called-shot targeting, settings, all required screens, headless capture, and seven accessibility checks. |
| SPEC-005 Security | PASS | Bounded parsers, hostile fixtures, path/symlink and executable rejection, fixed-history protection, redaction, deterministic seeded fuzzing, and zero network symbols in the default release binary. The replay server is excluded from the shipped feature set and binds only `127.0.0.1` when explicitly compiled. |
| SPEC-006 Errors and recovery | PASS | Stable CLI/content/save error contracts; exact save incompatibility and tamper refusal; structured redacted logs; four-file crash bundle; deterministic repro; six operational runbooks; no silent save migration. |
| SPEC-007 Observability | PASS | Exact nine-metric vocabulary, build/rules/content log fields, filters, 8 MiB rotation, trace of all shot stages plus RNG addresses and AI candidates, in-game metrics overlay, crash reproduction, and performance budgets on authored workloads. |
| SPEC-008 Verification/release | PASS at candidate level | Offline vendored build, format/clippy/reality/security/dependency gates, zero ignored tests, workspace coverage at least 70%, aggregate kernel coverage at least 85%, golden campaign, deterministic goldens, signed reproducible archive, release index, released-artifact smoke/live fire, idempotent publish, and rehearsed rollback. Final ship status is granted only by `scripts/production-readiness-check.sh --ship-gate` on the clean tagged candidate. |

## Cross-spec invariants

All thirteen invariants have direct gates:

1. deterministic replay;
2. no floating-point simulation state;
3. one-way import law;
4. journal replay identity;
5. historical immutability (`E-HIST-001`);
6. representation fields and forbidden-token lint;
7. permadeath propagation with zero reachable dangling references;
8. save and Ledger integrity;
9. no network surface in the default release;
10. AP conservation;
11. exact save/resume;
12. rules/content identity;
13. append-only Ledger semantics.

`scripts/production-readiness-check.sh` runs the full proof chain, while
`scripts/live-fire.sh --released` proves player-visible outcomes using only unpacked shipped
binaries and packaged data.

## Reconciled defects

- Replaced discovery-era claims that derived stats, progression, the complete event vocabulary,
  atlas generation, and replay serving were absent; each now has runtime code and tests.
- Replaced nine unreferenced fantasy/alien/god weapon records and an alien faction with nine
  period-correct arms. The roster is now exactly 26 historical records, each with introduction year
  and sources.
- Made RNG diagnostic tracing thread-local so concurrent tests cannot perturb one another.
- Replaced count-only atlas and byte-heuristic image tools with deterministic PNG decoding/packing.
- Added a complete reachable-branch campaign replay, coverage enforcement, and isolated two-build
  reproducibility proof.
- Made release smoke and live fire consume the released package, including its fixtures, rather
  than borrowing files or binaries from the worktree.

## Deliberate v1 boundaries

The supported v1 artifact is Linux x86_64, English, keyboard/mouse, local saves, no cloud,
multiplayer, analytics, or telemetry. Controller support, localization content, additional desktop
targets, consoles/handhelds, an editor, and external itch.io publication are deferred. These are
declared non-goals, not hidden unfinished implementations. The itch.io command remains a human-only
manual step and no repository script executes it.

## Mechanical decision

The reconciliation is green only when all of these succeed from the same candidate:

```sh
sh scripts/spec-coverage-check.sh
sh scripts/verify.sh
sh scripts/coverage-check.sh
sh scripts/golden-campaign.sh
sh scripts/production-readiness-check.sh --ship-gate
```

Any failure changes the overall status to not shippable; this document cannot override a failing
gate.
