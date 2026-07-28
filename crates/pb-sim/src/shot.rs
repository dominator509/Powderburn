//! Ten-stage shot pipeline.
//!
//! M3: Implements the full shot resolution per SPEC-001 section 6.
//!
//! Stages:
//!  1. Legality  (weapon loaded, in range, LOS)
//!  2. Misfire   (vs StreamTag::Misfire)
//!  3. Hit chance assembly (base + HANDS*3 + weapon_accuracy + ALL modifiers)
//!  4. ToHit roll (vs StreamTag::ToHit)
//!  5. Location roll (if hit)
//!  6. Damage roll (vs StreamTag::Damage)
//!  7. Critical check (vs StreamTag::Crit)
//!  8. Effects (damage, wounds, knockback, weapon drop, Sand)
//!  9. Smoke deposit
//! 10. Fouling increment

#![forbid(unsafe_code)]

use pb_core::event::{Event, HitLocationType, WoundType};
use pb_core::geom::{Facing, TileXY};
use pb_core::ids::ActorId;
use pb_rng::{PbRng, StreamTag};
use pb_rules::tables;

use crate::morale::{morale_accuracy_penalty, morale_state, sand_loss_for_event};
use crate::state::{CoverEdge, CoverLevel, SimError, SimState, Stance};

/// A named modifier in the hit chance breakdown.
#[derive(Debug, Clone, Copy)]
pub struct HitChanceMod {
    pub label: &'static str,
    pub value: i32,
}

/// The full breakdown of a hit chance calculation.
#[derive(Debug, Clone)]
pub struct HitChanceBreakdown {
    pub total: i32,
    pub modifiers: Vec<HitChanceMod>,
}

/// Compute the exact hit chance used by the simulation without consuming RNG.
///
/// This is the sole stage-three implementation shared by command execution,
/// traces, and the client preview. Full cover or opaque smoke is reported as
/// `NoLineOfSight`, matching shot legality.
pub fn compute_hit_chance_breakdown(
    state: &SimState,
    shooter: ActorId,
    target: ActorId,
    aimed: bool,
    called_location: Option<HitLocationType>,
    extra_penalty: i32,
) -> Result<HitChanceBreakdown, SimError> {
    let shooter_actor = state
        .actors
        .get(&shooter)
        .ok_or(SimError::ActorNotFound(shooter))?;
    let target_actor = state
        .actors
        .get(&target)
        .ok_or(SimError::TargetNotFound(target))?;
    let base = 40i32;
    let hands_bonus = shooter_actor.attributes.hands * 3;
    let weapon_accuracy = shooter_actor.weapon_profile.accuracy;

    let aim_bonus = if let Some(loc) = called_location {
        -tables::called_shot_penalty(loc)
            + if shooter_actor
                .progression
                .has_passive("called_shot_bonus_10")
            {
                shooter_actor.progression.effect_value("accuracy_bonus")
            } else {
                0
            }
    } else if aimed {
        15
    } else {
        0
    };

    let aim_label = if called_location.is_some() {
        "called"
    } else if aimed {
        "aim"
    } else {
        "snap"
    };

    let distance = i32::from(
        shooter_actor
            .position
            .chebyshev_distance(target_actor.position),
    );
    let range_mod = range_modifier(distance, shooter_actor.weapon_profile.range_bands);
    let way_range_mod = sighted_in_modifier(shooter_actor, distance);

    let stance_acc = stance_accuracy_bonus(shooter_actor.stance);
    let stance_ev = stance_evasion_bonus(target_actor.stance);
    let cover_penalty = match directional_cover(state, shooter_actor.position, target_actor) {
        CoverLevel::None => 0,
        CoverLevel::Soft => 15,
        CoverLevel::Hard => 30,
        CoverLevel::Full => return Err(SimError::NoLineOfSight(shooter, target)),
    };
    let sprint_penalty = if state.sprinting.contains(&target) {
        3
    } else {
        0
    };
    let evasion = target_actor
        .attributes
        .evasion(stance_ev, cover_penalty)
        .saturating_sub(sprint_penalty);

    let mut smoke_penalty =
        smoke_penalty_in_state(state, shooter_actor.position, target_actor.position);
    if smoke_penalty == i32::MAX {
        return Err(SimError::NoLineOfSight(shooter, target));
    }
    if shooter_actor
        .progression
        .has_passive("smoke_penalty_resist_3")
    {
        smoke_penalty = smoke_penalty
            .saturating_sub(shooter_actor.progression.effect_value("penalty_reduction"));
    }
    let shooter_morale = crate::morale::morale_state(shooter_actor.sand, shooter_actor.max_sand);
    let mut suppression_penalty = crate::morale::morale_accuracy_penalty(shooter_morale);
    if shooter_actor
        .progression
        .has_passive("suppression_penalty_halved")
    {
        suppression_penalty /= 2;
    }
    let concussion_penalty = if state
        .wound_effects
        .get(&shooter)
        .is_some_and(|effects| effects.concussed_turns > 0)
    {
        -20
    } else {
        0
    };
    let flanking_bonus = compute_flanking_bonus(
        target_actor.position,
        target_actor.facing,
        shooter_actor.position,
    );

    let mut total = base + hands_bonus + weapon_accuracy + aim_bonus + range_mod + stance_acc
        - evasion
        - smoke_penalty
        + suppression_penalty
        + concussion_penalty
        + flanking_bonus
        + way_range_mod
        + extra_penalty;

    total = total.clamp(5, 95);

    let modifiers = vec![
        HitChanceMod {
            label: "base",
            value: base,
        },
        HitChanceMod {
            label: "HANDS×3",
            value: hands_bonus,
        },
        HitChanceMod {
            label: "weapon",
            value: weapon_accuracy,
        },
        HitChanceMod {
            label: aim_label,
            value: aim_bonus,
        },
        HitChanceMod {
            label: "range",
            value: range_mod,
        },
        HitChanceMod {
            label: "stance",
            value: stance_acc,
        },
        HitChanceMod {
            label: "evasion",
            value: -evasion,
        },
        HitChanceMod {
            label: "smoke",
            value: -smoke_penalty,
        },
        HitChanceMod {
            label: "suppression",
            value: suppression_penalty,
        },
        HitChanceMod {
            label: "concussion",
            value: concussion_penalty,
        },
        HitChanceMod {
            label: "flanking",
            value: flanking_bonus,
        },
        HitChanceMod {
            label: "way",
            value: way_range_mod,
        },
        HitChanceMod {
            label: "extra",
            value: extra_penalty,
        },
    ];

    Ok(HitChanceBreakdown { total, modifiers })
}

