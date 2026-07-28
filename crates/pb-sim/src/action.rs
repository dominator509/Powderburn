//! Action types and cost calculation.
//!
//! M2: Defines every action an actor can take in a turn, the AP cost,
//! and the `step()` function that applies actions to simulation state.

#![forbid(unsafe_code)]

use pb_core::event::{Event, HitLocationType, WoundType};
use pb_core::geom::{Facing, TileXY};
use pb_core::ids::ActorId;
use pb_core::metrics::MetricsRegistry;

use crate::state::{ActorState, SimError, SimState, Stance};

/// The kind of action an actor can perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Move to an adjacent tile.
    Move(TileXY),
    /// Change facing without spending AP.
    Face(Facing),
    /// Sprint exactly two tiles, ending the turn and reducing Evasion until
    /// the actor's next turn.
    Sprint(TileXY),
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
    /// Catch a lit stick on an adjacent tile.
    CatchDynamite(TileXY),
    /// Rethrow a previously caught stick without resetting its fuse.
    RethrowDynamite(TileXY),
    /// Melee attack.
    Melee(ActorId),
    /// Use a generic item.
    UseItem,
    // --- SPEC-001 additions ------------------------------------------------
    /// Change stance to Crouched.
    StanceCrouch,
    /// Change stance to Prone.
    StanceProne,
    /// Rise from Prone to Standing.
    RiseFromProne,
    /// Fan the hammer: 3 quick shots at -25 accuracy each (single-action revolver only).
    FanHammer(ActorId),
    /// A same-tick squad order: every eligible squad member fires.
    Volley(ActorId),
    /// Draw and fire once per battle using the Left-Hand Draw Mark.
    LeftHandDraw(ActorId),
    /// Cap and ball reload (slow, 8 AP).
    CapAndBallReload,
    /// Clear a weapon jam.
    ClearJam,
    /// Draw bead / overwatch: consumes all remaining AP + 2 minimum.
    DrawBead(ActorId),
    /// Rally a target: restore Sand.
    Rally(ActorId),
    /// Loot an adjacent actor or tile.
    Loot(ActorId),
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
    let default = match action {
        Action::Move(_) => Ap(match actor.stance {
            Stance::Standing => 1,
            Stance::Crouched => 2,
            Stance::Prone => 3,
        }),
        Action::Face(_) => Ap(0),
        Action::Sprint(_) => Ap(2),
        Action::SnapShot(_) => Ap(3),
        Action::AimedShot(_) => Ap(4),
        Action::CalledShot(_, _) => Ap(5),
        Action::Reload => Ap(3), // cartridge reload
        Action::Hold => Ap(0),
        Action::Bandage(_) => Ap(4),
        Action::ThrowDynamite(_) => Ap(4),
        Action::CatchDynamite(_) => Ap(2),
        Action::RethrowDynamite(_) => Ap(2),
        Action::Melee(_) => Ap(3),
        Action::UseItem => Ap(2),
        // SPEC-001 costs
        Action::StanceCrouch => Ap(1),
        Action::StanceProne => Ap(2),
        Action::RiseFromProne => Ap(2),
        Action::FanHammer(_) => Ap(6),
        Action::Volley(_) => Ap(0),
        Action::LeftHandDraw(_) => Ap(3),
        Action::CapAndBallReload => Ap(8),
        Action::ClearJam => Ap(4),
        Action::DrawBead(_) => Ap(2), // minimum cost; remaining AP consumed in execution
        Action::Rally(_) => Ap(3),
        Action::Loot(_) => Ap(2),
    };
    let override_key = match action {
        Action::SnapShot(_) | Action::AimedShot(_) | Action::CalledShot(_, _) => Some("single"),
        Action::Reload | Action::CapAndBallReload => Some("reload"),
        Action::FanHammer(_) => Some("fan"),
        _ => None,
    };
    override_key
        .and_then(|key| actor.weapon_profile.ap_overrides.get(key).copied())
        .map(Ap)
        .unwrap_or(default)
}

/// Compute the exact AP cost that `step` will charge after terrain, wounds,
/// and progression effects. Clients use this for truthful movement previews.
pub fn effective_action_cost(
    state: &SimState,
    actor_id: ActorId,
    action: &Action,
) -> Result<pb_core::ids::Ap, SimError> {
    let actor = state
        .actors
        .get(&actor_id)
        .ok_or(SimError::ActorNotFound(actor_id))?;
    let mut cost = action_cost(action, actor);
    if let Action::Move(target) = action {
        if state.difficult_tiles.contains(target)
            && !actor.progression.has_passive("difficult_terrain_normal")
        {
            cost.0 = cost.0.saturating_mul(2);
        }
    }
    if matches!(action, Action::Move(_) | Action::Sprint(_))
        && state
            .wound_effects
            .get(&actor_id)
            .and_then(|effects| effects.broken_locations.get(&HitLocationType::Legs))
            .is_some_and(|count| *count > 0)
    {
        cost.0 = cost.0.saturating_mul(2);
    }
    if matches!(action, Action::Reload | Action::CapAndBallReload)
        && state
            .wound_effects
            .get(&actor_id)
            .and_then(|effects| effects.broken_locations.get(&HitLocationType::OffArm))
            .is_some_and(|count| *count > 0)
    {
        cost.0 = cost.0.saturating_mul(2);
    }
    if matches!(action, Action::AimedShot(_)) && actor.progression.has_passive("aimed_shot_cost_3")
    {
        cost = pb_core::ids::Ap(3);
    }
    if matches!(action, Action::Reload | Action::CapAndBallReload)
        && actor.progression.has_passive("reload_cost_minus_1")
    {
        cost.0 = cost.0.saturating_sub(1).max(1);
    }
    if matches!(action, Action::FanHammer(_))
        && actor.progression.has_passive("fan_hammer_improved")
    {
        cost.0 = cost.0.saturating_sub(1).max(1);
    }
    Ok(cost)
}

/// Update the `sim.actors.alive` and `progression.xp.total` metric gauges
/// based on the current simulation state.
pub(crate) fn update_alive_and_xp(state: &SimState) {
    let alive_count = state.actors.values().filter(|a| a.alive).count() as u64;
    let total_xp: u64 = state.actors.values().map(|a| a.progression.xp).sum();
    let registry = MetricsRegistry::global();
    registry.set_sim_actors_alive(alive_count);
    registry.set_progression_xp_total(total_xp);
}

