//! Integration tests for the morale system.
//!
//! Tests Sand loss/gain values, state transitions, and
//! morale-based accuracy penalties.

use pb_sim::morale::{
    morale_accuracy_penalty, morale_state, sand_gain_for_event, sand_loss_for_event, MoraleState,
};

/// Verify all Sand loss values are correct.
#[test]
fn sand_loss_values() {
    assert_eq!(sand_loss_for_event("shot_at_missed"), 2);
    assert_eq!(sand_loss_for_event("shot_at_hit"), 4);
    assert_eq!(sand_loss_for_event("shot_at_critical"), 6);
    assert_eq!(sand_loss_for_event("adjacent_ally_killed"), 8);
    assert_eq!(sand_loss_for_event("any_ally_killed"), 3);
    assert_eq!(sand_loss_for_event("leader_falls"), 10);
    assert_eq!(sand_loss_for_event("nonexistent_event"), 0);
}

/// Verify all Sand gain values are correct.
#[test]
fn sand_gain_values() {
    assert_eq!(sand_gain_for_event("kill_enemy"), 3);
    assert_eq!(sand_gain_for_event("rally"), 5);
    assert_eq!(sand_gain_for_event("nonexistent_event"), 0);
}

/// State transitions: sand decreasing from Steady to Routed.
#[test]
fn morale_state_transitions_descending() {
    let max_sand = 100;

    // Start Steady (> 60%)
    assert_eq!(morale_state(80, max_sand), MoraleState::Steady);
    assert_eq!(morale_state(61, max_sand), MoraleState::Steady);

    // Rattled (25-60%)
    assert_eq!(morale_state(60, max_sand), MoraleState::Rattled);
    assert_eq!(morale_state(40, max_sand), MoraleState::Rattled);
    assert_eq!(morale_state(26, max_sand), MoraleState::Rattled);

    // Broken (< 25%)
    assert_eq!(morale_state(25, max_sand), MoraleState::Broken);
    assert_eq!(morale_state(10, max_sand), MoraleState::Broken);
    assert_eq!(morale_state(1, max_sand), MoraleState::Broken);

    // Routed (0)
    assert_eq!(morale_state(0, max_sand), MoraleState::Routed);
}

/// Sand recovery can move an actor back up through states.
#[test]
fn morale_state_transitions_ascending() {
    let max_sand = 100;

    // Start from 0 (Routed)
    assert_eq!(morale_state(0, max_sand), MoraleState::Routed);

    // Gain to 5 (still Broken since 5 <= 25)
    assert_eq!(morale_state(5, max_sand), MoraleState::Broken);

    // Gain to 30 (Rattled since 25 < 30 <= 60)
    assert_eq!(morale_state(30, max_sand), MoraleState::Rattled);

    // Gain to 70 (Steady since 70 > 60)
    assert_eq!(morale_state(70, max_sand), MoraleState::Steady);
}

/// Different max_sand values produce correct thresholds.
#[test]
fn morale_state_different_max_sand() {
    // With max_sand = 50:
    // Steady: > 30, Rattled: 13-30, Broken: 1-12, Routed: 0
    assert_eq!(morale_state(31, 50), MoraleState::Steady);
    assert_eq!(morale_state(30, 50), MoraleState::Rattled);
    assert_eq!(morale_state(13, 50), MoraleState::Rattled);
    assert_eq!(morale_state(12, 50), MoraleState::Broken);
    assert_eq!(morale_state(1, 50), MoraleState::Broken);
    assert_eq!(morale_state(0, 50), MoraleState::Routed);

    // With max_sand = 20:
    // Steady: > 12, Rattled: 6-12, Broken: 1-5, Routed: 0
    assert_eq!(morale_state(13, 20), MoraleState::Steady);
    assert_eq!(morale_state(12, 20), MoraleState::Rattled);
    // 5/20 = exactly 25%, not > 25%, so Broken
    assert_eq!(morale_state(5, 20), MoraleState::Broken);
    assert_eq!(morale_state(4, 20), MoraleState::Broken);
    assert_eq!(morale_state(1, 20), MoraleState::Broken);
    assert_eq!(morale_state(0, 20), MoraleState::Routed);
}