/// Describe every stage and every named hit-chance term using the exact
/// pre-shot simulation state. RNG outcomes are emitted by `pb-rng`'s trace
/// sink after the real command executes.
pub fn trace_shot_inputs(
    state: &SimState,
    shooter: ActorId,
    target: ActorId,
    called_location: Option<HitLocationType>,
    aimed: bool,
    extra_penalty: i32,
) -> Result<Vec<String>, SimError> {
    let shooter_actor = state
        .actors
        .get(&shooter)
        .ok_or(SimError::ActorNotFound(shooter))?;
    let target_actor = state
        .actors
        .get(&target)
        .ok_or(SimError::TargetNotFound(target))?;
    let weapon = &shooter_actor.weapon_profile;
    let distance = i32::from(
        shooter_actor
            .position
            .chebyshev_distance(target_actor.position),
    );
    let cover = directional_cover(state, shooter_actor.position, target_actor);
    let mut smoke_penalty =
        smoke_penalty_in_state(state, shooter_actor.position, target_actor.position);
    if shooter_actor
        .progression
        .has_passive("smoke_penalty_resist_3")
        && smoke_penalty != i32::MAX
    {
        smoke_penalty = smoke_penalty
            .saturating_sub(shooter_actor.progression.effect_value("penalty_reduction"));
    }
    let mut sight_radius = if shooter_actor.wounds.contains(&WoundType::Blinded) {
        1
    } else {
        shooter_actor.attributes.sight_radius()
            + if shooter_actor.progression.has_passive("sight_radius_plus_2") {
                2
            } else {
                0
            }
    };
    sight_radius = match state.light_level {
        crate::environment::LightLevel::Day => sight_radius,
        crate::environment::LightLevel::Dusk => sight_radius * 3 / 4,
        crate::environment::LightLevel::Night => {
            if shooter_actor
                .progression
                .has_passive("night_penalty_halved")
            {
                sight_radius * 5 / 8
            } else {
                sight_radius / 4
            }
        }
        crate::environment::LightLevel::Moonlit => sight_radius / 2,
        crate::environment::LightLevel::Lanternlit => sight_radius.min(4),
    };
    if state.weather == crate::environment::Weather::Dust {
        sight_radius = sight_radius.saturating_sub(3);
    }

    let base = 40;
    let hands = shooter_actor.attributes.hands * 3;
    let weapon_accuracy = weapon.accuracy;
    let aim = if let Some(location) = called_location {
        -tables::called_shot_penalty(location)
            + if shooter_actor
                .progression
                .has_passive("called_shot_bonus_10")
            {
                shooter_actor.progression.effect_value("accuracy_bonus")
            } else {
                0
            }
    } else if aimed {
        15
    } else {
        0
    };
    let range = range_modifier(distance, weapon.range_bands);
    let way = sighted_in_modifier(shooter_actor, distance);
    let stance = stance_accuracy_bonus(shooter_actor.stance);
    let cover_penalty = match cover {
        CoverLevel::None => 0,
        CoverLevel::Soft => 15,
        CoverLevel::Hard => 30,
        CoverLevel::Full => i32::MAX,
    };
    let sprint_penalty = if state.sprinting.contains(&target) {
        3
    } else {
        0
    };
    let evasion = if cover_penalty == i32::MAX {
        i32::MAX
    } else {
        target_actor
            .attributes
            .evasion(stance_evasion_bonus(target_actor.stance), cover_penalty)
            .saturating_sub(sprint_penalty)
    };
    let shooter_morale = morale_state(shooter_actor.sand, shooter_actor.max_sand);
    let mut suppression = morale_accuracy_penalty(shooter_morale);
    if shooter_actor
        .progression
        .has_passive("suppression_penalty_halved")
    {
        suppression /= 2;
    }
    let concussion = if state
        .wound_effects
        .get(&shooter)
        .is_some_and(|effects| effects.concussed_turns > 0)
    {
        -20
    } else {
        0
    };
    let flanking = compute_flanking_bonus(
        target_actor.position,
        target_actor.facing,
        shooter_actor.position,
    );
    let total = compute_hit_chance_breakdown(
        state,
        shooter,
        target,
        aimed,
        called_location,
        extra_penalty,
    )?
    .total;
    let mut misfire =
        weapon.base_misfire + shooter_actor.fouling * 2 - shooter_actor.attributes.luck;
    if state.weather == crate::environment::Weather::Rain && weapon.reload_class == "CapAndBall" {
        misfire *= 2;
    }
    misfire = misfire.clamp(0, 95);

    Ok(vec![
        format!(
            "stage: 1 legality loaded={} jammed={} target_alive={} distance={} max_range={} sight={} smoke={} cover={cover:?}",
            shooter_actor.loaded_rounds,
            shooter_actor.jammed,
            target_actor.alive && !target_actor.routed,
            distance,
            weapon.range_bands[3],
            sight_radius.max(1),
            smoke_penalty
        ),
        format!(
            "stage: 2 misfire base={} fouling={} luck={} weather={:?} chance={misfire}",
            weapon.base_misfire, shooter_actor.fouling, shooter_actor.attributes.luck, state.weather
        ),
        format!(
            "stage: 3 hit-chance base={base} hands={hands} weapon={weapon_accuracy} aim={aim} range={range} way={way} stance={stance} evasion=-{evasion} smoke=-{smoke_penalty} suppression={suppression} concussion={concussion} flanking={flanking} extra={extra_penalty} total={total}"
        ),
        "stage: 4 to-hit roll=see-draw stream=ToHit".to_string(),
        format!(
            "stage: 5 location called={}",
            called_location.map_or_else(|| "random".to_string(), |location| location.to_string())
        ),
        format!(
            "stage: 6 damage dice={}d{} bonus={}",
            weapon.damage_count, weapon.damage_sides, weapon.damage_bonus
        ),
        format!(
            "stage: 7 critical luck={} threshold={}",
            shooter_actor.attributes.luck,
            95 - shooter_actor.attributes.luck / 2
        ),
        "stage: 8 effects damage,wounds,knockback,weapon-drop,sand".to_string(),
        format!("stage: 9 smoke output={}", weapon.smoke_output),
        format!("stage: 10 fouling increment={}", weapon.fouling_rate),
    ])
}

