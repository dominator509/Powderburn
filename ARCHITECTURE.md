# ARCHITECTURE

## Purpose

This file is the module and import law for the software itself. It is not the 6LAYER document
hierarchy; do not conflate them. Every rule below is concrete and checkable.

## System overview

POWDERBURN is one Rust workspace producing three binaries. The center is a pure, deterministic
simulation kernel with no I/O. Everything else is either data feeding it, a driver stepping it, or a
presentation layer reading it. The kernel does not know that a window, a file, a clock, or a network
exists, and that ignorance is the product's most valuable property because every ship criterion rests
on it.

## Repository map

    Cargo.toml              workspace, exact versions only
    Cargo.lock              committed
    rust-toolchain.toml     1.85.0
    rustfmt.toml            formatting law
    .cargo/config.toml      offline, vendored sources
    vendor/                 every dependency, committed
    crates/
      pb-core/    ids, Fix32, geometry, events, hashing
      pb-rng/     counter-based deterministic generator
      pb-rules/   loaded rule tables and lookup
      pb-sim/     the kernel: clock, actors, actions, shot pipeline, environment
      pb-ai/      utility scoring and squad behavior
      pb-content/ RON schema, loader, validator, campaign graph
      pb-save/    save format, Ledger chain, refusal on mismatch
      pb-render/  wgpu isometric renderer and headless capture
      pb-audio/   mixer and cues
      pb-app/     winit shell, screens, input; binary `powderburn`
      pb-cli/     binary `pbcli`
      pb-tools/   binary `pbtool`
    content/                the game as data
    assets/                 art, audio, fonts
    tests/                  journals, fixtures, golden corpus
    scripts/                every gate

## The import law (LBI-03)

    pb-core  <- pb-rng  <- pb-rules <- pb-sim <- pb-ai
    pb-core  <- pb-content
    pb-sim, pb-content <- pb-save
    pb-sim, pb-content <- pb-render
    everything <- pb-app, pb-cli, pb-tools

Stated as prohibitions, because prohibitions are checkable:
- pb-core imports no workspace crate.
- pb-sim may import pb-core, pb-rng, pb-rules. It may never import pb-content, pb-save, pb-render,
  pb-audio, pb-app, pb-cli, or pb-tools.
- pb-content may never import pb-sim. Content is data; it does not simulate.
- pb-render may read pb-sim types. It may never call a function that mutates `SimState`.
- No crate may import pb-app.

Enforced by `scripts/lint-determinism.sh` and by the absence of the dependency in each Cargo.toml.

## Determinism boundary (LBI-01, LBI-02)

Inside pb-core, pb-rng, pb-rules, pb-sim, pb-ai, pb-content:
- no `f32` or `f64`;
- no `SystemTime`, `Instant`, or `std::time`;
- no `rand`, `thread_rng`, `getrandom`, or `OsRng`;
- no `HashMap` or `HashSet` in anything that reaches state or iteration order; use `BTreeMap`,
  `BTreeSet`, or an index-keyed `Vec`;
- no threads whose scheduling affects state; parallelism is permitted only where the fold is
  associative and the reduction order is fixed;
- no filesystem, no environment, no locale, no network.

Outside that set the rules relax: pb-render uses floats freely because presentation is not state.

## Data flow

Content files are parsed once by pb-content into an immutable `Content`. A `Scenario` plus a seed
produces a `SimState`. A `Command`, from the player, the AI, or a journal line, enters
`pb_sim::step`, which either applies it entirely and returns events or returns an error and changes
nothing. Events flow outward to the renderer, the audio cue system, the after-action report, and the
event log. `state_hash` over `SimState` is the single scalar that every determinism proof compares.

Campaign flow is the same shape one level up: a `CampaignNode` plus flags produce a `Scenario`; a
scenario outcome produces flag grants and, for every named death, a `LedgerEntry` appended to the
hash chain.

## State management rules

`SimState` is the only mutable simulation state and it lives in one place. There are no globals, no
lazily initialized statics that affect behavior, and no interior mutability in the kernel. The
renderer holds its own presentation state, which is never hashed and never read by the kernel. The
app holds screen state, which is a finite state machine with enumerated transitions.

## Persistence boundaries

Only pb-save writes save files. Only pb-content reads content. Only pb-app and pb-cli read
`$PB_CONFIG_DIR`. No other crate touches the filesystem, and pb-sim touches it never.

## External integration boundaries

