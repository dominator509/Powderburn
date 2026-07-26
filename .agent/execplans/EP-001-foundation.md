NODE-META-BEGIN
ID: EP-001
DEPS: EP-000
MAX_ATTEMPTS_PER_MILESTONE: 6
VERIFY: sh scripts/verify.sh
VERIFY_SENTINEL: verify: ok
GREEN_TAG: green/EP-001
NODE-META-END

# EP-001 Foundation

## 1. Purpose and Big Picture

Build the workspace that everything else lives in, and prove the whole gate chain green on almost
nothing. When this node is done the repository compiles offline from vendored sources, is formatted
and linted under a deny-warnings policy, has one real passing test, and `verify.sh` runs end to end.
Every later node inherits a green baseline, so any red is caused by that node's own work.

## 2. Scope

Workspace manifest with exact versions; `rust-toolchain.toml`; `rustfmt.toml`; clippy deny set;
twelve crate skeletons with correct dependency edges; `cargo vendor` and the offline cargo config;
committed `Cargo.lock` and `vendor/`; one real passing test; `.gitignore` finalized; the determinism
lint proven to fire and then proven clean.

## 3. Non-goals

- No simulation rules. EP-002 owns them.
- No content schema. EP-003 owns it.
- No CLI subcommands beyond `--version`. EP-004 owns the surface.
- No rendering. EP-005 owns it.
- No optimization of any kind. Nothing is hot yet.

## 4. Context and Orientation

Entry state is `green/EP-000`: an initialized repository with a directory skeleton and no Rust. The
invariants that begin to bind here are LBI-02 (no floats in kernel crates), LBI-03 (the import law),
and the dependency budget of 60 crates from ARCHITECTURE.md.

## 5. Files to Read First

    ARCHITECTURE.md
    .agent/specs/SPEC-002-data-model.md
    ENVIRONMENT.md
    COMMANDS.md
    scripts/lint-determinism.sh

## 6. Expected Changed Files

    Cargo.toml
    Cargo.lock
    rust-toolchain.toml
    rustfmt.toml
    .cargo/config.toml
    crates/pb-core/Cargo.toml
    crates/pb-core/src/lib.rs
    crates/pb-rng/Cargo.toml
    crates/pb-rng/src/lib.rs
    crates/pb-rules/Cargo.toml
    crates/pb-rules/src/lib.rs
    crates/pb-sim/Cargo.toml
    crates/pb-sim/src/lib.rs
    crates/pb-ai/Cargo.toml
    crates/pb-ai/src/lib.rs
    crates/pb-content/Cargo.toml
    crates/pb-content/src/lib.rs
    crates/pb-save/Cargo.toml
    crates/pb-save/src/lib.rs
    crates/pb-render/Cargo.toml
    crates/pb-render/src/lib.rs
    crates/pb-audio/Cargo.toml
    crates/pb-audio/src/lib.rs
    crates/pb-app/Cargo.toml
    crates/pb-app/src/main.rs
    crates/pb-cli/Cargo.toml
    crates/pb-cli/src/main.rs
    crates/pb-tools/Cargo.toml
    crates/pb-tools/src/main.rs
    vendor/
    .agent/dep-waivers

## 7. Interfaces and Contracts

Crate names, purposes, and the permitted import edges are fixed by SPEC-002 section 1 and
ARCHITECTURE.md. No crate may declare a dependency that the import law forbids. The three binaries
are `powderburn` in pb-app, `pbcli` in pb-cli, `pbtool` in pb-tools, exactly those names.

## 8. Milestones

### M1: Workspace manifest and toolchain pin
GOAL: `cargo metadata` resolves a twelve-member workspace under a pinned compiler.
READ: ARCHITECTURE.md repository map, SPEC-002 section 1
CHANGE: Cargo.toml, rust-toolchain.toml, rustfmt.toml
CONTENT: transcribe exactly.

`rust-toolchain.toml`:

    [toolchain]
    channel = "1.85.0"
    components = ["rustfmt", "clippy"]
    profile = "minimal"

`rustfmt.toml`:

    edition = "2021"
    max_width = 100
    use_field_init_shorthand = true
    newline_style = "Unix"

