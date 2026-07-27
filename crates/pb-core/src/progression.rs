//! Progression system: XP, levels, skills, Marks (perks), Ways (traits).
//!
//! This module provides the data structures and formulas for the character
//! advancement system. See SPEC-001 §12 and SPEC‑003 for design details.

#![forbid(unsafe_code)]

use core::fmt;

/// A skill identifier. Skills are the trainable combat and utility abilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SkillId(pub u32);

impl fmt::Display for SkillId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SkillId({})", self.0)
    }
}

/// A Mark (perk) identifier. Marks are special abilities gained every 3rd level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MarkId(pub u32);

impl fmt::Display for MarkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MarkId({})", self.0)
    }
}

/// A Way (trait) identifier. A Way is a starting trait chosen at character creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WayId(pub u32);

impl fmt::Display for WayId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WayId({})", self.0)
    }
}

/// The 8 skill lines in the game.
///
/// These govern weapon accuracy, utility, and social effectiveness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SkillLine {
    Pistols,
    LongGuns,
    Scatterguns,
    Blades,
    Explosives,
    FieldMedicine,
    Scouting,
    Talk,
}

impl SkillLine {
    /// All 8 skill lines, in canonical order.
    pub const ALL: [SkillLine; 8] = [
        SkillLine::Pistols,
        SkillLine::LongGuns,
        SkillLine::Scatterguns,
        SkillLine::Blades,
        SkillLine::Explosives,
        SkillLine::FieldMedicine,
        SkillLine::Scouting,
        SkillLine::Talk,
    ];

    /// Human-readable display name.
    pub fn display_name(self) -> &'static str {
        match self {
            SkillLine::Pistols => "Pistols",
            SkillLine::LongGuns => "Long Guns",
            SkillLine::Scatterguns => "Scatterguns",
            SkillLine::Blades => "Blades",
            SkillLine::Explosives => "Explosives",
            SkillLine::FieldMedicine => "Field Medicine",
            SkillLine::Scouting => "Scouting",
            SkillLine::Talk => "Talk",
        }
    }

    /// A short description of what the skill does.
    pub fn description(self) -> &'static str {
        match self {
            SkillLine::Pistols => "Accuracy with revolvers and pistols (+1 per level).",
            SkillLine::LongGuns => "Accuracy with rifles and carbines (+1 per level).",
            SkillLine::Scatterguns => "Accuracy with shotguns and blunderbusses (+1 per level).",
            SkillLine::Blades => "Accuracy with knives, tomahawks, and melee weapons (+1 per level).",
            SkillLine::Explosives => "Accuracy with thrown dynamite and grenades; reduces misfire chance (+1 per level).",
            SkillLine::FieldMedicine => "Healing effectiveness; bandages and surgery restore more HP (+1 HP per level).",
            SkillLine::Scouting => "Sight radius, ambush detection, and overwatch duration (+1 tile per level).",
            SkillLine::Talk => "Dialogue options, rally effectiveness, and sand recovery from conversation (+1 per level).",
        }
    }

    /// The maximum level (rank) for any skill line.
    pub const MAX_LEVEL: u32 = 10;
}

impl fmt::Display for SkillLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

// ---------------------------------------------------------------------------
// XP thresholds
// ---------------------------------------------------------------------------

/// Calculate the XP required to reach a given level.
///
/// Formula: `xp_for_level(n) = 100 * n * (n-1) / 2`
///
/// This gives: level 2 = 100, level 3 = 300, level 4 = 600, level 5 = 1000,
/// level 6 = 1500, level 7 = 2100, level 8 = 2800, level 9 = 3600, level 10 = 4500.
///
/// Level 1 requires 0 XP (starting level).
pub const fn xp_for_level(level: u32) -> u64 {
    if level <= 1 {
        return 0;
    }
    let n = level as u64;
    100 * n * (n - 1) / 2
}

/// Calculate the current level from total XP earned.
///
/// Returns the highest level for which `xp_for_level(level) <= total_xp`.
pub fn level_from_xp(total_xp: u64) -> u32 {
    // Level is capped at 20 in the progression system.
    for level in (1..=20).rev() {
        if xp_for_level(level) <= total_xp {
            return level;
        }
    }
    1
}

