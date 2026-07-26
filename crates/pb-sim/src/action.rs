//! Action types and cost calculation.
//!
//! M2: Defines every action an actor can take in a turn, the AP cost,
//! and the `step()` function that applies actions to simulation state.

#![forbid(unsafe_code)]

use pb_core::event::{Event, HitLocationType};
use pb_core::geom::TileXY;
use pb_core::ids::ActorId;
use pb_rng::{PbRng, StreamTag};

use crate::state::{ActorState, SimError, SimState};

/// The kind of action an actor can perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Move to an adjacent tile.
    Move(TileXY),
    /// Snap-shot a target (quick, less accurate).
    SnapShot(ActorId),
    /// Aimed shot (takes more time, more accurate).
    AimedShot(ActorId),
    /// Called shot to a specific hit location.
    CalledShot(ActorId, HitLocationType),
    /// Reload the weapon.
    Reload,
    /// Hold (end turn, keep remaining AP for next turn).
    Hold,
    /// Bandage a wounded actor.
    Bandage(ActorId),
    /// Throw dynamite at a tile.
    ThrowDynamite(TileXY),
    /// Melee attack.
    Melee(ActorId),
    /// Use a generic item.
    UseItem,
}

/// A command submitted to the simulation step function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    /// The actor performing the action.
    pub actor_id: ActorId,
    /// The action to perform.
    pub action: Action,
}

/// Compute the AP cost of an action given the actor's state.
pub fn action_cost(action: &Action, actor: &ActorState) -> pb_core::ids::Ap {
    use pb_core::ids::Ap;
    match action {
        // Move costs depend on distance and terrain
        Action::Move(target) => {
            let dist = actor.position.chebyshev_distance(*target);
            if dist == 0 {
                Ap(0) // staying in place
            } else if dist == 1 {
                // Adjacent: Ap(1)
                Ap(1)
            } else if dist == 2 {
                // Sprint (2 tiles): Ap(4)
                Ap(4)
            } else {
                // Multi-tile movement: Ap(1) per tile
                Ap(dist.min(6)) // max 6 AP for movement
            }
        }
        Action::SnapShot(_) => Ap(3),
        Action::AimedShot(_) => Ap(4),
        Action::CalledShot(_, _) => Ap(5),
        Action::Reload => Ap(3), // cartridge reload
        Action::Hold => Ap(0),
        Action::Bandage(_) => Ap(4),
        Action::ThrowDynamite(_) => Ap(4),
        Action::Melee(_) => Ap(3),
        Action::UseItem => Ap(2),
    }
}

/// Step the simulation forward by applying a command.
///
/// Returns a vector of events that describe what happened.
/// Returns `Err(SimError)` if the action cannot be performed.
pub fn step(state: &mut SimState, cmd: Command) -> Result<Vec<Event>, SimError> {
    let actor = state
        .actors
        .get(&cmd.actor_id)
        .ok_or(SimError::ActorNotFound(cmd.actor_id))?;

    if !actor.alive {
        return Err(SimError::ActorDead(cmd.actor_id));
    }

    let cost = action_cost(&cmd.action, actor);
    if actor.ap < cost {
        return Err(SimError::InsufficientAp {
            actor: cmd.actor_id,
            have: actor.ap,
            need: cost,
        });
    }

    // Deduct AP
    let new_ap = pb_core::ids::Ap(actor.ap.0 - cost.0);
    if let Some(actor_mut) = state.actors.get_mut(&cmd.actor_id) {
        actor_mut.ap = new_ap;
    }

    match cmd.action {
        Action::Move(target) => Ok(execute_move(state, cmd.actor_id, target)),
        Action::SnapShot(target) => Ok(execute_shot(state, cmd.actor_id, target, false, None)),
        Action::AimedShot(target) => Ok(execute_shot(state, cmd.actor_id, target, true, None)),
        Action::CalledShot(target, loc) => {
            Ok(execute_shot(state, cmd.actor_id, target, true, Some(loc)))
        }
        Action::Reload => Ok(execute_reload(state, cmd.actor_id)),
        Action::Hold => Ok(vec![]), // Hold emits no events
        Action::Bandage(target) => Ok(execute_bandage(state, cmd.actor_id, target)),
        Action::ThrowDynamite(_) => Ok(vec![]), // stub
        Action::Melee(target) => Ok(execute_melee(state, cmd.actor_id, target)),
        Action::UseItem => Ok(vec![]), // stub
    }
}

/// Execute a move action.
fn execute_move(state: &mut SimState, actor_id: ActorId, target: TileXY) -> Vec<Event> {
    if let Some(actor) = state.actors.get_mut(&actor_id) {
        actor.position = target;
    }
    vec![]
}