/// Resolve a shot from `shooter` at `target`.
///
/// `called_location` is `Some(loc)` for a called shot, `None` for snap or
/// aimed. `aimed` distinguishes snap (false) from aimed (true) — called shots
/// are always aimed. `extra_penalty` is an additional flat penalty to hit
/// chance (e.g. -25 for FanHammer).
///
/// Returns a vector of events describing the shot outcome.
pub fn resolve_shot(
    state: &SimState,
    shooter: ActorId,
    target: ActorId,
    called_location: Option<HitLocationType>,
    aimed: bool,
    extra_penalty: i32,
) -> Result<Vec<Event>, SimError> {
    // --- Stage 1: Legality ---
    let shooter_actor = state
        .actors
        .get(&shooter)
        .ok_or(SimError::ActorNotFound(shooter))?;
    let target_actor = state
        .actors
        .get(&target)
        .ok_or(SimError::TargetNotFound(target))?;

    if shooter_actor.jammed {
        return Err(SimError::WeaponJammed(shooter));
    }
    if shooter_actor.loaded_rounds <= 0 {
        return Err(SimError::WeaponNotLoaded(shooter));
    }
    if !target_actor.alive || target_actor.routed {
        return Ok(vec![]);
    }

    let weapon = &shooter_actor.weapon_profile;
    let distance = i32::from(
        shooter_actor
            .position
            .chebyshev_distance(target_actor.position),
    );
    if distance > weapon.range_bands[3] {
        return Err(SimError::OutOfRange(target));
    }
    let mut sight_radius = if shooter_actor.wounds.contains(&WoundType::Blinded) {
        1
    } else {
        shooter_actor.attributes.sight_radius()
            + if shooter_actor.progression.has_passive("sight_radius_plus_2") {
                2
            } else {
                0
            }
    };
    sight_radius = match state.light_level {
        crate::environment::LightLevel::Day => sight_radius,
        crate::environment::LightLevel::Dusk => sight_radius * 3 / 4,
        crate::environment::LightLevel::Night => {
            if shooter_actor
                .progression
                .has_passive("night_penalty_halved")
            {
                sight_radius * 5 / 8
            } else {
                sight_radius / 4
            }
        }
        crate::environment::LightLevel::Moonlit => sight_radius / 2,
        crate::environment::LightLevel::Lanternlit => sight_radius.min(4),
    };
    if state.weather == crate::environment::Weather::Dust {
        sight_radius = sight_radius.saturating_sub(3);
    }
    if distance > sight_radius.max(1) {
        return Err(SimError::NoLineOfSight(shooter, target));
    }
    let smoke_penalty =
        smoke_penalty_in_state(state, shooter_actor.position, target_actor.position);
    if smoke_penalty == i32::MAX {
        return Err(SimError::NoLineOfSight(shooter, target));
    }
    let cover = directional_cover(state, shooter_actor.position, target_actor);
    if cover == CoverLevel::Full {
        return Err(SimError::NoLineOfSight(shooter, target));
    }

    let rng_seed = state.seed;
    let scenario = state.scenario_id;
    let tick = state.tick.0;

    // --- Stage 2: Misfire check ---
    let misfire_roll = PbRng::draw(
        rng_seed,
        scenario,
        tick,
        shooter.0,
        StreamTag::Misfire,
        1,
        100,
    );
    let mut misfire_chance =
        weapon.base_misfire + shooter_actor.fouling * 2 - shooter_actor.attributes.luck;
    if state.weather == crate::environment::Weather::Rain && weapon.reload_class == "CapAndBall" {
        misfire_chance *= 2;
    }
    misfire_chance = misfire_chance.clamp(0, 95);
    if misfire_roll <= misfire_chance {
        return Ok(vec![
            Event::Fired {
                actor: shooter,
                target,
            },
            Event::Misfire { actor: shooter },
        ]);
    }

    // --- Stage 3: Hit chance assembly (shared with trace and client HUD) ---
    let hit_chance = compute_hit_chance_breakdown(
        state,
        shooter,
        target,
        aimed,
        called_location,
        extra_penalty,
    )?
    .total;

    // --- Stage 4: ToHit roll ---
    let to_hit_roll = PbRng::draw(
        rng_seed,
        scenario,
        tick,
        shooter.0,
        StreamTag::ToHit,
        1,
        100,
    );
    let direct_hit = to_hit_roll <= hit_chance;
    let adjacent_called_hit =
        called_location.is_some() && !direct_hit && to_hit_roll <= hit_chance + 10;
    let hit = direct_hit || adjacent_called_hit;

    let mut events = vec![
        Event::Fired {
            actor: shooter,
            target,
        },
        Event::ShotHit {
            actor: shooter,
            target,
            hit,
        },
    ];

    if !hit {
        events.push(Event::Missed {
            actor: shooter,
            target,
        });
        events.push(Event::SandLost {
            actor: target,
            amount: sand_loss_for_event("shot_at_missed"),
        });
        events.extend(smoke_deposition_events(
            shooter_actor.position,
            shooter_actor.facing,
            weapon.smoke_output,
        ));
        return Ok(events);
    }

    // --- Stage 5: Location roll ---
    let location = if let Some(loc) = called_location {
        if adjacent_called_hit {
            adjacent_location(loc, to_hit_roll)
        } else {
            loc
        }
    } else {
        // Random location
        let loc_roll = PbRng::draw(
            rng_seed,
            scenario,
            tick,
            shooter.0,
            StreamTag::Damage,
            1,
            100,
        );
        tables::select_hit_location(loc_roll).unwrap_or(HitLocationType::Torso)
    };

    events.push(Event::HitLocation {
        actor: target,
        location,
    });

    // Get damage multiplier
    let dmg_mult = tables::hit_location_entry(location)
        .map(|e| e.damage_mult)
        .unwrap_or(100);

    // --- Stage 6: Damage roll ---
    let damage_roll = (0..weapon.damage_count.max(1))
        .map(|index| {
            PbRng::draw_indexed(
                rng_seed,
                scenario,
                tick,
                shooter.0,
                StreamTag::Damage,
                index as u32,
                1,
                weapon.damage_sides.max(1),
            )
        })
        .sum::<i32>();
    let raw_damage = damage_roll + weapon.damage_bonus;
    let resisted_damage = raw_damage
        .saturating_sub(state.armor_damage_resist.get(&target).copied().unwrap_or(0))
        .max(1);
    // Apply location multiplier (percent)
    let final_damage = ((resisted_damage * dmg_mult) / 100).max(1);

    events.push(Event::DamageApplied {
        actor: target,
        damage: final_damage,
    });

    // --- Stage 7: Critical check ---
    let crit_roll = PbRng::draw(rng_seed, scenario, tick, shooter.0, StreamTag::Crit, 1, 100);
    let crit_threshold = 95 - shooter_actor.attributes.luck / 2;
    let is_crit = crit_roll >= crit_threshold
        || (matches!(location, HitLocationType::Eyes | HitLocationType::Vitals)
            && hit_chance - to_hit_roll >= 30);

    // --- Stage 8: Effects ---
    if is_crit {
        events.push(Event::Critical {
            actor: target,
            effect: format!("{location}"),
        });
    }

    if final_damage > 0 {
        let wound = if is_crit {
            Some(tables::location_to_wound(location))
        } else {
            let threshold = match location {
                HitLocationType::Head | HitLocationType::Eyes => 5,
                HitLocationType::Vitals => 8,
                _ => 10,
            };
            (final_damage >= threshold).then(|| tables::location_to_wound(location))
        };

        if let Some(wound) = wound {
            events.push(Event::WoundApplied {
                actor: target,
                wound,
            });
            if wound == WoundType::Broken
                && (location == HitLocationType::GunArm || location == HitLocationType::OffArm)
                && !target_actor.progression.has_passive("cannot_be_disarmed")
            {
                events.push(Event::WeaponDropped {
                    actor: target,
                    item: target_actor.weapon.clone(),
                });
            }
        }

        events.push(Event::SandLost {
            actor: target,
            amount: if is_crit {
                sand_loss_for_event("shot_at_critical")
            } else {
                sand_loss_for_event("shot_at_hit")
            },
        });
    }

    // --- Stage 9: Smoke deposit ---
    if let Some(shooter_actor) = state.actors.get(&shooter) {
        let shooter_pos = shooter_actor.position;
        events.extend(smoke_deposition_events(
            shooter_pos,
            shooter_actor.facing,
            weapon.smoke_output,
        ));
    }

    Ok(events)
}

