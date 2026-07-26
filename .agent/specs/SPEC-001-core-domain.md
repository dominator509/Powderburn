# SPEC-001 Core Domain - the POWDERBURN combat kernel

Layer L2. This spec defines behavior, not code layout. Every rule here is deterministic, integer or
fixed-point, and reproducible from (RulesetHash, ScenarioHash, Seed, InputJournal).

## 1. Numeric law

All simulation state is integer. Where a fraction is needed the type is `Fix32`, a signed 32 bit
fixed-point value with 10 fractional bits, so one unit is 1/1024. Multiplication rounds toward
negative infinity. Division by zero is a panic in debug and a saturating result in release, and the
rules never divide by a quantity that can be zero. Floating point is forbidden in pb-core, pb-rng,
pb-rules, pb-sim, pb-ai, and pb-content and is enforced by scripts/lint-determinism.sh.

Randomness comes only from `PbRng`, a splittable counter-based generator seeded from
(campaign_seed, scenario_id, tick, actor_id, stream_tag). Because the stream is addressed rather than
sequential, drawing a number for one actor cannot perturb another actor's rolls, which is what makes
save-resume exact. Streams in use: `ToHit`, `Damage`, `Crit`, `Misfire`, `Scatter`, `Morale`,
`AiTieBreak`, `Loot`.

## 2. Attributes

Seven, range 1 to 10, sum at creation 40, no attribute above 9 at creation.

| Attribute | Governs |
| --- | --- |
| GRIT | Hit points, carry load, melee damage, resistance to Broken wounds |
| NERVE | Sand pool, resistance to suppression, recovery from Rattled |
| WIND | Action points, sprint distance, fatigue accumulation |
| HANDS | Base accuracy, reload speed, jam clearing, melee skill |
| EYES | Sight radius, called shot accuracy, spotting concealed actors, range band shifts |
| SAVVY | Skill point gain, dialogue options, first aid, trap and lock work |
| LUCK | Critical chance, misfire avoidance, loot quality, tie breaks in the player's favor |

## 3. Derived statistics

- `ActionPoints = 5 + floor(WIND / 2)` per turn, range 5 to 10. Unspent AP up to 4 carry into
  Overwatch as reaction points and are otherwise lost.
- `HitPoints = 20 + (GRIT * 3) + (level * 2)`.
- `Sand = 10 + (NERVE * 2)`. The morale pool. Sand is spent by being shot at, by seeing an ally fall,
  and by close artillery or dynamite. At 0 the actor becomes Broken.
- `Evasion = 5 + floor(HANDS / 2) + stance modifier + cover modifier`. Higher is harder to hit.
- `SightRadius = 8 + EYES` tiles in Day light, modified by section 9.
- `CarryLoad = 25 + (GRIT * 5)` pounds. Over load costs 1 AP per turn per 10 pounds over.
- `Sequence = 2 + floor((EYES + HANDS) / 4)`. Feeds the sequence clock in section 4.

## 4. The sequence clock (continuous turn based)

There are no rounds. A single monotonically increasing integer `tick` advances. Each actor holds a
`next_act_at` tick. The scheduler always advances to the smallest `next_act_at` in the active set,
breaking ties by `Sequence` descending, then by `actor_id` ascending, which is total and
deterministic. When an actor acts it spends AP; when its AP are exhausted or it declares Hold, its
`next_act_at` becomes `tick + turn_length`, where `turn_length = 100 - (Sequence * 4)`, clamped to
the range 50 to 96. A faster actor therefore genuinely acts more often rather than merely acting
first, which is the behavioral difference between this and initiative-order systems.

`tick` is also the clock for every timed effect: bleeding, smoke decay, burning, fuse timers, and the
readiness of a reloaded weapon.

## 5. Action costs

| Action | AP | Notes |
| --- | --- | --- |
| Move one tile | 1 | Diagonal costs 1; difficult terrain costs 2; changing facing is free |
| Sprint | 2 per two tiles | Ends the turn, sets Evasion -3 until next act |
| Stance change to Crouched | 1 | Evasion +2, accuracy +1, move cost doubled |
| Stance change to Prone | 2 | Evasion +4, accuracy +2, move cost tripled, cannot be adjacent-attacked well |
| Rise from Prone | 2 | |
| Snap shot | 3 | No aim bonus |
| Aimed shot | 4 | Accuracy +15, enables called shot |
| Called shot | 5 | Aimed shot at a chosen location, per section 7 |
| Fan the hammer | 6 | Single action revolver only, 3 shots, accuracy -25 each, +3 fouling |
| Volley (squad order) | varies | All eligible squad members fire at one target at the same tick |
| Reload, cartridge | 3 | Winchester, Springfield, cartridge revolver |
| Reload, cap and ball | 8 | Or 3 to swap a preloaded cylinder if one is carried |
| Clear a jam | 4 | HANDS check against fouling level |
| Draw a bead (overwatch) | 2 plus all remaining AP | Reserves a reaction shot within a facing cone |
| Bandage | 4 | Stops Bleeding, restores no HP |
| Rally | 3 | NERVE check, restores Sand to one adjacent ally |
| Throw dynamite | 4 | Fuse in ticks, scatter per section 8 |
| Melee | 3 | Knife, tomahawk, rifle butt |
| Loot adjacent | 2 | |
| Use item | 2 | |

