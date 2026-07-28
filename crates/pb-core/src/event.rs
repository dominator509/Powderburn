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

use crate::geom::TileXY;
use crate::ids::ActorId;

/// Which hit location was struck during a called shot.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
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
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
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
    TurnBegin {
        actor: ActorId,
        tick: u64,
    },
    TurnEnd {
        actor: ActorId,
        tick: u64,
    },
    Moved {
        actor: ActorId,
        from: TileXY,
        to: TileXY,
    },
    StanceChanged {
        actor: ActorId,
        stance: String,
    },
    FacingChanged {
        actor: ActorId,
        facing: String,
    },
    Fired {
        actor: ActorId,
        target: ActorId,
    },
    Misfire {
        actor: ActorId,
    },
    Jammed {
        actor: ActorId,
    },
    Missed {
        actor: ActorId,
        target: ActorId,
    },
    /// Compatibility detail event retained for consumers that need a hit boolean.
    ShotHit {
        actor: ActorId,
        target: ActorId,
        hit: bool,
    },
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
    Critical {
        actor: ActorId,
        effect: String,
    },
    WeaponDropped {
        actor: ActorId,
        item: String,
    },
    SmokeDeposited {
        tile: TileXY,
        density: u32,
    },
    SmokeDecayed {
        tile: TileXY,
        density: u32,
    },
    SmokeDrifted {
        from: TileXY,
        to: TileXY,
        density: u32,
    },
    OverwatchSet {
        actor: ActorId,
        reaction_points: u8,
    },
    ReactionShot {
        actor: ActorId,
        target: ActorId,
    },
    DynamiteLit {
        actor: ActorId,
        tile: TileXY,
        detonate_at: u64,
    },
    DynamiteCaught {
        actor: ActorId,
        tile: TileXY,
    },
    DynamiteRethrown {
        actor: ActorId,
        tile: TileXY,
        detonate_at: u64,
    },
    DynamiteExploded {
        actor: ActorId,
        tile: TileXY,
    },
    CoverDamaged {
        tile: TileXY,
        facing: String,
        level: String,
    },
    Revealed {
        actor: ActorId,
        until_tick: u64,
    },
    TrackLeft {
        actor: ActorId,
        tile: TileXY,
    },
    SandLost {
        actor: ActorId,
        amount: i32,
    },
    SandGained {
        actor: ActorId,
        amount: i32,
    },
    MoraleStateChanged {
        actor: ActorId,
        state: String,
    },
    Routed {
        actor: ActorId,
    },
    ActorKilled {
        actor: ActorId,
    },
    CompanionKilled {
        id: String,
    },
    ObjectiveComplete {
        id: String,
    },
    LedgerEntryWritten {
        index: u32,
    },
    ScenarioEnded {
        outcome: String,
    },
    /// An actor gained XP and may have levelled up.
    XpGained {
        actor: ActorId,
        xp: u64,
        total_xp: u64,
        new_level: Option<u32>,
    },
    /// An actor levelled up.
    LevelUp {
        actor: ActorId,
        new_level: u32,
        skill_points_granted: u32,
        marks_granted: u32,
    },
    /// An actor gained a Mark (perk).
    MarkGained {
        actor: ActorId,
        mark_id: String,
        level: u32,
    },
    /// An actor spent a skill point on a skill line.
    SkillPointSpent {
        actor: ActorId,
        skill: String,
        new_level: u32,
    },
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::TurnBegin { actor, tick } => write!(f, "event: TurnBegin actor={actor} tick={tick}"),
            Event::TurnEnd { actor, tick } => write!(f, "event: TurnEnd actor={actor} tick={tick}"),
            Event::Moved { actor, from, to } => {
                write!(f, "event: Moved actor={actor} from={from} to={to}")
            }
            Event::StanceChanged { actor, stance } => {
                write!(f, "event: StanceChanged actor={actor} stance={stance}")
            }
            Event::FacingChanged { actor, facing } => {
                write!(f, "event: FacingChanged actor={actor} facing={facing}")
            }
            Event::Fired { actor, target } => {
                write!(f, "event: Fired actor={actor} target={target}")
            }
            Event::Misfire { actor } => write!(f, "event: Misfire actor={actor}"),
            Event::Jammed { actor } => write!(f, "event: Jammed actor={actor}"),
            Event::Missed { actor, target } => {
                write!(f, "event: Missed actor={actor} target={target}")
            }
            Event::ShotHit { actor, target, hit } => {
                write!(f, "event: ShotHit actor={actor} target={target} hit={hit}")
            }
            Event::HitLocation { actor, location } => {
                write!(f, "event: HitLocation actor={actor} location={location}")
            }
            Event::DamageApplied { actor, damage } => {
                write!(f, "event: DamageApplied actor={actor} damage={damage}")
            }
            Event::WoundApplied { actor, wound } => {
                write!(f, "event: WoundApplied actor={actor} wound={wound}")
            }
            Event::Critical { actor, effect } => {
                write!(f, "event: Critical actor={actor} effect={effect}")
            }
            Event::WeaponDropped { actor, item } => {
                write!(f, "event: WeaponDropped actor={actor} item={item}")
            }
            Event::SmokeDeposited { tile, density } => {
                write!(f, "event: SmokeDeposited tile={tile} density={density}")
            }
            Event::SmokeDecayed { tile, density } => {
                write!(f, "event: SmokeDecayed tile={tile} density={density}")
            }
            Event::SmokeDrifted { from, to, density } => {
                write!(f, "event: SmokeDrifted from={from} to={to} density={density}")
            }
            Event::OverwatchSet {
                actor,
                reaction_points,
            } => write!(
                f,
                "event: OverwatchSet actor={actor} reaction_points={reaction_points}"
            ),
            Event::ReactionShot { actor, target } => {
                write!(f, "event: ReactionShot actor={actor} target={target}")
            }
            Event::DynamiteLit {
                actor,
                tile,
                detonate_at,
            } => write!(
                f,
                "event: DynamiteLit actor={actor} tile={tile} detonate_at={detonate_at}"
            ),
            Event::DynamiteCaught { actor, tile } => {
                write!(f, "event: DynamiteCaught actor={actor} tile={tile}")
            }
            Event::DynamiteRethrown {
                actor,
                tile,
                detonate_at,
            } => write!(
                f,
                "event: DynamiteRethrown actor={actor} tile={tile} detonate_at={detonate_at}"
            ),
            Event::DynamiteExploded { actor, tile } => {
                write!(f, "event: DynamiteExploded actor={actor} tile={tile}")
            }
            Event::CoverDamaged {
                tile,
                facing,
                level,
            } => write!(
                f,
                "event: CoverDamaged tile={tile} facing={facing} level={level}"
            ),
            Event::Revealed { actor, until_tick } => {
                write!(f, "event: Revealed actor={actor} until_tick={until_tick}")
            }
            Event::TrackLeft { actor, tile } => {
                write!(f, "event: TrackLeft actor={actor} tile={tile}")
            }
            Event::SandLost { actor, amount } => {
                write!(f, "event: SandLost actor={actor} amount={amount}")
            }
            Event::SandGained { actor, amount } => {
                write!(f, "event: SandGained actor={actor} amount={amount}")
            }
            Event::MoraleStateChanged { actor, state } => {
                write!(f, "event: MoraleStateChanged actor={actor} state={state}")
            }
            Event::Routed { actor } => write!(f, "event: Routed actor={actor}"),
            Event::ActorKilled { actor } => write!(f, "event: ActorKilled actor={actor}"),
            Event::CompanionKilled { id } => write!(f, "event: CompanionKilled id={id}"),
            Event::ObjectiveComplete { id } => write!(f, "event: ObjectiveComplete id={id}"),
            Event::LedgerEntryWritten { index } => {
                write!(f, "event: LedgerEntryWritten index={index}")
            }
            Event::ScenarioEnded { outcome } => {
                write!(f, "event: ScenarioEnded outcome={outcome}")
            }
            Event::XpGained {
                actor,
                xp,
                total_xp,
                new_level,
            } => write!(
                f,
                "event: XpGained actor={actor} xp={xp} total_xp={total_xp} new_level={new_level:?}"
            ),
            Event::LevelUp {
                actor,
                new_level,
                skill_points_granted,
                marks_granted,
            } => write!(
                f,
                "event: LevelUp actor={actor} new_level={new_level} sp={skill_points_granted} marks={marks_granted}"
            ),
            Event::MarkGained {
                actor,
                mark_id,
                level,
            } => write!(f, "event: MarkGained actor={actor} mark={mark_id} level={level}"),
            Event::SkillPointSpent {
                actor,
                skill,
                new_level,
            } => write!(
                f,
                "event: SkillPointSpent actor={actor} skill={skill} new_level={new_level}"
            ),
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
        assert_eq!(
            e.to_string(),
            "event: HitLocation actor=ActorId(42) location=Head"
        );
    }

    #[test]
    fn event_damage_applied_format() {
        let e = Event::DamageApplied {
            actor: ActorId(7),
            damage: 15,
        };
        assert_eq!(
            e.to_string(),
            "event: DamageApplied actor=ActorId(7) damage=15"
        );
    }

    #[test]
    fn event_wound_applied_format() {
        let e = Event::WoundApplied {
            actor: ActorId(3),
            wound: WoundType::Bleeding,
        };
        assert_eq!(
            e.to_string(),
            "event: WoundApplied actor=ActorId(3) wound=Bleeding"
        );
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
        assert_eq!(e.to_string(), "event: CompanionKilled id=companion_wyatt");
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
