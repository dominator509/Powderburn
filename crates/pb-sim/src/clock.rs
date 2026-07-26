//! Sequence clock and AP economy.
//!
//! M2: Determines which actor acts next, grants action points,
//! and manages the turn sequence.

#![forbid(unsafe_code)]

use crate::state::{ActorState, SimState, Stance};
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
/// Formula: `5 + floor(wind_speed / 2)`, clamped to [5, 10].
pub fn grant_ap(wind_speed: i32) -> Ap {
    let raw = 5 + wind_speed / 2;
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
/// 4. Grant AP (based on wind speed) with carry-over from the previous
///    turn (up to 4 AP capped).
/// 5. Schedule the actor's next turn: `next_act_at += turn_length`.
pub fn advance_to_next_actor(state: &mut SimState) -> Option<ActorId> {
    if state.actors.is_empty() {
        return None;
    }

    // Find the actor with the smallest next_act_at, applying tie-breakers.
    let selected = state
        .sequence_clock
        .iter()
        .filter(|(id, _)| state.actors.get(id).is_some_and(|a| a.alive))
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

    // Set tick to the actor's next_act_at
    let next_tick = state.sequence_clock.get(&actor_id).copied().unwrap_or(0);
    state.tick = Tick(next_tick);

    // Grant AP with carry-over (up to 4 previous AP carry forward, capped at max AP)
    let fresh_ap = grant_ap(state.wind_speed);
    let actor = state.actors.get_mut(&actor_id)?;

    // Carry forward: the remaining AP from previous turn, up to 4
    let carry = actor.ap.0.clamp(0, 4);
    let total_ap = fresh_ap.0 + carry;
    let capped_ap = total_ap.min(fresh_ap.0 + 4).max(fresh_ap.0);
    actor.ap = Ap(capped_ap);

    // Schedule next turn
    let tl = turn_length(actor.sequence);
    state
        .sequence_clock
        .entry(actor_id)
        .and_modify(|t| *t = next_tick.wrapping_add(tl));

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
        ap: Ap(0),
        position: pos,
        facing: Facing::South,
        sequence: seq,
        hit_points: hp,
        max_hp: hp,
        name: name.to_string(),
        alive: true,
        wounds: Vec::new(),
        sand,
        max_sand: sand,
        stance: Stance::Standing,
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
    use pb_core::ids::ActorId;

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
        assert_eq!(state.actors[&id].ap, Ap(5));
    }

    #[test]
    fn advance_advances_tick_by_turn_length() {
        let mut state = SimState::new(42, 1);
        let id = ActorId(1);
        let actor = build_actor(id, "Slow", 2, 20, 10, TileXY::new(0, 0));
        register_actor(&mut state, id, actor);

        advance_to_next_actor(&mut state); // tick 0
        assert_eq!(state.tick, Tick(0));

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
        // Second: Seq 5, ActorId 1 (Alpha) because ActorId 1 < ActorId 2
        assert_eq!(advance_to_next_actor(&mut state), Some(id_a));
        // Third: ActorId 2 (Beta)
        assert_eq!(advance_to_next_actor(&mut state), Some(id_b));
    }
}