There are none. The product integrates with nothing. The single network-capable code path in the
workspace is `crates/pb-cli/src/replay_server.rs`, behind the `replay-server` cargo feature, bound to
127.0.0.1, used only by the EP-007 replay differ, and never compiled into a release artifact. This is
asserted by `scripts/security-check.sh`.

## Security boundaries

Every boundary in SPEC-005 section 1 is a function in exactly one place with an explicit limit
constant: `pb_save::load`, `pb_content::load`, `pb_content::mods`, `pb_render::decode`. Parsing an
untrusted byte never allocates before a bound is checked.

## Validation and error boundaries

Validation happens at the edge: content at load, journal at parse, save at read, config at read.
Once inside, values are typed such that invalid states are unrepresentable where practical: an
`ActorId` is not a `u32`, an `Ap` is not an `i32`, a `Tick` is not a `u64`. Errors are values with
stable codes from SPEC-006. Panics are legal only for violated kernel invariants.

## Observability boundaries

Logging is a facade in pb-core with no I/O; the sink is installed by pb-app or pb-cli at startup.
The kernel emits events, not log lines. Metrics are collected by the driver, never by the kernel.

## Architectural invariants (cite these numbers in code comments)

- LBI-01 DETERMINISM. `SimState` is a pure function of ruleset, content, seed, and journal.
- LBI-02 FIXED POINT. No float in determinism-critical crates.
- LBI-03 IMPORT LAW. As above.
- LBI-04 JOURNAL COMPLETENESS. Every mutation originates from a journal command or a deterministic
  consequence of one.
- LBI-05 HISTORICAL IMMUTABILITY. No content edge alters a HISTORICAL_FIXED outcome.
- LBI-06 REPRESENTATION LAW. Nation, community, sources, and the forbidden-token lint.
- LBI-07 PERMADEATH PROPAGATION. Zero dangling references to a dead companion in reachable content.
- LBI-08 SAVE COMPATIBILITY. Hash mismatch is a refusal, never a silent migration.
- LBI-09 NO NETWORK. Zero network symbols in the shipped binary.
- LBI-10 AP CONSERVATION. No action executes above remaining AP; negative AP panics.
- LBI-11 ASSET PROVENANCE. Every asset has a PROVENANCE.toml entry with a license.
- LBI-12 ACCESSIBILITY FLOOR. The seven checks of SPEC-004 section 4.
- LBI-13 LEDGER CHAIN. In-game Ledger entries are append-only and hash-chained; the chain head is the
  save integrity root.

## Forbidden moves

Adding a dependency to reach a standard-library result. Introducing async anywhere in the kernel.
Putting game rules in code instead of `content/rules/`. Reading the clock to seed anything. Caching a
value derived from state in a way that is not recomputed on load. Adding a "just for testing" branch
to production code. Weakening a gate. Regenerating a golden hash to make a test pass without an ADR
explaining what changed and why the new value is correct.

## How to add a feature

1. Write or amend the behavior in the relevant SPEC. 2. Add the vocabulary to SPEC-002 if new names
appear. 3. Add or extend the rule table in `content/rules/` if it is data, which it usually is.
4. Write the failing test first, in the layer that owns the behavior. 5. Implement in the lowest
layer that can own it. 6. Add the event to the SPEC-002 event vocabulary if it emits one. 7. Update
the TESTING.md validation matrix. 8. Run the determinism triple-run before committing.

## How to add a dependency

Justify it against the standard library. Pin an exact version. Re-run `cargo vendor`. Commit
`Cargo.lock` and `vendor/`. Write an ADR. Update ENVIRONMENT.md and `scripts/install.sh`. Confirm the
dependency count stays at or under 60. Confirm the crate has a license file.

## How to change the schema

Content schema changes bump `content_hash`, which by LBI-08 invalidates saves. Therefore: bump
`format_version` in pb-save, add the refusal message naming the old and new versions, note it in
RELEASE.md, and regenerate goldens with `pbtool golden refresh` on a clean tree with an ADR
explaining the delta.

## Architecture review checklist

- Does any determinism-critical crate now contain a float, a clock, an OS random, or a hash map?
- Does any Cargo.toml add an edge the import law forbids?
- Does the diff put a rule in code that belongs in `content/rules/`?
- Does any new untrusted parse path lack a limit constant?
- Does any new UI state communicate by color alone?
- Did the state hash change, and if so, is the change explained by an ADR?