/// Execute a shot action.
///
/// This is a simplified shot pipeline for M2. The full ten-stage pipeline
/// lives in `crate::shot`.
fn execute_shot(
    state: &mut SimState,
    actor_id: ActorId,
    target: ActorId,
    aimed: bool,
    called: Option<HitLocationType>,
) -> Vec<Event> {
    // Check target exists
    let target_alive = state.actors.get(&target).is_some_and(|a| a.alive);

    if !target_alive {
        return vec![];
    }

    // Simple hit check: use RNG
    let tick = state.tick.0;
    let seed = state.seed;
    let scenario = state.scenario_id;
    let actor_num = actor_id.0;

    // Aimed shots get +15 to hit bonus
    let base_hit: i32 = if aimed { 75 } else { 60 };
    let hit_roll = PbRng::draw(seed, scenario, tick, actor_num, StreamTag::ToHit, 0, 99);
    let hit = hit_roll < base_hit;

    let mut events = vec![Event::ShotHit {
        actor: actor_id,
        target,
        hit,
    }];

    if hit {
        // If called shot, use the specified location; otherwise random
        let location = if let Some(loc) = called {
            loc
        } else {
            let loc_roll = PbRng::draw(seed, scenario, tick, actor_num, StreamTag::Damage, 0, 99);
            if loc_roll < 10 {
                HitLocationType::Head
            } else if loc_roll < 13 {
                HitLocationType::Eyes
            } else if loc_roll < 48 {
                HitLocationType::Torso
            } else if loc_roll < 60 {
                HitLocationType::Vitals
            } else if loc_roll < 75 {
                HitLocationType::GunArm
            } else if loc_roll < 85 {
                HitLocationType::OffArm
            } else {
                HitLocationType::Legs
            }
        };

        events.push(Event::HitLocation {
            actor: target,
            location,
        });

        // Simple damage
        let damage = PbRng::draw(seed, scenario, tick, actor_num, StreamTag::Damage, 5, 15);
        events.push(Event::DamageApplied {
            actor: target,
            damage,
        });

        // If damage > 0, apply wound
        if damage > 5 {
            let wound_type = pb_core::event::WoundType::Bleeding;
            events.push(Event::WoundApplied {
                actor: target,
                wound: wound_type,
            });

            if let Some(t_actor) = state.actors.get_mut(&target) {
                t_actor.wounds.push(wound_type);
                t_actor.hit_points -= damage;
                if t_actor.hit_points <= 0 {
                    t_actor.alive = false;
                    events.push(Event::ActorKilled { actor: target });
                }
            }
        }
    }

    events
}

/// Execute a reload action (stub).
fn execute_reload(_state: &mut SimState, _actor_id: ActorId) -> Vec<Event> {
    vec![]
}

/// Execute a bandage action (stub).
fn execute_bandage(_state: &mut SimState, _actor_id: ActorId, _target: ActorId) -> Vec<Event> {
    vec![]
}

/// Execute a melee action (stub).
fn execute_melee(_state: &mut SimState, _actor_id: ActorId, _target: ActorId) -> Vec<Event> {
    vec![]
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::manual_range_contains
)]
mod tests {
    use super::*;
    use crate::state::Stance;
    use pb_core::geom::Facing;
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
        }
    }

    #[test]
    fn action_cost_snap_shot() {
        let actor = make_actor();
        assert_eq!(action_cost(&Action::SnapShot(ActorId(1)), &actor), Ap(3));
    }

    #[test]
    fn action_cost_aimed_shot() {
        let actor = make_actor();
        assert_eq!(action_cost(&Action::AimedShot(ActorId(1)), &actor), Ap(4));
    }

    #[test]
    fn action_cost_called_shot() {
        let actor = make_actor();
        assert_eq!(
            action_cost(
                &Action::CalledShot(ActorId(1), HitLocationType::Head),
                &actor
            ),
            Ap(5)
        );
    }

    #[test]
    fn action_cost_melee() {
        let actor = make_actor();
        assert_eq!(action_cost(&Action::Melee(ActorId(1)), &actor), Ap(3));
    }

    #[test]
    fn action_cost_reload() {
        let actor = make_actor();
        assert_eq!(action_cost(&Action::Reload, &actor), Ap(3));
    }

    #[test]
    fn action_cost_move_adjacent() {
        let actor = make_actor();
        let target = TileXY::new(1, 0);
        assert_eq!(action_cost(&Action::Move(target), &actor), Ap(1));
    }

    #[test]
    fn action_cost_move_sprint() {
        let actor = make_actor();
        let target = TileXY::new(2, 0);
        assert_eq!(action_cost(&Action::Move(target), &actor), Ap(4));
    }

    #[test]
    fn step_returns_insufficient_ap() {
        let mut state = SimState::new(42, 1);
        let id = ActorId(1);
        let mut actor = make_actor();
        actor.ap = Ap(2);
        state.actors.insert(id, actor);

        let cmd = Command {
            actor_id: id,
            action: Action::SnapShot(ActorId(2)),
        };

        let result = step(&mut state, cmd);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            SimError::InsufficientAp {
                actor: id,
                have: Ap(2),
                need: Ap(3),
            }
        );
    }

    #[test]
    fn step_deducts_ap_on_success() {
        let mut state = SimState::new(42, 1);
        let id = ActorId(1);
        let actor = make_actor(); // Ap(10)
        state.actors.insert(id, actor);
        let target = ActorId(2);
        let mut target_actor = make_actor();
        target_actor.alive = true;
        state.actors.insert(target, target_actor);

        let cmd = Command {
            actor_id: id,
            action: Action::SnapShot(target),
        };

        let result = step(&mut state, cmd);
        assert!(result.is_ok());
        assert_eq!(state.actors[&id].ap, Ap(7)); // 10 - 3
    }

    #[test]
    fn step_actor_not_found() {
        let mut state = SimState::new(42, 1);
        let cmd = Command {
            actor_id: ActorId(99),
            action: Action::Hold,
        };
        assert_eq!(
            step(&mut state, cmd).unwrap_err(),
            SimError::ActorNotFound(ActorId(99))
        );
    }

    #[test]
    fn step_dead_actor() {
        let mut state = SimState::new(42, 1);
        let id = ActorId(1);
        let mut actor = make_actor();
        actor.alive = false;
        state.actors.insert(id, actor);

        let cmd = Command {
            actor_id: id,
            action: Action::Hold,
        };
        assert_eq!(step(&mut state, cmd).unwrap_err(), SimError::ActorDead(id));
    }
}
