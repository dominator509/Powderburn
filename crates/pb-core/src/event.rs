//! Simulation event types.
//!
//! Every meaningful occurrence during a tick is recorded as an `Event` value.
//! Events are consumed by the renderer, the audio system, the save system,
//! and the ledger.
//!
//! Each variant implements `Display` with the format:
//! `event: <Name> <field1>=<value1> <field2>=<value2>...`

#![forbid(unsafe_code)]

use core::fmt;

use crate::ids::ActorId;
use crate::geom::TileXY;

/// Which hit location was struck during a called shot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitLocationType {
    Head,
    Eyes,
    Torso,
    Vitals,
    GunArm,
    OffArm,
    Legs,
}

impl fmt::Display for HitLocationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HitLocationType::Head => write!(f, "Head"),
            HitLocationType::Eyes => write!(f, "Eyes"),
            HitLocationType::Torso => write!(f, "Torso"),
            HitLocationType::Vitals => write!(f, "Vitals"),
            HitLocationType::GunArm => write!(f, "GunArm"),
            HitLocationType::OffArm => write!(f, "OffArm"),
            HitLocationType::Legs => write!(f, "Legs"),
        }
    }
}

/// The type of a wound applied to an actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WoundType {
    Bleeding,
    Broken,
    Concussed,
    Winded,
    Blinded,
    Burned,
    Shocked,
}

impl fmt::Display for WoundType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WoundType::Bleeding => write!(f, "Bleeding"),
            WoundType::Broken => write!(f, "Broken"),
            WoundType::Concussed => write!(f, "Concussed"),
            WoundType::Winded => write!(f, "Winded"),
            WoundType::Blinded => write!(f, "Blinded"),
            WoundType::Burned => write!(f, "Burned"),
            WoundType::Shocked => write!(f, "Shocked"),
        }
    }
}