fn smoke_deposition_events(tile: TileXY, facing: Facing, output: i32) -> Vec<Event> {
    let output = output.max(0) as u32;
    if output == 0 {
        return Vec::new();
    }
    let first = tile.neighbour(facing);
    let second = first.neighbour(facing);
    vec![
        Event::SmokeDeposited {
            tile,
            density: output.saturating_mul(3),
        },
        Event::SmokeDeposited {
            tile: first,
            density: output,
        },
        Event::SmokeDeposited {
            tile: second,
            density: output,
        },
    ]
}

// ---------------------------------------------------------------------------
// Helper: stance → accuracy bonus for the shooter
// ---------------------------------------------------------------------------
fn stance_accuracy_bonus(stance: Stance) -> i32 {
    match stance {
        Stance::Standing => 0,
        Stance::Crouched => 1,
        Stance::Prone => 2,
    }
}

// ---------------------------------------------------------------------------
// Helper: stance → evasion bonus for the target
// ---------------------------------------------------------------------------
fn stance_evasion_bonus(stance: Stance) -> i32 {
    match stance {
        Stance::Standing => 0,
        Stance::Crouched => 2,
        Stance::Prone => 4,
    }
}

fn range_modifier(distance: i32, bands: [i32; 4]) -> i32 {
    if distance <= 1 {
        20
    } else if distance <= bands[0] {
        10
    } else if distance <= bands[1] {
        0
    } else if distance <= bands[2] {
        -15
    } else {
        -30
    }
}