`Cargo.toml`:

    [workspace]
    resolver = "2"
    members = [
      "crates/pb-core", "crates/pb-rng", "crates/pb-rules", "crates/pb-sim",
      "crates/pb-ai", "crates/pb-content", "crates/pb-save", "crates/pb-render",
      "crates/pb-audio", "crates/pb-app", "crates/pb-cli", "crates/pb-tools",
    ]

    [workspace.package]
    edition = "2021"
    version = "0.1.0"
    license = "GPL-3.0-only"
    rust-version = "1.85.0"

    [workspace.lints.rust]
    unsafe_code = "deny"
    missing_debug_implementations = "warn"

    [workspace.lints.clippy]
    unwrap_used = "deny"
    expect_used = "deny"
    panic_in_result_fn = "deny"
    float_arithmetic = "warn"

    [profile.release]
    opt-level = 3
    lto = "thin"
    codegen-units = 1
    panic = "unwind"
    debug = 1

RUN:
    cargo metadata --format-version 1 >/dev/null && echo "workspace: resolved"
EXPECT: `workspace: resolved`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-001 MILESTONE_PASS "M1 workspace: resolved"
FALLBACK: if a workspace lint key is rejected by 1.85.0, move that key into each crate's own
`[lints]` table rather than dropping the lint. Never drop a deny.
COMMIT: git add -A && git commit -m "[EP-001][M1] workspace manifest and toolchain pin"

### M2: Crate skeletons with the import law encoded
GOAL: Twelve crates exist; each Cargo.toml declares only the dependencies the import law permits.
READ: ARCHITECTURE.md import law
CHANGE: every `crates/*/Cargo.toml` and `crates/*/src/lib.rs` or `main.rs` in section 6
CONTENT: each library crate's `src/lib.rs` begins with exactly these two lines and nothing else until
later nodes add to it:

    //! See ARCHITECTURE.md for this crate's place in the import law.
    #![forbid(unsafe_code)]

Dependency edges, exactly: pb-core none; pb-rng on pb-core; pb-rules on pb-core and pb-rng; pb-sim on
pb-core, pb-rng, pb-rules; pb-ai on pb-core, pb-rng, pb-rules, pb-sim; pb-content on pb-core and
pb-rules; pb-save on pb-core, pb-sim, pb-content; pb-render on pb-core, pb-sim, pb-content; pb-audio
on pb-core; pb-app on all libraries; pb-cli on all libraries except pb-app; pb-tools on pb-core,
pb-content, pb-rules.
RUN:
    cargo check --workspace --all-targets && echo "skeletons: ok"
    sh scripts/lint-determinism.sh
EXPECT: `skeletons: ok` then `lint-determinism: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-001 MILESTONE_PASS "M2 skeletons: ok"
FALLBACK: none needed; the edges are enumerated above and the lint checks them.
COMMIT: git add -A && git commit -m "[EP-001][M2] crate skeletons with import law"

### M3: Vendor everything and go offline permanently
GOAL: The workspace builds with no network, from committed sources.
READ: ENVIRONMENT.md, scripts/install.sh, scripts/dependency-audit.sh
CHANGE: Cargo.lock, vendor/, .cargo/config.toml, .agent/dep-waivers
CONTENT: `.agent/dep-waivers` is created empty except for one comment line:
`# one crate name per line; each requires an ADR in DECISIONS.md`
RUN:
    sh scripts/install.sh
    sh scripts/dependency-audit.sh
    CARGO_NET_OFFLINE=true cargo check --offline --workspace --locked && echo "offline: ok"
EXPECT: `install: ok`, `dependency-audit: ok`, `offline: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-001 MILESTONE_PASS "M3 offline: ok"
FALLBACK: if the dependency count exceeds 60, remove the largest optional dependency and implement
the needed piece directly; record which one and why in the Decision Log. Raising the cap requires an
ADR and is the last resort, not the first.
COMMIT: git add -A && git commit -m "[EP-001][M3] vendor dependencies and go offline"

### M4: One real passing test
GOAL: The test harness is proven by a test that asserts real behavior and would fail if broken.
READ: TESTING.md
CHANGE: crates/pb-core/src/lib.rs
CONTENT: add to pb-core a real `Fix32` type with `from_int`, `to_int_floor`, `mul`, and `div`, plus a
`#[cfg(test)]` module asserting: `Fix32::from_int(3).mul(Fix32::from_int(4)).to_int_floor() == 12`;
that `Fix32::from_int(1).div(Fix32::from_int(3)).mul(Fix32::from_int(3)).to_int_floor() == 0`
because rounding is toward negative infinity; and that the representation is exactly `i32` with ten
fractional bits by asserting `Fix32::ONE.raw() == 1024`.
RUN:
    cargo test --offline --workspace --locked --lib
    sh scripts/test-unit.sh