/// Morale accuracy penalties at each state.
#[test]
fn morale_penalty_values() {
    assert_eq!(morale_accuracy_penalty(MoraleState::Steady), 0);
    assert_eq!(morale_accuracy_penalty(MoraleState::Rattled), -10);
    assert_eq!(morale_accuracy_penalty(MoraleState::Broken), -25);
    assert_eq!(morale_accuracy_penalty(MoraleState::Routed), 0);
}

/// Model a combat scenario: Sand loss from being shot at and gaining
/// Sand from killing an enemy.
#[test]
fn combat_sand_scenario() {
    let mut sand = 100i32;
    let max_sand = 100;

    // Initial state: Steady
    assert_eq!(morale_state(sand, max_sand), MoraleState::Steady);

    // Shot at and missed: lose 2 Sand
    sand = (sand - sand_loss_for_event("shot_at_missed")).max(0);
    assert_eq!(sand, 98);
    assert_eq!(morale_state(sand, max_sand), MoraleState::Steady);

    // Shot at and hit: lose 4 Sand
    sand = (sand - sand_loss_for_event("shot_at_hit")).max(0);
    assert_eq!(sand, 94);

    // Adjacent ally killed: lose 8 Sand
    sand = (sand - sand_loss_for_event("adjacent_ally_killed")).max(0);
    assert_eq!(sand, 86);

    // Still Steady
    assert_eq!(morale_state(sand, max_sand), MoraleState::Steady);

    // Multiple hits push to Rattled
    for _ in 0..5 {
        sand = (sand - sand_loss_for_event("shot_at_hit")).max(0);
    }
    // 86 - 20 = 66
    assert_eq!(sand, 66);
    // 66 > 60% = Steady still... let's push more
    sand = (sand - sand_loss_for_event("shot_at_hit")).max(0);
    // 66 - 4 = 62
    assert_eq!(sand, 62);
    // Still Steady (62 > 60)
    sand = (sand - sand_loss_for_event("shot_at_hit")).max(0);
    // 62 - 4 = 58
    assert_eq!(sand, 58);
    // Now Rattled (25 < 58 <= 60)
    assert_eq!(morale_state(sand, max_sand), MoraleState::Rattled);

    // Kill an enemy: gain 3 Sand
    sand += sand_gain_for_event("kill_enemy");
    assert_eq!(sand, 61);
    // Back to Steady
    assert_eq!(morale_state(sand, max_sand), MoraleState::Steady);

    // Leader falls: lose 10 Sand → 51 → still Rattled
    sand = (sand - sand_loss_for_event("leader_falls")).max(0);
    assert_eq!(sand, 51);
    assert_eq!(morale_state(sand, max_sand), MoraleState::Rattled);

    // Rally: gain 5 Sand → 56 → still Rattled
    sand += sand_gain_for_event("rally");
    assert_eq!(sand, 56);
    assert_eq!(morale_state(sand, max_sand), MoraleState::Rattled);
}

/// Sand at exactly boundary values produces correct states.
#[test]
fn morale_state_boundary_values() {
    // max_sand = 100
    assert_eq!(morale_state(60, 100), MoraleState::Rattled); // exactly 60% is Rattled
    assert_eq!(morale_state(25, 100), MoraleState::Broken); // exactly 25% is Broken

    // max_sand = 10
    // Steady: > 6 (60%). Rattled: 3-6. Broken: 1-2. Routed: 0.
    // Note: 6/10 = exactly 60% in Fix32, not > 60%, so Rattled.
    assert_eq!(morale_state(6, 10), MoraleState::Rattled);
    assert_eq!(morale_state(7, 10), MoraleState::Steady); // 7/10 = 70% > 60%
    assert_eq!(morale_state(3, 10), MoraleState::Rattled); // 3/10 = 30%, > 25%
    assert_eq!(morale_state(2, 10), MoraleState::Broken); // 2/10 = 20%, <= 25%
    assert_eq!(morale_state(1, 10), MoraleState::Broken); // 1/10 = 10%, <= 25%
    assert_eq!(morale_state(0, 10), MoraleState::Routed);
}