/// Check whether the given total XP would cause a level-up from the given
/// current level. Returns `Some(new_level)` if a level-up occurs.
pub fn check_level_up(current_level: u32, total_xp: u64) -> Option<u32> {
    let next_level = current_level + 1;
    if xp_for_level(next_level) <= total_xp {
        Some(next_level)
    } else {
        None
    }
}

/// Determine whether a level grants a Mark choice.
/// Marks are granted at levels 3, 6, 9, 12, 15, 18 (every 3rd level).
pub fn level_grants_mark(level: u32) -> bool {
    level >= 3 && level % 3 == 0
}

/// Determine how many Marks an actor should have earned by the given level.
pub fn marks_earned_by_level(level: u32) -> u32 {
    if level < 3 {
        0
    } else {
        (level - 3) / 3 + 1 // at 3, 6, 9, ...
    }
}

/// Calculate the accuracy bonus from a skill line for a given weapon category.
///
/// Each level in a matching skill grants +1 accuracy.
pub fn skill_accuracy_bonus(skill_level: u32) -> i32 {
    skill_level as i32
}

/// Calculate the bonus HP healed from Field Medicine.
pub fn medicine_heal_bonus(skill_level: u32) -> i32 {
    skill_level as i32
}

/// Calculate the sight radius bonus from Scouting.
pub fn scouting_sight_bonus(skill_level: u32) -> i32 {
    skill_level as i32
}

/// XP awarded for completing a scenario.
pub const BASE_SCENARIO_XP: u64 = 50;

/// Bonus XP for completing a primary objective.
pub const OBJECTIVE_BONUS_XP: u64 = 25;

/// Bonus XP per headshot kill.
pub const HEADSHOT_BONUS_XP: u64 = 10;

/// Bonus XP for completing a scenario with no companion casualties.
pub const NO_CASUALTIES_BONUS_XP: u64 = 30;

/// Bonus XP for completing a bonus objective.
pub const BONUS_OBJECTIVE_XP: u64 = 15;

/// Skill points granted per level-up.
pub const SKILL_POINTS_PER_LEVEL: u32 = 1;

/// Maximum possible character level.
pub const MAX_CHARACTER_LEVEL: u32 = 20;

