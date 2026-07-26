# ASSUMPTIONS

Every UNKNOWN from the INPUTS appears here with a verification step. An assumption is never left
silent, and never blocks silently.

| # | Assumption | Reason | Risk if wrong | How to verify | Blocks implementation |
| --- | --- | --- | --- | --- | --- |
| A-01 | The build and reference machine is x86_64 Linux with a software graphics adapter available | The operator's stated environment is self-hosted Linux; PREFLIGHT probes for llvmpipe or lavapipe | Headless capture LF-08 cannot run and EP-005 has no exit evidence | `sh scripts/probes/pb_headless_adapter.sh` | Yes, at EP-005 |
| A-02 | Rust 1.85.0 is installable and pinned; edition 2021 | Determinism requires an exactly pinned compiler; lint output is version sensitive | Gate sentinels drift and lint gates become noisy | `rustc --version` in `scripts/preflight.sh` | Yes, at EP-000 |
| A-03 | All dependencies can be vendored and the build runs fully offline afterward | The operator requires zero external dependency at build time | A node stalls waiting on a registry, violating the preflight covenant | `sh scripts/install.sh` then `cargo metadata --offline` | Yes, at EP-001 |
| A-04 | wgpu with a software adapter can render the isometric scene fast enough to capture a frame in CI-like conditions | Needed for a non-interactive renderer proof | LF-08 becomes a manual check, which the pack forbids | EP-005 M1 spike: capture a single triangle, then the real scene | Yes, at EP-005 |
| A-05 | A 60-actor battle can meet 120ms per AI turn in a single thread | Determinism forbids nondeterministic parallelism in the kernel | LF-10 fails and the budget must be renegotiated by ADR | `pbcli bench turn` at EP-002 M6 and again at EP-007 | No, until EP-007 |
| A-06 | Content authored in RON is fast enough to load in under 900ms | RON is human-diffable, which matters more than parse speed for a moddable game | Startup budget missed; fallback is a build-time binary cache | `pbcli selftest --emit-metrics` reading `content.load.ms` | No |
| A-07 | blake3 is acceptable as the hash for state, content, and the in-game Ledger chain | Fast, keyless, well specified, vendorable | Only a swap of hash function and a golden refresh | `cargo tree --offline` shows blake3 pinned; goldens regenerate | No |
| A-08 | minisign is available on the release machine for artifact signing | Simplest offline signing with a tiny verifier | Ship gate cannot sign; fallback is age or gpg detached signatures, decided by ADR | `sh scripts/probes/pb_release_signing_key.sh` | Yes, at EP-009 |
| A-09 | Historical dates and outcomes used in the campaign are as cited in `content/BIBLIOGRAPHY.md` | The immutability law is only meaningful if the fixed points are right | A HISTORICAL_FIXED node encodes a wrong fact and the game asserts something false | Every such node carries at least two citations; `pbtool validate content` enforces presence, a human reviews accuracy at EP-010 | No, but gates ship |
| A-10 | The nine-companion roster and four-act structure fit in the v1 scope | Scope must be fixed before EP-003 authors the campaign graph | Content overruns; fallback is Act IV reduced to four missions, pre-decided | Node count in `content/campaign/nodes.ron` at EP-003 M4 | No |
| A-11 | Sprite and tile assets can be authored or sourced with clear licenses recorded in PROVENANCE.toml | LBI-11 and LF-09 depend on it | Provenance gate fails and the release cannot ship | `pbtool validate provenance` | Yes, at EP-009 |
| A-12 | The operator accepts a Linux-only v1 with a Windows cross-build deferred to v1.1 | Keeps the ship gate reachable; recorded as a non-goal | A platform expectation is missed | Stated in PROJECT_BRIEF.md non-goals and in RELEASE.md | No |
| A-13 | Data-only mods are sufficient for the modding audience | Native mod loading would violate LBI-09 and the sandbox model | Modder disappointment; mitigated by a rich data schema and a public validator | SPEC-005 section 3 | No |
| A-14 | ExecPlans may specify canonical schemas, signatures, and acceptance tests verbatim rather than embedding every line of a large game's source | A full game exceeds any single transcription budget; test-first milestones fix the target surface exactly | The EXECUTOR composes more than it transcribes, raising hallucination risk; mitigated by locked vocabulary, embedded rule tables, and test-first ordering | ADR-0007 in DECISIONS.md; every milestone that composes carries a verification grep | No |

## Verified at EP-000
- A-01 confirmed: x86_64 Linux with llvmpipe (Mesa 25.2.8, LLVM 20.1.2) via Xvfb :99 — `sh scripts/probes/pb_headless_adapter.sh` returned ok
- A-02 confirmed: rustc 1.85.0 (4d91de4e4 2025-02-17) pinned as repository default toolchain
- A-03 pending (deferred to EP-001 M2): cargo vendor not yet executed; offline mode asserted by env var PB_CARGO_OFFLINE=1
- A-08 confirmed: minisign available at /usr/bin/minisign with secret key at /root/.keys/powderburn.key
