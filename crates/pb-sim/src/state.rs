//! Simulation state types for the POWDERBURN kernel.
//!
//! M2: Sequence clock, AP economy, actor state.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use pb_core::event::{HitLocationType, WoundType};
use pb_core::geom::{Facing, TileXY};
use pb_core::ids::{ActorId, Ap, Tick};

use crate::progression::ActorProgression;

/// An actor's stance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Stance {
    Standing,
    Crouched,
    Prone,
}

/// Directional cover strength attached to a tile edge.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum CoverLevel {
    None,
    Soft,
    Hard,
    Full,
}

/// A canonical directional tile-edge key.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct CoverEdge {
    pub tile: TileXY,
    pub facing: Facing,
}

/// Mutable cover state. Soft cover collapses on its third strike; hard cover
/// steps down to soft when hit by an explosive.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CoverState {
    pub level: CoverLevel,
    #[serde(default)]
    pub strikes: u8,
    #[serde(default)]
    pub half_height: bool,
    #[serde(default)]
    pub burning: bool,
}

/// Location-aware and timed wound consequences that cannot be represented by
/// the persistent wound-name list alone.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WoundEffects {
    #[serde(default)]
    pub concussed_turns: u8,
    #[serde(default)]
    pub heavy_bleeding: bool,
    #[serde(default)]
    pub winded: bool,
    #[serde(default)]
    pub broken_locations: BTreeMap<HitLocationType, u8>,
}

/// A thrown stick waiting for its hash-covered fuse deadline.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PendingExplosive {
    pub thrower: ActorId,
    pub position: TileXY,
    pub detonate_at: u64,
}

/// Deterministic combat fields copied from an authored weapon record.
///
/// Keeping the profile in simulation state preserves the crate import law:
/// `pb-sim` never imports `pb-content`, while every runtime rule still comes
/// from the content bundle selected by the client.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WeaponProfile {
    pub damage_count: i32,
    pub damage_sides: i32,
    pub damage_bonus: i32,
    pub accuracy: i32,
    /// Authored action-name to AP cost overrides.
    #[serde(default)]
    pub ap_overrides: BTreeMap<String, i16>,
    /// Close, medium, long, and extreme thresholds in tiles. Point blank is
    /// the shooter's adjacent tile.
    pub range_bands: [i32; 4],
    pub reload_class: String,
    pub fouling_rate: i32,
    pub base_misfire: i32,
    pub smoke_output: i32,
    pub two_handed: bool,
}

impl Default for WeaponProfile {
    fn default() -> Self {
        // Compatibility profile for legacy saves and isolated kernel tests.
        // Production actor construction always replaces this from content.
        Self {
            damage_count: 1,
            damage_sides: 8,
            damage_bonus: 2,
            accuracy: 5,
            ap_overrides: BTreeMap::new(),
            range_bands: [5, 15, 25, 40],
            reload_class: "CapAndBall".to_string(),
            fouling_rate: 3,
            base_misfire: 4,
            smoke_output: 1,
            two_handed: false,
        }
    }
}

/// Per-actor state within the simulation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ActorState {
    /// Authored faction identifier; simulation relationships never depend on
    /// display-name conventions.
    #[serde(default)]
    pub faction_id: String,
    /// Whether permanent-death campaign rules apply to this actor.
    #[serde(default)]
    pub is_companion: bool,
    /// Seven authored character attributes used by all derived statistics.
    #[serde(default)]
    pub attributes: pb_core::Attributes,
    /// Current action points.
    pub ap: Ap,
    /// Position on the tile grid.
    pub position: TileXY,
    /// Facing direction.
    pub facing: Facing,
    /// Sequence value (higher = faster turns).
    pub sequence: i32,
    /// Current hit points.
    pub hit_points: i32,
    /// Maximum hit points.
    pub max_hp: i32,
    /// Display name.
    pub name: String,
    /// Whether the actor is alive.
    pub alive: bool,
    /// Whether morale has driven the actor off the active battlefield.
    #[serde(default)]
    pub routed: bool,
    /// Active wounds.
    pub wounds: Vec<WoundType>,
    /// Current Sand (morale) value.
    pub sand: i32,
    /// Maximum Sand.
    pub max_sand: i32,
    /// Current stance.
    pub stance: Stance,
    /// Character progression data (XP, level, skills, marks, way).
    pub progression: ActorProgression,
    /// Weapon ID string.
    pub weapon: String,
    /// Hash-covered authored weapon rules used by the combat kernel.
    #[serde(default)]
    pub weapon_profile: WeaponProfile,
    /// Rounds loaded in weapon.
    pub loaded_rounds: i32,
    /// Weapon capacity.
    pub weapon_capacity: i32,
    /// Fouling level (0-10, affects misfire).
    pub fouling: i32,
    /// Whether weapon is jammed.
    pub jammed: bool,
}