/// Starting level for all new characters.
pub const STARTING_LEVEL: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // XP threshold math
    // -----------------------------------------------------------------------

    #[test]
    fn xp_for_level_1_is_zero() {
        assert_eq!(xp_for_level(1), 0);
    }

    #[test]
    fn xp_for_level_2_is_100() {
        assert_eq!(xp_for_level(2), 100);
    }

    #[test]
    fn xp_for_level_3_is_300() {
        assert_eq!(xp_for_level(3), 300);
    }

    #[test]
    fn xp_for_level_4_is_600() {
        assert_eq!(xp_for_level(4), 600);
    }

    #[test]
    fn xp_for_level_5_is_1000() {
        assert_eq!(xp_for_level(5), 1000);
    }

    #[test]
    fn xp_for_level_10_is_4500() {
        assert_eq!(xp_for_level(10), 4500);
    }

    #[test]
    fn xp_for_level_20_is_19000() {
        assert_eq!(xp_for_level(20), 19000);
    }

    // -----------------------------------------------------------------------
    // Level from XP
    // -----------------------------------------------------------------------

    #[test]
    fn level_from_xp_zero_is_1() {
        assert_eq!(level_from_xp(0), 1);
    }

    #[test]
    fn level_from_xp_50_is_1() {
        assert_eq!(level_from_xp(50), 1);
    }

    #[test]
    fn level_from_xp_100_is_2() {
        assert_eq!(level_from_xp(100), 2);
    }

    #[test]
    fn level_from_xp_299_is_2() {
        assert_eq!(level_from_xp(299), 2);
    }

    #[test]
    fn level_from_xp_300_is_3() {
        assert_eq!(level_from_xp(300), 3);
    }

    #[test]
    fn level_from_xp_4500_is_10() {
        assert_eq!(level_from_xp(4500), 10);
    }

    #[test]
    fn level_from_xp_19000_is_20() {
        assert_eq!(level_from_xp(19000), 20);
    }

    // -----------------------------------------------------------------------
    // Level-up detection
    // -----------------------------------------------------------------------

    #[test]
    fn check_level_up_at_threshold() {
        assert_eq!(check_level_up(1, 100), Some(2));
    }

    #[test]
    fn check_level_up_below_threshold() {
        assert_eq!(check_level_up(1, 99), None);
    }

    #[test]
    fn check_level_up_multiple_levels() {
        // From level 1 with 600 XP, should detect level-up (to at least 2)
        assert_eq!(check_level_up(1, 600), Some(2));
    }

    #[test]
    fn check_level_up_from_2_to_3() {
        assert_eq!(check_level_up(2, 300), Some(3));
    }

    #[test]
    fn check_level_up_no_op_at_max() {
        assert_eq!(check_level_up(20, 999_999), None);
    }

    // -----------------------------------------------------------------------
    // Mark granting
    // -----------------------------------------------------------------------

    #[test]
    fn level_grants_mark_at_3() {
        assert!(level_grants_mark(3));
    }

    #[test]
    fn level_grants_mark_at_6() {
        assert!(level_grants_mark(6));
    }

    #[test]
    fn level_does_not_grant_mark_at_2() {
        assert!(!level_grants_mark(2));
    }

    #[test]
    fn level_does_not_grant_mark_at_4() {
        assert!(!level_grants_mark(4));
    }

    #[test]
    fn level_grants_mark_at_9() {
        assert!(level_grants_mark(9));
    }

    #[test]
    fn level_does_not_grant_mark_at_1() {
        assert!(!level_grants_mark(1));
    }

    #[test]
    fn marks_earned_by_level_1() {
        assert_eq!(marks_earned_by_level(1), 0);
    }

    #[test]
    fn marks_earned_by_level_2() {
        assert_eq!(marks_earned_by_level(2), 0);
    }

    #[test]
    fn marks_earned_by_level_3() {
        assert_eq!(marks_earned_by_level(3), 1);
    }

    #[test]
    fn marks_earned_by_level_5() {
        assert_eq!(marks_earned_by_level(5), 1);
    }

    #[test]
    fn marks_earned_by_level_6() {
        assert_eq!(marks_earned_by_level(6), 2);
    }

    #[test]
    fn marks_earned_by_level_9() {
        assert_eq!(marks_earned_by_level(9), 3);
    }

    // -----------------------------------------------------------------------
    // Skill line display
    // -----------------------------------------------------------------------

    #[test]
    fn skill_line_display_names() {
        assert_eq!(SkillLine::Pistols.display_name(), "Pistols");
        assert_eq!(SkillLine::LongGuns.display_name(), "Long Guns");
        assert_eq!(SkillLine::Scatterguns.display_name(), "Scatterguns");
        assert_eq!(SkillLine::Blades.display_name(), "Blades");
        assert_eq!(SkillLine::Explosives.display_name(), "Explosives");
        assert_eq!(SkillLine::FieldMedicine.display_name(), "Field Medicine");
        assert_eq!(SkillLine::Scouting.display_name(), "Scouting");
        assert_eq!(SkillLine::Talk.display_name(), "Talk");
    }

    #[test]
    fn skill_line_all_count() {
        assert_eq!(SkillLine::ALL.len(), 8);
    }

    #[test]
    fn skill_line_max_level() {
        assert_eq!(SkillLine::MAX_LEVEL, 10);
    }

    // -----------------------------------------------------------------------
    // Derived bonuses
    // -----------------------------------------------------------------------

    #[test]
    fn skill_accuracy_bonus_scales() {
        assert_eq!(skill_accuracy_bonus(0), 0);
        assert_eq!(skill_accuracy_bonus(5), 5);
        assert_eq!(skill_accuracy_bonus(10), 10);
    }

    #[test]
    fn medicine_heal_bonus_scales() {
        assert_eq!(medicine_heal_bonus(0), 0);
        assert_eq!(medicine_heal_bonus(3), 3);
    }

    #[test]
    fn scouting_sight_bonus_scales() {
        assert_eq!(scouting_sight_bonus(0), 0);
        assert_eq!(scouting_sight_bonus(7), 7);
    }

    // -----------------------------------------------------------------------
    // Constants
    // -----------------------------------------------------------------------

    #[test]
    fn base_xp_constant() {
        assert_eq!(BASE_SCENARIO_XP, 50);
    }

    #[test]
    fn skill_points_per_level_constant() {
        assert_eq!(SKILL_POINTS_PER_LEVEL, 1);
    }
}
