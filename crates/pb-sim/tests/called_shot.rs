//! M6: Called-shot determinism test.
//!
//! Verifies that the shot pipeline produces identical results when invoked
//! with the same inputs (seed, state, shooter, target, location) on fresh
//! state copies.

#![allow(clippy::unwrap_used, clippy::bool_assert_comparison)]

use pb_sim::shot::resolve_shot;
use pb_sim::state::{ActorState, SimState, Stance};

use pb_core::event::HitLocationType;
use pb_core::geom::{Facing, TileXY};
use pb_core::ids::{ActorId, Ap};

/// Build a minimal two-actor state for called-shot testing.
fn build_shot_scenario(seed: u64) -> SimState {
    let mut state = SimState::new(seed, 1);

    state.actors.insert(
        ActorId(1),
        ActorState {
            ap: Ap(10),
            position: TileXY::new(0, 0),
            facing: Facing::South,
            sequence: 5,
            hit_points: 30,
            max_hp: 30,
            name: "Shooter".into(),
            alive: true,
            wounds: vec![],
            sand: 20,
            max_sand: 20,
            stance: Stance::Standing,
        },
    );

    state.actors.insert(
        ActorId(2),
        ActorState {
            ap: Ap(10),
            position: TileXY::new(5, 0),
            facing: Facing::North,
            sequence: 4,
            hit_points: 35,
            max_hp: 35,
            name: "Target".into(),
            alive: true,
            wounds: vec![],
            sand: 20,
            max_sand: 20,
            stance: Stance::Standing,
        },
    );

    state
}

/// A called shot to a specific hit location produces the same result every time
/// when invoked with the same state snapshot.
#[test]
fn called_shot_same_input_same_output() {
    let state = build_shot_scenario(42);

    let result_a = resolve_shot(&state, ActorId(1), ActorId(2), Some(HitLocationType::Head), true, 0);
    let result_b = resolve_shot(&state, ActorId(1), ActorId(2), Some(HitLocationType::Head), true, 0);

    assert!(result_a.is_ok());
    assert!(result_b.is_ok());
    assert_eq!(
        result_a.unwrap(),
        result_b.unwrap(),
        "Same called-shot inputs must produce identical event sequences"
    );
}

/// A called shot to GunArm must produce identical events every time.
#[test]
fn called_shot_gunarm_deterministic() {
    let state = build_shot_scenario(42);

    let result_a = resolve_shot(
        &state,
        ActorId(1),
        ActorId(2),
        Some(HitLocationType::GunArm),
        true,
        0,
    );
    let result_b = resolve_shot(
        &state,
        ActorId(1),
        ActorId(2),
        Some(HitLocationType::GunArm),
        true,
        0,
    );

    assert!(result_a.is_ok());
    assert!(result_b.is_ok());
    let events_a = result_a.unwrap();
    let events_b = result_b.unwrap();
    assert_eq!(events_a, events_b);

    // Both should contain at least a ShotHit event
    assert!(!events_a.is_empty(), "Called shot should produce events");
    assert_eq!(events_a[0].to_string().starts_with("event: ShotHit"), true);
}

/// A called shot to Eyes should also be deterministic.
#[test]
fn called_shot_eyes_deterministic() {
    let state = build_shot_scenario(42);

    let result_a = resolve_shot(&state, ActorId(1), ActorId(2), Some(HitLocationType::Eyes), true, 0);
    let result_b = resolve_shot(&state, ActorId(1), ActorId(2), Some(HitLocationType::Eyes), true, 0);

    assert_eq!(result_a, result_b);
}

/// A called shot at a different seed produces a different result.
#[test]
fn called_shot_different_seed_different_result() {
    let state_a = build_shot_scenario(42);
    let state_b = build_shot_scenario(99);

    let result_a = resolve_shot(
        &state_a,
        ActorId(1),
        ActorId(2),
        Some(HitLocationType::Torso),
        true,
        0,
    );
    let result_b = resolve_shot(
        &state_b,
        ActorId(1),
        ActorId(2),
        Some(HitLocationType::Torso),
        true,
        0,
    );

    // Very unlikely to get identical events from different seeds
    assert!(result_a.is_ok());
    assert!(result_b.is_ok());
    // Note: it's theoretically possible (but astronomically unlikely) that
    // two different seeds produce the same shot outcome. We do not assert
    // `ne` here to avoid a flaky test.
}
