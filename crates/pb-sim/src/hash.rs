//! Simulation state hashing for determinism verification.
//!
//! M6: Provides `compute_state_hash` for proving deterministic replay.
//! Every field is serialized in canonical order and fed through
//! `pb_core::hash::hash_state` to produce a 32-byte digest.

#![forbid(unsafe_code)]

use pb_core::event::WoundType;
use pb_core::hash::hash_state;
use pb_core::progression::SkillLine;

use crate::state::{CoverLevel, SimState, Stance};

/// Helper: map a WoundType to a canonical u8 discriminant.
fn wound_to_u8(w: &WoundType) -> u8 {
    match w {
        WoundType::Bleeding => 0,
        WoundType::Broken => 1,
        WoundType::Concussed => 2,
        WoundType::Winded => 3,
        WoundType::Blinded => 4,
        WoundType::Burned => 5,
        WoundType::Shocked => 6,
    }
}

fn push_bytes(buf: &mut Vec<u8>, bytes: &[u8]) {
    let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(bytes);
}

fn push_str(buf: &mut Vec<u8>, value: &str) {
    push_bytes(buf, value.as_bytes());
}

fn skill_to_u8(skill: SkillLine) -> u8 {
    match skill {
        SkillLine::Pistols => 0,
        SkillLine::LongGuns => 1,
        SkillLine::Scatterguns => 2,
        SkillLine::Blades => 3,
        SkillLine::Explosives => 4,
        SkillLine::FieldMedicine => 5,
        SkillLine::Scouting => 6,
        SkillLine::Talk => 7,
    }
}

fn stance_to_u8(stance: Stance) -> u8 {
    match stance {
        Stance::Standing => 0,
        Stance::Crouched => 1,
        Stance::Prone => 2,
    }
}

fn cover_to_u8(level: CoverLevel) -> u8 {
    match level {
        CoverLevel::None => 0,
        CoverLevel::Soft => 1,
        CoverLevel::Hard => 2,
        CoverLevel::Full => 3,
    }
}