/// Every event the simulation kernel can produce during a tick.
///
/// The `Display` implementation produces the canonical string form:
/// `event: <VariantName> <key1>=<value1> <key2>=<value2>...`
/// with fields in declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    HitLocation {
        actor: ActorId,
        location: HitLocationType,
    },
    DamageApplied {
        actor: ActorId,
        damage: i32,
    },
    WoundApplied {
        actor: ActorId,
        wound: WoundType,
    },
    WeaponDropped {
        actor: ActorId,
        item: String,
    },
    Misfire {
        actor: ActorId,
    },
    ShotHit {
        actor: ActorId,
        target: ActorId,
        hit: bool,
    },
    ActorKilled {
        actor: ActorId,
    },
    SmokeDeposited {
        tile: TileXY,
        density: u32,
    },
    CompanionKilled {
        id: String,
    },
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::HitLocation { actor, location } => {
                write!(f, "event: HitLocation actor={} location={}", actor, location)
            }
            Event::DamageApplied { actor, damage } => {
                write!(f, "event: DamageApplied actor={} damage={}", actor, damage)
            }
            Event::WoundApplied { actor, wound } => {
                write!(f, "event: WoundApplied actor={} wound={}", actor, wound)
            }
            Event::WeaponDropped { actor, item } => {
                write!(f, "event: WeaponDropped actor={} item={}", actor, item)
            }
            Event::Misfire { actor } => {
                write!(f, "event: Misfire actor={}", actor)
            }
            Event::ShotHit { actor, target, hit } => {
                write!(f, "event: ShotHit actor={} target={} hit={}", actor, target, hit)
            }
            Event::ActorKilled { actor } => {
                write!(f, "event: ActorKilled actor={}", actor)
            }
            Event::SmokeDeposited { tile, density } => {
                write!(f, "event: SmokeDeposited tile={} density={}", tile, density)
            }
            Event::CompanionKilled { id } => {
                write!(f, "event: CompanionKilled id={}", id)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // HitLocationType Display
    // -----------------------------------------------------------------------

    #[test]
    fn hit_location_type_display() {
        assert_eq!(HitLocationType::Head.to_string(), "Head");
        assert_eq!(HitLocationType::Eyes.to_string(), "Eyes");
        assert_eq!(HitLocationType::Torso.to_string(), "Torso");
        assert_eq!(HitLocationType::Vitals.to_string(), "Vitals");
        assert_eq!(HitLocationType::GunArm.to_string(), "GunArm");
        assert_eq!(HitLocationType::OffArm.to_string(), "OffArm");
        assert_eq!(HitLocationType::Legs.to_string(), "Legs");
    }

    // -----------------------------------------------------------------------
    // WoundType Display
    // -----------------------------------------------------------------------

    #[test]
    fn wound_type_display() {
        assert_eq!(WoundType::Bleeding.to_string(), "Bleeding");
        assert_eq!(WoundType::Broken.to_string(), "Broken");
        assert_eq!(WoundType::Concussed.to_string(), "Concussed");
        assert_eq!(WoundType::Winded.to_string(), "Winded");
        assert_eq!(WoundType::Blinded.to_string(), "Blinded");
        assert_eq!(WoundType::Burned.to_string(), "Burned");
        assert_eq!(WoundType::Shocked.to_string(), "Shocked");
    }

    // -----------------------------------------------------------------------
    // Event Display format
    // -----------------------------------------------------------------------

    #[test]
    fn event_hit_location_format() {
        let e = Event::HitLocation {
            actor: ActorId(42),
            location: HitLocationType::Head,
        };
        assert_eq!(e.to_string(), "event: HitLocation actor=ActorId(42) location=Head");
    }

    #[test]
    fn event_damage_applied_format() {
        let e = Event::DamageApplied {
            actor: ActorId(7),
            damage: 15,
        };
        assert_eq!(e.to_string(), "event: DamageApplied actor=ActorId(7) damage=15");
    }

    #[test]
    fn event_wound_applied_format() {
        let e = Event::WoundApplied {
            actor: ActorId(3),
            wound: WoundType::Bleeding,
        };
        assert_eq!(e.to_string(), "event: WoundApplied actor=ActorId(3) wound=Bleeding");
    }

    #[test]
    fn event_weapon_dropped_format() {
        let e = Event::WeaponDropped {
            actor: ActorId(1),
            item: "SharpsRifle".to_string(),
        };
        assert_eq!(
            e.to_string(),
            "event: WeaponDropped actor=ActorId(1) item=SharpsRifle"
        );
    }

    #[test]
    fn event_misfire_format() {
        let e = Event::Misfire { actor: ActorId(9) };
        assert_eq!(e.to_string(), "event: Misfire actor=ActorId(9)");
    }

    #[test]
    fn event_shot_hit_format_hit() {
        let e = Event::ShotHit {
            actor: ActorId(2),
            target: ActorId(5),
            hit: true,
        };
        assert_eq!(
            e.to_string(),
            "event: ShotHit actor=ActorId(2) target=ActorId(5) hit=true"
        );
    }

    #[test]
    fn event_shot_hit_format_miss() {
        let e = Event::ShotHit {
            actor: ActorId(2),
            target: ActorId(5),
            hit: false,
        };
        assert_eq!(
            e.to_string(),
            "event: ShotHit actor=ActorId(2) target=ActorId(5) hit=false"
        );
    }

    #[test]
    fn event_actor_killed_format() {
        let e = Event::ActorKilled { actor: ActorId(99) };
        assert_eq!(e.to_string(), "event: ActorKilled actor=ActorId(99)");
    }

    #[test]
    fn event_smoke_deposited_format() {
        let e = Event::SmokeDeposited {
            tile: TileXY::new(3, 7),
            density: 42,
        };
        assert_eq!(
            e.to_string(),
            "event: SmokeDeposited tile=(3, 7) density=42"
        );
    }

    #[test]
    fn event_companion_killed_format() {
        let e = Event::CompanionKilled {
            id: "companion_wyatt".to_string(),
        };
        assert_eq!(
            e.to_string(),
            "event: CompanionKilled id=companion_wyatt"
        );
    }

    // -----------------------------------------------------------------------
    // Event equality (derived)
    // -----------------------------------------------------------------------

    #[test]
    fn event_equality() {
        let a = Event::DamageApplied {
            actor: ActorId(1),
            damage: 10,
        };
        let b = Event::DamageApplied {
            actor: ActorId(1),
            damage: 10,
        };
        let c = Event::DamageApplied {
            actor: ActorId(1),
            damage: 20,
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn event_clone() {
        let e = Event::Misfire { actor: ActorId(5) };
        let cloned = e.clone();
        assert_eq!(e, cloned);
    }
}