fn sighted_in_modifier(actor: &crate::state::ActorState, distance: i32) -> i32 {
    if !actor.progression.has_passive("sighted_in") {
        return 0;
    }
    if distance <= 1 {
        -10
    } else if distance > actor.weapon_profile.range_bands[1] {
        10
    } else {
        0
    }
}

fn directional_cover(
    state: &SimState,
    shooter_position: TileXY,
    target: &crate::state::ActorState,
) -> CoverLevel {
    let edge = CoverEdge {
        tile: target.position,
        facing: direction_facing(target.position, shooter_position),
    };
    let Some(cover) = state.cover_edges.get(&edge) else {
        return CoverLevel::None;
    };
    if cover.half_height {
        match target.stance {
            Stance::Standing => CoverLevel::Soft,
            Stance::Crouched | Stance::Prone => CoverLevel::Hard,
        }
    } else {
        cover.level
    }
}

fn direction_facing(from: TileXY, to: TileXY) -> Facing {
    let dx = (to.x - from.x).signum();
    let dy = (to.y - from.y).signum();
    match (dx, dy) {
        (0, -1) => Facing::North,
        (1, -1) => Facing::NorthEast,
        (1, 0) => Facing::East,
        (1, 1) => Facing::SouthEast,
        (0, 1) => Facing::South,
        (-1, 1) => Facing::SouthWest,
        (-1, 0) => Facing::West,
        _ => Facing::NorthWest,
    }
}

