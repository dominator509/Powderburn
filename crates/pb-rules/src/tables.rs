//! Rule tables for the POWDERBURN simulation kernel.
//!
//! Provides hit-location tables, weapon-stat tables, called-shot accuracy
//! modifiers, and basic lookup functions used by the shot pipeline.
//!
//! All values are deterministic — no floats, no randomness.

#![forbid(unsafe_code)]

use pb_core::event::HitLocationType;

// ---------------------------------------------------------------------------
// Hit-location table entries
// ---------------------------------------------------------------------------

/// A row in the hit-location distribution table.
///
/// Each row gives the base chance weight for a random hit landing at that
/// location, a damage multiplier applied after the weapon's base damage,
/// and a critical effect enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitLocationEntry {
    /// The location described by this row.
    pub location: HitLocationType,
    /// Integer weight used for selecting the location via weighted random draw.
    pub base_chance: i32,
    /// Damage multiplier (percent, e.g. 100 = 1.0x, 150 = 1.5x).
    pub damage_mult: i32,
    /// Index into the critical-effect table (0 = none).
    pub crit_effect: i32,
}

/// The canonical hit-location distribution table.
///
/// Weights add up to 100 for convenience. Order matches the standard priority
/// from SPEC-001 section 7.
pub const HIT_LOCATION_TABLE: &[HitLocationEntry] = &[
    HitLocationEntry {
        location: HitLocationType::Head,
        base_chance: 10,
        damage_mult: 200,
        crit_effect: 3,
    },
    HitLocationEntry {
        location: HitLocationType::Eyes,
        base_chance: 3,
        damage_mult: 300,
        crit_effect: 5,
    },
    HitLocationEntry {
        location: HitLocationType::Torso,
        base_chance: 35,
        damage_mult: 100,
        crit_effect: 1,
    },
    HitLocationEntry {
        location: HitLocationType::Vitals,
        base_chance: 12,
        damage_mult: 150,
        crit_effect: 4,
    },
    HitLocationEntry {
        location: HitLocationType::GunArm,
        base_chance: 15,
        damage_mult: 100,
        crit_effect: 2,
    },
    HitLocationEntry {
        location: HitLocationType::OffArm,
        base_chance: 10,
        damage_mult: 100,
        crit_effect: 2,
    },
    HitLocationEntry {
        location: HitLocationType::Legs,
        base_chance: 15,
        damage_mult: 100,
        crit_effect: 1,
    },
];

/// Look up a hit-location entry by `HitLocationType`.
pub fn hit_location_entry(loc: HitLocationType) -> Option<&'static HitLocationEntry> {
    HIT_LOCATION_TABLE.iter().find(|e| e.location == loc)
}

/// Return the total weight sum of the hit-location table.
pub fn hit_location_total_weight() -> i32 {
    HIT_LOCATION_TABLE.iter().map(|e| e.base_chance).sum()
}

/// Select a hit location by walking the weight table from a cumulative offset.
///
/// `roll` must be in `0..total_weight`. Returns `None` if roll is out of
/// range (defensive).
pub fn select_hit_location(roll: i32) -> Option<HitLocationType> {
    let total = hit_location_total_weight();
    if roll < 0 || roll >= total {
        return None;
    }
    let mut cumulative = 0i32;
    for entry in HIT_LOCATION_TABLE {
        cumulative += entry.base_chance;
        if roll < cumulative {
            return Some(entry.location);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Weapon table entries
// ---------------------------------------------------------------------------

/// A row in the weapon statistics table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponEntry {
    /// Unique weapon identifier.
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Base damage (before hit-location multiplier).
    pub base_damage: i32,
    /// Accuracy bonus (added to base hit chance).
    pub accuracy: i32,
    /// Maximum effective range in tiles.
    pub max_range: i32,
    /// Number of rounds the weapon can hold.
    pub capacity: i32,
    /// Whether the weapon is cap-and-ball (true) or cartridge (false).
    pub cap_and_ball: bool,
    /// Base misfire chance (0-100, rolled against on every shot).
    pub misfire_chance: i32,
}

/// The weapon table.
pub const WEAPON_TABLE: &[WeaponEntry] = &[
    WeaponEntry {
        id: "colt_army_1860",
        name: "Colt Army Model 1860",
        base_damage: 14,
        accuracy: 5,
        max_range: 24,
        capacity: 6,
        cap_and_ball: true,
        misfire_chance: 5,
    },
    WeaponEntry {
        id: "winchester_1866",
        name: "Winchester Model 1866",
        base_damage: 12,
        accuracy: 8,
        max_range: 40,
        capacity: 13,
        cap_and_ball: false,
        misfire_chance: 3,
    },
    WeaponEntry {
        id: "sharps_1874",
        name: "Sharps 1874",
        base_damage: 22,
        accuracy: 12,
        max_range: 60,
        capacity: 1,
        cap_and_ball: false,
        misfire_chance: 2,
    },
];

/// Look up a weapon entry by its string identifier.
pub fn weapon_entry(id: &str) -> Option<&'static WeaponEntry> {
    WEAPON_TABLE.iter().find(|w| w.id == id)
}

// ---------------------------------------------------------------------------
// Called-shot accuracy modifiers
// ---------------------------------------------------------------------------

/// A called-shot modifier record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalledShotMod {
    /// The hit location being aimed at.
    pub location: HitLocationType,
    /// Penalty to the hit-chance roll for this called shot.
    pub accuracy_penalty: i32,
}