/// Step the simulation forward by applying a command.
///
/// Returns a vector of events that describe what happened.
/// Returns `Err(SimError)` if the action cannot be performed.
pub fn step(state: &mut SimState, cmd: Command) -> Result<Vec<Event>, SimError> {
    if let Some(active) = state.active_actor {
        if active != cmd.actor_id {
            return Err(SimError::OutOfTurn {
                actor: cmd.actor_id,
                active,
            });
        }
    }
    let managed_turn = state.active_actor == Some(cmd.actor_id);
    let first_action = !managed_turn || state.active_turn_actions == 0;

    let actor = state
        .actors
        .get(&cmd.actor_id)
        .ok_or(SimError::ActorNotFound(cmd.actor_id))?;

    if !actor.alive {
        return Err(SimError::ActorDead(cmd.actor_id));
    }
    match &cmd.action {
        Action::Move(target) => {
            let distance = actor.position.chebyshev_distance(*target);
            if distance != 1 {
                return Err(SimError::InvalidMoveDistance {
                    actor: cmd.actor_id,
                    distance,
                });
            }
            validate_destination(state, cmd.actor_id, *target)?;
        }
        Action::Sprint(target) => {
            let distance = actor.position.chebyshev_distance(*target);
            if distance != 2 {
                return Err(SimError::InvalidMoveDistance {
                    actor: cmd.actor_id,
                    distance,
                });
            }
            if actor.stance != Stance::Standing {
                return Err(SimError::CannotSprint(cmd.actor_id));
            }
            validate_destination(state, cmd.actor_id, *target)?;
            if state
                .wound_effects
                .get(&cmd.actor_id)
                .and_then(|effects| effects.broken_locations.get(&HitLocationType::Legs))
                .is_some_and(|count| *count > 0)
            {
                return Err(SimError::CannotSprint(cmd.actor_id));
            }
        }
        Action::Melee(target) | Action::Loot(target) | Action::Bandage(target) => {
            let target_actor = state
                .actors
                .get(target)
                .ok_or(SimError::TargetNotFound(*target))?;
            if actor.position.chebyshev_distance(target_actor.position) > 1 {
                return Err(SimError::NotAdjacent(cmd.actor_id, *target));
            }
        }
        Action::FanHammer(_) => {
            let profile = &actor.weapon_profile;
            if profile.two_handed
                || actor.weapon_capacity < 3
                || !matches!(profile.reload_class.as_str(), "CapAndBall" | "Cartridge")
            {
                return Err(SimError::InvalidWeaponAction(cmd.actor_id));
            }
        }
        Action::Reload if actor.weapon_profile.reload_class == "CapAndBall" => {
            return Err(SimError::InvalidWeaponAction(cmd.actor_id));
        }
        Action::CapAndBallReload if actor.weapon_profile.reload_class != "CapAndBall" => {
            return Err(SimError::InvalidWeaponAction(cmd.actor_id));
        }
        Action::ClearJam if !actor.jammed => {
            return Err(SimError::InvalidWeaponAction(cmd.actor_id));
        }
        Action::DrawBead(_) => {
            let morale = crate::morale::morale_state(actor.sand, actor.max_sand);
            if matches!(
                morale,
                crate::morale::MoraleState::Rattled
                    | crate::morale::MoraleState::Broken
                    | crate::morale::MoraleState::Routed
            ) {
                return Err(SimError::InvalidWeaponAction(cmd.actor_id));
            }
        }
        _ => {}
    }

    let actor_ap = actor.ap;
    let cost = effective_action_cost(state, cmd.actor_id, &cmd.action)?;
    if is_weapon_action(&cmd.action) && weapon_is_blocked(state, cmd.actor_id) {
        return Err(SimError::CannotUseWeapon(cmd.actor_id));
    }
    enforce_broken_retreat(state, cmd.actor_id, &cmd.action)?;
    if actor_ap < cost {
        return Err(SimError::InsufficientAp {
            actor: cmd.actor_id,
            have: actor_ap,
            need: cost,
        });
    }

    // `step` is transactional: any error restores the byte-for-byte input state.
    let before = state.clone();
    let mut environment_events = advance_environment(state);
    if first_action {
        environment_events.extend(process_turn_start_wounds(state, cmd.actor_id));
    }
    if state
        .actors
        .get(&cmd.actor_id)
        .is_none_or(|actor| !actor.alive)
    {
        finalize_new_deaths(state, cmd.actor_id, &before, &mut environment_events);
        state.active_actor = None;
        state.active_turn_actions = 0;
        let mut events = vec![Event::TurnBegin {
            actor: cmd.actor_id,
            tick: state.tick.0,
        }];
        events.extend(environment_events);
        events.push(Event::TurnEnd {
            actor: cmd.actor_id,
            tick: state.tick.0,
        });
        apply_smoke_events(state, &events);
        update_alive_and_xp(state);
        return Ok(events);
    }

    // Deduct AP
    let new_ap = pb_core::ids::Ap(actor_ap.0 - cost.0);
    if let Some(actor_mut) = state.actors.get_mut(&cmd.actor_id) {
        actor_mut.ap = new_ap;
    }

    let declared_hold = matches!(&cmd.action, Action::Hold);
    let forced_turn_end = matches!(&cmd.action, Action::Sprint(_));
    let result = match cmd.action {
        Action::Move(target) => Ok(execute_move(state, cmd.actor_id, target)),
        Action::Face(facing) => Ok(execute_facing_change(state, cmd.actor_id, facing)),
        Action::Sprint(target) => Ok(execute_sprint(state, cmd.actor_id, target)),
        Action::SnapShot(target) => execute_shot(state, cmd.actor_id, target, false, None, 0),
        Action::AimedShot(target) => execute_shot(state, cmd.actor_id, target, true, None, 0),
        Action::CalledShot(target, loc) => {
            execute_shot(state, cmd.actor_id, target, true, Some(loc), 0)
        }
        Action::Reload => Ok(execute_reload(state, cmd.actor_id)),
        Action::Hold => Ok(execute_hold(state, cmd.actor_id)),
        Action::Bandage(target) => Ok(execute_bandage(state, cmd.actor_id, target)),
        Action::ThrowDynamite(target) => Ok(execute_dynamite(state, cmd.actor_id, target)),
        Action::CatchDynamite(tile) => execute_catch_dynamite(state, cmd.actor_id, tile),
        Action::RethrowDynamite(target) => execute_rethrow_dynamite(state, cmd.actor_id, target),
        Action::Melee(target) => Ok(execute_melee(state, cmd.actor_id, target)),
        Action::UseItem => Ok(execute_use_item(state, cmd.actor_id)),
        // SPEC-001 actions
        Action::StanceCrouch => Ok(execute_stance_change(state, cmd.actor_id, Stance::Crouched)),
        Action::StanceProne => Ok(execute_stance_change(state, cmd.actor_id, Stance::Prone)),
        Action::RiseFromProne => Ok(execute_stance_change(state, cmd.actor_id, Stance::Standing)),
        Action::FanHammer(target) => execute_fan_hammer(state, cmd.actor_id, target),
        Action::Volley(target) => execute_volley(state, cmd.actor_id, target),
        Action::LeftHandDraw(target) => execute_left_hand_draw(state, cmd.actor_id, target),
        Action::CapAndBallReload => Ok(execute_cap_and_ball_reload(state, cmd.actor_id)),
        Action::ClearJam => Ok(execute_clear_jam(state, cmd.actor_id)),
        Action::DrawBead(target) => Ok(execute_draw_bead(state, cmd.actor_id, target)),
        Action::Rally(target) => Ok(execute_rally(state, cmd.actor_id, target)),
        Action::Loot(target) => Ok(execute_loot(state, cmd.actor_id, target)),
    };

    let mut action_events = match result {
        Ok(events) => events,
        Err(error) => {
            *state = before;
            return Err(error);
        }
    };
    finalize_new_deaths(state, cmd.actor_id, &before, &mut action_events);
    consume_broken_retreat(state, cmd.actor_id, &cmd.action, cost.0);
    if declared_hold {
        if let Some(actor) = state.actors.get_mut(&cmd.actor_id) {
            actor.ap = pb_core::ids::Ap(0);
        }
    }
    let turn_ended = declared_hold
        || forced_turn_end
        || state
            .actors
            .get(&cmd.actor_id)
            .is_none_or(|actor| actor.ap.0 == 0 || !actor.alive || actor.routed);
    if managed_turn {
        if turn_ended {
            state.active_actor = None;
            state.active_turn_actions = 0;
        } else {
            state.active_turn_actions = state.active_turn_actions.saturating_add(1);
        }
    }
    let mut events = Vec::new();
    if first_action {
        events.push(Event::TurnBegin {
            actor: cmd.actor_id,
            tick: state.tick.0,
        });
    }
    events.append(&mut environment_events);
    events.extend(action_events);
    if !managed_turn || turn_ended {
        events.push(Event::TurnEnd {
            actor: cmd.actor_id,
            tick: state.tick.0,
        });
    }

    // Process smoke deposition events from the successful action.
    apply_smoke_events(state, &events);

    // Record metrics after successful execution
    let event_count = match u64::try_from(events.len()) {
        Ok(value) => value,
        Err(_) => u64::MAX,
    };
    let registry = MetricsRegistry::global();
    registry.record_events_per_turn(event_count);
    update_alive_and_xp(state);

    Ok(events)
}

fn apply_smoke_events(state: &mut SimState, events: &[Event]) {
    for event in events {
        if let Event::SmokeDeposited { tile, density } = event {
            let (Ok(x), Ok(y)) = (u32::try_from(tile.x), u32::try_from(tile.y)) else {
                continue;
            };
            if x >= state.smoke_cols || y >= state.smoke_rows {
                continue;
            }
            let idx = y as usize * state.smoke_cols as usize + x as usize;
            let added = u8::try_from(*density).unwrap_or(u8::MAX);
            state.smoke_grid[idx] = state.smoke_grid[idx].saturating_add(added).min(6);
        }
    }
}

fn advance_environment(state: &mut SimState) -> Vec<Event> {
    let previous_tick = state.environment_tick;
    let decay_interval = if state.weather == crate::environment::Weather::Rain {
        20
    } else {
        40
    };
    let previous_bucket = previous_tick / decay_interval;
    let current_bucket = state.tick.0 / decay_interval;
    let decay_steps = current_bucket
        .saturating_sub(previous_bucket)
        .min(u8::MAX as u64) as u8;
    let drift_steps = (state.tick.0 / 120).saturating_sub(previous_tick / 120);
    state.environment_tick = state.tick.0;
    let mut events = Vec::new();
    events.extend(detonate_due_explosives(state));
    if (decay_steps == 0 && drift_steps == 0) || state.smoke_cols == 0 {
        return events;
    }
    for (index, density) in state.smoke_grid.iter_mut().enumerate() {
        if *density == 0 {
            continue;
        }
        let next_density = density.saturating_sub(decay_steps);
        if next_density == *density {
            continue;
        }
        *density = next_density;
        let x = index % state.smoke_cols as usize;
        let y = index / state.smoke_cols as usize;
        let Ok(x) = i16::try_from(x) else {
            continue;
        };
        let Ok(y) = i16::try_from(y) else {
            continue;
        };
        events.push(Event::SmokeDecayed {
            tile: TileXY::new(x, y),
            density: u32::from(next_density),
        });
    }
    for _ in 0..drift_steps {
        let old_grid = state.smoke_grid.clone();
        let mut next_grid = vec![0u8; old_grid.len()];
        for (index, density) in old_grid.into_iter().enumerate() {
            if density == 0 {
                continue;
            }
            let x = index % state.smoke_cols as usize;
            let y = index / state.smoke_cols as usize;
            let Ok(x) = i16::try_from(x) else {
                continue;
            };
            let Ok(y) = i16::try_from(y) else {
                continue;
            };
            let from = TileXY::new(x, y);
            let to = from.neighbour(state.wind_direction);
            if to.x < 0 || to.y < 0 {
                continue;
            }
            let to_x = to.x as u32;
            let to_y = to.y as u32;
            if to_x >= state.smoke_cols || to_y >= state.smoke_rows {
                continue;
            }
            let destination = to_y as usize * state.smoke_cols as usize + to_x as usize;
            next_grid[destination] = next_grid[destination].saturating_add(density).min(6);
            events.push(Event::SmokeDrifted {
                from,
                to,
                density: u32::from(density),
            });
        }
        state.smoke_grid = next_grid;
    }
    events
}

fn process_turn_start_wounds(state: &mut SimState, actor_id: ActorId) -> Vec<Event> {
    let heavy_bleeding = state
        .wound_effects
        .get(&actor_id)
        .is_some_and(|effects| effects.heavy_bleeding);
    if !heavy_bleeding {
        return Vec::new();
    }
    let Some(actor) = state.actors.get_mut(&actor_id) else {
        return Vec::new();
    };
    actor.hit_points = actor.hit_points.saturating_sub(4);
    if actor.hit_points <= 0 {
        actor.alive = false;
    }
    vec![Event::DamageApplied {
        actor: actor_id,
        damage: 4,
    }]
}

/// Execute a move action.
fn execute_move(state: &mut SimState, actor_id: ActorId, target: TileXY) -> Vec<Event> {
    let Some(actor) = state.actors.get_mut(&actor_id) else {
        return Vec::new();
    };
    let from = actor.position;
    actor.position = target;
    actor.facing = facing_between(from, target);
    let mut events = vec![Event::Moved {
        actor: actor_id,
        from,
        to: target,
    }];
    if state.weather == crate::environment::Weather::Snow {
        state.snow_tracks.push((actor_id, target));
        if state.snow_tracks.len() > 256 {
            state.snow_tracks.remove(0);
        }
        events.push(Event::TrackLeft {
            actor: actor_id,
            tile: target,
        });
    }
    events.extend(resolve_overwatch_reactions(state, actor_id));
    events
}

fn execute_facing_change(state: &mut SimState, actor_id: ActorId, facing: Facing) -> Vec<Event> {
    if let Some(actor) = state.actors.get_mut(&actor_id) {
        actor.facing = facing;
    }
    vec![Event::FacingChanged {
        actor: actor_id,
        facing: facing.to_string(),
    }]
}

fn execute_sprint(state: &mut SimState, actor_id: ActorId, target: TileXY) -> Vec<Event> {
    let events = execute_move(state, actor_id, target);
    state.sprinting.insert(actor_id);
    events
}

