NODE-META-BEGIN
ID: EP-005
DEPS: EP-004
MAX_ATTEMPTS_PER_MILESTONE: 6
VERIFY: sh scripts/test-e2e.sh
VERIFY_SENTINEL: test-e2e: ok
GREEN_TAG: green/EP-005
NODE-META-END

# EP-005 Client: the isometric renderer and the player-facing shell

## 1. Purpose and Big Picture

Everything before this node is provable without a screen. This node builds the screen: a wgpu
isometric renderer, the input map, the screens a player moves through, the readability rules that
make a tactical game legible, and the accessibility floor that LBI-12 makes non-negotiable. It also
builds the headless capture path, which is the only reason the renderer can be proven in CI at all.

This node runs in parallel with EP-006. They touch disjoint files. Whichever finishes second rebases
onto the first and reruns its own VERIFY before tagging.

## 2. Scope

pb-render: device setup, the isometric projection, the tile and sprite pipeline, the smoke volume
pass, the fog and LOS overlay, the UI layer, text. pb-audio: mixer, positional cues, ducking, the
subtitle bus. pb-app: window, event loop, screen state machine, input map, settings persistence,
`--headless` capture. The accessibility checks behind `pbcli a11y-report`.

## 3. Non-goals

- No new simulation rules; the renderer reads `SimState` and never writes it.
- No content authoring beyond the placeholder art needed to prove the pipeline.
- No shader micro-optimization. The budget is 16ms at 1080p on the reference machine; meet it, then
  stop.
- No controller support. Keyboard and mouse only for 1.0; recorded as a known limitation.

## 4. Context and Orientation

Entry is `green/EP-004`. The load-bearing risk is that the renderer becomes a second source of truth
for game state. It must not. `pb-render` takes `&SimState` and produces pixels; if it needs a fact
the state does not expose, the fix is to expose it from `pb-sim`, never to recompute it in the
renderer. Binding invariants: LBI-03, LBI-11, LBI-12.

## 5. Files to Read First

    .agent/specs/SPEC-004-ui-ux-behavior.md
    .agent/specs/SPEC-001-core-domain.md sections 8 and 9
    ARCHITECTURE.md import law
    .agent/specs/SPEC-008-production-readiness.md

## 6. Expected Changed Files

    crates/pb-render/src/{lib,device,camera,iso,tiles,sprites,smoke,overlay,ui,text,capture}.rs
    crates/pb-render/shaders/{tile.wgsl,sprite.wgsl,smoke.wgsl,overlay.wgsl,ui.wgsl}
    crates/pb-render/tests/capture.rs
    crates/pb-audio/src/{lib,mixer,cues,subtitles}.rs
    crates/pb-app/src/{main,window,screens,input,settings,headless}.rs
    crates/pb-app/tests/screens.rs
    crates/pb-cli/src/cmd_a11y.rs
    crates/pb-cli/tests/a11y.rs
    assets/sprites/README.md
    assets/fonts/README.md
    content/PROVENANCE.toml

## 7. Interfaces and Contracts

    pub fn render(state: &SimState, view: &ViewState, target: &mut Frame) -> Result<(), RenderError>;
    pub fn capture(state: &SimState, view: &ViewState, out: &Path) -> Result<CaptureMeta, RenderError>;

`render` is pure with respect to `SimState`: it takes an immutable reference and the compiler enforces
it. `capture` writes a PNG and returns its dimensions and checksum. The headless adapter is chosen by
`PB_HEADLESS_ADAPTER`, defaulting to `lavapipe`.

## 8. Milestones

### M1: Device, headless adapter, and a first captured frame
GOAL: A frame renders without a window and is written to disk.
READ: ENVIRONMENT.md, .agent/specs/SPEC-004 section 1
CHANGE: crates/pb-render/src/{lib,device,capture}.rs, crates/pb-app/src/headless.rs
CONTENT: device selection prefers a real adapter when a surface exists and falls back to the software
adapter named by `PB_HEADLESS_ADAPTER` when there is none. A capture renders to an offscreen texture,
maps it, and writes a PNG. The first frame may be a solid clear color; it proves the path, and M2
gives it content.
RUN:
    cargo run --offline -q -p pb-app --bin powderburn -- --headless --capture "$PB_CACHE_DIR/f0.png" --scenario prov_full_battle --tick 0
    test -s "$PB_CACHE_DIR/f0.png" && echo "capture: wrote frame"
