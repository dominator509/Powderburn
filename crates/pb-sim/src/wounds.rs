//! Wound application and effects.
//!
//! M3: Maps hit locations to wound types and provides wound effect
//! descriptions per SPEC-001 section 7.

#![forbid(unsafe_code)]

use pb_core::event::{HitLocationType, WoundType};

use crate::state::ActorState;

/// Apply a wound of the given type to an actor, based on the hit location.
///
/// Returns the `WoundType` that was applied. If the actor already has this
/// wound type, it still applies (wounds can stack severity).
pub fn apply_wound(actor: &mut ActorState, location: HitLocationType) -> WoundType {
    let wound = wound_for_location(location);
    actor.wounds.push(wound);
    wound
}

/// Determine the wound type for a given hit location.
///
/// Based on SPEC-001 section 7 wound mapping table.
pub fn wound_for_location(location: HitLocationType) -> WoundType {
    match location {
        HitLocationType::Head => WoundType::Concussed,
        HitLocationType::Eyes => WoundType::Blinded,
        HitLocationType::Torso => WoundType::Bleeding,
        HitLocationType::Vitals => WoundType::Bleeding,
        HitLocationType::GunArm => WoundType::Broken,
        HitLocationType::OffArm => WoundType::Broken,
        HitLocationType::Legs => WoundType::Broken,
    }
}

/// Return a human-readable description of the wound's gameplay effects.
pub fn wound_effects(wound: WoundType) -> &'static str {
    match wound {
        WoundType::Bleeding => "Loses 1 HP per turn until bandaged",
        WoundType::Broken => "Cannot use the affected limb; -4 to all actions",
        WoundType::Concussed => "All actions cost +1 AP; -2 to hit",
        WoundType::Winded => "AP per turn reduced by 2 until rested",
        WoundType::Blinded => "Cannot see; all ranged attacks auto-miss",
        WoundType::Burned => "Takes 2 HP damage per turn",
        WoundType::Shocked => "Cannot act for 1d2 turns",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Stance;
    use pb_core::geom::Facing;
    use pb_core::geom::TileXY;
    use pb_core::ids::Ap;

    fn make_actor() -> ActorState {
        ActorState {
            ap: Ap(10),
            position: TileXY::new(0, 0),
            facing: Facing::South,
            sequence: 5,
            hit_points: 20,
            max_hp: 20,
            name: "Test".to_string(),
            alive: true,
            wounds: vec![],
            sand: 10,
            max_sand: 10,
            stance: Stance::Standing,
            progression: crate::progression::ActorProgression::new(),
            weapon: "colt_army_1860".to_string(),
            loaded_rounds: 6,
            weapon_capacity: 6,
            fouling: 0,
            jammed: false,
        }
    }

    #[test]
    fn head_wound_is_concussed() {
        assert_eq!(
            wound_for_location(HitLocationType::Head),
            WoundType::Concussed
        );
    }

    #[test]
    fn eyes_wound_is_blinded() {
        assert_eq!(
            wound_for_location(HitLocationType::Eyes),
            WoundType::Blinded
        );
    }

    #[test]
    fn torso_wound_is_bleeding() {
        assert_eq!(
            wound_for_location(HitLocationType::Torso),
            WoundType::Bleeding
        );
    }

    #[test]
    fn gun_arm_wound_is_broken() {
        assert_eq!(
            wound_for_location(HitLocationType::GunArm),
            WoundType::Broken
        );
    }

    #[test]
    fn apply_wound_adds_to_actor_wounds() {
        let mut actor = make_actor();
        assert_eq!(actor.wounds.len(), 0);

        let wound = apply_wound(&mut actor, HitLocationType::GunArm);
        assert_eq!(wound, WoundType::Broken);
        assert_eq!(actor.wounds.len(), 1);
        assert_eq!(actor.wounds[0], WoundType::Broken);
    }

    #[test]
    fn apply_wound_multiple_times() {
        let mut actor = make_actor();
        apply_wound(&mut actor, HitLocationType::Head);
        apply_wound(&mut actor, HitLocationType::Torso);
        assert_eq!(actor.wounds.len(), 2);
    }

    #[test]
    fn wound_effects_returns_description() {
        let desc = wound_effects(WoundType::Broken);
        assert!(!desc.is_empty());
        assert!(desc.contains("limb"));
    }
}