/// Called-shot accuracy penalty table.
///
/// These are subtracted from the base hit chance before the roll.
pub const CALLED_SHOT_TABLE: &[CalledShotMod] = &[
    CalledShotMod {
        location: HitLocationType::Head,
        accuracy_penalty: 20,
    },
    CalledShotMod {
        location: HitLocationType::Eyes,
        accuracy_penalty: 40,
    },
    CalledShotMod {
        location: HitLocationType::Torso,
        accuracy_penalty: 0,
    },
    CalledShotMod {
        location: HitLocationType::Vitals,
        accuracy_penalty: 15,
    },
    CalledShotMod {
        location: HitLocationType::GunArm,
        accuracy_penalty: 15,
    },
    CalledShotMod {
        location: HitLocationType::OffArm,
        accuracy_penalty: 15,
    },
    CalledShotMod {
        location: HitLocationType::Legs,
        accuracy_penalty: 10,
    },
];

/// Look up the accuracy penalty for a called shot to the given location.
pub fn called_shot_penalty(loc: HitLocationType) -> i32 {
    CALLED_SHOT_TABLE
        .iter()
        .find(|m| m.location == loc)
        .map(|m| m.accuracy_penalty)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Wound mapping
// ---------------------------------------------------------------------------

/// Map a hit location to the most common wound type for damage exceeding a
/// threshold.
///
/// Based on SPEC-001 section 7 wound tables.
pub fn location_to_wound(loc: HitLocationType) -> pb_core::event::WoundType {
    match loc {
        HitLocationType::Head => pb_core::event::WoundType::Concussed,
        HitLocationType::Eyes => pb_core::event::WoundType::Blinded,
        HitLocationType::Torso => pb_core::event::WoundType::Bleeding,
        HitLocationType::Vitals => pb_core::event::WoundType::Bleeding,
        HitLocationType::GunArm => pb_core::event::WoundType::Broken,
        HitLocationType::OffArm => pb_core::event::WoundType::Broken,
        HitLocationType::Legs => pb_core::event::WoundType::Broken,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_location_table_weights_sum_to_100() {
        let total = hit_location_total_weight();
        assert_eq!(total, 100);
    }

    #[test]
    fn select_hit_location_each_entry_appears() {
        let mut seen = std::collections::BTreeSet::new();
        for roll in 0..100 {
            let loc = select_hit_location(roll).expect("roll in range");
            seen.insert(loc);
        }
        assert_eq!(seen.len(), 7, "all seven locations should be reachable");
    }

    #[test]
    fn select_hit_location_none_for_bad_roll() {
        assert!(select_hit_location(-1).is_none());
        assert!(select_hit_location(100).is_none());
    }

    #[test]
    fn hit_location_entry_lookup() {
        let e = hit_location_entry(HitLocationType::Head).expect("head exists");
        assert_eq!(e.damage_mult, 200);
    }

    #[test]
    fn weapon_entry_lookup() {
        let w = weapon_entry("colt_army_1860").expect("colt exists");
        assert_eq!(w.base_damage, 14);
        assert_eq!(w.cap_and_ball, true);
    }

    #[test]
    fn weapon_entry_missing() {
        assert!(weapon_entry("nonexistent").is_none());
    }

    #[test]
    fn called_shot_penalty_values() {
        assert_eq!(called_shot_penalty(HitLocationType::Head), 20);
        assert_eq!(called_shot_penalty(HitLocationType::Eyes), 40);
        assert_eq!(called_shot_penalty(HitLocationType::Torso), 0);
        assert_eq!(called_shot_penalty(HitLocationType::GunArm), 15);
    }

    #[test]
    fn location_to_wound_mapping() {
        assert_eq!(
            location_to_wound(HitLocationType::GunArm),
            pb_core::event::WoundType::Broken
        );
        assert_eq!(
            location_to_wound(HitLocationType::Head),
            pb_core::event::WoundType::Concussed
        );
        assert_eq!(
            location_to_wound(HitLocationType::Eyes),
            pb_core::event::WoundType::Blinded
        );
    }
}