EXPECT: `test-unit: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-001 MILESTONE_PASS "M4 test-unit: ok"
FALLBACK: none needed; Fix32 is the smallest real thing this project needs and EP-002 depends on it.
COMMIT: git add -A && git commit -m "[EP-001][M4] Fix32 and the first real test"

### M5: Prove the determinism lint fires
GOAL: The gate that protects LBI-01 and LBI-02 is proven to actually fail on a violation.
READ: scripts/lint-determinism.sh
CHANGE: none permanently; this milestone makes a temporary edit and reverts it
CONTENT: temporarily add the line `pub fn temp_probe(x: f32) -> f32 { x }` to
`crates/pb-core/src/lib.rs`, run the lint, observe the failure, then `git checkout --
crates/pb-core/src/lib.rs`.
RUN:
    printf 'pub fn temp_probe(x: f32) -> f32 { x }\n' >> crates/pb-core/src/lib.rs
    if sh scripts/lint-determinism.sh; then echo "GATE DID NOT FIRE"; exit 1; else echo "gate: fires"; fi
    git checkout -- crates/pb-core/src/lib.rs
    sh scripts/lint-determinism.sh
EXPECT: `gate: fires` then `lint-determinism: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-001 MILESTONE_PASS "M5 gate: fires"
FALLBACK: none needed; a gate that has never been observed to fail is not a gate.
COMMIT: git add -A && git commit -m "[EP-001][M5] prove the determinism gate fires" || true

### M6: Green verify on the skeleton
GOAL: The whole gate chain runs end to end and prints `verify: ok`.
READ: scripts/verify.sh, .agent/checklists/validation.md
CHANGE: none
CONTENT: none. Where a gate is not yet meaningful because the code does not exist, the gate must
still run and pass honestly on an empty set. `live-fire.sh` will fail at this stage because no
binaries exist; that is expected, and this milestone runs `verify.sh` with the live-fire and e2e
stages temporarily unreachable only in the sense that the binaries are absent. Do NOT edit
`verify.sh` to skip them. Instead this milestone runs the gate chain up to and including
`build.sh` and records that `test-e2e`, `smoke-test`, and `live-fire` become meaningful at EP-004,
EP-004, and EP-010 respectively, writing that fact into the Decision Log.
RUN:
    sh scripts/format-check.sh
    sh scripts/lint.sh
    sh scripts/typecheck.sh
    sh scripts/reality-gate.sh
    sh scripts/test-unit.sh
    sh scripts/build.sh
    sh scripts/security-check.sh
    sh scripts/dependency-audit.sh
EXPECT: `format-check: ok`, `lint: ok`, `typecheck: ok`, `reality gate: ok`, `test-unit: ok`,
`build: ok`, `security-check: ok`, `dependency-audit: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-001 MILESTONE_PASS "M6 foundation gates green"
FALLBACK: if `build.sh` fails because a binary crate has no main, add a `main` that prints the
version string from `CARGO_PKG_VERSION` and exits zero. That is real behavior, not a stub.
COMMIT: git add -A && git commit -m "[EP-001][M6] foundation gate chain green"

## 9. Validation and Acceptance

| Criterion | Command | Sentinel |
| --- | --- | --- |
| Workspace resolves offline | `cargo check --offline --workspace --locked` | exit 0 |
| Import law holds | `sh scripts/lint-determinism.sh` | `lint-determinism: ok` |
| Dependencies vendored and licensed | `sh scripts/dependency-audit.sh` | `dependency-audit: ok` |
| A real test passes | `sh scripts/test-unit.sh` | `test-unit: ok` |
| The determinism gate fires on a violation | M5 | `gate: fires` |
| Foundation gates green | M6 | all eight sentinels |

## 10. Idempotence and Recovery

`git reset --hard green/EP-000` restores the entry state exactly. `vendor/` and `Cargo.lock` are
regenerated by `sh scripts/install.sh`. No external state is created by this node.

## 11. Progress
- [ ] M1 Workspace manifest and toolchain pin
- [ ] M2 Crate skeletons with the import law encoded
- [ ] M3 Vendor everything and go offline permanently
- [ ] M4 One real passing test
- [ ] M5 Prove the determinism lint fires
- [ ] M6 Green verify on the skeleton

## 12. Surprises and Discoveries

## 13. Decision Log

## 14. Outcomes and Retrospective
Decision Log: smoke-test and live-fire gates blocked at EP-001 because they require game content (scenarios, journals, campaign data) from EP-002/EP-003 and a built  subcommand from EP-004. The full verify.sh will be proven at EP-007 when the graph rejoins.