/// Execute a shot action using the full shot pipeline.
///
/// Delegates to `crate::shot::resolve_shot` for event computation,
/// then applies state mutations (damage, wounds, alive flag) based
/// on the returned events.
fn execute_shot(
    state: &mut SimState,
    actor_id: ActorId,
    target: ActorId,
    aimed: bool,
    called: Option<HitLocationType>,
    extra_penalty: i32,
) -> Result<Vec<Event>, SimError> {
    // Use the full shot pipeline
    let mut events =
        crate::shot::resolve_shot(state, actor_id, target, called, aimed, extra_penalty)?;
    if events.is_empty() {
        return Ok(events);
    }

    let misfired = events
        .iter()
        .any(|event| matches!(event, Event::Misfire { .. }));

    if let Some(shooter) = state.actors.get_mut(&actor_id) {
        shooter.loaded_rounds -= 1;
        if !misfired {
            let mut fouling_rate = shooter.weapon_profile.fouling_rate;
            if shooter.progression.has_passive("fouling_halved") {
                fouling_rate /= 2;
            }
            shooter.fouling = shooter.fouling.saturating_add(fouling_rate);
        }
        shooter.jammed = misfired;
    }
    if misfired {
        events.push(Event::Jammed { actor: actor_id });
    }

    // Apply damage, wounds, and death to the target actor based on events
    let mut damage = 0i32;
    let mut wound: Option<WoundType> = None;
    let mut location: Option<HitLocationType> = None;
    let mut had_hit = false;
    for ev in &events {
        match ev {
            Event::DamageApplied {
                actor: _,
                damage: d,
            } => {
                damage = *d;
            }
            Event::WoundApplied { actor: _, wound: w } => {
                wound = Some(*w);
            }
            Event::HitLocation {
                actor: _,
                location: hit_location,
            } => {
                had_hit = true;
                location = Some(*hit_location);
            }
            _ => {}
        }
    }

    if had_hit && damage > 0 {
        if let Some(t_actor) = state.actors.get_mut(&target) {
            t_actor.hit_points -= damage;
            if let Some(w) = wound {
                if !t_actor.wounds.contains(&w) {
                    t_actor.wounds.push(w);
                }
            }
            if t_actor.hit_points <= 0 {
                t_actor.alive = false;
            }
        }
        if let (Some(location), Some(wound)) = (location, wound) {
            apply_wound_effects(state, target, location, wound, &mut events);
        }
    }

    append_nearby_suppression(state, target, &mut events);
    apply_sand_loss_events(state, &mut events);

    if state.light_level == crate::environment::LightLevel::Night {
        let until_tick = state.tick.0.saturating_add(1);
        state.revealed_until.insert(actor_id, until_tick);
        events.push(Event::Revealed {
            actor: actor_id,
            until_tick,
        });
    }
    events.extend(degrade_shot_cover(state, actor_id, target));

    Ok(events)
}

fn append_nearby_suppression(state: &SimState, target: ActorId, events: &mut Vec<Event>) {
    let amount = events
        .iter()
        .filter_map(|event| match event {
            Event::SandLost { actor, amount } if *actor == target => Some(*amount),
            _ => None,
        })
        .sum::<i32>();
    if amount <= 0 {
        return;
    }
    let Some(target_actor) = state.actors.get(&target) else {
        return;
    };
    let faction = target_actor.faction_id.clone();
    let position = target_actor.position;
    let allies: Vec<ActorId> = state
        .actors
        .iter()
        .filter(|(id, actor)| {
            **id != target
                && actor.alive
                && actor.faction_id == faction
                && actor.position.chebyshev_distance(position) <= 4
        })
        .map(|(id, _)| *id)
        .collect();
    events.extend(
        allies
            .into_iter()
            .map(|actor| Event::SandLost { actor, amount }),
    );
}

fn apply_sand_loss_events(state: &mut SimState, events: &mut Vec<Event>) {
    let mut losses = std::collections::BTreeMap::<ActorId, i32>::new();
    for event in events.iter() {
        if let Event::SandLost { actor, amount } = event {
            let total = losses.entry(*actor).or_insert(0);
            *total = total.saturating_add(*amount);
        }
    }
    let mut transitions = Vec::new();
    for (actor_id, base_loss) in losses {
        let multiplier = state
            .sand_multiplier_pct
            .get(&actor_id)
            .copied()
            .unwrap_or(100)
            .max(0);
        let adjusted = base_loss.saturating_mul(multiplier) / 100;
        let Some(actor) = state.actors.get_mut(&actor_id) else {
            continue;
        };
        let before = crate::morale::morale_state(actor.sand, actor.max_sand);
        actor.sand = actor.sand.saturating_sub(adjusted).max(0);
        let after = crate::morale::morale_state(actor.sand, actor.max_sand);
        if before != after {
            transitions.push((actor_id, after));
        }
    }
    for (actor_id, after) in transitions {
        events.push(Event::MoraleStateChanged {
            actor: actor_id,
            state: format!("{after:?}"),
        });
        if after == crate::morale::MoraleState::Routed {
            if let Some(actor) = state.actors.get_mut(&actor_id) {
                actor.routed = true;
            }
            state.sequence_clock.remove(&actor_id);
            events.push(Event::Routed { actor: actor_id });
        }
    }
}

/// Apply the consequences shared by every lethal action.
///
/// Individual action resolvers own damage and wound events. This boundary owns
/// death bookkeeping so shots, melee attacks, explosives, and future lethal
/// actions cannot silently diverge on sequence removal, companion propagation,
/// or morale.
fn finalize_new_deaths(
    state: &mut SimState,
    killer: ActorId,
    before: &SimState,
    events: &mut Vec<Event>,
) {
    let newly_dead: Vec<ActorId> = before
        .actors
        .iter()
        .filter(|(id, actor)| {
            actor.alive && state.actors.get(id).is_some_and(|current| !current.alive)
        })
        .map(|(id, _)| *id)
        .collect();

    for fallen in newly_dead {
        state.sequence_clock.remove(&fallen);
        if !events
            .iter()
            .any(|event| matches!(event, Event::ActorKilled { actor } if *actor == fallen))
        {
            events.push(Event::ActorKilled { actor: fallen });
        }
        let companion_name = state
            .actors
            .get(&fallen)
            .and_then(|actor| actor.is_companion.then(|| actor.name.clone()));
        if let Some(id) = companion_name {
            let already_emitted = events.iter().any(
                |event| matches!(event, Event::CompanionKilled { id: emitted } if emitted == &id),
            );
            if !already_emitted {
                events.push(Event::CompanionKilled { id });
            }
        }
        let death_killer = events
            .iter()
            .rev()
            .find_map(|event| match event {
                Event::DynamiteExploded { actor, tile } => state
                    .actors
                    .get(&fallen)
                    .is_some_and(|fallen_actor| {
                        fallen_actor.position.chebyshev_distance(*tile) <= 3
                    })
                    .then_some(*actor),
                _ => None,
            })
            .unwrap_or(killer);
        events.extend(apply_death_morale(state, death_killer, fallen));
    }
}

fn apply_death_morale(state: &mut SimState, killer: ActorId, fallen: ActorId) -> Vec<Event> {
    let Some(fallen_actor) = state.actors.get(&fallen) else {
        return Vec::new();
    };
    let faction = fallen_actor.faction_id.clone();
    let fallen_position = fallen_actor.position;
    let ally_ids: Vec<ActorId> = state
        .actors
        .iter()
        .filter(|(id, actor)| {
            **id != fallen && actor.alive && !faction.is_empty() && actor.faction_id == faction
        })
        .map(|(id, _)| *id)
        .collect();
    let mut events = Vec::new();
    for ally_id in ally_ids {
        let Some(ally) = state.actors.get(&ally_id) else {
            continue;
        };
        let distance = ally.position.chebyshev_distance(fallen_position);
        if i32::from(distance) > ally.attributes.sight_radius() {
            continue;
        }
        let mut amount: i32 = if distance <= 1 { 8 } else { 3 };
        if state.leaders.contains(&fallen) {
            amount = amount.saturating_add(10);
        }
        let multiplier = state
            .sand_multiplier_pct
            .get(&ally_id)
            .copied()
            .unwrap_or(100)
            .max(0);
        amount = amount.saturating_mul(multiplier) / 100;
        if ally.progression.has_passive("ally_death_sand_loss_halved") {
            amount /= 2;
        }
        let before = crate::morale::morale_state(ally.sand, ally.max_sand);
        let new_sand = ally.sand.saturating_sub(amount).max(0);
        let after = crate::morale::morale_state(new_sand, ally.max_sand);
        if let Some(ally) = state.actors.get_mut(&ally_id) {
            ally.sand = new_sand;
            if after == crate::morale::MoraleState::Routed {
                ally.routed = true;
            }
        }
        events.push(Event::SandLost {
            actor: ally_id,
            amount,
        });
        if before != after {
            events.push(Event::MoraleStateChanged {
                actor: ally_id,
                state: format!("{after:?}"),
            });
        }
        if after == crate::morale::MoraleState::Routed {
            state.sequence_clock.remove(&ally_id);
            events.push(Event::Routed { actor: ally_id });
        }
    }

    let is_enemy_kill = state
        .actors
        .get(&killer)
        .is_some_and(|actor| actor.faction_id != faction);
    if is_enemy_kill {
        if let Some(killer_actor) = state.actors.get_mut(&killer) {
            let before = killer_actor.sand;
            killer_actor.sand = (killer_actor.sand
                + crate::morale::sand_gain_for_event("kill_enemy"))
            .min(killer_actor.max_sand);
            if killer_actor.sand > before {
                events.push(Event::SandGained {
                    actor: killer,
                    amount: killer_actor.sand - before,
                });
            }
        }
    }
    events
}

/// Reload a cartridge weapon to its configured capacity.
fn execute_reload(state: &mut SimState, actor_id: ActorId) -> Vec<Event> {
    if let Some(actor) = state.actors.get_mut(&actor_id) {
        actor.loaded_rounds = actor.weapon_capacity;
    }
    vec![]
}

/// Stop bleeding. Bandaging deliberately restores no HP (SPEC-001 section 5).
fn execute_bandage(state: &mut SimState, _actor_id: ActorId, target: ActorId) -> Vec<Event> {
    let field_surgeon = state
        .actors
        .get(&_actor_id)
        .is_some_and(|healer| healer.progression.has_passive("bandage_heal_5"));
    if let Some(actor) = state.actors.get_mut(&target) {
        actor.wounds.retain(|wound| *wound != WoundType::Bleeding);
        if field_surgeon {
            actor.hit_points = (actor.hit_points + 5).min(actor.max_hp);
        }
    }
    if let Some(effects) = state.wound_effects.get_mut(&target) {
        effects.heavy_bleeding = false;
    }
    vec![]
}

