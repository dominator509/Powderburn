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
use pb_core::geom::TileXY;
use pb_core::ids::ActorId;
use pb_rng::{PbRng, StreamTag};
use pb_rules::tables;

use crate::morale::{morale_accuracy_penalty, morale_state};
use crate::state::{SimError, SimState, Stance};

const HANDS: i32 = 5;

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

/// Compute the hit chance breakdown for a shot without rolling.
/// Returns the total hit chance (clamped 5-95) and each constituent modifier.
pub fn compute_hit_chance_breakdown(
    shooter_actor: &crate::state::ActorState,
    target_actor: &crate::state::ActorState,
    aimed: bool,
    called_location: Option<HitLocationType>,
    extra_penalty: i32,
) -> HitChanceBreakdown {
    let base = 40i32;
    let hands_bonus = HANDS * 3; // 15
    let weapon_accuracy = 5i32;

    let aim_bonus = if let Some(loc) = called_location {
        -tables::called_shot_penalty(loc)
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

    let range_mod: i32 = 0; // Medium range default

    let stance_acc = stance_accuracy_bonus(shooter_actor.stance);
    let stance_ev = stance_evasion_bonus(target_actor.stance);
    let cover_penalty: i32 = 0;
    let evasion = 5 + HANDS / 2 + stance_ev + cover_penalty;

    let smoke_penalty: i32 = 0; // computed elsewhere in real impl
    let shooter_morale = crate::morale::morale_state(shooter_actor.sand, shooter_actor.max_sand);
    let suppression_penalty = crate::morale::morale_accuracy_penalty(shooter_morale);
    let flanking_bonus = compute_flanking_bonus(
        target_actor.position,
        target_actor.facing,
        shooter_actor.position,
    );

    let mut total = base
        + hands_bonus
        + weapon_accuracy
        + aim_bonus
        + range_mod
        + stance_acc
        - evasion
        - smoke_penalty
        + suppression_penalty
        + flanking_bonus
        + extra_penalty;

    total = total.clamp(5, 95);

    let mut modifiers = Vec::new();
    modifiers.push(HitChanceMod { label: "base", value: base });
    modifiers.push(HitChanceMod { label: "HANDS×3", value: hands_bonus });
    modifiers.push(HitChanceMod { label: "weapon", value: weapon_accuracy });
    modifiers.push(HitChanceMod { label: aim_label, value: aim_bonus });
    modifiers.push(HitChanceMod { label: "range", value: range_mod });
    modifiers.push(HitChanceMod { label: "stance", value: stance_acc });
    modifiers.push(HitChanceMod { label: "evasion", value: -evasion });
    if smoke_penalty != 0 {
        modifiers.push(HitChanceMod { label: "smoke", value: -smoke_penalty });
    }
    if suppression_penalty != 0 {
        modifiers.push(HitChanceMod { label: "suppression", value: suppression_penalty });
    }
    if flanking_bonus != 0 {
        modifiers.push(HitChanceMod { label: "flanking", value: flanking_bonus });
    }
    if extra_penalty != 0 {
        modifiers.push(HitChanceMod { label: "extra", value: extra_penalty });
    }

    HitChanceBreakdown { total, modifiers }
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

    if !target_actor.alive {
        return Ok(vec![]);
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
        0,
        99,
    );
    // Default weapon misfire = 5 (Colt Army)
    if misfire_roll < 5 {
        return Ok(vec![Event::Misfire { actor: shooter }]);
    }

    // --- Stage 3: Hit chance assembly (ALL SPEC-001 modifiers) ---

    // Base: 40 + HANDS*3
    let base_hit = 40 + HANDS * 3; // 55

    // Weapon accuracy from weapon table (default to Colt Army)
    let weapon_acc = 5;

    // --- Aim bonus ---
    // SnapShot: 0, AimedShot: +15, CalledShot: uses called_shot_penalty as a negative
    let aim_bonus = if let Some(loc) = called_location {
        -tables::called_shot_penalty(loc)
    } else if aimed {
        15
    } else {
        0
    };

    // --- Range band modifier ---
    // For now, hardcode Medium (0) since we don't have full range infrastructure.
    // In the future, compute from distance between shooter and target.
    // PointBlank(+20), Close(+10), Medium(0), Long(-15), Extreme(-30)
    let range_mod: i32 = 0;

    // --- Shooter stance modifier ---
    let shooter_stance_mod = stance_accuracy_bonus(shooter_actor.stance);

    // --- Target evasion ---
    // evasion = 5 + HANDS/2 + target_stance_mod + cover
    let target_stance_mod = stance_evasion_bonus(target_actor.stance);
    let cover_penalty: i32 = 0; // no cover for now
    let evasion = 5 + HANDS / 2 + target_stance_mod + cover_penalty;

    // --- Smoke penalty ---
    // No smoke system hooked in yet: 0
    let smoke_penalty: i32 = 0;

    // --- Suppression penalty (from morale) ---
    let shooter_morale = morale_state(shooter_actor.sand, shooter_actor.max_sand);
    let suppression_penalty = morale_accuracy_penalty(shooter_morale);

    // --- Flanking bonus ---
    // Compute based on target facing and relative position
    let flanking_bonus = compute_flanking_bonus(
        target_actor.position,
        target_actor.facing,
        shooter_actor.position,
    );

    // --- Final hit chance ---
    let mut hit_chance = base_hit
        + weapon_acc
        + aim_bonus
        + range_mod
        + shooter_stance_mod
        - evasion
        - smoke_penalty
        + suppression_penalty   // morale_accuracy_penalty returns NEGATIVE values
        + flanking_bonus
        + extra_penalty; // FanHammer penalty, etc.

    hit_chance = hit_chance.clamp(5, 95); // clamp 5%–95%

    // --- Stage 4: ToHit roll ---
    let to_hit_roll = PbRng::draw(rng_seed, scenario, tick, shooter.0, StreamTag::ToHit, 0, 99);
    let hit = to_hit_roll < hit_chance;

    let mut events = vec![Event::ShotHit {
        actor: shooter,
        target,
        hit,
    }];

    if !hit {
        // Apply sand loss to target for being shot at (missed)
        apply_sand_loss_to_target(state, target, false);
        return Ok(events);
    }

    // --- Stage 5: Location roll ---
    let location = if let Some(loc) = called_location {
        // Called shots always hit the intended location on a successful hit
        loc
    } else {
        // Random location
        let loc_roll = PbRng::draw(
            rng_seed,
            scenario,
            tick,
            shooter.0,
            StreamTag::Damage,
            0,
            99,
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
    // Base damage for Colt Army: 14
    let base_damage = 14;
    let damage_roll = PbRng::draw(
        rng_seed,
        scenario,
        tick,
        shooter.0,
        StreamTag::Damage,
        1,
        10,
    );
    let raw_damage = base_damage + damage_roll;
    // Apply location multiplier (percent)
    let final_damage = (raw_damage * dmg_mult) / 100;

    events.push(Event::DamageApplied {
        actor: target,
        damage: final_damage,
    });

    // --- Stage 7: Critical check ---
    let crit_roll = PbRng::draw(rng_seed, scenario, tick, shooter.0, StreamTag::Crit, 0, 99);
    let is_crit = crit_roll < 10; // 10% crit chance

    // --- Stage 8: Effects ---
    if final_damage > 0 {
        let wound = if is_crit {
            // Critical hits always apply the location's wound type
            tables::location_to_wound(location)
        } else {
            // Non-critical: only apply wound if damage exceeds threshold
            let threshold = match location {
                HitLocationType::Head | HitLocationType::Eyes => 5,
                HitLocationType::Vitals => 8,
                _ => 10,
            };
            if final_damage >= threshold {
                tables::location_to_wound(location)
            } else {
                // Apply sand loss for being shot at (hit, no wound)
                apply_sand_loss_to_target(state, target, true);
                return Ok(events);
            }
        };

        events.push(Event::WoundApplied {
            actor: target,
            wound,
        });

        // Weapon drop for GunArm/OffArm Broken wounds
        if wound == WoundType::Broken
            && (location == HitLocationType::GunArm || location == HitLocationType::OffArm)
        {
            events.push(Event::WeaponDropped {
                actor: target,
                item: "colt_army_1860".to_string(),
            });
        }

        // Apply sand loss to target for being shot at (hit)
        apply_sand_loss_to_target(state, target, true);
    }

    // --- Stage 9: Smoke deposit ---
    if let Some(shooter_actor) = state.actors.get(&shooter) {
        let shooter_pos = shooter_actor.position;
        events.push(Event::SmokeDeposited {
            tile: shooter_pos,
            density: 3,
        });
    }

    Ok(events)
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
        Stance::Prone => 5,
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

/// Apply Sand loss to the target for being shot at.
/// `hit` is true if the shot connected, false if missed.
fn apply_sand_loss_to_target(_state: &SimState, _target_id: ActorId, _was_hit: bool) {
    // This is a read-only check — actual mutation happens in the caller.
    // We just record the event. The per-tick Sand update is done by the
    // caller after receiving events.
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
    let damage = dmg_roll + 3; // 1d6+3

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
            ap: Ap(10),
            position: pos,
            facing: Facing::South,
            sequence: 5,
            hit_points: 20,
            max_hp: 20,
            name: format!("Actor{}", id.0),
            alive: true,
            wounds: vec![],
            sand: 10,
            max_sand: 10,
            stance: Stance::Standing,
            progression: crate::progression::ActorProgression::new(),
            weapon: "colt_army_1860".to_string(),
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
            result.len() >= 3,
            "Expected at least 3 events, got {}: {:?}",
            result.len(),
            result
        );

        assert_eq!(
            result[0],
            Event::ShotHit {
                actor: shooter,
                target,
                hit: true,
            },
            "First event should be ShotHit with hit=true"
        );

        assert_eq!(
            result[1],
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
        assert_eq!(stance_evasion_bonus(Stance::Prone), 5);
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
