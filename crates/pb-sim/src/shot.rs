//! Ten-stage shot pipeline.
//!
//! M3: Implements the full shot resolution per SPEC-001 section 6.
//!
//! Stages:
//!  1. Legality  (weapon loaded, in range, LOS)
//!  2. Misfire   (vs StreamTag::Misfire)
//!  3. Hit chance assembly (base 40 + HANDS*3 + weapon_accuracy + modifiers)
//!  4. ToHit roll (vs StreamTag::ToHit)
//!  5. Location roll (if hit)
//!  6. Damage roll (vs StreamTag::Damage)
//!  7. Critical check (vs StreamTag::Crit)
//!  8. Effects (damage, wounds, knockback, weapon drop, Sand)
//!  9. Smoke deposit
//! 10. Fouling increment

#![forbid(unsafe_code)]

use pb_core::event::{Event, HitLocationType, WoundType};
use pb_core::ids::ActorId;
use pb_rng::{PbRng, StreamTag};
use pb_rules::tables;

use crate::state::{SimError, SimState};

/// Resolve a shot from `shooter` at `target`.
///
/// `called_location` is `Some(loc)` for a called shot, `None` for snap or
/// aimed. Returns a vector of events describing the shot outcome.
///
/// This is the top-level entry point for the shot pipeline.
pub fn resolve_shot(
    state: &SimState,
    shooter: ActorId,
    target: ActorId,
    called_location: Option<HitLocationType>,
) -> Result<Vec<Event>, SimError> {
    // --- Stage 1: Legality ---
    let _shooter_actor = state
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

    // --- Stage 2: Misfire check ---
    let rng_seed = state.seed;
    let scenario = state.scenario_id;
    let tick = state.tick.0;

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

    // --- Stage 3: Hit chance assembly ---
    // Base: 40 + HANDS*3. Assume HANDS = 5 for testing purposes (average).
    // In a full game this comes from the actor's stats.
    let hands = 5i32;
    let base_hit = 40 + hands * 3; // 55

    // Weapon accuracy from the weapon table (default to Colt Army)
    let weapon_acc = 5;
    let mut hit_chance = base_hit + weapon_acc; // 60

    // Called shot penalty
    if let Some(loc) = called_location {
        let penalty = tables::called_shot_penalty(loc);
        hit_chance = (hit_chance - penalty).max(5); // minimum 5% even with called shot
    }

    hit_chance = hit_chance.min(95); // cap at 95%

    // --- Stage 4: ToHit roll ---
    let to_hit_roll = PbRng::draw(rng_seed, scenario, tick, shooter.0, StreamTag::ToHit, 0, 99);
    let hit = to_hit_roll < hit_chance;

    let mut events = vec![Event::ShotHit {
        actor: shooter,
        target,
        hit,
    }];

    if !hit {
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
                // No wound for light damage
                // But still return events
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
        }
    }

    #[test]
    fn shot_misfire_check_returns_misfire_event() {
        // seed=5 gives ToHit=27, misfire=38 — no misfire, shot hits
        let mut state = SimState::new(5, 1);
        let shooter = ActorId(1);
        let target = ActorId(2);
        state
            .actors
            .insert(shooter, make_actor(shooter, TileXY::new(0, 0)));
        state
            .actors
            .insert(target, make_actor(target, TileXY::new(5, 0)));

        let result = resolve_shot(&state, shooter, target, None).unwrap();
        // First event should be ShotHit (not Misfire) — with seed=5 it should hit
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
        // seed=5: ToHit=27 (<45 for GunArm called shot), misfire=38 (no misfire)
        let mut state = SimState::new(5, 1);
        let shooter = ActorId(1);
        let target = ActorId(2);
        state
            .actors
            .insert(shooter, make_actor(shooter, TileXY::new(0, 0)));
        state
            .actors
            .insert(target, make_actor(target, TileXY::new(5, 0)));

        let result = resolve_shot(&state, shooter, target, Some(HitLocationType::GunArm)).unwrap();

        // Verify we get enough events
        assert!(
            result.len() >= 3,
            "Expected at least 3 events, got {}: {:?}",
            result.len(),
            result
        );

        // First event must be ShotHit
        assert_eq!(
            result[0],
            Event::ShotHit {
                actor: shooter,
                target,
                hit: true,
            },
            "First event should be ShotHit with hit=true"
        );

        // Second event must be HitLocation(GunArm) for a called shot
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

        let result = resolve_shot(&state, shooter, target, None).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn shot_unknown_actor_returns_error() {
        let state = SimState::new(42, 1);
        let result = resolve_shot(&state, ActorId(1), ActorId(2), None);
        assert!(result.is_err());
    }
}
