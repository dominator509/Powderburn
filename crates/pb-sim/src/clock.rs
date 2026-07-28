//! Sequence clock and AP economy.
//!
//! M2: Determines which actor acts next, grants action points,
//! and manages the turn sequence.

#![forbid(unsafe_code)]

use crate::action::update_alive_and_xp;
use crate::state::{ActorState, SimState, Stance};
use pb_core::event::HitLocationType;
use pb_core::geom::{Facing, TileXY};
use pb_core::ids::{ActorId, Ap, Tick};

/// Compute the turn length (in ticks) for an actor with the given sequence.
///
/// Formula: `clamp(100 - 4*seq, 50, 96)`
pub fn turn_length(sequence: i32) -> u64 {
    let raw = 100 - 4 * sequence;
    let clamped = raw.clamp(50, 96);
    clamped as u64
}

/// Compute the action points granted when an actor takes a turn.
///
/// Formula: `5 + floor(WIND / 2)`, clamped to [5, 10].
pub fn grant_ap(wind_attribute: i32) -> Ap {
    let raw = 5 + wind_attribute / 2;
    let clamped = raw.clamp(5, 10);
    Ap(clamped as i16)
}

/// Advance the sequence clock to the next acting actor.
///
/// Returns `Some(actor_id)` if an actor is ready to act, or `None` if
/// there are no actors in the simulation.
///
/// Selection rules:
/// 1. Find the actor with the smallest `next_act_at`.
/// 2. Break ties by Sequence descending, then ActorId ascending.
/// 3. Set `state.tick` to that actor's `next_act_at`.
/// 4. Grant fresh AP based on WIND and active wound/encumbrance effects.
/// 5. Schedule the actor's next turn: `next_act_at += turn_length`.
pub fn advance_to_next_actor(state: &mut SimState) -> Option<ActorId> {
    if state.actors.is_empty() {
        return None;
    }

    if let Some(actor_id) = state.active_actor {
        if state
            .actors
            .get(&actor_id)
            .is_some_and(|actor| actor.alive && !actor.routed && actor.ap.0 > 0)
        {
            return Some(actor_id);
        }
        state.active_actor = None;
    }

    // Find the actor with the smallest next_act_at, applying tie-breakers.
    let selected = state
        .sequence_clock
        .iter()
        .filter(|(id, _)| {
            state
                .actors
                .get(id)
                .is_some_and(|actor| actor.alive && !actor.routed)
        })
        .min_by(|(id_a, tick_a), (id_b, tick_b)| {
            // Primary: smallest next_act_at
            tick_a
                .cmp(tick_b)
                // Secondary: Sequence descending
                .then_with(|| {
                    let seq_a = state.actors.get(id_a).map(|a| a.sequence).unwrap_or(0);
                    let seq_b = state.actors.get(id_b).map(|a| a.sequence).unwrap_or(0);
                    seq_b.cmp(&seq_a)
                })
                // Tertiary: ActorId ascending
                .then_with(|| id_a.cmp(id_b))
        })
        .map(|(id, _)| *id);

    let actor_id = selected?;
    state.sprinting.remove(&actor_id);
    state.overwatch.remove(&actor_id);
    state.reaction_points.remove(&actor_id);

    // Set tick to the actor's next_act_at
    let next_tick = state.sequence_clock.get(&actor_id).copied().unwrap_or(0);
    state.tick = Tick(next_tick);

    // Ordinary unspent AP is lost. Only DrawBead can deliberately convert up
    // to four AP into reaction points.
    let fresh_ap = state
        .actors
        .get(&actor_id)
        .map(|actor| grant_ap(actor.attributes.wind))?;
    let (grit, ap_bonus) = state.actors.get(&actor_id).map(|actor| {
        (
            actor.attributes.grit,
            actor.progression.effect_value("ap_bonus"),
        )
    })?;
    let effects = state
        .wound_effects
        .get(&actor_id)
        .cloned()
        .unwrap_or_default();
    let concussion_penalty = i16::from(effects.concussed_turns > 0) * 3;
    let winded_penalty = i16::from(effects.winded) * 2;
    let carry_capacity = 25 + grit * 5;
    let carried = state.carry_weight_lbs.get(&actor_id).copied().unwrap_or(0);
    let overload = carried.saturating_sub(carry_capacity);
    let overload_penalty = if overload > 0 {
        ((overload + 9) / 10).clamp(0, i32::from(i16::MAX)) as i16
    } else {
        0
    };
    let mark_bonus = ap_bonus.clamp(0, i32::from(i16::MAX)) as i16;
    let granted =
        (fresh_ap.0 + mark_bonus - concussion_penalty - winded_penalty - overload_penalty).max(0);
    let actor = state.actors.get_mut(&actor_id)?;
    actor.ap = Ap(granted);
    if effects
        .broken_locations
        .get(&HitLocationType::Legs)
        .copied()
        .unwrap_or(0)
        >= 2
    {
        actor.stance = Stance::Prone;
    }

    if let Some(effects) = state.wound_effects.get_mut(&actor_id) {
        effects.concussed_turns = effects.concussed_turns.saturating_sub(1);
        effects.winded = false;
    }
    if crate::morale::morale_state(actor.sand, actor.max_sand) == crate::morale::MoraleState::Broken
    {
        state.broken_retreat_remaining.insert(actor_id, 2);
    } else {
        state.broken_retreat_remaining.remove(&actor_id);
    }
    state
        .revealed_until
        .retain(|_, until| *until >= state.tick.0);

    // Schedule next turn
    let tl = turn_length(actor.sequence);
    state
        .sequence_clock
        .entry(actor_id)
        .and_modify(|t| *t = next_tick.wrapping_add(tl));
    state.active_actor = Some(actor_id);
    state.active_turn_actions = 0;

    // Update alive actor count and XP total gauges after each advance
    update_alive_and_xp(state);

    Some(actor_id)
}