EXPECT: `capture: wrote frame`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-005 MILESTONE_PASS "M1 capture: wrote frame"
FALLBACK: if no software adapter is available in the environment, PREFLIGHT should have caught it.
Re-run `sh scripts/preflight.sh`. If the adapter genuinely cannot be installed, halt with a blocked
report; do not stub the renderer, because LF-08 is a ship criterion and a stub cannot pass it.
COMMIT: git add -A && git commit -m "[EP-005][M1] wgpu device, headless adapter, frame capture"

### M2: Isometric projection, tiles, sprites
GOAL: A scenario renders as a legible isometric battlefield.
READ: SPEC-004 sections 1 and 2
CHANGE: crates/pb-render/src/{camera,iso,tiles,sprites}.rs, shaders/{tile,sprite}.wgsl,
assets/sprites/README.md
CONTENT: the projection is a fixed two-to-one dimetric with tile footprint 64 by 32 and four camera
rotations at ninety degrees. Depth sorting is by tile y then x then elevation then entity id, which
is total and therefore stable. Sprites are drawn from a packed atlas built by `pbtool atlas pack`.
Placeholder art is flat-color silhouettes with distinct hues per faction, committed with a
`PROVENANCE.toml` entry declaring them project-authored.
RUN:
    cargo run --offline -q -p pb-app --bin powderburn -- --headless --capture "$PB_CACHE_DIR/f1.png" --scenario prov_full_battle --tick 120
    cargo run --offline -q -p pb-tools --bin pbtool -- image stats "$PB_CACHE_DIR/f1.png"
EXPECT: `dimensions: 1920x1080` and `unique-colors:` reporting more than 16
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-005 MILESTONE_PASS "M2 battlefield renders"
FALLBACK: if atlas packing is not ready, render from individual textures with a comment naming the
follow-up; the atlas is a performance concern, not a correctness one.
COMMIT: git add -A && git commit -m "[EP-005][M2] isometric projection, tiles, sprites"

### M3: Smoke, fog, and the LOS overlay
GOAL: The signature mechanic is visible, and what the player sees matches what the rules compute.
READ: SPEC-001 section 8, SPEC-004 section 3
CHANGE: crates/pb-render/src/{smoke,overlay}.rs, shaders/{smoke,overlay}.wgsl
CONTENT: smoke reads the same per-tile density the rules use and renders it as a layered translucent
volume whose opacity is a fixed function of density, so a player can read density by eye. The overlay
draws movement range, the AP cost of the hovered path, the current cover state per facing, and the
hit chance breakdown for the hovered target with every term from SPEC-001 section 6 stage 3 shown
separately. The renderer must call the same `pb_rules` function the simulation calls to compute the
displayed hit chance; computing it a second way in the renderer is forbidden and is the specific
defect this milestone exists to prevent.
RUN:
    cargo run --offline -q -p pb-app --bin powderburn -- --headless --capture "$PB_CACHE_DIR/f2.png" --scenario prov_full_battle --tick 400 --overlay hitchance
    cargo test --offline -p pb-render --locked --test capture
EXPECT: `test result: ok` and a capture whose overlay values equal the values `pbcli sim --trace-actor` prints for the same tick
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-005 MILESTONE_PASS "M3 smoke and overlay agree with rules"
FALLBACK: if the volumetric pass is too costly, render smoke as stacked billboards with the same
opacity function. The visual is negotiable; the agreement with the rules is not.
COMMIT: git add -A && git commit -m "[EP-005][M3] smoke rendering, fog, and the LOS overlay"