/// Compute the canonical 32-byte hash of the complete simulation state.
///
/// Every field that can affect future simulation is included. Collections use
/// their canonical `BTreeMap`/`BTreeSet` order and strings are length-prefixed,
/// preventing ambiguous concatenations. Presentation-only state is not part of
/// `SimState` and therefore cannot enter this hash.
pub fn compute_state_hash(state: &SimState) -> [u8; 32] {
    let mut buf = Vec::new();

    buf.extend_from_slice(&state.tick.0.to_le_bytes());
    buf.extend_from_slice(&state.environment_tick.to_le_bytes());
    buf.extend_from_slice(&state.seed.to_le_bytes());
    buf.extend_from_slice(&state.scenario_id.to_le_bytes());
    buf.extend_from_slice(&state.wind_speed.to_le_bytes());
    buf.push(match state.light_level {
        crate::environment::LightLevel::Day => 0,
        crate::environment::LightLevel::Dusk => 1,
        crate::environment::LightLevel::Night => 2,
        crate::environment::LightLevel::Moonlit => 3,
        crate::environment::LightLevel::Lanternlit => 4,
    });
    buf.push(match state.weather {
        crate::environment::Weather::Clear => 0,
        crate::environment::Weather::Rain => 1,
        crate::environment::Weather::Snow => 2,
        crate::environment::Weather::Dust => 3,
        crate::environment::Weather::Wind => 4,
    });
    buf.push(state.wind_direction.to_index() as u8);
    buf.extend_from_slice(&state.smoke_cols.to_le_bytes());
    buf.extend_from_slice(&state.smoke_rows.to_le_bytes());
    match state.active_actor {
        Some(actor) => {
            buf.push(1);
            buf.extend_from_slice(&actor.0.to_le_bytes());
        }
        None => buf.push(0),
    }
    buf.extend_from_slice(&state.active_turn_actions.to_le_bytes());

    buf.extend_from_slice(
        &u64::try_from(state.actors.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for (id, actor) in &state.actors {
        buf.extend_from_slice(&id.0.to_le_bytes());
        push_str(&mut buf, &actor.faction_id);
        buf.push(u8::from(actor.is_companion));
        buf.extend_from_slice(&actor.attributes.grit.to_le_bytes());
        buf.extend_from_slice(&actor.attributes.nerve.to_le_bytes());
        buf.extend_from_slice(&actor.attributes.wind.to_le_bytes());
        buf.extend_from_slice(&actor.attributes.hands.to_le_bytes());
        buf.extend_from_slice(&actor.attributes.eyes.to_le_bytes());
        buf.extend_from_slice(&actor.attributes.savvy.to_le_bytes());
        buf.extend_from_slice(&actor.attributes.luck.to_le_bytes());
        buf.extend_from_slice(&actor.ap.0.to_le_bytes());
        buf.extend_from_slice(&actor.position.x.to_le_bytes());
        buf.extend_from_slice(&actor.position.y.to_le_bytes());
        buf.push(actor.facing.to_index() as u8);
        buf.extend_from_slice(&actor.sequence.to_le_bytes());
        buf.extend_from_slice(&actor.hit_points.to_le_bytes());
        buf.extend_from_slice(&actor.max_hp.to_le_bytes());
        push_str(&mut buf, &actor.name);
        buf.push(u8::from(actor.alive));
        buf.push(u8::from(actor.routed));
        buf.extend_from_slice(&actor.sand.to_le_bytes());
        buf.extend_from_slice(&actor.max_sand.to_le_bytes());
        buf.push(stance_to_u8(actor.stance));

        buf.extend_from_slice(
            &u64::try_from(actor.wounds.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        for w in &actor.wounds {
            buf.push(wound_to_u8(w));
        }

        let progression = &actor.progression;
        buf.extend_from_slice(&progression.xp.to_le_bytes());
        buf.extend_from_slice(&progression.level.to_le_bytes());
        buf.extend_from_slice(&progression.skill_points.to_le_bytes());
        buf.extend_from_slice(
            &u64::try_from(progression.skill_levels.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        for (skill, level) in &progression.skill_levels {
            buf.push(skill_to_u8(*skill));
            buf.extend_from_slice(&level.to_le_bytes());
        }
        buf.extend_from_slice(
            &u64::try_from(progression.marks.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        for mark in &progression.marks {
            push_str(&mut buf, mark);
        }
        match &progression.way {
            Some(way) => {
                buf.push(1);
                push_str(&mut buf, way);
            }
            None => buf.push(0),
        }
        buf.extend_from_slice(
            &u64::try_from(progression.passives.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        for passive in &progression.passives {
            push_str(&mut buf, passive);
        }
        buf.extend_from_slice(
            &u64::try_from(progression.abilities.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        for ability in &progression.abilities {
            push_str(&mut buf, ability);
        }
        buf.extend_from_slice(
            &u64::try_from(progression.effect_values.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        for (key, value) in &progression.effect_values {
            push_str(&mut buf, key);
            buf.extend_from_slice(&value.to_le_bytes());
        }

        push_str(&mut buf, &actor.weapon);
        let profile = &actor.weapon_profile;
        buf.extend_from_slice(&profile.damage_count.to_le_bytes());
        buf.extend_from_slice(&profile.damage_sides.to_le_bytes());
        buf.extend_from_slice(&profile.damage_bonus.to_le_bytes());
        buf.extend_from_slice(&profile.accuracy.to_le_bytes());
        buf.extend_from_slice(
            &u64::try_from(profile.ap_overrides.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        for (action, cost) in &profile.ap_overrides {
            push_str(&mut buf, action);
            buf.extend_from_slice(&cost.to_le_bytes());
        }
        for threshold in profile.range_bands {
            buf.extend_from_slice(&threshold.to_le_bytes());
        }
        push_str(&mut buf, &profile.reload_class);
        buf.extend_from_slice(&profile.fouling_rate.to_le_bytes());
        buf.extend_from_slice(&profile.base_misfire.to_le_bytes());
        buf.extend_from_slice(&profile.smoke_output.to_le_bytes());
        buf.push(u8::from(profile.two_handed));
        buf.extend_from_slice(&actor.loaded_rounds.to_le_bytes());
        buf.extend_from_slice(&actor.weapon_capacity.to_le_bytes());
        buf.extend_from_slice(&actor.fouling.to_le_bytes());
        buf.push(u8::from(actor.jammed));
    }

    buf.extend_from_slice(
        &u64::try_from(state.sequence_clock.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for (actor, tick) in &state.sequence_clock {
        buf.extend_from_slice(&actor.0.to_le_bytes());
        buf.extend_from_slice(&tick.to_le_bytes());
    }

    buf.extend_from_slice(
        &u64::try_from(state.overwatch.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for actor in &state.overwatch {
        buf.extend_from_slice(&actor.0.to_le_bytes());
    }

    buf.extend_from_slice(
        &u64::try_from(state.reaction_points.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for (actor, points) in &state.reaction_points {
        buf.extend_from_slice(&actor.0.to_le_bytes());
        buf.push(*points);
    }

    buf.extend_from_slice(
        &u64::try_from(state.sprinting.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for actor in &state.sprinting {
        buf.extend_from_slice(&actor.0.to_le_bytes());
    }

    buf.extend_from_slice(
        &u64::try_from(state.wound_effects.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for (actor, effects) in &state.wound_effects {
        buf.extend_from_slice(&actor.0.to_le_bytes());
        buf.push(effects.concussed_turns);
        buf.push(u8::from(effects.heavy_bleeding));
        buf.push(u8::from(effects.winded));
        buf.extend_from_slice(
            &u64::try_from(effects.broken_locations.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        for (location, count) in &effects.broken_locations {
            buf.push(match location {
                pb_core::event::HitLocationType::Head => 0,
                pb_core::event::HitLocationType::Eyes => 1,
                pb_core::event::HitLocationType::Torso => 2,
                pb_core::event::HitLocationType::Vitals => 3,
                pb_core::event::HitLocationType::GunArm => 4,
                pb_core::event::HitLocationType::OffArm => 5,
                pb_core::event::HitLocationType::Legs => 6,
            });
            buf.push(*count);
        }
    }

    buf.extend_from_slice(
        &u64::try_from(state.broken_retreat_remaining.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for (actor, remaining) in &state.broken_retreat_remaining {
        buf.extend_from_slice(&actor.0.to_le_bytes());
        buf.extend_from_slice(&remaining.to_le_bytes());
    }

    buf.extend_from_slice(
        &u64::try_from(state.pending_explosives.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for explosive in &state.pending_explosives {
        buf.extend_from_slice(&explosive.thrower.0.to_le_bytes());
        buf.extend_from_slice(&explosive.position.x.to_le_bytes());
        buf.extend_from_slice(&explosive.position.y.to_le_bytes());
        buf.extend_from_slice(&explosive.detonate_at.to_le_bytes());
    }

    buf.extend_from_slice(
        &u64::try_from(state.held_explosives.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for (actor, explosive) in &state.held_explosives {
        buf.extend_from_slice(&actor.0.to_le_bytes());
        buf.extend_from_slice(&explosive.thrower.0.to_le_bytes());
        buf.extend_from_slice(&explosive.position.x.to_le_bytes());
        buf.extend_from_slice(&explosive.position.y.to_le_bytes());
        buf.extend_from_slice(&explosive.detonate_at.to_le_bytes());
    }

    buf.extend_from_slice(
        &u64::try_from(state.difficult_tiles.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for tile in &state.difficult_tiles {
        buf.extend_from_slice(&tile.x.to_le_bytes());
        buf.extend_from_slice(&tile.y.to_le_bytes());
    }

    buf.extend_from_slice(
        &u64::try_from(state.cover_edges.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for (edge, cover) in &state.cover_edges {
        buf.extend_from_slice(&edge.tile.x.to_le_bytes());
        buf.extend_from_slice(&edge.tile.y.to_le_bytes());
        buf.push(edge.facing.to_index() as u8);
        buf.push(cover_to_u8(cover.level));
        buf.push(cover.strikes);
        buf.push(u8::from(cover.half_height));
        buf.push(u8::from(cover.burning));
    }

    buf.extend_from_slice(
        &u64::try_from(state.leaders.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for actor in &state.leaders {
        buf.extend_from_slice(&actor.0.to_le_bytes());
    }

    for map in [&state.sand_multiplier_pct, &state.carry_weight_lbs] {
        buf.extend_from_slice(&u64::try_from(map.len()).unwrap_or(u64::MAX).to_le_bytes());
        for (actor, value) in map {
            buf.extend_from_slice(&actor.0.to_le_bytes());
            buf.extend_from_slice(&value.to_le_bytes());
        }
    }

    buf.extend_from_slice(
        &u64::try_from(state.armor_damage_resist.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for (actor, resistance) in &state.armor_damage_resist {
        buf.extend_from_slice(&actor.0.to_le_bytes());
        buf.extend_from_slice(&resistance.to_le_bytes());
    }

    buf.extend_from_slice(
        &u64::try_from(state.revealed_until.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for (actor, tick) in &state.revealed_until {
        buf.extend_from_slice(&actor.0.to_le_bytes());
        buf.extend_from_slice(&tick.to_le_bytes());
    }

    buf.extend_from_slice(
        &u64::try_from(state.snow_tracks.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for (actor, tile) in &state.snow_tracks {
        buf.extend_from_slice(&actor.0.to_le_bytes());
        buf.extend_from_slice(&tile.x.to_le_bytes());
        buf.extend_from_slice(&tile.y.to_le_bytes());
    }

    buf.extend_from_slice(
        &u64::try_from(state.consumed_battle_effects.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for (actor, effect) in &state.consumed_battle_effects {
        buf.extend_from_slice(&actor.0.to_le_bytes());
        push_str(&mut buf, effect);
    }

    push_bytes(&mut buf, &state.smoke_grid);

    hash_state(&buf)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::state::{ActorState, Stance};
    use pb_core::geom::{Facing, TileXY};
    use pb_core::ids::{ActorId, Ap, Tick};
    use std::collections::BTreeMap;

    fn sample_state() -> SimState {
        let mut actors = BTreeMap::new();
        actors.insert(
            ActorId(1),
            ActorState {
                faction_id: String::new(),
                is_companion: false,
                attributes: pb_core::Attributes::BALANCED,
                ap: Ap(10),
                position: TileXY::new(5, 5),
                facing: Facing::South,
                sequence: 5,
                hit_points: 30,
                max_hp: 30,
                name: "Ally1".into(),
                alive: true,
                routed: false,
                wounds: vec![],
                sand: 20,
                max_sand: 20,
                stance: Stance::Standing,
                progression: crate::progression::ActorProgression::new(),
                weapon: "colt_army_1860".to_string(),
                weapon_profile: Default::default(),
                loaded_rounds: 6,
                weapon_capacity: 6,
                fouling: 0,
                jammed: false,
            },
        );
        SimState {
            tick: Tick(42),
            environment_tick: 42,
            actors,
            sequence_clock: BTreeMap::new(),
            active_actor: None,
            active_turn_actions: 0,
            seed: 12345,
            scenario_id: 1,
            wind_speed: 0,
            light_level: Default::default(),
            weather: Default::default(),
            wind_direction: pb_core::geom::Facing::North,
            overwatch: std::collections::BTreeSet::new(),
            reaction_points: BTreeMap::new(),
            sprinting: std::collections::BTreeSet::new(),
            wound_effects: BTreeMap::new(),
            broken_retreat_remaining: BTreeMap::new(),
            pending_explosives: Vec::new(),
            held_explosives: BTreeMap::new(),
            difficult_tiles: std::collections::BTreeSet::new(),
            terrain_tiles: BTreeMap::new(),
            tile_elevations: BTreeMap::new(),
            cover_edges: BTreeMap::new(),
            leaders: std::collections::BTreeSet::new(),
            sand_multiplier_pct: BTreeMap::new(),
            carry_weight_lbs: BTreeMap::new(),
            armor_damage_resist: BTreeMap::new(),
            revealed_until: BTreeMap::new(),
            snow_tracks: Vec::new(),
            consumed_battle_effects: std::collections::BTreeSet::new(),
            smoke_grid: vec![0u8; 20 * 12],
            smoke_cols: 20,
            smoke_rows: 12,
        }
    }

    #[test]
    fn compute_state_hash_is_deterministic() {
        let state = sample_state();
        let a = compute_state_hash(&state);
        let b = compute_state_hash(&state);
        assert_eq!(a, b);
    }

    #[test]
    fn compute_state_hash_not_all_zeros() {
        let state = sample_state();
        let h = compute_state_hash(&state);
        assert_ne!(h, [0u8; 32]);
    }

    #[test]
    fn compute_state_hash_differs_on_tick_change() {
        let s1 = sample_state();
        let mut s2 = sample_state();
        s2.tick = Tick(99);
        assert_ne!(compute_state_hash(&s1), compute_state_hash(&s2));
    }

    #[test]
    fn hash_covers_every_future_affecting_state_category() {
        let baseline = sample_state();
        let expected = compute_state_hash(&baseline);

        let mut cases = Vec::new();

        let mut changed = baseline.clone();
        changed.seed += 1;
        cases.push(("seed", changed));
        let mut changed = baseline.clone();
        changed.scenario_id += 1;
        cases.push(("scenario_id", changed));
        let mut changed = baseline.clone();
        changed.wind_speed += 1;
        cases.push(("wind_speed", changed));
        let mut changed = baseline.clone();
        changed.sequence_clock.insert(ActorId(1), 99);
        cases.push(("sequence_clock", changed));
        let mut changed = baseline.clone();
        changed.active_actor = Some(ActorId(1));
        cases.push(("active_actor", changed));
        let mut changed = baseline.clone();
        changed.active_turn_actions = 1;
        cases.push(("active_turn_actions", changed));
        let mut changed = baseline.clone();
        changed.overwatch.insert(ActorId(1));
        cases.push(("overwatch", changed));
        let mut changed = baseline.clone();
        changed.reaction_points.insert(ActorId(1), 4);
        cases.push(("reaction_points", changed));
        let mut changed = baseline.clone();
        changed.sprinting.insert(ActorId(1));
        cases.push(("sprinting", changed));
        let mut changed = baseline.clone();
        changed.wound_effects.insert(
            ActorId(1),
            crate::state::WoundEffects {
                concussed_turns: 3,
                ..Default::default()
            },
        );
        cases.push(("wound_effects", changed));
        let mut changed = baseline.clone();
        changed.broken_retreat_remaining.insert(ActorId(1), 2);
        cases.push(("broken_retreat_remaining", changed));
        let mut changed = baseline.clone();
        changed
            .pending_explosives
            .push(crate::state::PendingExplosive {
                thrower: ActorId(1),
                position: TileXY::new(2, 2),
                detonate_at: 120,
            });
        cases.push(("pending_explosives", changed));
        let mut changed = baseline.clone();
        changed.held_explosives.insert(
            ActorId(1),
            crate::state::PendingExplosive {
                thrower: ActorId(2),
                position: TileXY::new(2, 2),
                detonate_at: 120,
            },
        );
        cases.push(("held_explosives", changed));
        let mut changed = baseline.clone();
        changed.difficult_tiles.insert(TileXY::new(2, 2));
        cases.push(("difficult_tiles", changed));
        let mut changed = baseline.clone();
        changed.cover_edges.insert(
            crate::state::CoverEdge {
                tile: TileXY::new(2, 2),
                facing: Facing::North,
            },
            crate::state::CoverState {
                level: crate::state::CoverLevel::Hard,
                strikes: 1,
                half_height: false,
                burning: false,
            },
        );
        cases.push(("cover_edges", changed));
        let mut changed = baseline.clone();
        changed.leaders.insert(ActorId(1));
        cases.push(("leaders", changed));
        let mut changed = baseline.clone();
        changed.sand_multiplier_pct.insert(ActorId(1), 80);
        cases.push(("sand_multiplier_pct", changed));
        let mut changed = baseline.clone();
        changed.carry_weight_lbs.insert(ActorId(1), 75);
        cases.push(("carry_weight_lbs", changed));
        let mut changed = baseline.clone();
        changed.armor_damage_resist.insert(ActorId(1), 2);
        cases.push(("armor_damage_resist", changed));
        let mut changed = baseline.clone();
        changed.revealed_until.insert(ActorId(1), 43);
        cases.push(("revealed_until", changed));
        let mut changed = baseline.clone();
        changed.snow_tracks.push((ActorId(1), TileXY::new(3, 3)));
        cases.push(("snow_tracks", changed));
        let mut changed = baseline.clone();
        changed
            .consumed_battle_effects
            .insert((ActorId(1), "draw_and_fire_once".to_string()));
        cases.push(("consumed_battle_effects", changed));
        let mut changed = baseline.clone();
        changed.smoke_grid[0] = 1;
        cases.push(("smoke", changed));

        let mut changed = baseline.clone();
        changed.actors.get_mut(&ActorId(1)).unwrap().max_hp += 1;
        cases.push(("max_hp", changed));
        let mut changed = baseline.clone();
        changed.actors.get_mut(&ActorId(1)).unwrap().max_sand += 1;
        cases.push(("max_sand", changed));
        let mut changed = baseline.clone();
        changed.actors.get_mut(&ActorId(1)).unwrap().stance = Stance::Prone;
        cases.push(("stance", changed));
        let mut changed = baseline.clone();
        changed.actors.get_mut(&ActorId(1)).unwrap().weapon = "other".into();
        cases.push(("weapon", changed));
        let mut changed = baseline.clone();
        changed.actors.get_mut(&ActorId(1)).unwrap().loaded_rounds -= 1;
        cases.push(("loaded_rounds", changed));
        let mut changed = baseline.clone();
        changed.actors.get_mut(&ActorId(1)).unwrap().weapon_capacity += 1;
        cases.push(("weapon_capacity", changed));
        let mut changed = baseline.clone();
        changed.actors.get_mut(&ActorId(1)).unwrap().fouling += 1;
        cases.push(("fouling", changed));
        let mut changed = baseline.clone();
        changed.actors.get_mut(&ActorId(1)).unwrap().jammed = true;
        cases.push(("jammed", changed));
        let mut changed = baseline.clone();
        changed.actors.get_mut(&ActorId(1)).unwrap().progression.xp += 1;
        cases.push(("progression", changed));
        let mut changed = baseline.clone();
        changed
            .actors
            .get_mut(&ActorId(1))
            .unwrap()
            .progression
            .passives
            .insert("fouling_halved".to_string());
        cases.push(("progression_effects", changed));
        let mut changed = baseline.clone();
        changed
            .actors
            .get_mut(&ActorId(1))
            .unwrap()
            .weapon_profile
            .ap_overrides
            .insert("single".to_string(), 4);
        cases.push(("weapon_ap_overrides", changed));

        for (field, state) in cases {
            assert_ne!(
                expected,
                compute_state_hash(&state),
                "state hash omitted {field}"
            );
        }
    }

    #[test]
    fn compute_state_hash_output_is_32_bytes() {
        let state = sample_state();
        assert_eq!(compute_state_hash(&state).len(), 32);
    }
}