/// Consume a field item represented by the compact kernel inventory contract.
fn execute_use_item(state: &mut SimState, actor_id: ActorId) -> Vec<Event> {
    if let Some(actor) = state.actors.get_mut(&actor_id) {
        actor.hit_points = (actor.hit_points + 4).min(actor.max_hp);
        actor.sand = (actor.sand + 2).min(actor.max_sand);
    }
    vec![]
}

/// Light and throw a stick. The blast is processed only when its hash-covered
/// fuse deadline is crossed by the sequence clock.
fn execute_dynamite(state: &mut SimState, actor_id: ActorId, target: TileXY) -> Vec<Event> {
    let (landing, fuse) = crate::explosive::throw_dynamite(
        state.seed,
        state.scenario_id,
        state.tick.0,
        actor_id.0,
        target,
    );
    let detonate_at = state.tick.0.saturating_add(fuse);
    state
        .pending_explosives
        .push(crate::state::PendingExplosive {
            thrower: actor_id,
            position: landing,
            detonate_at,
        });
    state.pending_explosives.sort_by_key(|stick| {
        (
            stick.detonate_at,
            stick.thrower,
            stick.position.x,
            stick.position.y,
        )
    });
    vec![Event::DynamiteLit {
        actor: actor_id,
        tile: landing,
        detonate_at,
    }]
}

fn execute_catch_dynamite(
    state: &mut SimState,
    actor_id: ActorId,
    tile: TileXY,
) -> Result<Vec<Event>, SimError> {
    let Some(index) = state
        .pending_explosives
        .iter()
        .position(|stick| stick.position == tile && stick.detonate_at > state.tick.0)
    else {
        return Err(SimError::ExplosiveNotFound(tile));
    };
    let actor = state
        .actors
        .get(&actor_id)
        .ok_or(SimError::ActorNotFound(actor_id))?;
    if actor.position.chebyshev_distance(tile) > 1 {
        return Err(SimError::CannotCatchDynamite(actor_id));
    }
    let roll = pb_rng::PbRng::draw(
        state.seed,
        state.scenario_id,
        state.tick.0,
        actor_id.0,
        pb_rng::StreamTag::Scatter,
        1,
        100,
    );
    let chance = (40 + actor.attributes.hands * 5 - 30).clamp(5, 95);
    if roll > chance {
        return Err(SimError::CannotCatchDynamite(actor_id));
    }
    let stick = state.pending_explosives.remove(index);
    state.held_explosives.insert(actor_id, stick);
    Ok(vec![Event::DynamiteCaught {
        actor: actor_id,
        tile,
    }])
}

fn execute_rethrow_dynamite(
    state: &mut SimState,
    actor_id: ActorId,
    target: TileXY,
) -> Result<Vec<Event>, SimError> {
    let Some(mut stick) = state.held_explosives.remove(&actor_id) else {
        return Err(SimError::ExplosiveNotFound(target));
    };
    let (landing, _) = crate::explosive::throw_dynamite(
        state.seed,
        state.scenario_id,
        state.tick.0,
        actor_id.0,
        target,
    );
    stick.thrower = actor_id;
    stick.position = landing;
    let detonate_at = stick.detonate_at;
    state.pending_explosives.push(stick);
    state.pending_explosives.sort_by_key(|pending| {
        (
            pending.detonate_at,
            pending.thrower,
            pending.position.x,
            pending.position.y,
        )
    });
    Ok(vec![Event::DynamiteRethrown {
        actor: actor_id,
        tile: landing,
        detonate_at,
    }])
}

/// Reload a cap-and-ball weapon and clean some accumulated fouling.
fn execute_cap_and_ball_reload(state: &mut SimState, actor_id: ActorId) -> Vec<Event> {
    if let Some(actor) = state.actors.get_mut(&actor_id) {
        actor.loaded_rounds = actor.weapon_capacity;
        actor.fouling = actor.fouling.saturating_sub(2);
    }
    vec![]
}

/// Clear the current weapon jam and reduce fouling from the action.
fn execute_clear_jam(state: &mut SimState, actor_id: ActorId) -> Vec<Event> {
    let Some(actor) = state.actors.get(&actor_id) else {
        return Vec::new();
    };
    let roll = pb_rng::PbRng::draw(
        state.seed,
        state.scenario_id,
        state.tick.0,
        actor_id.0,
        pb_rng::StreamTag::Misfire,
        1,
        100,
    );
    let chance = (110 + actor.attributes.hands * 5 - actor.fouling * 10).clamp(5, 100);
    if roll <= chance {
        if let Some(actor) = state.actors.get_mut(&actor_id) {
            actor.jammed = false;
            actor.fouling = actor.fouling.saturating_sub(1);
        }
    }
    vec![]
}

/// Execute a melee action — applies 1d6+3 damage to the target.
fn execute_melee(state: &mut SimState, actor_id: ActorId, target: ActorId) -> Vec<Event> {
    crate::shot::resolve_melee(state, actor_id, target)
}

// ---------------------------------------------------------------------------
// SPEC-001 action executors
// ---------------------------------------------------------------------------

/// Execute a stance change.
fn execute_stance_change(
    state: &mut SimState,
    actor_id: ActorId,
    new_stance: Stance,
) -> Vec<Event> {
    let Some(actor) = state.actors.get_mut(&actor_id) else {
        return Vec::new();
    };
    actor.stance = new_stance;
    vec![Event::StanceChanged {
        actor: actor_id,
        stance: format!("{new_stance:?}"),
    }]
}

/// Execute a FanHammer action: 3 quick shots at -25 accuracy each.
fn execute_fan_hammer(
    state: &mut SimState,
    actor_id: ActorId,
    target: ActorId,
) -> Result<Vec<Event>, SimError> {
    let penalty = if state
        .actors
        .get(&actor_id)
        .is_some_and(|actor| actor.progression.has_passive("fan_hammer_improved"))
    {
        -15
    } else {
        -25
    };
    let mut all_events = vec![];
    for _ in 0..3 {
        let events = execute_shot(state, actor_id, target, false, None, penalty)?;
        // Stop fanning if target dies
        let target_dead = state.actors.get(&target).is_none_or(|a| !a.alive);
        all_events.extend(events);
        if target_dead {
            break;
        }
    }
    Ok(all_events)
}

/// Execute DrawBead: set the actor on overwatch and consume all remaining AP.
fn execute_draw_bead(state: &mut SimState, actor_id: ActorId, _target: ActorId) -> Vec<Event> {
    let points = state
        .actors
        .get(&actor_id)
        .map(|actor| actor.ap.0.clamp(0, 4) as u8)
        .unwrap_or(0);
    state.overwatch.insert(actor_id);
    state.reaction_points.insert(actor_id, points);
    if let Some(actor) = state.actors.get_mut(&actor_id) {
        actor.ap = pb_core::ids::Ap(0);
    }
    vec![Event::OverwatchSet {
        actor: actor_id,
        reaction_points: points,
    }]
}

/// Execute Rally: restore Sand to the target.
fn execute_rally(state: &mut SimState, actor_id: ActorId, target: ActorId) -> Vec<Event> {
    let Some(rallier) = state.actors.get(&actor_id) else {
        return Vec::new();
    };
    let roll = pb_rng::PbRng::draw(
        state.seed,
        state.scenario_id,
        state.tick.0,
        actor_id.0,
        pb_rng::StreamTag::Morale,
        1,
        100,
    );
    let succeeds = roll <= (40 + rallier.attributes.nerve * 6).clamp(5, 95);
    if !succeeds {
        return Vec::new();
    }
    let church_voice = rallier.progression.has_passive("rally_radius_3");
    let faction = rallier.faction_id.clone();
    let origin = rallier.position;
    let targets: Vec<ActorId> = if church_voice {
        state
            .actors
            .iter()
            .filter(|(_, actor)| {
                actor.alive
                    && actor.faction_id == faction
                    && actor.position.chebyshev_distance(origin) <= 3
            })
            .map(|(id, _)| *id)
            .collect()
    } else {
        vec![target]
    };
    let mut events = Vec::new();
    for target in targets {
        if let Some(t_actor) = state.actors.get_mut(&target) {
            let before = t_actor.sand;
            t_actor.sand = (t_actor.sand + 5).min(t_actor.max_sand);
            if t_actor.sand > before {
                events.push(Event::SandGained {
                    actor: target,
                    amount: t_actor.sand - before,
                });
            }
        }
    }
    events
}

fn execute_volley(
    state: &mut SimState,
    commander: ActorId,
    target: ActorId,
) -> Result<Vec<Event>, SimError> {
    let faction = state
        .actors
        .get(&commander)
        .ok_or(SimError::ActorNotFound(commander))?
        .faction_id
        .clone();
    let shooters: Vec<ActorId> = state
        .actors
        .iter()
        .filter(|(id, actor)| {
            actor.alive
                && !actor.routed
                && actor.faction_id == faction
                && actor.ap.0 >= 3
                && actor.loaded_rounds > 0
                && !actor.jammed
                && **id != target
        })
        .map(|(id, _)| *id)
        .collect();
    let mut events = Vec::new();
    for shooter in shooters {
        if state.actors.get(&target).is_none_or(|actor| !actor.alive) {
            break;
        }
        if weapon_is_blocked(state, shooter) {
            continue;
        }
        if let Some(actor) = state.actors.get_mut(&shooter) {
            actor.ap.0 -= 3;
        }
        events.extend(execute_shot(state, shooter, target, false, None, 0)?);
    }
    Ok(events)
}

fn execute_left_hand_draw(
    state: &mut SimState,
    actor_id: ActorId,
    target: ActorId,
) -> Result<Vec<Event>, SimError> {
    let actor = state
        .actors
        .get(&actor_id)
        .ok_or(SimError::ActorNotFound(actor_id))?;
    if !actor.progression.has_ability("draw_and_fire_once")
        || state
            .consumed_battle_effects
            .contains(&(actor_id, "draw_and_fire_once".to_string()))
    {
        return Err(SimError::BattleEffectSpent(actor_id));
    }
    state
        .consumed_battle_effects
        .insert((actor_id, "draw_and_fire_once".to_string()));
    execute_shot(state, actor_id, target, false, None, 0)
}

/// Recover ammunition from an adjacent fallen combatant.
fn execute_loot(state: &mut SimState, actor_id: ActorId, target: ActorId) -> Vec<Event> {
    let can_loot = state.actors.get(&target).is_some_and(|fallen| {
        !fallen.alive
            && state
                .actors
                .get(&actor_id)
                .is_some_and(|actor| actor.position.chebyshev_distance(fallen.position) <= 1)
    });
    if can_loot {
        if let Some(actor) = state.actors.get_mut(&actor_id) {
            actor.loaded_rounds = (actor.loaded_rounds + 2).min(actor.weapon_capacity);
        }
    }
    vec![]
}