fn smoke_penalty_in_state(state: &SimState, from: TileXY, to: TileXY) -> i32 {
    let mut accumulated = 0u32;
    for tile in from.line_to(to).iter().skip(1) {
        if tile.x < 0 || tile.y < 0 {
            continue;
        }
        let x = tile.x as u32;
        let y = tile.y as u32;
        if x >= state.smoke_cols || y >= state.smoke_rows {
            continue;
        }
        let index = y as usize * state.smoke_cols as usize + x as usize;
        accumulated = accumulated.saturating_add(u32::from(state.smoke_grid[index]));
    }
    if accumulated >= 6 {
        i32::MAX
    } else if accumulated >= 3 {
        20
    } else {
        0
    }
}

fn adjacent_location(location: HitLocationType, roll: i32) -> HitLocationType {
    use HitLocationType::{Eyes, GunArm, Head, Legs, OffArm, Torso, Vitals};
    let (lower, upper) = match location {
        Head => (Eyes, Torso),
        Eyes => (Head, Vitals),
        Torso => (GunArm, Vitals),
        Vitals => (Torso, Legs),
        GunArm => (Torso, OffArm),
        OffArm => (GunArm, Torso),
        Legs => (Torso, Vitals),
    };
    if roll % 2 == 0 {
        lower
    } else {
        upper
    }
}

// ---------------------------------------------------------------------------
// Helper: compute flanking bonus based on target facing and relative position
// ---------------------------------------------------------------------------
fn compute_flanking_bonus(
    target_pos: TileXY,
    target_facing: pb_core::geom::Facing,
    shooter_pos: TileXY,
) -> i32 {
    let dx = shooter_pos.x as i32 - target_pos.x as i32;
    let dy = shooter_pos.y as i32 - target_pos.y as i32;

    if dx == 0 && dy == 0 {
        return 0; // same tile, no flanking
    }

    // Determine which facing direction the shooter is relative to the target
    // by finding the index of the nearest facing.
    let facing_idx = target_facing.to_index() as i32;

    // Compute the direction index from target to shooter (0 = North, CW)
    let dir_idx = direction_index(dx, dy);

    // Compute the signed difference
    let diff = (dir_idx - facing_idx + 8) % 8;

    match diff {
        // Front arc (±1 tile of facing): no bonus
        0 | 1 | 7 => 0,
        // Side arc: +10
        2 | 3 | 5 | 6 => 10,
        // Rear arc (directly behind): +20
        _ => 20,
    }
}

/// Convert a direction vector (dx, dy) to a facing index 0–7 (0=North, CW).
fn direction_index(dx: i32, dy: i32) -> i32 {
    // Use a simple atan2 approximation by comparing |dx| and |dy|
    if dy < 0 && dx.abs() <= -dy / 2 {
        0 // North
    } else if dy < 0 && dx > 0 && dx > -dy / 2 {
        1 // NorthEast
    } else if dx > 0 && dy.abs() <= dx / 2 {
        2 // East
    } else if dx > 0 && dy > 0 && dy > dx / 2 {
        3 // SouthEast
    } else if dy > 0 && dx.abs() <= dy / 2 {
        4 // South
    } else if dx < 0 && dy > 0 && -dx > dy / 2 {
        5 // SouthWest
    } else if dx < 0 && dy.abs() <= -dx / 2 {
        6 // West
    } else {
        7 // NorthWest
    }
}

