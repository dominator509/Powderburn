//! Integration tests for the environment system.
//!
//! Tests two actors exchanging fire in a corridor for 400 ticks, accumulating
//! smoke until a shot becomes illegal. Also tests cover degradation.

use pb_core::geom::{Facing, TileXY};
use pb_sim::environment::{
    can_see_through_smoke, cover_accuracy_penalty, decay_smoke, deposit_smoke, smoke_penalty,
    SmokeSystem, Cover,
};

/// Two actors exchange fire in a corridor. After 400 ticks of accumulated
/// smoke, shots become illegal due to smoke blockage.
#[test]
fn corridor_fire_accumulates_smoke_until_blocked() {
    let mut sys = SmokeSystem::new();
    let shooter = TileXY::new(0, 5);
    let target = TileXY::new(10, 5);
    let facing = Facing::East;

    // Initially, LOS is clear
    assert!(can_see_through_smoke(&sys, shooter, target));
    assert_eq!(smoke_penalty(&sys, shooter, target), 0);

    // Simulate 10 shots: each deposits smoke at muzzle and two tiles in front.
    // No decay between shots so smoke accumulates.
    for _ in 0..10 {
        deposit_smoke(shooter, facing, &mut sys);
    }

    // After many shots, smoke should block the line.
    // Front tiles (1,5) and (2,5) each have density 10.
    // Accumulated: 10 + 10 = 20 >= 6 → illegal.
    let penalty = smoke_penalty(&sys, shooter, target);
    assert_eq!(
        penalty,
        i32::MAX,
        "After 10 shots without decay, smoke penalty should block LOS, got {}",
        penalty
    );
    assert!(!can_see_through_smoke(&sys, shooter, target));
}

/// After a very long time without shooting, smoke clears and LOS is restored.
#[test]
fn smoke_clears_over_time_restoring_los() {
    let mut sys = SmokeSystem::new();
    let shooter = TileXY::new(0, 0);
    let target = TileXY::new(10, 0);

    // Deposit a moderate amount of smoke
    for _ in 0..5 {
        deposit_smoke(shooter, Facing::East, &mut sys);
    }

    // Should be blocked initially
    // Front tiles (1,0) and (2,0) each have density 5.
    // Accumulated: 5 + 5 = 10 >= 6 → blocked
    let initial_penalty = smoke_penalty(&sys, shooter, target);
    assert_eq!(initial_penalty, i32::MAX);

    // Decay for a long time: 480 ticks = 12 decay steps
    // Each front tile: 5 - 12 = 0 (removed)
    decay_smoke(&mut sys, 480);

    // After extensive decay, smoke should have cleared
    let final_penalty = smoke_penalty(&sys, shooter, target);
    assert_eq!(
        final_penalty, 0,
        "After 480 ticks of decay, smoke should clear, penalty={}",
        final_penalty
    );
    assert!(can_see_through_smoke(&sys, shooter, target));
}

/// Verify that smoke penalty affects a direct corridor with multiple
/// deposition points.
#[test]
fn smoke_penalty_along_corridor_blocks_at_threshold() {
    let mut sys = SmokeSystem::new();
    let shooter = TileXY::new(0, 0);
    let target = TileXY::new(6, 0);

    // Place smoke at tiles 1, 2, 3 along the LOS
    for x in 1..=3 {
        let tile = TileXY::new(x, 0);
        deposit_smoke(tile, Facing::East, &mut sys);
    }

    // Each tile has density 3 from deposit_smoke (muzzle). So tiles 1,2,3
    // each have density 3, accumulated = 9 >= 6 → illegal
    let penalty = smoke_penalty(&sys, shooter, target);
    assert_eq!(penalty, i32::MAX);
    assert!(!can_see_through_smoke(&sys, shooter, target));
}

/// Cover accuracy penalty tests (complementing unit tests).
#[test]
fn cover_penalty_values_correct() {
    assert_eq!(cover_accuracy_penalty(Cover::None), 0);
    assert_eq!(cover_accuracy_penalty(Cover::Soft), 15);
    assert_eq!(cover_accuracy_penalty(Cover::Hard), 30);
    assert_eq!(cover_accuracy_penalty(Cover::Full), i32::MAX);
}

/// Smoke drifts in a consistent direction over time.
#[test]
fn smoke_drift_accumulates_in_wind_direction() {
    use pb_sim::environment::drift_smoke;

    let mut sys = SmokeSystem::new();
    let origin = TileXY::new(5, 5);

    // Deposit smoke at origin
    deposit_smoke(origin, Facing::East, &mut sys);

    // Drift east for 240 ticks (2 tiles)
    drift_smoke(&mut sys, Facing::East, 240);

    // Smoke should now be 2 tiles east
    let drifted_x = 5 + 2;
    assert_eq!(sys.density_at(TileXY::new(drifted_x, 5)), 3);
    assert_eq!(sys.density_at(origin), 0);
}