### M4: Screens, input, settings, audio
GOAL: A player can move from the title through camp into a mission and back out.
READ: SPEC-004 sections 4, 5, 6
CHANGE: crates/pb-app/src/{main,window,screens,input,settings}.rs, crates/pb-audio/src/*,
crates/pb-app/tests/screens.rs
CONTENT: the screen state machine is explicit: Title, Camp, Ledger, Roster, Map, Mission, Debrief,
Settings, Quit. Every transition is a named variant, so an illegal transition is a compile error.
Input is a remappable map persisted to `$PB_HOME/settings.ron`; the default map is in SPEC-004
section 5. Audio ducks all cues to thirty percent under dialogue and every cue has a subtitle string,
because LBI-12 requires it.
RUN:
    cargo test --offline -p pb-app --locked --test screens
    cargo run --offline -q -p pb-app --bin powderburn -- --headless --script tests/journals/branch_spare_teague.script --emit-screens
EXPECT: `test result: ok` and the emitted screen sequence ending `screen: Debrief`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-005 MILESTONE_PASS "M4 screens ok"
FALLBACK: none needed.
COMMIT: git add -A && git commit -m "[EP-005][M4] screen state machine, input map, audio"

### M5: The accessibility floor
GOAL: Every LBI-12 requirement is machine-checked, and the check fails when a requirement is broken.
READ: SPEC-008 section 4, ARCHITECTURE.md LBI-12
CHANGE: crates/pb-cli/src/cmd_a11y.rs, crates/pb-cli/tests/a11y.rs
CONTENT: the report checks, each printed as its own line: text contrast at or above 4.5 to 1 against
its actual background sampled from a real capture; a colorblind-safe faction palette verified by
simulating protanopia, deuteranopia, and tritanopia and asserting pairwise distinguishability; every
audio cue having a subtitle; full keyboard reachability of every screen asserted by walking the screen
graph; a text scale setting from 100 to 200 percent that does not clip, asserted by capturing at 200
percent and checking no glyph box exceeds its container. The test deliberately breaks one palette
entry, asserts the report fails, and restores it.
RUN:
    cargo run --offline -q -p pb-cli --bin pbcli -- a11y-report
    cargo test --offline -p pb-cli --locked --test a11y
EXPECT: `a11y: ok` then `test result: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-005 MILESTONE_PASS "M5 a11y: ok"
FALLBACK: none. If a check cannot be automated, it is not a floor. Lower the requirement in
ARCHITECTURE.md with an ADR, or automate it; do not carry an unchecked promise.
COMMIT: git add -A && git commit -m "[EP-005][M5] accessibility floor with machine checks"

### M6: Frame budget and end to end
GOAL: The renderer meets its budget on the reference scenario and the e2e gate is green.
READ: SPEC-008 section 2, scripts/test-e2e.sh
CHANGE: whichever files the profile indicts
CONTENT: measure with `pbcli bench frame --scenario prov_sixty_actors --frames 600`; the ninety-fifth
percentile must be at or under 16ms at 1080p on the reference machine. Optimize only what the profile
names.
RUN:
    cargo run --offline -q -p pb-cli --bin pbcli -- bench frame --scenario prov_sixty_actors --frames 600
    sh scripts/test-e2e.sh
EXPECT: `p95-frame-ms:` at or under 16 then `test-e2e: ok`
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-005 MILESTONE_PASS "M6 test-e2e: ok"
FALLBACK: if the budget is missed after profiling, reduce the smoke pass resolution by half before
touching anything else, and record the change in the Decision Log with the measured before and after.
COMMIT: git add -A && git commit -m "[EP-005][M6] frame budget and e2e green"

## 9. Validation and Acceptance

| Criterion | Command | Sentinel |
| --- | --- | --- |
| Headless capture works, LF-08 | M1 command | `capture: wrote frame` |
| Battlefield is legible | `pbtool image stats` | `dimensions: 1920x1080` |
| Overlay agrees with the rules | `--test capture` | `test result: ok` |
| Screens and input | `--test screens` | `test result: ok` |
| Accessibility floor, LBI-12 | `pbcli a11y-report` | `a11y: ok` |
| Frame budget | `pbcli bench frame` | `p95-frame-ms:` at or under 16 |
| Whole node | `sh scripts/test-e2e.sh` | `test-e2e: ok` |

## 10. Idempotence and Recovery

`git reset --hard green/EP-004`. Delete `$PB_CACHE_DIR` and `$PB_HOME/settings.ron` freely. If EP-006
tagged first, rebase onto `green/EP-006` and rerun `sh scripts/test-e2e.sh` before tagging.

## 11. Progress
- [x] M1 Device, headless adapter, and a first captured frame
- [x] M2 Isometric projection, tiles, sprites
- [x] M3 Smoke, fog, and the LOS overlay
- [x] M4 Screens, input, settings, audio
- [x] M5 The accessibility floor
- [x] M6 Frame budget and end to end

## 12. Surprises and Discoveries

- Headless WSL reports Mesa/EGL discovery warnings before falling back to llvmpipe; capture and
  rendering remain functional. Presentation work initially exceeded the authored 60-actor budget
  until atlas batching and presentation-only prop derivation were tightened.

## 13. Decision Log

- 2026-07-27: Visual state is derived from simulation state and never feeds the deterministic hash.
  Accessibility supports dynamic wrapping and 200 percent text rather than fixed-layout scaling.

## 14. Outcomes and Retrospective

Completed. The client has a legal screen graph, real campaign/combat/save flow, isometric textured
terrain, sprites and props, portraits, music/SFX/subtitles, keyboard/mouse controls, non-color
cues, palettes, slow presentation clock, and headless golden capture. The authored crowded-scene
frame p95 is below 16 ms on the reference environment.