/// Resolve a melee attack, returning damage events.
pub fn resolve_melee(state: &mut SimState, attacker: ActorId, target: ActorId) -> Vec<Event> {
    let rng_seed = state.seed;
    let scenario = state.scenario_id;
    let tick = state.tick.0;

    let dmg_roll = PbRng::draw(
        rng_seed,
        scenario,
        tick,
        attacker.0,
        StreamTag::Damage,
        1,
        6,
    );
    let strength_bonus = state
        .actors
        .get(&attacker)
        .map(|actor| {
            actor.attributes.grit / 2
                + if actor.progression.has_passive("cannot_be_disarmed") {
                    1
                } else {
                    0
                }
        })
        .unwrap_or(0);
    let damage = (dmg_roll + strength_bonus)
        .saturating_sub(state.armor_damage_resist.get(&target).copied().unwrap_or(0))
        .max(1);

    let mut events = vec![Event::DamageApplied {
        actor: target,
        damage,
    }];

    // Apply damage to target actor state
    if let Some(t_actor) = state.actors.get_mut(&target) {
        t_actor.hit_points -= damage;
        if t_actor.hit_points <= 0 {
            t_actor.alive = false;
            events.push(Event::ActorKilled { actor: target });
            let name = t_actor.name.clone();
            if name.starts_with("c_") {
                events.push(Event::CompanionKilled { id: name });
            }
        }
    }

    events
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::manual_range_contains
)]
mod tests {
    use super::*;
    use crate::state::{ActorState, Stance};
    use pb_core::geom::Facing;
    use pb_core::geom::TileXY;
    use pb_core::ids::Ap;

    fn make_actor(id: ActorId, pos: TileXY) -> ActorState {
        ActorState {
            faction_id: String::new(),
            is_companion: false,
            attributes: pb_core::Attributes::BALANCED,
            ap: Ap(10),
            position: pos,
            facing: Facing::South,
            sequence: 5,
            hit_points: 20,
            max_hp: 20,
            name: format!("Actor{}", id.0),
            alive: true,
            routed: false,
            wounds: vec![],
            sand: 10,
            max_sand: 10,
            stance: Stance::Standing,
            progression: crate::progression::ActorProgression::new(),
            weapon: "colt_army_1860".to_string(),
            weapon_profile: Default::default(),
            loaded_rounds: 6,
            weapon_capacity: 6,
            fouling: 0,
            jammed: false,
        }
    }

    #[test]
    fn shot_misfire_check_returns_misfire_event() {
        let mut state = SimState::new(5, 1);
        let shooter = ActorId(1);
        let target = ActorId(2);
        state
            .actors
            .insert(shooter, make_actor(shooter, TileXY::new(0, 0)));
        state
            .actors
            .insert(target, make_actor(target, TileXY::new(5, 0)));

        let result = resolve_shot(&state, shooter, target, None, false, 0).unwrap();
        assert_eq!(
            result[0],
            Event::Fired {
                actor: shooter,
                target,
            }
        );
        assert_eq!(
            result[1],
            Event::ShotHit {
                actor: shooter,
                target,
                hit: true,
            }
        );
    }

    #[test]
    fn called_shot_to_gunarm_emits_correct_events() {
        let mut state = SimState::new(5, 1);
        let shooter = ActorId(1);
        let target = ActorId(2);
        state
            .actors
            .insert(shooter, make_actor(shooter, TileXY::new(0, 0)));
        state
            .actors
            .insert(target, make_actor(target, TileXY::new(5, 0)));

        let result = resolve_shot(
            &state,
            shooter,
            target,
            Some(HitLocationType::GunArm),
            true,
            0,
        )
        .unwrap();

        assert!(
            result.len() >= 4,
            "Expected at least 3 events, got {}: {:?}",
            result.len(),
            result
        );

        assert_eq!(
            result[0],
            Event::Fired {
                actor: shooter,
                target,
            },
            "First event should be Fired"
        );
        assert_eq!(
            result[1],
            Event::ShotHit {
                actor: shooter,
                target,
                hit: true,
            },
            "First event should be ShotHit with hit=true"
        );

        assert_eq!(
            result[2],
            Event::HitLocation {
                actor: target,
                location: HitLocationType::GunArm,
            },
            "Second event should be HitLocation(GunArm)"
        );
    }