An action may never execute when its cost exceeds remaining AP. The scheduler asserts non-negative AP
after every action; a violation is a panic, not a clamp. This is LBI-10.

## 6. The shot resolution pipeline

Every shot resolves through exactly these stages, in this order, with no shortcuts:

1. Legality. Weapon loaded, in range band, target in line of sight, AP sufficient.
2. Misfire check. `misfire_chance = base_misfire + fouling * 2 - LUCK`, drawn from the `Misfire`
   stream. Cap and ball weapons in Rain double it. A misfire consumes the AP and the round and sets
   the weapon to Jammed.
3. Hit chance assembly, all integers, summed then clamped to the range 5 to 95:
   `base = 40 + (HANDS * 3) + weapon_accuracy`
   `+ aim_bonus (0, 15, or the called shot value)`
   `+ range_band_modifier (PointBlank +20, Close +10, Medium 0, Long -15, Extreme -30)`
   `+ stance_modifier (shooter) - evasion (target)`
   `- cover_penalty (Soft 15, Hard 30, Full: shot is illegal)`
   `- smoke_penalty (section 9)`
   `- light_penalty (section 9)`
   `- suppression_penalty (10 if Rattled, 25 if Broken)`
   `+ flanking_bonus (10 if outside the target facing arc, 20 if directly behind)`
4. Roll. Draw 1 to 100 from the `ToHit` stream. Hit if the roll is at or under the assembled chance.
5. Location. On a hit without a called shot, roll the location table in section 7. On a called shot,
   the location is the declared one; a miss by 10 or less strikes an adjacent location rather than
   missing entirely.
6. Damage. `damage = weapon_dice roll from the Damage stream + strength_bonus (melee only)`, then
   `- armor_damage_resist`, then multiplied by the location multiplier, floor 1 on a hit.
7. Critical. Threshold `crit_on = 95 + floor(LUCK / 2)` on the raw hit roll, or automatically on a
   called shot to Eyes or Vitals that beats its chance by 30 or more. On a critical, roll the
   location critical table in section 7.
8. Effects. Apply damage, wounds, knockback, weapon drop, Sand loss to the target and to every ally
   within 4 tiles who has line of sight.
9. Smoke. Every black powder discharge deposits a Smoke volume at the muzzle tile per section 9.
10. Fouling. Increment weapon fouling by the weapon fouling rate.

Every stage emits an event into the event log. The event log is what LF-02 greps and what the
after-action report renders. Nothing is resolved silently.

## 7. Hit locations, multipliers, and criticals

| Location | Base chance | Damage multiplier | Critical effect |
| --- | --- | --- | --- |
| Head | 5 | 2.0 | Concussed: -3 AP, -20 accuracy for 3 turns |
| Eyes | 2 | 1.5 | Blinded: sight radius 1 until treated |
| Torso | 45 | 1.0 | Winded: -2 AP, Sand -4 |
| Vitals | 8 | 2.5 | Bleeding heavy: 4 HP per turn until bandaged |
| Gun arm | 12 | 0.8 | Broken: weapon dropped, cannot use two-handed weapons |
| Off arm | 10 | 0.7 | Broken: reload cost doubled |
| Legs | 18 | 0.9 | Broken: movement halved, cannot Sprint, forced Prone on a second break |

Called shot accuracy modifiers: Head -25, Eyes -40, Torso 0, Vitals -30, Gun arm -15, Off arm -18,
Legs -10.

Wound types are `Bleeding`, `Broken`, `Concussed`, `Winded`, `Blinded`, `Burned`, `Shocked`. Wounds
persist across missions and heal in camp interludes at a rate governed by SAVVY of the company
surgeon and by whether Tomas Alcantara is alive and sober.

## 8. Explosives

Dynamite exists from 1867 in this campaign, which is historically correct for the period after
Nobel's 1867 patent and consistent with the mining and railroad presence in Act II. A thrown stick
has a fuse expressed in ticks, a scatter roll from the `Scatter` stream displacing it up to 3 tiles
against range and WIND, a blast radius of 3 with damage falling by a third per tile, and it converts
struck cover from Hard to Soft to None. A stick may be caught mid-fuse and rethrown, which is a
HANDS check at -30, and which players will do exactly once.