/// Top-level simulation state.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SimState {
    /// Current simulation tick.
    pub tick: Tick,
    /// Last tick at which deterministic environment effects were processed.
    #[serde(default)]
    pub environment_tick: u64,
    /// All actors in the simulation, keyed by ActorId.
    pub actors: BTreeMap<ActorId, ActorState>,
    /// Sequence clock: the next tick at which each actor may act.
    pub sequence_clock: BTreeMap<ActorId, u64>,
    /// Actor whose current turn is open. While set, additional commands from
    /// that actor execute at the same tick until Hold or AP exhaustion.
    #[serde(default)]
    pub active_actor: Option<ActorId>,
    /// Number of commands already completed in the open actor turn.
    #[serde(default)]
    pub active_turn_actions: u16,
    /// Scenario seed for deterministic RNG.
    pub seed: u64,
    /// Scenario identifier.
    pub scenario_id: u32,
    /// Current wind speed (0-10), influences AP grant.
    pub wind_speed: i32,
    /// Authored battlefield light level.
    #[serde(default)]
    pub light_level: crate::environment::LightLevel,
    /// Authored battlefield weather.
    #[serde(default)]
    pub weather: crate::environment::Weather,
    /// Authored wind direction used by smoke drift.
    #[serde(default = "default_wind_direction")]
    pub wind_direction: Facing,
    /// Actors currently on overwatch (DrawBead).
    pub overwatch: BTreeSet<ActorId>,
    /// AP deliberately converted to Overwatch reaction points.
    #[serde(default)]
    pub reaction_points: BTreeMap<ActorId, u8>,
    /// Actors with the temporary -3 Evasion penalty after Sprinting.
    #[serde(default)]
    pub sprinting: BTreeSet<ActorId>,
    /// Timed and location-specific wound state.
    #[serde(default)]
    pub wound_effects: BTreeMap<ActorId, WoundEffects>,
    /// Mandatory AP that a Broken actor must spend retreating this turn.
    #[serde(default)]
    pub broken_retreat_remaining: BTreeMap<ActorId, i16>,
    /// Dynamite sticks whose fuses are still burning.
    #[serde(default)]
    pub pending_explosives: Vec<PendingExplosive>,
    /// Mid-fuse sticks successfully caught and not yet rethrown.
    #[serde(default)]
    pub held_explosives: BTreeMap<ActorId, PendingExplosive>,
    /// Authored difficult terrain tiles.
    #[serde(default)]
    pub difficult_tiles: BTreeSet<TileXY>,
    /// Authored terrain identity retained for deterministic capture and resume.
    ///
    /// Gameplay cost is represented separately by `difficult_tiles`; content
    /// hashes bind this presentation metadata, so it is not part of the
    /// terminal simulation hash.
    #[serde(default)]
    pub terrain_tiles: BTreeMap<TileXY, String>,
    /// Authored visual elevation retained for battlefield presentation.
    #[serde(default)]
    pub tile_elevations: BTreeMap<TileXY, i32>,
    /// Directional, degradable cover attached to tile edges.
    #[serde(default)]
    pub cover_edges: BTreeMap<CoverEdge, CoverState>,
    /// Company or faction leaders whose fall applies the additional Sand loss.
    #[serde(default)]
    pub leaders: BTreeSet<ActorId>,
    /// Per-actor authored Sand loss multiplier in percent.
    #[serde(default)]
    pub sand_multiplier_pct: BTreeMap<ActorId, i32>,
    /// Per-actor carried weight in pounds; capacity is derived from GRIT.
    #[serde(default)]
    pub carry_weight_lbs: BTreeMap<ActorId, i32>,
    /// Authored equipped armor damage resistance.
    #[serde(default)]
    pub armor_damage_resist: BTreeMap<ActorId, i32>,
    /// Night muzzle-flash reveal deadlines.
    #[serde(default)]
    pub revealed_until: BTreeMap<ActorId, u64>,
    /// Bounded snow tracks, oldest first.
    #[serde(default)]
    pub snow_tracks: Vec<(ActorId, TileXY)>,
    /// One-use-per-battle progression effects already consumed.
    #[serde(default)]
    pub consumed_battle_effects: BTreeSet<(ActorId, String)>,
    /// Per-tile smoke density grid (col-major, density 0-6).
    pub smoke_grid: Vec<u8>,
    /// Grid columns for smoke.
    pub smoke_cols: u32,
    /// Grid rows for smoke.
    pub smoke_rows: u32,
}