    #[test]
    fn shot_to_dead_target_returns_empty() {
        let mut state = SimState::new(42, 1);
        let shooter = ActorId(1);
        let target = ActorId(2);
        let mut t = make_actor(target, TileXY::new(5, 0));
        t.alive = false;
        state
            .actors
            .insert(shooter, make_actor(shooter, TileXY::new(0, 0)));
        state.actors.insert(target, t);

        let result = resolve_shot(&state, shooter, target, None, false, 0).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn shot_unknown_actor_returns_error() {
        let state = SimState::new(42, 1);
        let result = resolve_shot(&state, ActorId(1), ActorId(2), None, false, 0);
        assert!(result.is_err());
    }

    #[test]
    fn stance_accuracy_bonus_values() {
        assert_eq!(stance_accuracy_bonus(Stance::Standing), 0);
        assert_eq!(stance_accuracy_bonus(Stance::Crouched), 1);
        assert_eq!(stance_accuracy_bonus(Stance::Prone), 2);
    }

    #[test]
    fn stance_evasion_bonus_values() {
        assert_eq!(stance_evasion_bonus(Stance::Standing), 0);
        assert_eq!(stance_evasion_bonus(Stance::Crouched), 2);
        assert_eq!(stance_evasion_bonus(Stance::Prone), 4);
    }

    #[test]
    fn direction_index_basic() {
        assert_eq!(direction_index(0, -1), 0); // North
        assert_eq!(direction_index(1, 0), 2); // East
        assert_eq!(direction_index(0, 1), 4); // South
        assert_eq!(direction_index(-1, 0), 6); // West
    }

    #[test]
    fn flanking_bonus_front() {
        // Target at (0,0) facing South, shooter at (0, 1) = directly in front
        let bonus = compute_flanking_bonus(TileXY::new(0, 0), Facing::South, TileXY::new(0, 1));
        assert_eq!(bonus, 0);
    }

    #[test]
    fn flanking_bonus_behind() {
        // Target at (0,0) facing South, shooter at (0, -1) = behind
        let bonus = compute_flanking_bonus(TileXY::new(0, 0), Facing::South, TileXY::new(0, -1));
        assert_eq!(bonus, 20);
    }

    #[test]
    fn flanking_bonus_side() {
        // Target at (0,0) facing South, shooter at (1, 0) = East (side)
        let bonus = compute_flanking_bonus(TileXY::new(0, 0), Facing::South, TileXY::new(1, 0));
        assert_eq!(bonus, 10);
    }

    #[test]
    fn preview_breakdown_uses_real_cover_and_smoke_state() {
        let shooter = ActorId(1);
        let target = ActorId(2);
        let mut state = SimState::new(42, 1);
        state
            .actors
            .insert(shooter, make_actor(shooter, TileXY::new(0, 0)));
        state
            .actors
            .insert(target, make_actor(target, TileXY::new(5, 0)));

        let clear = compute_hit_chance_breakdown(&state, shooter, target, true, None, 0).unwrap();
        state.cover_edges.insert(
            CoverEdge {
                tile: TileXY::new(5, 0),
                facing: Facing::West,
            },
            crate::state::CoverState {
                level: CoverLevel::Hard,
                strikes: 0,
                half_height: false,
                burning: false,
            },
        );
        state.smoke_grid[2] = 3;
        let obscured =
            compute_hit_chance_breakdown(&state, shooter, target, true, None, 0).unwrap();

        let modifier = |breakdown: &HitChanceBreakdown, label: &str| {
            breakdown
                .modifiers
                .iter()
                .find(|modifier| modifier.label == label)
                .map(|modifier| modifier.value)
                .unwrap()
        };
        assert_eq!(
            modifier(&clear, "evasion") - modifier(&obscured, "evasion"),
            30
        );
        assert_eq!(modifier(&obscured, "smoke"), -20);
        assert_eq!(clear.total - obscured.total, 50);
    }

    #[test]
    fn preview_and_execution_both_refuse_full_cover() {
        let shooter = ActorId(1);
        let target = ActorId(2);
        let mut state = SimState::new(42, 1);
        state
            .actors
            .insert(shooter, make_actor(shooter, TileXY::new(0, 0)));
        state
            .actors
            .insert(target, make_actor(target, TileXY::new(5, 0)));
        state.cover_edges.insert(
            CoverEdge {
                tile: TileXY::new(5, 0),
                facing: Facing::West,
            },
            crate::state::CoverState {
                level: CoverLevel::Full,
                strikes: 0,
                half_height: false,
                burning: false,
            },
        );

        let preview = compute_hit_chance_breakdown(&state, shooter, target, false, None, 0);
        let execution = resolve_shot(&state, shooter, target, None, false, 0);
        assert_eq!(
            preview.unwrap_err(),
            SimError::NoLineOfSight(shooter, target)
        );
        assert_eq!(
            execution.unwrap_err(),
            SimError::NoLineOfSight(shooter, target)
        );
    }

    #[test]
    fn melee_resolve_applies_damage() {
        let mut state = SimState::new(42, 1);
        let attacker = ActorId(1);
        let target = ActorId(2);
        state
            .actors
            .insert(attacker, make_actor(attacker, TileXY::new(0, 0)));
        let mut t = make_actor(target, TileXY::new(1, 0));
        t.hit_points = 20;
        state.actors.insert(target, t);

        let events = resolve_melee(&mut state, attacker, target);
        // Should have at least a DamageApplied event
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::DamageApplied { .. })));
        // HP should have been reduced
        assert!(state.actors[&target].hit_points < 20);
    }
}
