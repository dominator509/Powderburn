//! Morale system: Sand pool management and morale states.
//!
//! M4: Tracks Sand (morale resource), provides functions for Sand loss and
//! gain from events, determines MoraleState, and applies accuracy penalties
//! based on morale.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use pb_core::fix32::Fix32;

/// The morale state of an actor based on their current Sand relative to max.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoraleState {
    /// > 60% of max Sand: normal combat effectiveness.
    Steady,
    /// 25-60% of max Sand: shaken, accuracy penalty.
    Rattled,
    /// < 25% of max Sand: severely shaken, large accuracy penalty.
    Broken,
    /// 0 Sand: has fled or is combat-ineffective.
    Routed,
}

/// Return the Sand loss for a given event type string.
///
/// Loss values (negative morale impact):
/// - "shot_at_missed": 2
/// - "shot_at_hit": 4
/// - "shot_at_critical": 6
/// - "adjacent_ally_killed": 8
/// - "any_ally_killed": 3
/// - "leader_falls": 10
///
/// Unknown event types return 0.
pub fn sand_loss_for_event(event_type: &str) -> i32 {
    match event_type {
        "shot_at_missed" => 2,
        "shot_at_hit" => 4,
        "shot_at_critical" => 6,
        "adjacent_ally_killed" => 8,
        "any_ally_killed" => 3,
        "leader_falls" => 10,
        _ => 0,
    }
}

/// Return the Sand gain for a given event type string.
///
/// Gain values (positive morale boost):
/// - "kill_enemy": 3
/// - "rally": 5
///
/// Unknown event types return 0.
pub fn sand_gain_for_event(event_type: &str) -> i32 {
    match event_type {
        "kill_enemy" => 3,
        "rally" => 5,
        _ => 0,
    }
}

/// Determine the morale state based on current Sand and maximum Sand.
///
/// States:
/// - `Steady`: sand > 60% of max_sand
/// - `Rattled`: 25% < sand <= 60%
/// - `Broken`: sand > 0 and sand <= 25%
/// - `Routed`: sand == 0
///
/// If `max_sand` is 0, returns `Steady` (degenerate case).
pub fn morale_state(sand: i32, max_sand: i32) -> MoraleState {
    if sand <= 0 {
        return MoraleState::Routed;
    }
    if max_sand <= 0 {
        return MoraleState::Steady;
    }

    // Compute percentage thresholds using Fix32 for deterministic integer math
    let sand_f = Fix32::from_int(sand);
    let max_f = Fix32::from_int(max_sand);
    let ratio = sand_f / max_f;

    let sixty_pct = Fix32::from_int(60) / Fix32::from_int(100);
    let twentyfive_pct = Fix32::from_int(25) / Fix32::from_int(100);

    if ratio > sixty_pct {
        MoraleState::Steady
    } else if ratio > twentyfive_pct {
        MoraleState::Rattled
    } else {
        MoraleState::Broken
    }
}

/// Return the accuracy penalty for a given morale state.
///
/// - `Steady`: 0
/// - `Rattled`: -10
/// - `Broken`: -25
/// - `Routed`: 0 (no actions possible, the actor has fled)
pub fn morale_accuracy_penalty(state: MoraleState) -> i32 {
    match state {
        MoraleState::Steady => 0,
        MoraleState::Rattled => -10,
        MoraleState::Broken => -25,
        MoraleState::Routed => 0,
    }
}