impl SimState {
    /// Create a new empty simulation state.
    pub fn new(seed: u64, scenario_id: u32) -> Self {
        SimState {
            tick: Tick(0),
            environment_tick: 0,
            actors: BTreeMap::new(),
            sequence_clock: BTreeMap::new(),
            active_actor: None,
            active_turn_actions: 0,
            seed,
            scenario_id,
            wind_speed: 0,
            light_level: crate::environment::LightLevel::Day,
            weather: crate::environment::Weather::Clear,
            wind_direction: Facing::North,
            overwatch: BTreeSet::new(),
            reaction_points: BTreeMap::new(),
            sprinting: BTreeSet::new(),
            wound_effects: BTreeMap::new(),
            broken_retreat_remaining: BTreeMap::new(),
            pending_explosives: Vec::new(),
            held_explosives: BTreeMap::new(),
            difficult_tiles: BTreeSet::new(),
            terrain_tiles: BTreeMap::new(),
            tile_elevations: BTreeMap::new(),
            cover_edges: BTreeMap::new(),
            leaders: BTreeSet::new(),
            sand_multiplier_pct: BTreeMap::new(),
            carry_weight_lbs: BTreeMap::new(),
            armor_damage_resist: BTreeMap::new(),
            revealed_until: BTreeMap::new(),
            snow_tracks: Vec::new(),
            consumed_battle_effects: BTreeSet::new(),
            smoke_grid: vec![0u8; 20 * 12], // 20x12 grid
            smoke_cols: 20,
            smoke_rows: 12,
        }
    }
}

fn default_wind_direction() -> Facing {
    Facing::North
}

/// Errors that can arise during simulation stepping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimError {
    /// The actor does not have enough AP to perform the action.
    InsufficientAp { actor: ActorId, have: Ap, need: Ap },
    /// The actor is not alive.
    ActorDead(ActorId),
    /// A command was submitted for an actor other than the active actor.
    OutOfTurn { actor: ActorId, active: ActorId },
    /// The actor was not found.
    ActorNotFound(ActorId),
    /// The target actor was not found.
    TargetNotFound(ActorId),
    /// Shot is out of range.
    OutOfRange(ActorId),
    /// No line of sight.
    NoLineOfSight(ActorId, ActorId),
    /// Weapon is not loaded.
    WeaponNotLoaded(ActorId),
    /// Weapon is jammed and must be cleared before firing.
    WeaponJammed(ActorId),
    /// Movement must be exactly one tile, or exactly two for Sprint.
    InvalidMoveDistance { actor: ActorId, distance: i16 },
    /// Sprinting is only legal while Standing.
    CannotSprint(ActorId),
    /// A stance action requested the stance the actor already occupies.
    AlreadyInStance(ActorId, Stance),
    /// A location-specific wound prevents use of the equipped weapon.
    CannotUseWeapon(ActorId),
    /// A Broken actor must spend its first two AP retreating.
    MustRetreat(ActorId),
    /// A target must be adjacent for this action.
    NotAdjacent(ActorId, ActorId),
    /// No matching lit explosive exists at the requested tile.
    ExplosiveNotFound(TileXY),
    /// The actor failed or cannot attempt the mid-fuse catch.
    CannotCatchDynamite(ActorId),
    /// The requested one-use battle effect has already been consumed.
    BattleEffectSpent(ActorId),
    /// Movement cannot end on an occupied tile.
    TileOccupied(TileXY),
    /// Movement cannot leave the authored battlefield bounds.
    OutOfBounds(TileXY),
    /// The equipped weapon does not support the requested special action.
    InvalidWeaponAction(ActorId),
}