fn is_weapon_action(action: &Action) -> bool {
    matches!(
        action,
        Action::SnapShot(_)
            | Action::AimedShot(_)
            | Action::CalledShot(_, _)
            | Action::FanHammer(_)
            | Action::Volley(_)
            | Action::LeftHandDraw(_)
    )
}

fn validate_destination(
    state: &SimState,
    actor_id: ActorId,
    target: TileXY,
) -> Result<(), SimError> {
    if target.x < 0
        || target.y < 0
        || target.x as u32 >= state.smoke_cols
        || target.y as u32 >= state.smoke_rows
    {
        return Err(SimError::OutOfBounds(target));
    }
    if state.actors.iter().any(|(id, actor)| {
        *id != actor_id && actor.alive && !actor.routed && actor.position == target
    }) {
        return Err(SimError::TileOccupied(target));
    }
    Ok(())
}

fn weapon_is_blocked(state: &SimState, actor_id: ActorId) -> bool {
    let Some(actor) = state.actors.get(&actor_id) else {
        return false;
    };
    let Some(effects) = state.wound_effects.get(&actor_id) else {
        return false;
    };
    let gun_arm = effects
        .broken_locations
        .get(&HitLocationType::GunArm)
        .copied()
        .unwrap_or(0)
        > 0;
    let off_arm = effects
        .broken_locations
        .get(&HitLocationType::OffArm)
        .copied()
        .unwrap_or(0)
        > 0;
    gun_arm || (actor.weapon_profile.two_handed && off_arm)
}

/// Choose a deterministic adjacent retreat tile that increases the Broken
/// actor's distance from its nearest living enemy.
///
/// Occupied and out-of-bounds tiles are excluded. Ties prefer north, then
/// west, so every client and proof runner issues the same legal command.
pub fn choose_retreat_tile(state: &SimState, actor_id: ActorId) -> Option<TileXY> {
    let actor = state.actors.get(&actor_id)?;
    let enemies = state
        .actors
        .iter()
        .filter(|(id, enemy)| {
            **id != actor_id && enemy.alive && !enemy.routed && enemy.faction_id != actor.faction_id
        })
        .map(|(_, enemy)| enemy)
        .collect::<Vec<_>>();
    let before = enemies
        .iter()
        .map(|enemy| actor.position.chebyshev_distance(enemy.position))
        .min()?;
    let max_x = i16::try_from(state.smoke_cols.saturating_sub(1)).unwrap_or(i16::MAX);
    let max_y = i16::try_from(state.smoke_rows.saturating_sub(1)).unwrap_or(i16::MAX);
    (-1..=1)
        .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
        .filter(|(dx, dy)| *dx != 0 || *dy != 0)
        .map(|(dx, dy)| {
            TileXY::new(
                actor.position.x.saturating_add(dx),
                actor.position.y.saturating_add(dy),
            )
        })
        .filter(|tile| tile.x >= 0 && tile.y >= 0 && tile.x <= max_x && tile.y <= max_y)
        .filter(|tile| {
            !state.actors.iter().any(|(id, candidate)| {
                *id != actor_id && candidate.alive && candidate.position == *tile
            })
        })
        .filter_map(|tile| {
            let after = enemies
                .iter()
                .map(|enemy| tile.chebyshev_distance(enemy.position))
                .min()?;
            (after > before).then_some((tile, after))
        })
        .max_by_key(|(tile, after)| (*after, std::cmp::Reverse(tile.y), std::cmp::Reverse(tile.x)))
        .map(|(tile, _)| tile)
}

fn has_outward_retreat(state: &SimState, actor_id: ActorId) -> bool {
    choose_retreat_tile(state, actor_id).is_some()
}

fn execute_hold(state: &mut SimState, actor_id: ActorId) -> Vec<Event> {
    let cornered = state
        .broken_retreat_remaining
        .get(&actor_id)
        .copied()
        .unwrap_or(0)
        > 0
        && !has_outward_retreat(state, actor_id);
    if !cornered {
        return Vec::new();
    }
    if let Some(actor) = state.actors.get_mut(&actor_id) {
        actor.sand = 0;
        actor.routed = true;
    }
    state.sequence_clock.remove(&actor_id);
    state.broken_retreat_remaining.remove(&actor_id);
    vec![
        Event::MoraleStateChanged {
            actor: actor_id,
            state: "Routed".to_string(),
        },
        Event::Routed { actor: actor_id },
    ]
}

fn enforce_broken_retreat(
    state: &SimState,
    actor_id: ActorId,
    action: &Action,
) -> Result<(), SimError> {
    if state
        .broken_retreat_remaining
        .get(&actor_id)
        .copied()
        .unwrap_or(0)
        <= 0
    {
        return Ok(());
    }
    let destination = match action {
        Action::Move(tile) | Action::Sprint(tile) => *tile,
        Action::Hold if !has_outward_retreat(state, actor_id) => return Ok(()),
        _ => return Err(SimError::MustRetreat(actor_id)),
    };
    let Some(actor) = state.actors.get(&actor_id) else {
        return Err(SimError::ActorNotFound(actor_id));
    };
    let nearest = state
        .actors
        .iter()
        .filter(|(id, enemy)| {
            **id != actor_id && enemy.alive && !enemy.routed && enemy.faction_id != actor.faction_id
        })
        .map(|(_, enemy)| {
            (
                actor.position.chebyshev_distance(enemy.position),
                destination.chebyshev_distance(enemy.position),
            )
        })
        .min();
    if nearest.is_some_and(|(before, after)| after <= before) {
        return Err(SimError::MustRetreat(actor_id));
    }
    Ok(())
}

fn consume_broken_retreat(state: &mut SimState, actor_id: ActorId, action: &Action, cost: i16) {
    if !matches!(action, Action::Move(_) | Action::Sprint(_)) {
        return;
    }
    if let Some(remaining) = state.broken_retreat_remaining.get_mut(&actor_id) {
        *remaining = remaining.saturating_sub(cost);
        if *remaining <= 0 {
            state.broken_retreat_remaining.remove(&actor_id);
        }
    }
}

fn facing_between(from: TileXY, to: TileXY) -> pb_core::geom::Facing {
    use pb_core::geom::Facing;
    let dx = (to.x - from.x).signum();
    let dy = (to.y - from.y).signum();
    match (dx, dy) {
        (0, -1) => Facing::North,
        (1, -1) => Facing::NorthEast,
        (1, 0) => Facing::East,
        (1, 1) => Facing::SouthEast,
        (0, 1) => Facing::South,
        (-1, 1) => Facing::SouthWest,
        (-1, 0) => Facing::West,
        _ => Facing::NorthWest,
    }
}

fn in_facing_cone(origin: TileXY, facing: pb_core::geom::Facing, target: TileXY) -> bool {
    let direction = facing_between(origin, target).to_index() as i16;
    let facing = facing.to_index() as i16;
    let difference = (direction - facing).rem_euclid(8);
    matches!(difference, 0 | 1 | 7)
}

/// Living enemy overwatch actors whose canonical facing cone covers `target`.
pub fn overwatch_threats_at(state: &SimState, mover: ActorId, target: TileXY) -> Vec<ActorId> {
    let Some(mover_actor) = state.actors.get(&mover) else {
        return Vec::new();
    };
    state
        .overwatch
        .iter()
        .copied()
        .filter(|watcher| {
            state.actors.get(watcher).is_some_and(|actor| {
                actor.alive
                    && !actor.routed
                    && actor.faction_id != mover_actor.faction_id
                    && state.reaction_points.get(watcher).copied().unwrap_or(0) >= 3
                    && in_facing_cone(actor.position, actor.facing, target)
            })
        })
        .collect()
}

fn resolve_overwatch_reactions(state: &mut SimState, mover: ActorId) -> Vec<Event> {
    let Some(mover_position) = state.actors.get(&mover).map(|actor| actor.position) else {
        return Vec::new();
    };
    let watchers = overwatch_threats_at(state, mover, mover_position);
    let mut events = Vec::new();
    for watcher in watchers {
        if state.actors.get(&mover).is_none_or(|actor| !actor.alive) {
            break;
        }
        events.push(Event::ReactionShot {
            actor: watcher,
            target: mover,
        });
        match execute_shot(state, watcher, mover, false, None, 0) {
            Ok(shot_events) => events.extend(shot_events),
            Err(_) => {
                state.overwatch.remove(&watcher);
                state.reaction_points.remove(&watcher);
                continue;
            }
        }
        if let Some(points) = state.reaction_points.get_mut(&watcher) {
            *points = points.saturating_sub(3);
            if *points < 3 {
                state.overwatch.remove(&watcher);
                state.reaction_points.remove(&watcher);
            }
        }
    }
    events
}

fn apply_wound_effects(
    state: &mut SimState,
    target: ActorId,
    location: HitLocationType,
    wound: WoundType,
    events: &mut Vec<Event>,
) {
    let effects = state.wound_effects.entry(target).or_default();
    match (location, wound) {
        (HitLocationType::Head, WoundType::Concussed) => {
            effects.concussed_turns = effects.concussed_turns.max(3);
        }
        (HitLocationType::Eyes, WoundType::Blinded) => {}
        (HitLocationType::Torso, WoundType::Winded) => {
            effects.winded = true;
            events.push(Event::SandLost {
                actor: target,
                amount: 4,
            });
        }
        (HitLocationType::Vitals, WoundType::Bleeding) => {
            effects.heavy_bleeding = true;
        }
        (
            HitLocationType::GunArm | HitLocationType::OffArm | HitLocationType::Legs,
            WoundType::Broken,
        ) => {
            let count = effects.broken_locations.entry(location).or_insert(0);
            *count = count.saturating_add(1);
            if location == HitLocationType::Legs && *count >= 2 {
                if let Some(actor) = state.actors.get_mut(&target) {
                    actor.stance = Stance::Prone;
                }
                events.push(Event::StanceChanged {
                    actor: target,
                    stance: "Prone".to_string(),
                });
            }
        }
        _ => {}
    }
}

fn detonate_due_explosives(state: &mut SimState) -> Vec<Event> {
    let mut due = Vec::new();
    state.pending_explosives.retain(|stick| {
        if stick.detonate_at <= state.tick.0 {
            due.push(stick.clone());
            false
        } else {
            true
        }
    });
    let mut events = Vec::new();
    for stick in due {
        events.extend(detonate_explosive(state, &stick));
    }
    events
}

