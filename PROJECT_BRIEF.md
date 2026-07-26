# PROJECT BRIEF - POWDERBURN

**Name.** POWDERBURN: The Ledger of Elk Creek.

**Problem.** The Fallout Tactics lineage of squad combat, action points, called shots, sequence
clocks, stances, and real line of sight, has almost no living descendants. Western games almost
always cast the player as one gunfighter and treat the 1870s as scenery. POWDERBURN puts a deep
squad kernel inside a Western that names its treaties and its dates.

**Target players.** Players of Fallout Tactics, Jagged Alliance 2, Silent Storm, X-COM, and Battle
Brothers. Narrative RPG players who will stay for nine characters with real arcs. Readers of frontier
history. Modders, who get a documented data format and a validator. All of them offline.

**Primary user outcomes (verbatim, each proved by live fire).**
1. Start a new campaign and complete the opening mission Elk Creek end to end.
2. A called shot to a hit location produces the exact mechanical consequence the rules promise.
3. The simulation is deterministic: same seed and journal, same terminal state hash, every time.
4. Save mid-firefight, load, and the state is exactly what it was, chain intact.
5. A dead companion stays dead, is memorialized, and leaves no dangling reference behind.
6. A moral choice in Act II changes what missions exist in Act III.
7. History cannot be rewritten; content that tries is rejected by the validator.
8. The renderer really draws the game, provably, with no display attached.
9. Every depicted nation and community carries its name and its sources; every asset its license.
10. A sixty-actor battle stays inside its turn budget.

**Business goals.** One paid, DRM-free, offline title sold direct and optionally on itch.io. No
services to run, no accounts to hold, no data to breach, no live-ops. The cost structure is a
one-time build and a static file host, deliberately.

**Technical goals.** A deterministic, replayable simulation kernel with a state hash; a content
pipeline validated by machine including historical and representation rules; a reproducible, signed,
offline build; zero network calls in the shipped product; and a headless command surface complete
enough that every ship criterion is a scripted proof rather than a person clicking.

**Out of scope.** Multiplayer, telemetry, accounts, launchers, DRM, real-time mode, procedural story,
native-code mods, microtransactions, macOS in v1, controller support in v1.

**Success metrics.** Ten live-fire proofs green on a clean tree. Golden campaign replay under nine
minutes. Zero network syscalls in the release binary. Validator clean on the whole content tree.
Two builds of the same commit producing identical hashes.

**Production readiness.** Defined by SPEC-008 and enumerated with a verifying command per line in
PRODUCTION_READINESS.md. The ship gate is AGENTS.md section 15.
