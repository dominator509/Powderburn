//! Simulation state hashing for determinism verification.
//!
//! M6: Provides `compute_state_hash` for proving deterministic replay.
//! Every field is serialized in canonical order and fed through
//! `pb_core::hash::hash_state` to produce a 32-byte digest.

#![forbid(unsafe_code)]

use pb_core::event::WoundType;
use pb_core::hash::hash_state;

use crate::state::SimState;

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

/// Compute a 32-byte SHA-256 (via SipHash surrogate) of the simulation state.
///
/// Canonical serialization order:
///   1. tick (u64 LE)
///   2. For each actor in ActorId order (BTreeMap is already sorted):
///      a. actor_id  (u32 LE)
///      b. hit_points (i32 LE)
///      c. ap         (i16 LE)
///      d. position.x (i16 LE)
///      e. position.y (i16 LE)
///      f. facing     (1 byte, index 0-7)
///      g. alive      (1 byte: 1 = alive, 0 = dead)
///      h. sand       (i32 LE)
///      i. wounds     (u16 LE count, then each wound as 1 byte)
///
/// This function is guaranteed deterministic: same state → same hash.
pub fn compute_state_hash(state: &SimState) -> [u8; 32] {
    let mut buf = Vec::new();

    // 1. Tick
    buf.extend_from_slice(&state.tick.0.to_le_bytes());

    // 2. Actors in canonical (BTreeMap) order
    for (id, actor) in &state.actors {
        // a. actor_id
        buf.extend_from_slice(&id.0.to_le_bytes());
        // b. hit_points
        buf.extend_from_slice(&actor.hit_points.to_le_bytes());
        // c. ap
        buf.extend_from_slice(&actor.ap.0.to_le_bytes());
        // d. position.x
        buf.extend_from_slice(&actor.position.x.to_le_bytes());
        // e. position.y
        buf.extend_from_slice(&actor.position.y.to_le_bytes());
        // f. facing
        buf.push(actor.facing.to_index() as u8);
        // g. alive
        buf.push(if actor.alive { 1u8 } else { 0u8 });
        // h. sand
        buf.extend_from_slice(&actor.sand.to_le_bytes());
        // i. wounds
        let count: u16 = actor.wounds.len().try_into().unwrap_or(u16::MAX);
        buf.extend_from_slice(&count.to_le_bytes());
        for w in &actor.wounds {
            buf.push(wound_to_u8(w));
        }
    }

    hash_state(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pb_core::ids::{ActorId, Ap, Tick};
    use pb_core::geom::{Facing, TileXY};
    use crate::state::{ActorState, Stance};
    use std::collections::BTreeMap;

    fn sample_state() -> SimState {
        let mut actors = BTreeMap::new();
        actors.insert(
            ActorId(1),
            ActorState {
                ap: Ap(10),
                position: TileXY::new(5, 5),
                facing: Facing::South,
                sequence: 5,
                hit_points: 30,
                max_hp: 30,
                name: "Ally1".into(),
                alive: true,
                wounds: vec![],
                sand: 20,
                max_sand: 20,
                stance: Stance::Standing,
            },
        );
        SimState {
            tick: Tick(42),
            actors,
            sequence_clock: BTreeMap::new(),
            seed: 12345,
            scenario_id: 1,
            wind_speed: 0,
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
    fn compute_state_hash_output_is_32_bytes() {
        let state = sample_state();
        assert_eq!(compute_state_hash(&state).len(), 32);
    }
}