## 9. The environment: smoke, light, weather, and cover

Smoke is the signature system. Every black powder discharge deposits a `Smoke` volume with a density
of 3 at the muzzle tile and 1 at the two tiles in front of it. Smoke decays by 1 per 40 ticks and
drifts one tile per 120 ticks in the scenario wind direction. Line of sight passing through smoke
accumulates density; total density 3 or more applies a 20 point accuracy penalty, 6 or more makes the
shot illegal. Smoke blinds both sides equally. A long firefight in a canyon fills with smoke and
becomes a knife fight, which is what happened.

Light values are Day, Dusk, Night, Moonlit, and Lanternlit, applying sight radius multipliers of
1, 3/4, 1/4, 1/2, and a 4 tile radius around the lantern. Muzzle flash reveals a firing actor at
night for 1 tick regardless of concealment.

Weather values are Clear, Rain, Snow, Dust, and Wind. Rain doubles misfire chance for cap and ball
weapons and halves smoke persistence. Dust reduces sight radius by 3. Snow leaves tracks that a
scout can follow between missions.

Cover is None, Soft, Hard, or Full and is a property of the tile edge, not the tile, so cover is
directional. Half-height cover gives Hard while Crouched and Soft while Standing. Cover degrades:
Soft cover struck three times becomes None; a wagon burns; a sod wall crumbles.

## 10. Morale

Sand is spent, not rolled. Losses: 2 for being shot at and missed, 4 for being hit, 6 for a critical,
8 for an adjacent ally killed, 3 for any ally killed in sight, 10 for the company leader falling.
Gains: 3 for killing an enemy, 5 for a Rally, full restore in a camp interlude.

States: Steady at above 60 percent of pool. Rattled at 25 to 60 percent: -10 accuracy, cannot Draw a
Bead. Broken below 25 percent: -25 accuracy, must spend the first 2 AP of each turn moving away from
the nearest visible enemy. Routed at 0: the actor leaves the field. A routed companion is not dead
and can be recovered in the camp interlude, at a cost to their arc.

Enemies use the same rules. Bandits rout early. The 10th Cavalry does not. This is authored per
faction as a Sand multiplier and is the primary lever that makes different enemies feel different
without changing their statistics.

## 11. Weapons

Weapons are data, not code. Every weapon record carries: damage dice, accuracy, range bands, AP cost
overrides, capacity, reload class, fouling rate, base misfire, smoke output, two-handed flag, first
year available, and a `historical_note`. The starting roster spans the Colt Model 1860 Army,
the Remington New Model Army, the LeMat, the Colt Single Action Army from 1873, the Henry rifle, the
Winchester Model 1866 and Model 1873, the Spencer carbine, the Sharps Model 1874, the trapdoor
Springfield Model 1873, a ten gauge coach gun, a derringer, the Bowie knife, the tomahawk, the lance,
the ash bow, dynamite from 1867, and an emplaced Gatling that appears exactly twice in the campaign
and is terrifying both times.

`first_year_available` is enforced by the content validator: a scenario dated 1868 that places a
Colt Single Action Army fails with E-ANACHRONISM-001.

## 12. Progression

Experience is awarded for objectives, not for kills, at a ratio that makes a pacifist route viable
and never optimal. Levels grant skill points spent on skill lines (Pistols, Long Guns, Scatterguns,
Blades, Explosives, Field Medicine, Scouting, Talk) and, every third level, one Mark.

Marks are the perk system. Examples with exact effects, all of which are data: Steady Hands (aimed
shot costs 3 instead of 4), Powder Sense (fouling accumulates at half rate), Cool in Smoke (ignore
the first 3 points of smoke penalty), Reader of Men (see enemy Sand values), Left-Hand Draw (draw and
fire in one action once per battle), Long Wind (+2 AP), Field Surgeon (Bandage also restores 5 HP),
Ledger Keeper (Weight accrues at half rate).

Ways are traits chosen at creation, each a real trade: Old Wound (-1 WIND, +2 SAVVY), Quick and Loud
(+2 Sequence, -20 percent Sand), Church Voice (Rally affects all allies within 3, -1 EYES), Sighted
In (+10 accuracy at Long and Extreme, -10 at PointBlank).

## 13. What the kernel may not do

The kernel does not read the wall clock, the filesystem, the network, the locale, or an environment
variable. It does not allocate based on unbounded input. It does not iterate a hash map. It does not
spawn threads whose scheduling affects state. It does not know that a renderer exists. Every one of
those is enforced by lint or by the import law in ARCHITECTURE.md, and every one of them, violated,
would break LF-03.