/// Build a new `ActorState` with the given parameters.
pub fn build_actor(
    _actor_id: ActorId,
    name: &str,
    seq: i32,
    hp: i32,
    sand: i32,
    pos: TileXY,
) -> ActorState {
    ActorState {
        faction_id: String::new(),
        is_companion: false,
        attributes: pb_core::Attributes::BALANCED,
        ap: Ap(0),
        position: pos,
        facing: Facing::South,
        sequence: seq,
        hit_points: hp,
        max_hp: hp,
        name: name.to_string(),
        alive: true,
        routed: false,
        wounds: Vec::new(),
        sand,
        max_sand: sand,
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

/// Register an actor in the simulation state, initializing its sequence clock.
pub fn register_actor(state: &mut SimState, actor_id: ActorId, actor: ActorState) {
    state.sequence_clock.insert(actor_id, 0);
    state.actors.insert(actor_id, actor);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{step, Action, Command};
    use pb_core::ids::ActorId;

    fn hold(state: &mut SimState, actor_id: ActorId) {
        let result = step(
            state,
            Command {
                actor_id,
                action: Action::Hold,
            },
        );
        assert!(result.is_ok(), "active actor can Hold: {result:?}");
    }

    #[test]
    fn turn_length_calculation() {
        assert_eq!(turn_length(2), 92);
        assert_eq!(turn_length(5), 80);
        assert_eq!(turn_length(9), 64);
        // Clamp lower bound
        assert_eq!(turn_length(20), 50);
        // Clamp upper bound (seq < 1 gives >96 but we clamp)
        assert_eq!(turn_length(0), 96);
    }

    #[test]
    fn turn_length_clamps() {
        assert_eq!(turn_length(-10), 96); // 100-(-40)=140 clamp to 96
        assert_eq!(turn_length(13), 50); // 100-52=48 clamp to 50
    }

    #[test]
    fn grant_ap_default() {
        assert_eq!(grant_ap(0), Ap(5));
    }

    #[test]
    fn grant_ap_with_wind() {
        assert_eq!(grant_ap(2), Ap(6)); // 5 + 2/2 = 6
        assert_eq!(grant_ap(4), Ap(7)); // 5 + 4/2 = 7
        assert_eq!(grant_ap(10), Ap(10)); // 5 + 10/2 = 10
    }

    #[test]
    fn grant_ap_clamp_low() {
        assert_eq!(grant_ap(-10), Ap(5)); // clamped to min 5
    }

    #[test]
    fn grant_ap_clamp_high() {
        assert_eq!(grant_ap(20), Ap(10)); // clamped to max 10
    }

    #[test]
    fn build_actor_creates_valid_state() {
        let id = ActorId(1);
        let actor = build_actor(id, "TestGuy", 5, 20, 10, TileXY::new(5, 5));
        assert_eq!(actor.name, "TestGuy");
        assert_eq!(actor.sequence, 5);
        assert_eq!(actor.hit_points, 20);
        assert_eq!(actor.max_hp, 20);
        assert_eq!(actor.sand, 10);
        assert_eq!(actor.position, TileXY::new(5, 5));
        assert_eq!(actor.ap, Ap(0));
        assert!(actor.alive);
        assert_eq!(actor.stance, Stance::Standing);
    }

    #[test]
    fn advance_to_next_actor_empty_state() {
        let mut state = SimState::new(42, 1);
        assert_eq!(advance_to_next_actor(&mut state), None);
    }

    #[test]
    fn advance_to_next_actor_grants_ap_and_advances_tick() {
        let mut state = SimState::new(42, 1);
        let id = ActorId(1);
        let actor = build_actor(id, "Quick", 5, 20, 10, TileXY::new(0, 0));
        register_actor(&mut state, id, actor);

        let result = advance_to_next_actor(&mut state);
        assert_eq!(result, Some(id));
        assert_eq!(state.tick, Tick(0));
        assert_eq!(state.actors[&id].ap, Ap(8));
    }

    #[test]
    fn advance_advances_tick_by_turn_length() {
        let mut state = SimState::new(42, 1);
        let id = ActorId(1);
        let actor = build_actor(id, "Slow", 2, 20, 10, TileXY::new(0, 0));
        register_actor(&mut state, id, actor);

        advance_to_next_actor(&mut state); // tick 0
        assert_eq!(state.tick, Tick(0));
        hold(&mut state, id);

        advance_to_next_actor(&mut state); // tick 92 (seq 2)
        assert_eq!(state.tick, Tick(92));
    }

    #[test]
    fn tie_break_by_sequence_desc_then_actor_id_asc() {
        let mut state = SimState::new(42, 1);
        let id_a = ActorId(1);
        let id_b = ActorId(2);
        let id_c = ActorId(3);

        // All start at next_act_at=0
        // Seq 5, 5, 9 — Seq 9 goes first, then Seq 5 tie-break: ActorId 1 < ActorId 2
        let actor_a = build_actor(id_a, "Alpha", 5, 20, 10, TileXY::new(0, 0));
        let actor_b = build_actor(id_b, "Beta", 5, 20, 10, TileXY::new(1, 0));
        let actor_c = build_actor(id_c, "Gamma", 9, 20, 10, TileXY::new(2, 0));

        register_actor(&mut state, id_a, actor_a);
        register_actor(&mut state, id_b, actor_b);
        register_actor(&mut state, id_c, actor_c);

        // First: Seq 9 (Gamma)
        assert_eq!(advance_to_next_actor(&mut state), Some(id_c));
        hold(&mut state, id_c);
        // Second: Seq 5, ActorId 1 (Alpha) because ActorId 1 < ActorId 2
        assert_eq!(advance_to_next_actor(&mut state), Some(id_a));
        hold(&mut state, id_a);
        // Third: ActorId 2 (Beta)
        assert_eq!(advance_to_next_actor(&mut state), Some(id_b));
    }

    #[test]
    fn active_actor_keeps_the_tick_until_hold() {
        let mut state = SimState::new(42, 1);
        let first = ActorId(1);
        let second = ActorId(2);
        register_actor(
            &mut state,
            first,
            build_actor(first, "First", 5, 20, 10, TileXY::new(0, 0)),
        );
        register_actor(
            &mut state,
            second,
            build_actor(second, "Second", 4, 20, 10, TileXY::new(1, 0)),
        );

        assert_eq!(advance_to_next_actor(&mut state), Some(first));
        let initial_ap = state.actors[&first].ap;
        let result = step(
            &mut state,
            Command {
                actor_id: first,
                action: Action::StanceCrouch,
            },
        );
        assert!(result.is_ok(), "first action failed: {result:?}");
        assert_eq!(advance_to_next_actor(&mut state), Some(first));
        assert_eq!(state.tick, Tick(0));
        assert_eq!(state.actors[&first].ap, Ap(initial_ap.0 - 1));

        hold(&mut state, first);
        assert_eq!(advance_to_next_actor(&mut state), Some(second));
        assert_eq!(state.tick, Tick(0));
    }
}
