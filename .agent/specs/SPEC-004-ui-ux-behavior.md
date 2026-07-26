# SPEC-004 UI and UX Behavior

Layer L2. The client is a native desktop window. Behavior here is testable through the headless
capture path and the a11y report; nothing in this spec depends on a human looking at it.

## 1. Screens

`Title`, `NewCompany`, `Camp`, `MapTravel`, `Briefing`, `Battle`, `AfterAction`, `LedgerView`,
`Settings`, `Bibliography`. Each is a state in a single explicit state machine in pb-app. Transitions
are enumerated; an undeclared transition is a panic in debug and a logged refusal in release.

## 2. The battle screen

Isometric, two-to-one tile projection, eight facings, camera pan and edge scroll, three zoom steps.
Persistent elements: the sequence strip along the top showing the next eight actors in clock order
with their `next_act_at`; the AP pips for the selected actor; the Sand bar; the wound doll showing
the seven hit locations with their current state; the smoke overlay as a translucent density field;
the event ticker.

Targeting: hovering a target shows the assembled hit chance and, expanded with a key, the full
breakdown from SPEC-001 section 6 stage 3 as a list of named modifiers with their signed values. The
player can always see exactly why a number is what it is. This is a hard requirement, not a nicety:
a game with this much modifier arithmetic that hides the arithmetic is unplayable.

Called shots open a location wheel showing each location's chance and its critical effect.

Movement preview shows AP cost, the tiles that will leave cover, and the enemy overwatch cones the
path crosses, drawn as hatching rather than color alone.

## 3. Input

Fully keyboard operable. Every action has a default binding and every binding is remappable from
`Settings` and persisted to `$PB_CONFIG_DIR/input.ron`. Mouse is fully supported and never required.
Defaults: number keys select squad members, WASD or arrows pan, Q and E rotate facing, Space ends
turn, Tab cycles targets, C crouch, X prone, R reload, F fire, G called shot, O draw a bead, Escape
opens the menu. Controller support is out of scope for v1 and is recorded as a non-goal.

## 4. Accessibility floor (LBI-12, gated by `pbcli a11y-report`)

1. No information is communicated by color alone. Every color-coded state also carries a glyph, a
   pattern, or a label. The report enumerates every UI state and asserts a non-color channel exists.
2. Text scale is adjustable from 100 to 200 percent and the layout reflows without clipping. The
   report renders every screen at 200 percent and asserts zero clipped text runs.
3. Full keyboard control including every menu, verified by a scripted key-only traversal of every
   screen.
4. Three palettes ship: default, deuteranopia, tritanopia. Contrast of text against its background is
   at least 4.5 to 1 in all three, computed and asserted by the report.
5. Camera shake, flashing, and screen-fill effects are individually disableable, defaulting to on but
   capped below three flashes per second at all times.
6. All spoken or diegetic audio has subtitles, on by default, with a speaker label.
7. A slow-clock option multiplies all animation durations without touching the simulation, because
   animation is presentation and the sequence clock is state.

`a11y-report` fails with `a11y: FAIL <check>` naming the first failing check. It runs in
`production-readiness-check.sh`.

## 5. Loading, empty, and error states

Every screen defines all three. Loading shows a determinate progress line with the asset group being
loaded. Empty states are authored text, never a blank panel: an empty Ledger reads as Elias's
untouched first page. Errors surface the SPEC-006 error code and a plain sentence, never a raw
Rust error string, and never silently swallow.

## 6. The Ledger view

Reachable from Camp and from the main menu. Renders entries in order with index, name, place, date,
and the chosen line, in a hand-set serif face. Shows the chain head hash at the foot, small, as a
quiet signal that this document is the save's integrity root. Filterable by act and by whether the
entry is an ally. Not editable, ever, by any UI path.

## 7. After action

Renders the event log as prose: who fired, at what, through how much smoke, what broke. Lists Sand
spent and recovered, wounds carried forward, ammunition expended, and fouling accumulated per weapon.
Offers the Ledger writing step for every named death, presenting three authored lines per death,
chosen by the player. Skipping is allowed and records the default line, because the game should not
punish a player for not wanting to write today.

## 8. Determinism boundary

Nothing in pb-app or pb-render may write to simulation state. Presentation reads `SimState` and the
event log. Animation timing, camera position, and audio are not state and are not hashed. A frame
capture is deterministic given a state and a tick, which is what LF-08 asserts.
