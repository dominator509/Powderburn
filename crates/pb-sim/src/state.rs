//! Simulation state types for the POWDERBURN kernel.
//!
//! M2: Sequence clock, AP economy, actor state.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use pb_core::event::WoundType;
use pb_core::geom::{Facing, TileXY};
use pb_core::ids::{ActorId, Ap, Tick};

/// An actor's stance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stance {
    Standing,
    Crouched,
    Prone,
}

/// Per-actor state within the simulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorState {
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
    /// Active wounds.
    pub wounds: Vec<WoundType>,
    /// Current Sand (morale) value.
    pub sand: i32,
    /// Maximum Sand.
    pub max_sand: i32,
    /// Current stance.
    pub stance: Stance,
}

/// Top-level simulation state.
#[derive(Debug, Clone)]
pub struct SimState {
    /// Current simulation tick.
    pub tick: Tick,
    /// All actors in the simulation, keyed by ActorId.
    pub actors: BTreeMap<ActorId, ActorState>,
    /// Sequence clock: the next tick at which each actor may act.
    pub sequence_clock: BTreeMap<ActorId, u64>,
    /// Scenario seed for deterministic RNG.
    pub seed: u64,
    /// Scenario identifier.
    pub scenario_id: u32,
    /// Current wind speed (0-10), influences AP grant.
    pub wind_speed: i32,
}

impl SimState {
    /// Create a new empty simulation state.
    pub fn new(seed: u64, scenario_id: u32) -> Self {
        SimState {
            tick: Tick(0),
            actors: BTreeMap::new(),
            sequence_clock: BTreeMap::new(),
            seed,
            scenario_id,
            wind_speed: 0,
        }
    }
}

/// Errors that can arise during simulation stepping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimError {
    /// The actor does not have enough AP to perform the action.
    InsufficientAp { actor: ActorId, have: Ap, need: Ap },
    /// The actor is not alive.
    ActorDead(ActorId),
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
}