/// Return a map of faction names to their Sand multipliers.
///
/// Sand multipliers modify Sand gain/loss for different factions (e.g.,
/// higher for hardened fighters, lower for green recruits).
pub fn sand_multipliers() -> BTreeMap<&'static str, Fix32> {
    let mut m = BTreeMap::new();
    // Veteran factions have higher Sand (harder to break)
    m.insert("outlaw", Fix32::from_int(12) / Fix32::from_int(10)); // 1.2
    m.insert("cavalry", Fix32::ONE); // 1.0
    m.insert("settler", Fix32::from_int(8) / Fix32::from_int(10)); // 0.8
    m.insert("native", Fix32::from_int(11) / Fix32::from_int(10)); // 1.1
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Sand loss
    // -----------------------------------------------------------------------

    #[test]
    fn sand_loss_shot_at_missed() {
        assert_eq!(sand_loss_for_event("shot_at_missed"), 2);
    }

    #[test]
    fn sand_loss_shot_at_hit() {
        assert_eq!(sand_loss_for_event("shot_at_hit"), 4);
    }

    #[test]
    fn sand_loss_shot_at_critical() {
        assert_eq!(sand_loss_for_event("shot_at_critical"), 6);
    }

    #[test]
    fn sand_loss_adjacent_ally_killed() {
        assert_eq!(sand_loss_for_event("adjacent_ally_killed"), 8);
    }

    #[test]
    fn sand_loss_any_ally_killed() {
        assert_eq!(sand_loss_for_event("any_ally_killed"), 3);
    }

    #[test]
    fn sand_loss_leader_falls() {
        assert_eq!(sand_loss_for_event("leader_falls"), 10);
    }

    #[test]
    fn sand_loss_unknown_event_returns_zero() {
        assert_eq!(sand_loss_for_event("unknown"), 0);
    }

    // -----------------------------------------------------------------------
    // Sand gain
    // -----------------------------------------------------------------------

    #[test]
    fn sand_gain_kill_enemy() {
        assert_eq!(sand_gain_for_event("kill_enemy"), 3);
    }

    #[test]
    fn sand_gain_rally() {
        assert_eq!(sand_gain_for_event("rally"), 5);
    }

    #[test]
    fn sand_gain_unknown_event_returns_zero() {
        assert_eq!(sand_gain_for_event("unknown"), 0);
    }

    // -----------------------------------------------------------------------
    // Morale state
    // -----------------------------------------------------------------------

    #[test]
    fn morale_state_steady_above_60_percent() {
        assert_eq!(morale_state(70, 100), MoraleState::Steady);
        assert_eq!(morale_state(61, 100), MoraleState::Steady);
    }

    #[test]
    fn morale_state_rattled_between_25_and_60() {
        assert_eq!(morale_state(60, 100), MoraleState::Rattled);
        assert_eq!(morale_state(40, 100), MoraleState::Rattled);
        assert_eq!(morale_state(26, 100), MoraleState::Rattled);
    }

    #[test]
    fn morale_state_broken_below_25_percent() {
        assert_eq!(morale_state(25, 100), MoraleState::Broken);
        assert_eq!(morale_state(10, 100), MoraleState::Broken);
        assert_eq!(morale_state(1, 100), MoraleState::Broken);
    }

    #[test]
    fn morale_state_routed_at_zero() {
        assert_eq!(morale_state(0, 100), MoraleState::Routed);
        assert_eq!(morale_state(0, 50), MoraleState::Routed);
    }

    #[test]
    fn morale_state_steady_when_max_sand_zero() {
        assert_eq!(morale_state(10, 0), MoraleState::Steady);
    }

    #[test]
    fn morale_state_steady_at_exact_boundary() {
        // 60/100 = exactly 60%, should be Rattled (not > 60%)
        assert_eq!(morale_state(60, 100), MoraleState::Rattled);
        // 25/100 = exactly 25%, should be Broken (not > 25%)
        assert_eq!(morale_state(25, 100), MoraleState::Broken);
    }

    // -----------------------------------------------------------------------
    // Morale accuracy penalties
    // -----------------------------------------------------------------------

    #[test]
    fn morale_penalty_steady_is_zero() {
        assert_eq!(morale_accuracy_penalty(MoraleState::Steady), 0);
    }

    #[test]
    fn morale_penalty_rattled_is_negative_10() {
        assert_eq!(morale_accuracy_penalty(MoraleState::Rattled), -10);
    }

    #[test]
    fn morale_penalty_broken_is_negative_25() {
        assert_eq!(morale_accuracy_penalty(MoraleState::Broken), -25);
    }

    #[test]
    fn morale_penalty_routed_is_zero() {
        assert_eq!(morale_accuracy_penalty(MoraleState::Routed), 0);
    }

    // -----------------------------------------------------------------------
    // Sand multipliers
    // -----------------------------------------------------------------------

    #[test]
    fn sand_multipliers_contains_expected_factions() {
        let mults = sand_multipliers();
        assert!(mults.contains_key("outlaw"));
        assert!(mults.contains_key("cavalry"));
        assert!(mults.contains_key("settler"));
        assert!(mults.contains_key("native"));
    }

    #[test]
    fn sand_multipliers_cavalry_is_one() {
        let mults = sand_multipliers();
        assert_eq!(*mults.get("cavalry").unwrap(), Fix32::ONE);
    }

    #[test]
    fn sand_multipliers_outlaw_above_one() {
        let mults = sand_multipliers();
        assert!(*mults.get("outlaw").unwrap() > Fix32::ONE);
    }
}