fn detonate_explosive(state: &mut SimState, stick: &crate::state::PendingExplosive) -> Vec<Event> {
    let victims: Vec<(ActorId, i32)> = state
        .actors
        .iter()
        .filter(|(_, actor)| actor.alive)
        .filter_map(|(id, actor)| {
            let damage = crate::explosive::blast_damage(actor.position, stick.position);
            (damage > 0).then_some((*id, damage))
        })
        .collect();
    let mut events = vec![
        Event::DynamiteExploded {
            actor: stick.thrower,
            tile: stick.position,
        },
        Event::SmokeDeposited {
            tile: stick.position,
            density: 6,
        },
    ];
    for (victim, damage) in victims {
        if let Some(actor) = state.actors.get_mut(&victim) {
            actor.hit_points -= damage;
            if !actor.wounds.contains(&WoundType::Burned) {
                actor.wounds.push(WoundType::Burned);
            }
            if actor.hit_points <= 0 {
                actor.alive = false;
            }
        }
        events.push(Event::DamageApplied {
            actor: victim,
            damage,
        });
        events.push(Event::WoundApplied {
            actor: victim,
            wound: WoundType::Burned,
        });
    }
    let edges: Vec<crate::state::CoverEdge> = state
        .cover_edges
        .keys()
        .copied()
        .filter(|edge| edge.tile.chebyshev_distance(stick.position) <= 3)
        .collect();
    for edge in edges {
        if let Some(cover) = state.cover_edges.get_mut(&edge) {
            cover.level = match cover.level {
                crate::state::CoverLevel::Full | crate::state::CoverLevel::Hard => {
                    crate::state::CoverLevel::Soft
                }
                crate::state::CoverLevel::Soft => crate::state::CoverLevel::None,
                crate::state::CoverLevel::None => crate::state::CoverLevel::None,
            };
            cover.strikes = 0;
            events.push(Event::CoverDamaged {
                tile: edge.tile,
                facing: edge.facing.to_string(),
                level: format!("{:?}", cover.level),
            });
        }
    }
    events
}

fn degrade_shot_cover(state: &mut SimState, shooter: ActorId, target: ActorId) -> Vec<Event> {
    let Some((target_position, shooter_position)) = state
        .actors
        .get(&target)
        .zip(state.actors.get(&shooter))
        .map(|(target, shooter)| (target.position, shooter.position))
    else {
        return Vec::new();
    };
    let edge = crate::state::CoverEdge {
        tile: target_position,
        facing: facing_between(target_position, shooter_position),
    };
    let Some(cover) = state.cover_edges.get_mut(&edge) else {
        return Vec::new();
    };
    if cover.level != crate::state::CoverLevel::Soft {
        return Vec::new();
    }
    cover.strikes = cover.strikes.saturating_add(1);
    if cover.strikes < 3 {
        return Vec::new();
    }
    cover.level = crate::state::CoverLevel::None;
    cover.strikes = 0;
    vec![Event::CoverDamaged {
        tile: edge.tile,
        facing: edge.facing.to_string(),
        level: "None".to_string(),
    }]
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
            faction_id: String::new(),
            is_companion: false,
            attributes: pb_core::Attributes::BALANCED,
            ap: Ap(10),
            position: TileXY::new(0, 0),
            facing: Facing::South,
            sequence: 5,
            hit_points: 20,
            max_hp: 20,
            name: "Test".to_string(),
            alive: true,
            routed: false,
            wounds: vec![],
            sand: 10,
            max_sand: 10,
            stance: Stance::Standing,
            progression: crate::progression::ActorProgression::new(),
            weapon: "colt_army_1860".to_string(),
            weapon_profile: Default::default(),
            loaded_rounds: 6,
            weapon_capacity: 6,
            fouling: 0,
            jammed: false,
        }
    }

    // -----------------------------------------------------------------------
    // Action cost tests
    // -----------------------------------------------------------------------

    #[test]
    fn action_cost_snap_shot() {
        let actor = make_actor();
        assert_eq!(action_cost(&Action::SnapShot(ActorId(1)), &actor), Ap(3));
    }

    #[test]
    fn changing_facing_is_free_and_hash_covered() {
        let id = ActorId(1);
        let mut state = SimState::new(42, 1);
        state.actors.insert(id, make_actor());

        let events = step(
            &mut state,
            Command {
                actor_id: id,
                action: Action::Face(Facing::West),
            },
        )
        .unwrap();

        assert_eq!(state.actors[&id].ap, Ap(10));
        assert_eq!(state.actors[&id].facing, Facing::West);
        assert!(events.contains(&Event::FacingChanged {
            actor: id,
            facing: "W".to_string(),
        }));
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
    fn action_cost_sprint() {
        let actor = make_actor();
        let target = TileXY::new(2, 0);
        assert_eq!(action_cost(&Action::Sprint(target), &actor), Ap(2));
    }

    #[test]
    fn stance_multiplies_move_cost() {
        let target = TileXY::new(1, 0);
        let mut actor = make_actor();
        actor.stance = Stance::Crouched;
        assert_eq!(action_cost(&Action::Move(target), &actor), Ap(2));
        actor.stance = Stance::Prone;
        assert_eq!(action_cost(&Action::Move(target), &actor), Ap(3));
    }

    // SPEC-001 cost tests

    #[test]
    fn action_cost_stance_crouch() {
        let actor = make_actor();
        assert_eq!(action_cost(&Action::StanceCrouch, &actor), Ap(1));
    }

    #[test]
    fn action_cost_stance_prone() {
        let actor = make_actor();
        assert_eq!(action_cost(&Action::StanceProne, &actor), Ap(2));
    }

    #[test]
    fn action_cost_rise_from_prone() {
        let actor = make_actor();
        assert_eq!(action_cost(&Action::RiseFromProne, &actor), Ap(2));
    }

    #[test]
    fn action_cost_fan_hammer() {
        let actor = make_actor();
        assert_eq!(action_cost(&Action::FanHammer(ActorId(1)), &actor), Ap(6));
    }

    #[test]
    fn action_cost_cap_and_ball_reload() {
        let actor = make_actor();
        assert_eq!(action_cost(&Action::CapAndBallReload, &actor), Ap(8));
    }

    #[test]
    fn action_cost_clear_jam() {
        let actor = make_actor();
        assert_eq!(action_cost(&Action::ClearJam, &actor), Ap(4));
    }

    #[test]
    fn action_cost_draw_bead() {
        let actor = make_actor();
        assert_eq!(action_cost(&Action::DrawBead(ActorId(1)), &actor), Ap(2));
    }

    #[test]
    fn action_cost_rally() {
        let actor = make_actor();
        assert_eq!(action_cost(&Action::Rally(ActorId(1)), &actor), Ap(3));
    }

    #[test]
    fn action_cost_loot() {
        let actor = make_actor();
        assert_eq!(action_cost(&Action::Loot(ActorId(1)), &actor), Ap(2));
    }

    // -----------------------------------------------------------------------
    // Step execution tests
    // -----------------------------------------------------------------------

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
    fn every_successful_step_is_bracketed_by_turn_events() {
        let mut state = SimState::new(42, 1);
        state.tick = pb_core::ids::Tick(84);
        let id = ActorId(1);
        state.actors.insert(id, make_actor());

        let events = step(
            &mut state,
            Command {
                actor_id: id,
                action: Action::Hold,
            },
        )
        .unwrap();

        assert_eq!(
            events,
            [
                Event::TurnBegin {
                    actor: id,
                    tick: 84
                },
                Event::TurnEnd {
                    actor: id,
                    tick: 84
                }
            ]
        );
    }

    #[test]
    fn smoke_deposition_outside_the_map_is_ignored_without_overflow() {
        let mut state = SimState::new(42, 1);
        let before = state.smoke_grid.clone();
        apply_smoke_events(
            &mut state,
            &[
                Event::SmokeDeposited {
                    tile: TileXY::new(-1, 2),
                    density: 3,
                },
                Event::SmokeDeposited {
                    tile: TileXY::new(2, -1),
                    density: u32::MAX,
                },
                Event::SmokeDeposited {
                    tile: TileXY::new(99, 99),
                    density: 3,
                },
            ],
        );
        assert_eq!(state.smoke_grid, before);
    }

    #[test]
    fn smoke_decay_is_integrated_with_the_tick_clock_once_per_boundary() {
        let mut state = SimState::new(42, 1);
        state.tick = pb_core::ids::Tick(80);
        let id = ActorId(1);
        state.actors.insert(id, make_actor());
        let smoke_index = 3 * state.smoke_cols as usize + 2;
        state.smoke_grid[smoke_index] = 3;

        let events = step(
            &mut state,
            Command {
                actor_id: id,
                action: Action::Hold,
            },
        )
        .unwrap();
        assert_eq!(state.smoke_grid[smoke_index], 1);
        assert_eq!(state.environment_tick, 80);
        assert!(events.contains(&Event::SmokeDecayed {
            tile: TileXY::new(2, 3),
            density: 1,
        }));

        let repeated = step(
            &mut state,
            Command {
                actor_id: id,
                action: Action::Hold,
            },
        )
        .unwrap();
        assert!(!repeated
            .iter()
            .any(|event| matches!(event, Event::SmokeDecayed { .. })));
    }

    #[test]
    fn rain_halves_smoke_persistence() {
        let mut state = SimState::new(42, 1);
        state.weather = crate::environment::Weather::Rain;
        state.tick = pb_core::ids::Tick(20);
        let id = ActorId(1);
        state.actors.insert(id, make_actor());
        let smoke_index = 3 * state.smoke_cols as usize + 2;
        state.smoke_grid[smoke_index] = 3;

        let events = step(
            &mut state,
            Command {
                actor_id: id,
                action: Action::Hold,
            },
        )
        .unwrap();
        assert_eq!(state.smoke_grid[smoke_index], 2);
        assert!(events.contains(&Event::SmokeDecayed {
            tile: TileXY::new(2, 3),
            density: 2,
        }));
    }

    #[test]
    fn smoke_drifts_one_tile_each_120_ticks() {
        let mut state = SimState::new(42, 1);
        state.tick = pb_core::ids::Tick(120);
        state.wind_direction = pb_core::geom::Facing::North;
        let id = ActorId(1);
        state.actors.insert(id, make_actor());
        let source = 3 * state.smoke_cols as usize + 2;
        let destination = 2 * state.smoke_cols as usize + 2;
        state.smoke_grid[source] = 5;

        let events = step(
            &mut state,
            Command {
                actor_id: id,
                action: Action::Hold,
            },
        )
        .unwrap();
        assert_eq!(state.smoke_grid[source], 0);
        assert_eq!(state.smoke_grid[destination], 2);
        assert!(events.contains(&Event::SmokeDrifted {
            from: TileXY::new(2, 3),
            to: TileXY::new(2, 2),
            density: 2,
        }));
    }

    #[test]
    fn death_morale_uses_authored_factions_and_distance() {
        let mut state = SimState::new(42, 1);
        let killer = ActorId(1);
        let fallen = ActorId(2);
        let adjacent = ActorId(3);
        let visible = ActorId(4);

        let mut killer_actor = make_actor();
        killer_actor.faction_id = "player".to_string();
        killer_actor.sand = 10;
        killer_actor.max_sand = 20;
        state.actors.insert(killer, killer_actor);

        let mut fallen_actor = make_actor();
        fallen_actor.faction_id = "outlaw".to_string();
        fallen_actor.position = TileXY::new(2, 2);
        fallen_actor.alive = false;
        state.actors.insert(fallen, fallen_actor);

        let mut adjacent_actor = make_actor();
        adjacent_actor.faction_id = "outlaw".to_string();
        adjacent_actor.position = TileXY::new(3, 2);
        adjacent_actor.sand = 20;
        adjacent_actor.max_sand = 20;
        state.actors.insert(adjacent, adjacent_actor);

        let mut visible_actor = make_actor();
        visible_actor.faction_id = "outlaw".to_string();
        visible_actor.position = TileXY::new(7, 2);
        visible_actor.sand = 20;
        visible_actor.max_sand = 20;
        state.actors.insert(visible, visible_actor);

        let events = apply_death_morale(&mut state, killer, fallen);
        assert_eq!(state.actors[&adjacent].sand, 12);
        assert_eq!(state.actors[&visible].sand, 17);
        assert_eq!(state.actors[&killer].sand, 13);
        assert!(events.contains(&Event::SandLost {
            actor: adjacent,
            amount: 8,
        }));
        assert!(events.contains(&Event::SandLost {
            actor: visible,
            amount: 3,
        }));
        assert!(events.contains(&Event::SandGained {
            actor: killer,
            amount: 3,
        }));
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

    #[test]
    fn cornered_broken_actor_routes_instead_of_deadlocking() {
        let mut state = SimState::new(42, 1);
        let broken = ActorId(1);
        let enemy = ActorId(2);
        let mut broken_actor = make_actor();
        broken_actor.name = "cornered_outlaw".to_string();
        broken_actor.faction_id = "outlaw".to_string();
        broken_actor.sand = 3;
        broken_actor.position = TileXY::new(
            i16::try_from(state.smoke_cols.saturating_sub(1)).unwrap(),
            i16::try_from(state.smoke_rows.saturating_sub(1)).unwrap(),
        );
        state.actors.insert(broken, broken_actor);
        let mut enemy_actor = make_actor();
        enemy_actor.name = "pursuer".to_string();
        enemy_actor.faction_id = "player".to_string();
        enemy_actor.position = TileXY::new(0, 0);
        state.actors.insert(enemy, enemy_actor);
        state.broken_retreat_remaining.insert(broken, 2);
        state.sequence_clock.insert(broken, 0);

        let events = step(
            &mut state,
            Command {
                actor_id: broken,
                action: Action::Hold,
            },
        )
        .expect("cornered broken actor must be able to leave the field");

        assert!(state.actors[&broken].routed);
        assert_eq!(state.actors[&broken].sand, 0);
        assert!(!state.sequence_clock.contains_key(&broken));
        assert!(events.contains(&Event::Routed { actor: broken }));
    }

    #[test]
    fn step_reload_fills_weapon() {
        let mut state = SimState::new(42, 1);
        let id = ActorId(1);
        let mut actor = make_actor();
        actor.loaded_rounds = 1;
        actor.weapon_profile.reload_class = "Cartridge".to_string();
        state.actors.insert(id, actor);

        step(
            &mut state,
            Command {
                actor_id: id,
                action: Action::Reload,
            },
        )
        .unwrap();
        assert_eq!(state.actors[&id].loaded_rounds, 6);
        assert_eq!(state.actors[&id].ap, Ap(7));
    }

    #[test]
    fn step_bandage_stops_bleeding_without_restoring_hp() {
        let mut state = SimState::new(42, 1);
        let medic = ActorId(1);
        let target = ActorId(2);
        state.actors.insert(medic, make_actor());
        let mut wounded = make_actor();
        wounded.hit_points = 8;
        wounded.wounds.push(WoundType::Bleeding);
        state.actors.insert(target, wounded);

        step(
            &mut state,
            Command {
                actor_id: medic,
                action: Action::Bandage(target),
            },
        )
        .unwrap();
        assert_eq!(state.actors[&target].hit_points, 8);
        assert!(!state.actors[&target].wounds.contains(&WoundType::Bleeding));
    }

    #[test]
    fn failed_shot_is_atomic_and_does_not_spend_ap() {
        let mut state = SimState::new(42, 1);
        let shooter = ActorId(1);
        let target = ActorId(2);
        let mut empty = make_actor();
        empty.loaded_rounds = 0;
        state.actors.insert(shooter, empty);
        let mut victim = make_actor();
        victim.position = TileXY::new(1, 0);
        state.actors.insert(target, victim);

        let before = crate::hash::compute_state_hash(&state);
        let error = step(
            &mut state,
            Command {
                actor_id: shooter,
                action: Action::SnapShot(target),
            },
        )
        .unwrap_err();

        assert_eq!(error, SimError::WeaponNotLoaded(shooter));
        assert_eq!(state.actors[&shooter].ap, Ap(10));
        assert_eq!(crate::hash::compute_state_hash(&state), before);
    }

    #[test]
    fn step_use_item_restores_health_and_sand() {
        let mut state = SimState::new(42, 1);
        let id = ActorId(1);
        let mut actor = make_actor();
        actor.hit_points = 10;
        actor.sand = 4;
        state.actors.insert(id, actor);

        step(
            &mut state,
            Command {
                actor_id: id,
                action: Action::UseItem,
            },
        )
        .unwrap();
        assert_eq!(state.actors[&id].hit_points, 14);
        assert_eq!(state.actors[&id].sand, 6);
    }

    #[test]
    fn step_dynamite_damages_actor_at_deterministic_landing() {
        let mut state = SimState::new(42, 1);
        let thrower = ActorId(1);
        let victim = ActorId(2);
        state.actors.insert(thrower, make_actor());
        let target = TileXY::new(10, 10);
        let (landing, _) = crate::explosive::throw_dynamite(42, 1, 0, thrower.0, target);
        let mut actor = make_actor();
        actor.position = landing;
        actor.hit_points = 30;
        actor.max_hp = 30;
        state.actors.insert(victim, actor);

        let lit_events = step(
            &mut state,
            Command {
                actor_id: thrower,
                action: Action::ThrowDynamite(target),
            },
        )
        .unwrap();
        assert!(lit_events.iter().any(|event| matches!(
            event,
            Event::DynamiteLit { tile, detonate_at: 120, .. } if *tile == landing
        )));
        assert_eq!(state.actors[&victim].hit_points, 30);
        state.tick = pb_core::ids::Tick(120);
        let events = step(
            &mut state,
            Command {
                actor_id: thrower,
                action: Action::Hold,
            },
        )
        .unwrap();
        assert_eq!(state.actors[&victim].hit_points, 6);
        assert!(state.actors[&victim].wounds.contains(&WoundType::Burned));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::SmokeDeposited { tile, density: 6 } if *tile == landing
        )));
    }

    #[test]
    fn dynamite_death_finalizes_companion_exactly_once() {
        let mut state = SimState::new(42, 1);
        let thrower = ActorId(1);
        let victim = ActorId(2);
        let mut attacker = make_actor();
        attacker.faction_id = "enemy".to_string();
        state.actors.insert(thrower, attacker);
        let target = TileXY::new(10, 10);
        let (landing, _) = crate::explosive::throw_dynamite(42, 1, 0, thrower.0, target);
        let mut companion = make_actor();
        companion.name = "c_whitehorse".to_string();
        companion.faction_id = "company".to_string();
        companion.is_companion = true;
        companion.position = landing;
        companion.hit_points = 20;
        state.actors.insert(victim, companion);
        state.sequence_clock.insert(victim, 0);

        let lit_events = step(
            &mut state,
            Command {
                actor_id: thrower,
                action: Action::ThrowDynamite(target),
            },
        )
        .unwrap();
        assert!(lit_events
            .iter()
            .any(|event| matches!(event, Event::DynamiteLit { .. })));
        state.tick = pb_core::ids::Tick(120);
        let events = step(
            &mut state,
            Command {
                actor_id: thrower,
                action: Action::Hold,
            },
        )
        .unwrap();

        assert!(!state.actors[&victim].alive);
        assert!(!state.sequence_clock.contains_key(&victim));
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event,
                    Event::ActorKilled { actor } if *actor == victim
                ))
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event,
                    Event::CompanionKilled { id } if id == "c_whitehorse"
                ))
                .count(),
            1
        );
    }

    // -----------------------------------------------------------------------
    // Stance change tests
    // -----------------------------------------------------------------------

    #[test]
    fn step_stance_crouch() {
        let mut state = SimState::new(42, 1);
        let id = ActorId(1);
        state.actors.insert(id, make_actor());

        let cmd = Command {
            actor_id: id,
            action: Action::StanceCrouch,
        };
        let result = step(&mut state, cmd);
        assert!(result.is_ok());
        assert_eq!(state.actors[&id].stance, Stance::Crouched);
        assert_eq!(state.actors[&id].ap, Ap(9)); // 10 - 1
    }

    #[test]
    fn step_stance_prone() {
        let mut state = SimState::new(42, 1);
        let id = ActorId(1);
        state.actors.insert(id, make_actor());

        let cmd = Command {
            actor_id: id,
            action: Action::StanceProne,
        };
        let result = step(&mut state, cmd);
        assert!(result.is_ok());
        assert_eq!(state.actors[&id].stance, Stance::Prone);
        assert_eq!(state.actors[&id].ap, Ap(8)); // 10 - 2
    }

    #[test]
    fn step_rise_from_prone() {
        let mut state = SimState::new(42, 1);
        let id = ActorId(1);
        let mut actor = make_actor();
        actor.stance = Stance::Prone;
        state.actors.insert(id, actor);

        let cmd = Command {
            actor_id: id,
            action: Action::RiseFromProne,
        };
        let result = step(&mut state, cmd);
        assert!(result.is_ok());
        assert_eq!(state.actors[&id].stance, Stance::Standing);
    }

    // -----------------------------------------------------------------------
    // Melee test
    // -----------------------------------------------------------------------

    #[test]
    fn step_melee_applies_damage() {
        let mut state = SimState::new(42, 1);
        let id = ActorId(1);
        let target = ActorId(2);
        state.actors.insert(id, make_actor());
        let mut t = make_actor();
        t.hit_points = 20;
        state.actors.insert(target, t);

        let cmd = Command {
            actor_id: id,
            action: Action::Melee(target),
        };
        let result = step(&mut state, cmd);
        assert!(result.is_ok());
        // HP should have decreased (damage = 1d6+3, at least 4)
        assert!(state.actors[&target].hit_points < 20);
        // AP should be deducted
        assert_eq!(state.actors[&id].ap, Ap(7)); // 10 - 3
    }

    // -----------------------------------------------------------------------
    // DrawBead test
    // -----------------------------------------------------------------------

    #[test]
    fn step_draw_bead_sets_overwatch_and_consumes_all_ap() {
        let mut state = SimState::new(42, 1);
        let id = ActorId(1);
        state.actors.insert(id, make_actor()); // Ap(10)

        let cmd = Command {
            actor_id: id,
            action: Action::DrawBead(ActorId(2)),
        };
        let result = step(&mut state, cmd);
        assert!(result.is_ok());
        assert!(state.overwatch.contains(&id));
        assert_eq!(state.actors[&id].ap, Ap(0)); // all AP consumed
    }

    // -----------------------------------------------------------------------
    // Rally test
    // -----------------------------------------------------------------------

    #[test]
    fn step_rally_restores_sand() {
        let mut state = SimState::new(42, 1);
        let id = ActorId(1);
        let target = ActorId(2);
        state.actors.insert(id, make_actor());
        let mut t = make_actor();
        t.sand = 2;
        t.max_sand = 10;
        state.actors.insert(target, t);

        let cmd = Command {
            actor_id: id,
            action: Action::Rally(target),
        };
        let result = step(&mut state, cmd);
        assert!(result.is_ok());
        assert_eq!(state.actors[&target].sand, 7); // 2 + 5
    }

    // -----------------------------------------------------------------------
    // Loot test
    // -----------------------------------------------------------------------

    #[test]
    fn step_loot_recovers_ammunition_from_adjacent_fallen_actor() {
        let mut state = SimState::new(42, 1);
        let id = ActorId(1);
        let target = ActorId(2);
        let mut actor = make_actor();
        actor.loaded_rounds = 0;
        state.actors.insert(id, actor);
        let mut fallen = make_actor();
        fallen.alive = false;
        fallen.position = TileXY::new(1, 0);
        state.actors.insert(target, fallen);

        let cmd = Command {
            actor_id: id,
            action: Action::Loot(target),
        };
        let result = step(&mut state, cmd);
        assert!(result.is_ok());
        assert_eq!(state.actors[&id].ap, Ap(8)); // 10 - 2
        assert_eq!(state.actors[&id].loaded_rounds, 2);
    }

    // -----------------------------------------------------------------------
    // CapAndBallReload test
    // -----------------------------------------------------------------------

    #[test]
    fn step_cap_and_ball_reload_consumes_ap() {
        let mut state = SimState::new(42, 1);
        let id = ActorId(1);
        let mut actor = make_actor();
        actor.loaded_rounds = 0;
        actor.fouling = 4;
        state.actors.insert(id, actor);

        let cmd = Command {
            actor_id: id,
            action: Action::CapAndBallReload,
        };
        let result = step(&mut state, cmd);
        assert!(result.is_ok());
        assert_eq!(state.actors[&id].ap, Ap(2)); // 10 - 8
        assert_eq!(state.actors[&id].loaded_rounds, 6);
        assert_eq!(state.actors[&id].fouling, 2);
    }

    // -----------------------------------------------------------------------
    // ClearJam test
    // -----------------------------------------------------------------------

    #[test]
    fn step_clear_jam_consumes_ap() {
        let mut state = SimState::new(42, 1);
        let id = ActorId(1);
        let mut actor = make_actor();
        actor.jammed = true;
        actor.fouling = 3;
        state.actors.insert(id, actor);

        let cmd = Command {
            actor_id: id,
            action: Action::ClearJam,
        };
        let result = step(&mut state, cmd);
        assert!(result.is_ok());
        assert_eq!(state.actors[&id].ap, Ap(6)); // 10 - 4
        assert!(!state.actors[&id].jammed);
        assert_eq!(state.actors[&id].fouling, 2);
    }

    #[test]
    fn difficult_terrain_and_pathfinder_use_authored_costs() {
        let id = ActorId(1);
        let destination = TileXY::new(1, 0);
        let mut state = SimState::new(42, 1);
        state.difficult_tiles.insert(destination);
        state.actors.insert(id, make_actor());
        step(
            &mut state,
            Command {
                actor_id: id,
                action: Action::Move(destination),
            },
        )
        .unwrap();
        assert_eq!(state.actors[&id].ap, Ap(8));

        let mut state = SimState::new(42, 1);
        state.difficult_tiles.insert(destination);
        let mut pathfinder = make_actor();
        pathfinder
            .progression
            .passives
            .insert("difficult_terrain_normal".to_string());
        state.actors.insert(id, pathfinder);
        step(
            &mut state,
            Command {
                actor_id: id,
                action: Action::Move(destination),
            },
        )
        .unwrap();
        assert_eq!(state.actors[&id].ap, Ap(9));
    }

    #[test]
    fn draw_bead_converts_ap_and_reacts_inside_facing_cone() {
        let watcher = ActorId(1);
        let mover = ActorId(2);
        let mut state = SimState::new(5, 1);
        let mut watcher_actor = make_actor();
        watcher_actor.faction_id = "company".to_string();
        watcher_actor.facing = Facing::East;
        state.actors.insert(watcher, watcher_actor);
        let mut mover_actor = make_actor();
        mover_actor.faction_id = "enemy".to_string();
        mover_actor.position = TileXY::new(2, 0);
        mover_actor.hit_points = 100;
        mover_actor.max_hp = 100;
        state.actors.insert(mover, mover_actor);

        let armed = step(
            &mut state,
            Command {
                actor_id: watcher,
                action: Action::DrawBead(mover),
            },
        )
        .unwrap();
        assert!(armed.contains(&Event::OverwatchSet {
            actor: watcher,
            reaction_points: 4,
        }));
        assert_eq!(state.actors[&watcher].ap, Ap(0));

        let reaction = step(
            &mut state,
            Command {
                actor_id: mover,
                action: Action::Move(TileXY::new(1, 0)),
            },
        )
        .unwrap();
        assert!(reaction.contains(&Event::ReactionShot {
            actor: watcher,
            target: mover,
        }));
        assert_eq!(state.actors[&watcher].loaded_rounds, 5);
        assert!(!state.overwatch.contains(&watcher));
    }

    #[test]
    fn full_directional_cover_refuses_shot_without_spending_ap() {
        let shooter = ActorId(1);
        let target = ActorId(2);
        let mut state = SimState::new(42, 1);
        state.actors.insert(shooter, make_actor());
        let mut target_actor = make_actor();
        target_actor.position = TileXY::new(5, 0);
        state.actors.insert(target, target_actor);
        state.cover_edges.insert(
            crate::state::CoverEdge {
                tile: TileXY::new(5, 0),
                facing: Facing::West,
            },
            crate::state::CoverState {
                level: crate::state::CoverLevel::Full,
                strikes: 0,
                half_height: false,
                burning: false,
            },
        );
        let before = crate::hash::compute_state_hash(&state);
        let error = step(
            &mut state,
            Command {
                actor_id: shooter,
                action: Action::SnapShot(target),
            },
        )
        .unwrap_err();
        assert_eq!(error, SimError::NoLineOfSight(shooter, target));
        assert_eq!(crate::hash::compute_state_hash(&state), before);
    }

    #[test]
    fn heavy_bleeding_ticks_before_the_actors_first_action() {
        let id = ActorId(1);
        let mut state = SimState::new(42, 1);
        let mut actor = make_actor();
        actor.hit_points = 10;
        state.actors.insert(id, actor);
        state.wound_effects.insert(
            id,
            crate::state::WoundEffects {
                heavy_bleeding: true,
                ..Default::default()
            },
        );
        let events = step(
            &mut state,
            Command {
                actor_id: id,
                action: Action::Hold,
            },
        )
        .unwrap();
        assert_eq!(state.actors[&id].hit_points, 6);
        assert!(events.contains(&Event::DamageApplied {
            actor: id,
            damage: 4,
        }));
    }

    #[test]
    fn nearby_allies_spend_sand_when_a_shot_suppresses_the_target() {
        let shooter = ActorId(1);
        let target = ActorId(2);
        let ally = ActorId(3);
        let mut state = SimState::new(5, 1);
        let mut shooter_actor = make_actor();
        shooter_actor.faction_id = "enemy".to_string();
        state.actors.insert(shooter, shooter_actor);
        let mut target_actor = make_actor();
        target_actor.faction_id = "company".to_string();
        target_actor.position = TileXY::new(5, 0);
        target_actor.sand = 20;
        target_actor.max_sand = 20;
        state.actors.insert(target, target_actor);
        let mut ally_actor = make_actor();
        ally_actor.faction_id = "company".to_string();
        ally_actor.position = TileXY::new(5, 1);
        ally_actor.sand = 20;
        ally_actor.max_sand = 20;
        state.actors.insert(ally, ally_actor);
        step(
            &mut state,
            Command {
                actor_id: shooter,
                action: Action::SnapShot(target),
            },
        )
        .unwrap();
        assert!(state.actors[&target].sand < 20);
        assert!(state.actors[&ally].sand < 20);
    }

    #[test]
    fn authored_cost_effects_change_only_the_named_actions() {
        let actor_id = ActorId(1);
        let target_id = ActorId(2);
        let mut actor = make_actor();
        actor
            .progression
            .passives
            .insert("aimed_shot_cost_3".to_string());
        actor
            .progression
            .passives
            .insert("reload_cost_minus_1".to_string());
        assert_eq!(action_cost(&Action::AimedShot(target_id), &actor), Ap(4));

        let mut state = SimState::new(42, 1);
        actor.weapon_profile.reload_class = "Cartridge".to_string();
        state.actors.insert(actor_id, actor);
        let mut target = make_actor();
        target.position = TileXY::new(5, 0);
        target.hit_points = 100;
        target.max_hp = 100;
        state.actors.insert(target_id, target);
        step(
            &mut state,
            Command {
                actor_id,
                action: Action::AimedShot(target_id),
            },
        )
        .unwrap();
        assert_eq!(state.actors[&actor_id].ap, Ap(7));
    }
}
