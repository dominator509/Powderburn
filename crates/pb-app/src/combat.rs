//! Combat screen â€” renders the isometric battlefield from SimState.
//!
//! Retains pb-render systems while displaying tiles, actors, overlays, and
//! smoke. Handles pointer selection/targeting and AI stepping.
//!
//! Also provides the full interactive combat loop: player clicks to select,
//! keyboard to choose actions, clicks to target, AI runs for enemies.

#![allow(dead_code)]

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use pb_content::load;
use pb_content::schema::Content;
use pb_core::geom::TileXY;
use pb_core::ids::ActorId;
use pb_core::metrics::MetricsRegistry;
use pb_render::camera::IsoCamera;
use pb_render::device::RenderDevice;
use pb_render::overlay::{OverlaySystem, OverlayTile, OverlayTileKind};
use pb_render::props::{prop_instances_from_state, PropSystem};
use pb_render::smoke::{SmokeSystem, SmokeTile};
use pb_render::sprites::{SpriteInstance, SpriteSystem};
use pb_render::tiles::{self, TileSystem, TileVisual};
use pb_sim::action::{
    effective_action_cost, legal_movement_cost, overwatch_threats_at, step, Action, Command,
};
use pb_sim::clock::advance_to_next_actor;
use pb_sim::shot::{compute_hit_chance_breakdown, HitChanceBreakdown};
use pb_sim::state::{ActorState, SimError, SimState, Stance};

use crate::state::{
    ActiveBattleDialogue, BattleAnimation, BattleAnimationKind, GameState, InteractionPhase,
    PlayerAction,
};
use pb_core::event::{Event, HitLocationType, WoundType};

// â”€â”€ Constants â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Default battlefield grid dimensions.
const GRID_COLS: u32 = 20;
const GRID_ROWS: u32 = 12;

/// Isometric tile dimensions in pixels (at zoom = 1.0).
const TILE_W: f32 = tiles::TILE_WIDTH;
const TILE_H: f32 = tiles::TILE_HEIGHT;
const GRID_EDGE_GUTTER: f32 = 24.0;

/// Keep the complete outer diamonds inside the viewport at every supported
/// resolution. The requested zoom remains the upper bound, so zooming out
/// still works while zooming in can never cut a perimeter tile in half.
fn fitted_battle_zoom(requested: f32, viewport_width: f32, viewport_height: f32) -> f32 {
    fitted_battle_zoom_for_grid(
        requested,
        viewport_width,
        viewport_height,
        GRID_COLS,
        GRID_ROWS,
    )
}

fn fitted_battle_zoom_for_grid(
    requested: f32,
    viewport_width: f32,
    viewport_height: f32,
    cols: u32,
    rows: u32,
) -> f32 {
    let board_width = (cols.max(1) + rows.max(1)) as f32 * TILE_W * 0.5;
    let board_height = (cols.max(1) + rows.max(1)) as f32 * TILE_H * 0.5;
    let available_width = (viewport_width - GRID_EDGE_GUTTER * 2.0).max(TILE_W);
    let available_height = (viewport_height - GRID_EDGE_GUTTER * 2.0).max(TILE_H);
    let fit = (available_width / board_width).min(available_height / board_height);

    requested.max(0.25).min(fit.max(0.25))
}

fn battle_grid_dimensions(game_state: &GameState) -> (u32, u32) {
    game_state
        .sim
        .as_ref()
        .map(|sim| (sim.smoke_cols.max(1), sim.smoke_rows.max(1)))
        .unwrap_or((GRID_COLS, GRID_ROWS))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MovementPreview {
    pub ap_cost: u8,
    pub remaining_ap: u8,
    pub distance: u8,
    pub ends_turn: bool,
    pub leaves_cover: bool,
    pub crosses_overwatch: bool,
}

// â”€â”€ Faction helpers â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Return true if the actor belongs to the authored player faction.
pub fn is_ally(actor: &ActorState) -> bool {
    actor.faction_id == "player"
}

/// Return true if the actor belongs to any non-player authored faction.
pub fn is_enemy(actor: &ActorState) -> bool {
    !actor.faction_id.is_empty() && actor.faction_id != "player"
}

/// Select a living company member by stable ActorId order.
///
/// Off-turn members remain inspectable, but the simulation still rejects any
/// command submitted out of turn.
pub fn select_squad_member(game_state: &mut GameState, index: usize) -> Result<(), String> {
    let sim = game_state
        .sim
        .as_ref()
        .ok_or_else(|| "E-UI-EMPTY: no battle is loaded".to_string())?;
    let members = sim
        .actors
        .iter()
        .filter(|(_, actor)| actor.alive && is_ally(actor))
        .collect::<Vec<_>>();
    let Some((id, actor)) = members.get(index).copied() else {
        return Err(format!(
            "E-UI-RANGE: squad member {} is unavailable",
            index + 1
        ));
    };
    game_state.phase = InteractionPhase::SelectedActor(*id);
    game_state.hovered_tile_x = actor.position.x;
    game_state.hovered_tile_y = actor.position.y;
    game_state.message = if sim.active_actor == Some(*id) {
        format!(
            "Selected {} â€” HP {}/{}, AP {}, facing {}",
            actor.name, actor.hit_points, actor.max_hp, actor.ap.0, actor.facing
        )
    } else {
        let active_name = sim
            .active_actor
            .and_then(|active| sim.actors.get(&active))
            .map_or("no one", |active| active.name.as_str());
        format!(
            "Inspecting {} â€” facing {}; {active_name} has the turn",
            actor.name, actor.facing
        )
    };
    Ok(())
}

/// Move keyboard focus to the next living enemy in stable ActorId order.
pub fn cycle_target(game_state: &mut GameState) -> Result<(), String> {
    let sim = game_state
        .sim
        .as_ref()
        .ok_or_else(|| "E-UI-EMPTY: no battle is loaded".to_string())?;
    let targets = sim
        .actors
        .iter()
        .filter(|(_, actor)| actor.alive && is_enemy(actor))
        .map(|(id, actor)| (*id, actor.position, actor.name.clone()))
        .collect::<Vec<_>>();
    if targets.is_empty() {
        return Err("E-UI-EMPTY: no living target".to_string());
    }
    let hovered = TileXY::new(game_state.hovered_tile_x, game_state.hovered_tile_y);
    let current = targets
        .iter()
        .position(|(_, position, _)| *position == hovered);
    let next = current.map_or(0, |index| (index + 1) % targets.len());
    let (_, position, name) = &targets[next];
    game_state.hovered_tile_x = position.x;
    game_state.hovered_tile_y = position.y;
    game_state.message = if let Some(breakdown) = compute_hit_chance_for_hover(game_state) {
        format!("Target: {name} â€” hit chance {}%", breakdown.total)
    } else {
        format!("Target: {name}")
    };
    Ok(())
}

/// Rotate the selected actor one eighth-turn through the simulation boundary.
pub fn rotate_selected_facing(game_state: &mut GameState, clockwise: bool) -> Result<(), String> {
    let actor_id = match game_state.phase {
        InteractionPhase::SelectedActor(id) | InteractionPhase::Targeting { actor: id, .. } => id,
        _ => return Err("E-UI-SELECTION: select an actor before rotating".to_string()),
    };
    let sim = game_state
        .sim
        .as_mut()
        .ok_or_else(|| "E-UI-EMPTY: no battle is loaded".to_string())?;
    let facing = sim
        .actors
        .get(&actor_id)
        .ok_or_else(|| "E-UI-SELECTION: selected actor is missing".to_string())?
        .facing;
    let delta = if clockwise { 1 } else { 7 };
    let next = pb_core::geom::Facing::from_index((facing.to_index() + delta) % 8);
    let events = step(
        sim,
        Command {
            actor_id,
            action: Action::Face(next),
        },
    )
    .map_err(|error| format!("E-ACTION-ILLEGAL: {error:?}"))?;
    game_state.battle_events.extend(events);
    game_state.message = format!("Facing {next} â€” changing facing costs 0 AP");
    Ok(())
}

/// Return the exact movement consequences for the selected actor and hovered
/// tile, sharing AP and overwatch rules with command execution.
pub fn movement_preview(game_state: &GameState) -> Option<MovementPreview> {
    movement_preview_for(
        game_state,
        TileXY::new(game_state.hovered_tile_x, game_state.hovered_tile_y),
    )
}

fn movement_preview_for(game_state: &GameState, destination: TileXY) -> Option<MovementPreview> {
    let sim = game_state.sim.as_ref()?;
    let actor_id = match game_state.phase {
        InteractionPhase::SelectedActor(id) => id,
        _ => return None,
    };
    let actor = sim.actors.get(&actor_id)?;
    let distance = actor.position.chebyshev_distance(destination);
    let sprint = match distance {
        1 => false,
        2 => true,
        _ => return None,
    };
    let cost = legal_movement_cost(sim, actor_id, destination, sprint)
        .ok()?
        .0;
    let ap_cost = u8::try_from(cost.max(0)).unwrap_or(u8::MAX);
    let remaining_ap = u8::try_from(actor.ap.0.saturating_sub(cost).max(0)).unwrap_or(0);
    let has_cover = |tile| {
        sim.cover_edges.iter().any(|(edge, cover)| {
            edge.tile == tile && cover.level != pb_sim::state::CoverLevel::None
        })
    };
    Some(MovementPreview {
        ap_cost,
        remaining_ap,
        distance: u8::try_from(distance.max(0)).unwrap_or(u8::MAX),
        ends_turn: sprint,
        leaves_cover: has_cover(actor.position) && !has_cover(destination),
        crosses_overwatch: !overwatch_threats_at(sim, actor_id, destination).is_empty(),
    })
}

fn movement_range(game_state: &GameState) -> Vec<(u32, u32, MovementPreview)> {
    let Some(sim) = game_state.sim.as_ref() else {
        return Vec::new();
    };
    let actor_id = match game_state.phase {
        InteractionPhase::SelectedActor(id) => id,
        _ => return Vec::new(),
    };
    let Some(actor) = sim.actors.get(&actor_id) else {
        return Vec::new();
    };

    let mut destinations = Vec::with_capacity(24);
    for y in actor.position.y.saturating_sub(2)..=actor.position.y.saturating_add(2) {
        for x in actor.position.x.saturating_sub(2)..=actor.position.x.saturating_add(2) {
            let destination = TileXY::new(x, y);
            if let Some(preview) = movement_preview_for(game_state, destination) {
                destinations.push((
                    u32::try_from(x).unwrap_or(0),
                    u32::try_from(y).unwrap_or(0),
                    preview,
                ));
            }
        }
    }
    destinations
}

// â”€â”€ Coordinate conversion â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Convert screen pixel coordinates to tile coordinates using the isometric
/// projection inverse.
pub fn screen_to_tile(
    mouse_x: f64,
    mouse_y: f64,
    camera_x: f32,
    camera_y: f32,
    camera_zoom: f32,
    viewport_width: f32,
    viewport_height: f32,
) -> TileXY {
    // The forward projection used in build_sprite_instances:
    //   wx = (x - y) * half_w
    //   wy = (x + y) * half_h
    //
    // The IsoCamera::world_to_screen adds viewport offset and camera pan:
    //   sx = wx + viewport_width/2 - camera_x
    //   sy = wy + viewport_height/2 - camera_y
    //
    // Inverse: compute (x, y) from (mouse_x, mouse_y)
    let zoom = camera_zoom.max(0.01);
    let sx = (mouse_x as f32 - viewport_width * 0.5) / zoom + camera_x;
    // Window coordinates grow down while the tactical world's +Y axis grows
    // up.  Mirroring here keeps pointer picking aligned with IsoCamera's
    // orthographic projection.
    let sy = (viewport_height * 0.5 - mouse_y as f32) / zoom + camera_y;

    let half_w = tiles::TILE_HALF_WIDTH;
    let half_h = tiles::TILE_HALF_HEIGHT;

    if half_w.abs() < f32::EPSILON || half_h.abs() < f32::EPSILON {
        return TileXY::new(0, 0);
    }

    let a = sx / half_w;
    let b = sy / half_h;

    // Rendered diamonds are centered on integer tile coordinates. Flooring
    // would instead center the selectable region on (x + 0.5, y + 0.5),
    // exactly where four rendered tile corners meet.
    let tile_x = ((a + b) * 0.5).round() as i16;
    let tile_y = ((b - a) * 0.5).round() as i16;

    TileXY::new(tile_x, tile_y)
}

/// Elevation-aware pointer picking against the actual raised tile diamonds.
pub fn screen_to_tile_in_state(
    game_state: &GameState,
    mouse_x: f64,
    mouse_y: f64,
    viewport_width: f32,
    viewport_height: f32,
) -> TileXY {
    let (cols, rows) = battle_grid_dimensions(game_state);
    let zoom = fitted_battle_zoom_for_grid(
        game_state.camera_zoom,
        viewport_width,
        viewport_height,
        cols,
        rows,
    );
    let fallback = screen_to_tile(
        mouse_x,
        mouse_y,
        game_state.camera_x,
        game_state.camera_y,
        zoom,
        viewport_width,
        viewport_height,
    );
    let Some(sim) = game_state.sim.as_ref() else {
        return fallback;
    };
    let world_x = (mouse_x as f32 - viewport_width * 0.5) / zoom + game_state.camera_x;
    let world_y = (viewport_height * 0.5 - mouse_y as f32) / zoom + game_state.camera_y;
    let mut best = None::<(f32, i32, TileXY)>;

    let (cols, rows) = battle_grid_dimensions(game_state);
    for y in 0..rows {
        for x in 0..cols {
            let tile = TileXY::new(x as i16, y as i16);
            let elevation = sim.tile_elevations.get(&tile).copied().unwrap_or(0);
            let distance = tiles::tile_diamond_distance(world_x, world_y, tile, elevation);
            if distance > 1.001 {
                continue;
            }
            if best
                .as_ref()
                .is_none_or(|(best_distance, best_elevation, _)| {
                    distance < *best_distance
                        || ((distance - *best_distance).abs() < 0.001
                            && elevation > *best_elevation)
                })
            {
                best = Some((distance, elevation, tile));
            }
        }
    }
    best.map_or(fallback, |(_, _, tile)| tile)
}

fn parse_stance(s: &str) -> Stance {
    match s.to_lowercase().as_str() {
        "crouched" => Stance::Crouched,
        "prone" => Stance::Prone,
        _ => Stance::Standing,
    }
}

fn parse_tile_key(key: &str) -> Option<TileXY> {
    let normalized = key
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .replace(':', ",");
    let mut parts = normalized.split(',').map(str::trim);
    let x = parts.next()?.parse::<i16>().ok()?;
    let y = parts.next()?.parse::<i16>().ok()?;
    (parts.next().is_none()).then_some(TileXY::new(x, y))
}

fn apply_authored_map(sim: &mut SimState, scenario: &pb_content::schema::ScenarioData) {
    sim.smoke_cols = scenario.map.width;
    sim.smoke_rows = scenario.map.height;
    sim.terrain_tiles.clear();
    sim.tile_elevations.clear();
    sim.difficult_tiles.clear();
    let cells = scenario
        .map
        .width
        .checked_mul(scenario.map.height)
        .and_then(|count| usize::try_from(count).ok())
        .unwrap_or(0);
    sim.smoke_grid = vec![0; cells];
    for (key, tile) in &scenario.map.tiles {
        let Some(position) = parse_tile_key(key) else {
            continue;
        };
        if !matches!(tile.terrain.as_str(), "Clear" | "Grass" | "Floor" | "Road") {
            sim.difficult_tiles.insert(position);
        }
        sim.terrain_tiles.insert(position, tile.terrain.clone());
        sim.tile_elevations.insert(position, tile.elevation);
        if position.x >= 0
            && position.y >= 0
            && (position.x as u32) < sim.smoke_cols
            && (position.y as u32) < sim.smoke_rows
        {
            let index = position.y as usize * sim.smoke_cols as usize + position.x as usize;
            sim.smoke_grid[index] = tile.smoke_density.min(6) as u8;
        }
        for (index, authored) in tile.cover_edges.iter().enumerate() {
            let (level, half_height) = match authored.as_str() {
                "Soft" => (pb_sim::state::CoverLevel::Soft, false),
                "Hard" => (pb_sim::state::CoverLevel::Hard, false),
                "Full" => (pb_sim::state::CoverLevel::Full, false),
                "HalfHeight" => (pb_sim::state::CoverLevel::Hard, true),
                _ => continue,
            };
            sim.cover_edges.insert(
                pb_sim::state::CoverEdge {
                    tile: position,
                    facing: pb_core::geom::Facing::from_index(index),
                },
                pb_sim::state::CoverState {
                    level,
                    strikes: 0,
                    half_height,
                    burning: tile.fire,
                },
            );
        }
    }
}

fn company_deployment_positions(
    scenario: &pb_content::schema::ScenarioData,
    count: usize,
) -> Vec<pb_content::schema::TileXYData> {
    let mut positions = Vec::with_capacity(count);
    let mut occupied = std::collections::BTreeSet::new();
    for position in scenario
        .deployment_zones
        .get("ally")
        .or_else(|| scenario.deployment_zones.get("player"))
        .into_iter()
        .flatten()
    {
        if occupied.insert((position.x, position.y)) {
            positions.push(position.clone());
            if positions.len() == count {
                return positions;
            }
        }
    }
    let max_x = (scenario.map.width / 3).max(2);
    for y in 1..scenario.map.height.saturating_sub(1) {
        for x in 1..max_x {
            let Ok(x) = i16::try_from(x) else {
                continue;
            };
            let Ok(y) = i16::try_from(y) else {
                continue;
            };
            if occupied.insert((x, y)) {
                positions.push(pb_content::schema::TileXYData { x, y });
                if positions.len() == count {
                    return positions;
                }
            }
        }
    }
    while positions.len() < count {
        positions.push(pb_content::schema::TileXYData {
            x: 1,
            y: positions.len().min(i16::MAX as usize) as i16,
        });
    }
    positions
}

fn hydrate_progression_effects(
    progression: &mut pb_sim::progression::ActorProgression,
    actor: &pb_content::schema::ActorData,
    content: &pb_content::schema::Content,
) {
    for mark_id in &actor.marks {
        let Some(mark) = content.marks.get(mark_id) else {
            continue;
        };
        if let Some(passive) = &mark.effects.passive {
            progression.passives.insert(passive.clone());
        }
        if let Some(ability) = &mark.effects.ability_grant {
            progression.abilities.insert(ability.clone());
        }
        for (key, value) in [
            ("ap_bonus", mark.effects.ap_bonus.map(i32::from)),
            ("accuracy_bonus", mark.effects.accuracy_bonus),
            ("penalty_reduction", mark.effects.penalty_reduction),
            ("cost_reduction", mark.effects.cost_reduction),
        ] {
            if let Some(value) = value {
                let total = progression
                    .effect_values
                    .entry(key.to_string())
                    .or_insert(0);
                *total = total.saturating_add(value);
            }
        }
    }
    if let Some(way) = actor
        .ways
        .first()
        .and_then(|way_id| content.ways.get(way_id))
    {
        progression
            .passives
            .extend(way.effects.passives.iter().cloned());
    }
}

fn apply_actor_runtime_metadata(
    sim: &mut SimState,
    actor_id: ActorId,
    actor: &pb_content::schema::ActorData,
    content: &pb_content::schema::Content,
) {
    let authored_faction = content.factions.get(&actor.faction_id).or_else(|| {
        let inferred = if actor.faction_id == "player" {
            None
        } else if actor.archetype_id.contains("bandit") || actor.faction_id == "enemy" {
            Some("f_bandits")
        } else if actor.archetype_id.contains("cavalry") {
            Some("f_10th_cavalry")
        } else if actor.archetype_id.contains("army") {
            Some("f_us_army")
        } else if actor.archetype_id.contains("law") {
            Some("f_lawmen")
        } else {
            None
        };
        inferred.and_then(|id| content.factions.get(id))
    });
    sim.sand_multiplier_pct.insert(
        actor_id,
        authored_faction.map_or(100, |faction| i32::from(faction.sand_multiplier_percent)),
    );
    let mut total_tenths = actor.inventory.iter().fold(0u64, |total, stack| {
        let item_weight = content
            .items
            .get(&stack.item_id)
            .map_or(0, |item| u64::from(item.weight_tenths_lb));
        total.saturating_add(item_weight.saturating_mul(u64::from(stack.count)))
    });
    if let Some(way) = actor
        .ways
        .first()
        .and_then(|way_id| content.ways.get(way_id))
    {
        for item_id in &way.starting_items {
            total_tenths = total_tenths.saturating_add(
                content
                    .items
                    .get(item_id)
                    .map_or(0, |item| u64::from(item.weight_tenths_lb)),
            );
        }
    }
    let pounds = ((total_tenths.saturating_add(9)) / 10).min(i32::MAX as u64) as i32;
    sim.carry_weight_lbs.insert(actor_id, pounds);
    let mut effects = pb_sim::state::WoundEffects {
        heavy_bleeding: actor.wounds.iter().any(|wound| wound == "Bleeding"),
        winded: actor.wounds.iter().any(|wound| wound == "Winded"),
        ..pb_sim::state::WoundEffects::default()
    };
    if actor.wounds.iter().any(|wound| wound == "Concussed") {
        effects.concussed_turns = 3;
    }
    if effects != pb_sim::state::WoundEffects::default() {
        sim.wound_effects.insert(actor_id, effects);
    }
}

// â”€â”€ Public API â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Load the selected scenario and create its deterministic simulation state.
/// Screen transitions remain the caller's responsibility.
pub fn init_combat(game_state: &mut GameState, content_root: &Path) -> Result<(), String> {
    game_state.battle_events.clear();
    game_state.battle_dialogue.clear();
    game_state.battle_dialogue_seen.clear();
    game_state.active_battle_dialogue = None;
    game_state.battle_dialogue_queue.clear();
    game_state.pending_ledger_writes.clear();
    game_state.ledger_write_cursor = 0;
    let content: Content =
        load::load_all(content_root).map_err(|e| format!("content load failed: {e}"))?;

    let mission_id = game_state
        .current_mission
        .as_deref()
        .ok_or_else(|| "no campaign mission selected".to_string())?;
    let scenario = crate::campaign::scenario_for_mission(&content, mission_id)?;
    let scenario_id = scenario.id.as_str();
    game_state.battle_dialogue = scenario.battle_dialogue.clone();

    let seed = game_state
        .campaign
        .as_ref()
        .map(|campaign| campaign.campaign_seed)
        .ok_or_else(|| "no active campaign".to_string())?;
    let scenario_hash = pb_core::hash::hash_state(scenario_id.as_bytes());
    let mut sim = SimState::new(
        seed,
        u32::from_le_bytes([
            scenario_hash[0],
            scenario_hash[1],
            scenario_hash[2],
            scenario_hash[3],
        ]),
    );
    sim.light_level = match scenario.light.as_str() {
        "Dusk" => pb_sim::environment::LightLevel::Dusk,
        "Night" => pb_sim::environment::LightLevel::Night,
        "Moonlit" => pb_sim::environment::LightLevel::Moonlit,
        "Lanternlit" => pb_sim::environment::LightLevel::Lanternlit,
        _ => pb_sim::environment::LightLevel::Day,
    };
    sim.weather = match scenario.weather.as_str() {
        "Rain" => pb_sim::environment::Weather::Rain,
        "Snow" => pb_sim::environment::Weather::Snow,
        "Dust" => pb_sim::environment::Weather::Dust,
        "Wind" => pb_sim::environment::Weather::Wind,
        _ => pb_sim::environment::Weather::Clear,
    };
    sim.wind_direction = match scenario.wind_dir.as_str() {
        "NE" => pb_core::geom::Facing::NorthEast,
        "E" => pb_core::geom::Facing::East,
        "SE" => pb_core::geom::Facing::SouthEast,
        "S" => pb_core::geom::Facing::South,
        "SW" => pb_core::geom::Facing::SouthWest,
        "W" => pb_core::geom::Facing::West,
        "NW" => pb_core::geom::Facing::NorthWest,
        _ => pb_core::geom::Facing::North,
    };
    apply_authored_map(&mut sim, scenario);

    let mut battle_actors: Vec<pb_content::schema::ActorData> = scenario
        .actors
        .iter()
        .filter(|actor| !matches!(actor.faction_id.as_str(), "player" | "ally"))
        .cloned()
        .collect();
    let living_company = game_state
        .campaign
        .as_ref()
        .map(|campaign| {
            campaign
                .company
                .iter()
                .filter(|actor| !actor.is_dead)
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if living_company.is_empty() {
        battle_actors.extend(
            scenario
                .actors
                .iter()
                .filter(|actor| matches!(actor.faction_id.as_str(), "player" | "ally"))
                .cloned(),
        );
    } else {
        let deployment = company_deployment_positions(scenario, living_company.len());
        for (index, mut actor) in living_company.into_iter().enumerate() {
            actor.faction_id = "player".to_string();
            actor.pos = deployment[index].clone();
            actor.ap = 0;
            battle_actors.push(actor);
        }
    }

    for actor_data in &battle_actors {
        let hash = pb_core::hash::hash_state(actor_data.id.as_bytes());
        let actor_id = ActorId(u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]]));
        let mut attributes = actor_data.attributes;
        if let Some(way) = actor_data
            .ways
            .first()
            .and_then(|way_id| content.ways.get(way_id))
        {
            attributes.grit = (attributes.grit + way.stat_mods.grit).clamp(1, 10);
            attributes.nerve = (attributes.nerve + way.stat_mods.nerve).clamp(1, 10);
            attributes.wind = (attributes.wind + way.stat_mods.wind).clamp(1, 10);
            attributes.hands = (attributes.hands + way.stat_mods.hands).clamp(1, 10);
            attributes.eyes = (attributes.eyes + way.stat_mods.eyes).clamp(1, 10);
            attributes.savvy = (attributes.savvy + way.stat_mods.savvy).clamp(1, 10);
            attributes.luck = (attributes.luck + way.stat_mods.luck).clamp(1, 10);
        }
        let weapon = actor_data
            .equipped_primary
            .clone()
            .or_else(|| actor_data.equipped_sidearm.clone())
            .unwrap_or_else(|| "colt_army_1860".to_string());
        let weapon_capacity = content
            .weapons
            .get(&weapon)
            .map_or(6, |weapon| weapon.capacity);
        let weapon_profile = content.weapons.get(&weapon).map_or_else(
            pb_sim::state::WeaponProfile::default,
            |weapon| pb_sim::state::WeaponProfile {
                damage_count: weapon.damage_dice.count,
                damage_sides: weapon.damage_dice.sides,
                damage_bonus: weapon.damage_dice.bonus,
                accuracy: weapon.accuracy,
                ap_overrides: weapon.ap_override.clone(),
                range_bands: weapon.range_bands,
                reload_class: weapon.reload_class.clone(),
                fouling_rate: weapon.fouling_rate,
                base_misfire: weapon.base_misfire,
                smoke_output: weapon.smoke_output,
                two_handed: weapon.two_handed,
            },
        );
        let wounds = actor_data
            .wounds
            .iter()
            .filter_map(|wound| match wound.as_str() {
                "Bleeding" => Some(pb_core::event::WoundType::Bleeding),
                "Broken" => Some(pb_core::event::WoundType::Broken),
                "Concussed" => Some(pb_core::event::WoundType::Concussed),
                "Winded" => Some(pb_core::event::WoundType::Winded),
                "Blinded" => Some(pb_core::event::WoundType::Blinded),
                "Burned" => Some(pb_core::event::WoundType::Burned),
                "Shocked" => Some(pb_core::event::WoundType::Shocked),
                _ => None,
            })
            .collect();
        let max_hp = attributes.hit_points(actor_data.level);
        let max_sand = attributes.sand();
        let hit_points = actor_data.hp.clamp(0, max_hp);
        let mut progression = pb_sim::progression::ActorProgression::at_level(actor_data.level);
        progression.xp = actor_data.xp;
        progression.skill_points = actor_data.skill_points;
        progression.skill_levels = actor_data
            .skill_levels
            .iter()
            .filter_map(|(name, level)| {
                let skill = match name.as_str() {
                    "Pistols" => pb_core::progression::SkillLine::Pistols,
                    "LongGuns" => pb_core::progression::SkillLine::LongGuns,
                    "Scatterguns" => pb_core::progression::SkillLine::Scatterguns,
                    "Blades" => pb_core::progression::SkillLine::Blades,
                    "Explosives" => pb_core::progression::SkillLine::Explosives,
                    "FieldMedicine" => pb_core::progression::SkillLine::FieldMedicine,
                    "Scouting" => pb_core::progression::SkillLine::Scouting,
                    "Talk" => pb_core::progression::SkillLine::Talk,
                    _ => return None,
                };
                Some((skill, *level))
            })
            .collect();
        progression.marks = actor_data.marks.clone();
        progression.way = actor_data.ways.first().cloned();
        hydrate_progression_effects(&mut progression, actor_data, &content);
        let (sequence_bonus, sand_percent) = actor_data
            .ways
            .first()
            .and_then(|way_id| content.ways.get(way_id))
            .map_or((0, 100), |way| {
                (way.effects.sequence_bonus, way.effects.sand_percent)
            });
        let adjusted_max_sand = max_sand.saturating_mul(i32::from(sand_percent)) / 100;

        let actor_state = ActorState {
            faction_id: actor_data.faction_id.clone(),
            is_companion: actor_data.is_companion,
            attributes,
            ap: pb_core::ids::Ap(0),
            position: TileXY::new(actor_data.pos.x, actor_data.pos.y),
            facing: pb_core::geom::Facing::from_index(actor_data.facing as usize % 8),
            sequence: attributes.sequence().saturating_add(sequence_bonus),
            hit_points,
            max_hp,
            name: actor_data.id.clone(),
            alive: !actor_data.is_dead && hit_points > 0,
            routed: false,
            wounds,
            sand: actor_data.sand.clamp(0, adjusted_max_sand),
            max_sand: adjusted_max_sand,
            stance: parse_stance(&actor_data.stance),
            progression,
            weapon,
            weapon_profile,
            loaded_rounds: weapon_capacity,
            weapon_capacity,
            fouling: 0,
            jammed: false,
        };

        sim.actors.insert(actor_id, actor_state);
        sim.sequence_clock.insert(actor_id, 0);
        apply_actor_runtime_metadata(&mut sim, actor_id, actor_data, &content);
    }

    game_state.sim = Some(sim);
    game_state.camera_x = (scenario.map.width as f32 - scenario.map.height as f32) * TILE_W * 0.25;
    game_state.camera_y = scenario
        .map
        .width
        .saturating_add(scenario.map.height)
        .saturating_sub(2) as f32
        * TILE_H
        * 0.25;
    game_state.camera_zoom = 1.35;
    game_state.phase = InteractionPhase::Idle;
    game_state.tick = 0;
    game_state.message = format!("{} loaded â€” preparing first turn", scenario.display_name);

    // Let the clock choose the first actor exactly once.  `run_enemy_ai`
    // advances through any opening enemy turns and stops with the first player
    // actor active; pre-advancing here used to skip that actor entirely.
    run_enemy_ai(game_state)?;
    check_victory_conditions(game_state);
    if game_state.screen == crate::state::GameScreen::Battle {
        let active_name = game_state
            .sim
            .as_ref()
            .and_then(|state| state.active_actor)
            .and_then(|id| {
                game_state
                    .sim
                    .as_ref()
                    .and_then(|state| state.actors.get(&id))
            })
            .map_or("your active ally", |actor| actor.name.as_str());
        game_state.message = format!(
            "{} loaded â€” left-click {active_name} to act",
            scenario.display_name
        );
    }

    Ok(())
}

/// Persistent tactical GPU resources.
///
/// Pipelines and decoded atlases are expensive to createâ€”particularly on
/// software Vulkan adaptersâ€”so they live for the battle instead of one frame.
#[allow(missing_debug_implementations)]
pub struct CombatRenderer {
    scenario_id: u32,
    cols: u32,
    rows: u32,
    surface_format: wgpu::TextureFormat,
    camera_bytes: [u8; 64],
    smoke_tiles: Vec<SmokeTile>,
    overlay_tiles: Vec<OverlayTile>,
    props: Vec<SpriteInstance>,
    sprites: Vec<SpriteInstance>,
    tile_system: TileSystem,
    smoke_system: SmokeSystem,
    overlay_system: OverlaySystem,
    prop_system: PropSystem,
    sprite_system: SpriteSystem,
}

impl CombatRenderer {
    fn new(
        game_state: &GameState,
        render_device: &Arc<RenderDevice>,
        surface_format: wgpu::TextureFormat,
        camera_bytes: &[u8; 64],
    ) -> Self {
        let tiles = build_tile_visuals(game_state);
        let smoke_tiles = build_smoke_grid(game_state);
        let overlay_tiles = build_overlay_tiles(game_state);
        let props = game_state
            .sim
            .as_ref()
            .map(prop_instances_from_state)
            .unwrap_or_default();
        let sprites = build_sprite_instances(game_state);
        let (cols, rows) = battle_grid_dimensions(game_state);
        let tile_system = TileSystem::new_with_format(
            render_device,
            cols,
            rows,
            &tiles,
            camera_bytes,
            surface_format,
        );
        let smoke_system = SmokeSystem::new_with_format(
            render_device,
            cols,
            rows,
            &smoke_tiles,
            camera_bytes,
            surface_format,
        );
        let overlay_system = OverlaySystem::new_with_format(
            render_device,
            &overlay_tiles,
            camera_bytes,
            surface_format,
        );
        let prop_system =
            PropSystem::new_with_format(render_device, &props, camera_bytes, surface_format);
        let sprite_system =
            SpriteSystem::new_with_format(render_device, &sprites, camera_bytes, surface_format);
        Self {
            scenario_id: game_state
                .sim
                .as_ref()
                .map_or(u32::MAX, |state| state.scenario_id),
            cols,
            rows,
            surface_format,
            camera_bytes: *camera_bytes,
            smoke_tiles,
            overlay_tiles,
            props,
            sprites,
            tile_system,
            smoke_system,
            overlay_system,
            prop_system,
            sprite_system,
        }
    }

    fn update(
        &mut self,
        game_state: &GameState,
        render_device: &Arc<RenderDevice>,
        camera_bytes: &[u8; 64],
    ) {
        let camera_changed = self.camera_bytes != *camera_bytes;
        let (cols, rows) = battle_grid_dimensions(game_state);
        let smoke_tiles = build_smoke_grid(game_state);
        let overlay_tiles = build_overlay_tiles(game_state);
        let props = game_state
            .sim
            .as_ref()
            .map(prop_instances_from_state)
            .unwrap_or_default();
        let sprites = build_sprite_instances(game_state);

        if smoke_tiles != self.smoke_tiles || (cols, rows) != (self.cols, self.rows) {
            self.smoke_system
                .update(render_device, cols, rows, &smoke_tiles, camera_bytes);
            self.smoke_tiles = smoke_tiles;
            self.cols = cols;
            self.rows = rows;
        }
        if overlay_tiles != self.overlay_tiles {
            self.overlay_system
                .update(render_device, &overlay_tiles, camera_bytes);
            self.overlay_tiles = overlay_tiles;
        }
        if props != self.props {
            self.prop_system.update(render_device, &props, camera_bytes);
            self.props = props;
        }
        if sprites != self.sprites {
            self.sprite_system
                .update(render_device, &sprites, camera_bytes);
            self.sprites = sprites;
        }
        if camera_changed {
            self.tile_system.update_camera(render_device, camera_bytes);
            self.smoke_system.update_camera(render_device, camera_bytes);
            self.overlay_system
                .update_camera(render_device, camera_bytes);
            self.prop_system.update_camera(render_device, camera_bytes);
            self.sprite_system
                .update_camera(render_device, camera_bytes);
            self.camera_bytes = *camera_bytes;
        }
    }
}

/// Render one frame of the combat screen using persistent GPU resources.
pub fn render_combat_frame(
    renderer: &mut Option<CombatRenderer>,
    game_state: &GameState,
    render_device: &Arc<RenderDevice>,
    view: &wgpu::TextureView,
    surface_format: wgpu::TextureFormat,
    viewport_width: u32,
    viewport_height: u32,
) {
    let _timer = Instant::now();
    let (cols, rows) = battle_grid_dimensions(game_state);

    // â”€â”€ Camera â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    let camera = IsoCamera {
        center_x: game_state.camera_x,
        center_y: game_state.camera_y,
        zoom: fitted_battle_zoom_for_grid(
            game_state.camera_zoom,
            viewport_width as f32,
            viewport_height as f32,
            cols,
            rows,
        ),
        viewport_width: viewport_width as f32,
        viewport_height: viewport_height as f32,
        tile_w: TILE_W,
        tile_h: TILE_H,
    };
    let camera_bytes = camera.ortho_matrix_bytes();

    let scenario_id = game_state
        .sim
        .as_ref()
        .map_or(u32::MAX, |state| state.scenario_id);
    let rebuild = renderer.as_ref().is_none_or(|cached| {
        cached.scenario_id != scenario_id
            || (cached.cols, cached.rows) != (cols, rows)
            || cached.surface_format != surface_format
    });
    if rebuild {
        *renderer = Some(CombatRenderer::new(
            game_state,
            render_device,
            surface_format,
            &camera_bytes,
        ));
    } else if let Some(cached) = renderer.as_mut() {
        cached.update(game_state, render_device, &camera_bytes);
    }
    let Some(renderer) = renderer.as_ref() else {
        return;
    };

    // â”€â”€ Command encoder & render pass â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    let mut encoder =
        render_device
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("combat encoder"),
            });

    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("combat pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.15,
                        g: 0.20,
                        b: 0.12,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        // Draw in z-order: terrain â†’ smoke â†’ overlay â†’ props â†’ sprites
        renderer.tile_system.render(&mut rpass);
        renderer.smoke_system.render(&mut rpass);
        renderer.overlay_system.render(&mut rpass);
        renderer.prop_system.render(&mut rpass);
        renderer.sprite_system.render(&mut rpass);
    }

    render_device
        .queue
        .submit(std::iter::once(encoder.finish()));

    let elapsed_ms = match u64::try_from(_timer.elapsed().as_millis()) {
        Ok(value) =>#~µ×«h‘éì¶»§q«^u”¹Á¡…Í”°%¹Ñ•É…Ñ¥½¹A¡…Í”èéM•±•Ñ•‘Ñ½È¡…Ñ½É}¥¤¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„ (€€€€€€€€€€€…µ”¹Í¥´¹…Í}É•˜ ¤¹•áÁ•Ð ‰Í¥µÕ±…Ñ¥½¸ˆ¤¹…Ñ½ÉÍl™…Ñ½É}¥‘t(€€€€€€€€€€€€€€€€¹…À(€€€€€€€€€€€€€€€€¸À°(€€€€€€€€€€€€Ä(€€€€€€€€¤ì(€€€€€€€…ÍÍ•ÉÐ„¡…µ”¹µ•ÍÍ…”¹½¹Ñ…¥¹Ì ‰Q%=8%1èMAI%9Pˆ¤¤ì(€€€€€€€…ÍÍ•ÉÐ„¡…µ”¹µ•ÍÍ…”¹½¹Ñ…¥¹Ì ‰@€Ä€´€À€ô€Äˆ¤¤ì(€€€€€€€…ÍÍ•ÉÐ„¡…µ”¹µ•ÍÍ…”¹½¹Ñ…¥¹Ì ˆÀ@ÍÁ•¹Ðˆ¤¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„ (€€€€€€€€€€€…µ”¹½µ‰…Ñ}±½œ¹±…ÍÐ ¤¹•áÁ•Ð ‰™…¥±ÕÉ”±½œˆ¤¹Ñ½¹”°(€€€€€€€€€€€É…Ñ”èéÍÑ…Ñ”èé½µ‰…Ñ1½Q½¹”èé…¥±ÕÉ”(€€€€€€€€¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸Ñ…Ñ¥…±}‰ÕÑÑ½¹Í}‘¥ÍÁ±…å}…ÕÑ¡½É¥Ñ…Ñ¥Ù•}…Á}½ÍÑÍ}…¹‘}•¹‘}ÑÕÉ¹}Í•µ…¹Ñ¥Ì ¤ì(€€€€€€€±•ÐµÕÐ…µ”€ô…µ•MÑ…Ñ”èé¹•Ü ¤ì(€€€€€€€±•Ð…Ñ½É}¥€ôÑ½É% Ä¤ì(€€€€€€€±•ÐµÕÐÍ¥´€ôM¥µMÑ…Ñ”èé¹•Ü ÐÈ°€Ä¤ì(€€€€€€€±•ÐµÕÐ…Ñ½È€ô‰Õ¥±‘}…Ñ½È¡…Ñ½É}¥°€‰A±…å•Èˆ°€Ô°€ÄÀÀ°€ÈÀ°Q¥±•adèé¹•Ü È°€È¤¤ì(€€€€€€€…Ñ½È¹™…Ñ¥½¹}¥€ô€‰Á±…å•Èˆ¹Ñ½}ÍÑÉ¥¹œ ¤ì(€€€€€€€…Ñ½È¹…À€ôÁ‰}½É”èé¥‘ÌèéÀ Ø¤ì(€€€€€€€Í¥´¹…Ñ¥Ù•}…Ñ½È€ôM½µ”¡…Ñ½É}¥¤ì(€€€€€€€Í¥´¹…Ñ½ÉÌ¹¥¹Í•ÉÐ¡…Ñ½É}¥°…Ñ½È¤ì(€€€€€€€…µ”¹Í¥´€ôM½µ”¡Í¥´¤ì(€€€€€€€…µ”¹Á¡…Í”€ô%¹Ñ•É…Ñ¥½¹A¡…Í”èéM•±•Ñ•‘Ñ½È¡…Ñ½É}¥¤ì((€€€€€€€…ÍÍ•ÉÑ}•Ä„ (€€€€€€€€€€€‰…ÑÑ±•}…Ñ¥½¹}‰ÕÑÑ½¹}±…‰•° ™…µ”°	…ÑÑ±•!Õ‘Ñ¥½¸èé¥É”¤°(€€€€€€€€€€€€‰%IlÌAtˆ(€€€€€€€€¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„ (€€€€€€€€€€€‰…ÑÑ±•}…Ñ¥½¹}‰ÕÑÑ½¹}±…‰•° ™…µ”°	…ÑÑ±•!Õ‘Ñ¥½¸èé¥´¤°(€€€€€€€€€€€€‰%4lÐAtˆ(€€€€€€€€¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„ (€€€€€€€€€€€‰…ÑÑ±•}…Ñ¥½¹}‰ÕÑÑ½¹}±…‰•° ™…µ”°	…ÑÑ±•!Õ‘Ñ¥½¸èé!½±¤°(€€€€€€€€€€€€‰9QUI8m10Atˆ(€€€€€€€€¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸±¥­}Ñ…É•Ñ¥¹}…¹‘}¥µµ•‘¥…Ñ•}…Ñ¥½¹Í}‘É¥Ù•}Ñ¡•}É•…±}­•É¹•° ¤ì(€€€€€€€±•ÐµÕÐÍÑ…Ñ”€ô…ÕÑ¡½É•‘}‰…ÑÑ±” ¤ì(€€€€€€€±•Ð…Ñ½É}¥€ô…Ñ¥Ù•}…±±ä ™ÍÑ…Ñ”¤ì(€€€€€€€ÍÑ…Ñ”¹Á¡…Í”€ô%¹Ñ•É…Ñ¥½¹A¡…Í”èéM•±•Ñ•‘Ñ½È¡…Ñ½É}¥¤ì(€€€€€€€•á•ÕÑ•}¥µµ•‘¥…Ñ•}…Ñ¥½¸ ™µÕÐÍÑ…Ñ”°A±…å•ÉÑ¥½¸èéÉ½Õ ¤¹•áÁ•Ð ‰É½Õ ˆ¤ì(€€€€€€€ÍÑ…Ñ”¹Á¡…Í”€ô%¹Ñ•É…Ñ¥½¹A¡…Í”èéM•±•Ñ•‘Ñ½È¡…Ñ½É}¥¤ì(€€€€€€€•á•ÕÑ•}¥µµ•‘¥…Ñ•}…Ñ¥½¸ ™µÕÐÍÑ…Ñ”°A±…å•ÉÑ¥½¸èéAÉ½¹”¤¹•áÁ•Ð ‰ÁÉ½¹”ˆ¤ì((€€€€€€€±•ÐµÕÐÍÑ…Ñ”€ô…ÕÑ¡½É•‘}‰…ÑÑ±” ¤ì(€€€€€€€±•Ð…Ñ½É}¥€ô…Ñ¥Ù•}…±±ä ™ÍÑ…Ñ”¤ì(€€€€€€€ÍÑ…Ñ”(€€€€€€€€€€€€¹Í¥´(€€€€€€€€€€€€¹…Í}µÕÐ ¤(€€€€€€€€€€€€¹…¹‘}Ñ¡•¸¡ñÍ¥µðÍ¥´¹…Ñ½ÉÌ¹•Ñ}µÕÐ ™…Ñ½É}¥¤¤(€€€€€€€€€€€€¹•áÁ•Ð ‰…Ñ¥Ù”…±±äˆ¤(€€€€€€€€€€€€¹ÍÑ…¹”€ôMÑ…¹”èéAÉ½¹”ì(€€€€€€€ÍÑ…Ñ”¹Á¡…Í”€ô%¹Ñ•É…Ñ¥½¹A¡…Í”èéM•±•Ñ•‘Ñ½È¡…Ñ½É}¥¤ì(€€€€€€€•á•ÕÑ•}¥µµ•‘¥…Ñ•}…Ñ¥½¸ ™µÕÐÍÑ…Ñ”°A±…å•ÉÑ¥½¸èéAÉ½¹”¤¹•áÁ•Ð ‰É¥Í”ˆ¤ì((€€€€€€€±•ÐµÕÐÍÑ…Ñ”€ô…ÕÑ¡½É•‘}‰…ÑÑ±” ¤ì(€€€€€€€±•Ð…Ñ½É}¥€ô…Ñ¥Ù•}…±±ä ™ÍÑ…Ñ”¤ì(€€€€€€€±•Ð€¡•¹•µå}¥°Ñ…É•Ñ}Á½Í¥Ñ¥½¸°…Ñ½É}Á½Í¥Ñ¥½¸¤€ôì(€€€€€€€€€€€±•ÐÍ¥´€ôÍÑ…Ñ”¹Í¥´¹…Í}É•˜ ¤¹•áÁ•Ð ‰Í¥µÕ±…Ñ¥½¸ˆ¤ì(€€€€€€€€€€€±•Ð•¹•µå}¥€ôÍ¥´(€€€€€€€€€€€€€€€€¹…Ñ½ÉÌ(€€€€€€€€€€€€€€€€¹¥Ñ•È ¤(€€€€€€€€€€€€€€€€¹™¥¹‘}µ…À¡ð¡¥°…Ñ½È¥ð¥Í}•¹•µä¡…Ñ½È¤¹Ñ¡•¹}Í½µ” ©¥¤¤(€€€€€€€€€€€€€€€€¹•áÁ•Ð ‰•¹•µäˆ¤ì(€€€€€€€€€€€€ (€€€€€€€€€€€€€€€•¹•µå}¥°(€€€€€€€€€€€€€€€Í¥´¹…Ñ½ÉÌ¹•Ð ™•¹•µå}¥¤¹•áÁ•Ð ‰•¹•µä…Ñ½Èˆ¤¹Á½Í¥Ñ¥½¸°(€€€€€€€€€€€€€€€Í¥´¹…Ñ½ÉÌ¹•Ð ™…Ñ½É}¥¤¹•áÁ•Ð ‰…Ñ¥Ù”…Ñ½Èˆ¤¹Á½Í¥Ñ¥½¸°(€€€€€€€€€€€€¤(€€€€€€€ôì(€€€€€€€ì(€€€€€€€€€€€±•ÐÍ¥´€ôÍÑ…Ñ”¹Í¥´¹…Í}µÕÐ ¤¹•áÁ•Ð ‰Í¥µÕ±…Ñ¥½¸ˆ¤ì(€€€€€€€€€€€Í¥´¹…Ñ½ÉÌ(€€€€€€€€€€€€€€€€¹É•Ñ…¥¸¡ñ¥°}ð€©¥€ôô…Ñ½É}¥ñð€©¥€ôô•¹•µå}¥¤ì(€€€€€€€€€€€Í¥´¹Í•ÅÕ•¹•}±½¬(€€€€€€€€€€€€€€€€¹É•Ñ…¥¸¡ñ¥°}ð€©¥€ôô…Ñ½É}¥ñð€©¥€ôô•¹•µå}¥¤ì(€€€€€€€€€€€Í¥´¹…Ñ¥Ù•}…Ñ½È€ôM½µ”¡…Ñ½É}¥¤ì(€€€€€€€€€€€Í¥´¹…Ñ½ÉÌ¹•Ñ}µÕÐ ™…Ñ½É}¥¤¹•áÁ•Ð ‰…Ñ¥Ù”…Ñ½Èˆ¤¹…À¸À€ô€ÈÀì(€€€€€€€€€€€±•Ð•¹•µä€ôÍ¥´¹…Ñ½ÉÌ¹•Ñ}µÕÐ ™•¹•µå}¥¤¹•áÁ•Ð ‰•¹•µä…Ñ½Èˆ¤ì(€€€€€€€€€€€•¹•µä¹Á½Í¥Ñ¥½¸€ô…Ñ½É}Á½Í¥Ñ¥½¸¹¹•¥¡‰½ÕÈ¡Á‰}½É”èé•½´èé…¥¹œèé…ÍÐ¤ì(€€€€€€€€€€€•¹•µä¹…±¥Ù”€ôÑÉÕ”ì(€€€€€€€€€€€•¹•µä¹¡¥Ñ}Á½¥¹ÑÌ€ô•¹•µä¹µ…á}¡Àì(€€€€€€€ô(€€€€€€€ÍÑ…Ñ”¹Á¡…Í”€ô%¹Ñ•É…Ñ¥½¹A¡…Í”èéQ…É•Ñ¥¹œì(€€€€€€€€€€€…Ñ½Èè…Ñ½É}¥°(€€€€€€€€€€€…Ñ¥½¸èA±…å•ÉÑ¥½¸èéM¹…ÁM¡½Ð°(€€€€€€€ôì(€€€€€€€±•Ð•¹•µå}Á½Í¥Ñ¥½¸€ôÍÑ…Ñ”(€€€€€€€€€€€€¹Í¥´(€€€€€€€€€€€€¹…Í}É•˜ ¤(€€€€€€€€€€€€¹…¹‘}Ñ¡•¸¡ñÍ¥µðÍ¥´¹…Ñ½ÉÌ¹•Ð ™•¹•µå}¥¤¤(€€€€€€€€€€€€¹•áÁ•Ð ‰•¹•µäˆ¤(€€€€€€€€€€€€¹Á½Í¥Ñ¥½¸ì(€€€€€€€ÍÑ…Ñ”¹¡½Ù•É•‘}Ñ¥±•}à€ô•¹•µå}Á½Í¥Ñ¥½¸¹àì(€€€€€€€ÍÑ…Ñ”¹¡½Ù•É•‘}Ñ¥±•}ä€ô•¹•µå}Á½Í¥Ñ¥½¸¹äì(€€€€€€€•á•ÕÑ•}Á±…å•É}…Ñ¥½¸ ™µÕÐÍÑ…Ñ”¤¹•áÁ•Ð ‰Í¹…ÀÍ¡½Ðˆ¤ì(€€€€€€€…ÍÍ•ÉÐ„ …ÍÑ…Ñ”¹‰…ÑÑ±•}•Ù•¹ÑÌ¹¥Í}•µÁÑä ¤¤ì((€€€€€€€ÍÑ…Ñ”¹Á¡…Í”€ô%¹Ñ•É…Ñ¥½¹A¡…Í”èé%‘±”ì(€€€€€€€ÍÑ…Ñ”¹¡½Ù•É•‘}Ñ¥±•}à€ôÑ…É•Ñ}Á½Í¥Ñ¥½¸¹àì(€€€€€€€ÍÑ…Ñ”¹¡½Ù•É•‘}Ñ¥±•}ä€ôÑ…É•Ñ}Á½Í¥Ñ¥½¸¹äì(€€€€€€€¡…¹‘±•}½µ‰…Ñ}±¥¬ ™µÕÐÍÑ…Ñ”°½µ‰…ÑA½¥¹Ñ•É	ÕÑÑ½¸èé1•™Ð°€ÄÈàÀ°€ÜÈÀ¤(€€€€€€€€€€€€¹•áÁ•Ð ‰•¹•µä±¥¬¡…¹‘±•ˆ¤ì(€€€€€€€…ÍÍ•ÉÐ„ …ÍÑ…Ñ”¹µ•ÍÍ…”¹¥Í}•µÁÑä ¤¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸¡½±‘}ÉÕ¹Í}•¹•µå}…¥}…¹‘}Ù¥Ñ½Éå}µ½Ù•Í}Ñ½}…™Ñ•É}…Ñ¥½¸ ¤ì(€€€€€€€±•ÐµÕÐÍÑ…Ñ”€ô…ÕÑ¡½É•‘}‰…ÑÑ±” ¤ì(€€€€€€€±•Ð…Ñ½É}¥€ô…Ñ¥Ù•}…±±ä ™ÍÑ…Ñ”¤ì(€€€€€€€ÍÑ…Ñ”¹Á¡…Í”€ô%¹Ñ•É…Ñ¥½¹A¡…Í”èéM•±•Ñ•‘Ñ½È¡…Ñ½É}¥¤ì(€€€€€€€•á•ÕÑ•}¥µµ•‘¥…Ñ•}…Ñ¥½¸ ™µÕÐÍÑ…Ñ”°A±…å•ÉÑ¥½¸èé!½±¤¹•áÁ•Ð ‰¡½±…¹•¹•µä$ˆ¤ì(€€€€€€€…ÍÍ•ÉÐ„ …ÍÑ…Ñ”¹µ•ÍÍ…”¹¥Í}•µÁÑä ¤¤ì((€€€€€€€ì(€€€€€€€€€€€±•ÐÍ¥´€ôÍÑ…Ñ”¹Í¥´¹…Í}µÕÐ ¤¹•áÁ•Ð ‰Í¥µÕ±…Ñ¥½¸ˆ¤ì(€€€€€€€€€€€™½È…Ñ½È¥¸Í¥´¹…Ñ½ÉÌ¹Ù…±Õ•Í}µÕÐ ¤¹™¥±Ñ•È¡ñ…Ñ½Éð¥Í}•¹•µä¡…Ñ½È¤¤ì(€€€€€€€€€€€€€€€…Ñ½È¹…±¥Ù”€ô™…±Í”ì(€€€€€€€€€€€€€€€…Ñ½È¹¡¥Ñ}Á½¥¹ÑÌ€ô€Àì(€€€€€€€€€€€ô(€€€€€€€ô(€€€€€€€ÍÑ…Ñ”¹ÍÉ••¸€ô…µ•MÉ••¸èé	…ÑÑ±”ì(€€€€€€€¡•­}Ù¥Ñ½Éå}½¹‘¥Ñ¥½¹Ì ™µÕÐÍÑ…Ñ”¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡ÍÑ…Ñ”¹ÍÉ••¸°…µ•MÉ••¸èé™Ñ•ÉÑ¥½¸¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡ÍÑ…Ñ”¹±…ÍÑ}Ù¥Ñ½Éä°M½µ”¡ÑÉÕ”¤¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸•¹‘}ÑÕÉ¹}¹•Ù•É}ÍÑ¥­Í}Ý¡•¹}…Ñ¥Ù•}…±±å}¡…Í}…}µ…¹‘…Ñ½Éå}É•ÑÉ•…Ð ¤ì(€€€€€€€±•ÐµÕÐÍÑ…Ñ”€ô…ÕÑ¡½É•‘}‰…ÑÑ±” ¤ì(€€€€€€€±•Ð…Ñ½É}¥€ô…Ñ¥Ù•}…±±ä ™ÍÑ…Ñ”¤ì(€€€€€€€ì(€€€€€€€€€€€±•ÐÍ¥´€ôÍÑ…Ñ”¹Í¥´¹…Í}µÕÐ ¤¹•áÁ•Ð ‰Í¥µÕ±…Ñ¥½¸ˆ¤ì(€€€€€€€€€€€Í¥´¹‰É½­•¹}É•ÑÉ•…Ñ}É•µ…¥¹¥¹œ¹¥¹Í•ÉÐ¡…Ñ½É}¥°€È¤ì(€€€€€€€€€€€±•Ð…Ñ½È€ôÍ¥´¹…Ñ½ÉÌ¹•Ñ}µÕÐ ™…Ñ½É}¥¤¹•áÁ•Ð ‰…Ñ¥Ù”…±±äˆ¤ì(€€€€€€€€€€€…Ñ½È¹Í…¹€ô€Äì(€€€€€€€€€€€…Ñ½È¹…À€ôÁ‰}½É”èé¥‘ÌèéÀ Ô¤ì(€€€€€€€ô(€€€€€€€ÍÑ…Ñ”¹Á¡…Í”€ô%¹Ñ•É…Ñ¥½¹A¡…Í”èéM•±•Ñ•‘Ñ½È¡…Ñ½É}¥¤ì((€€€€€€€•á•ÕÑ•}¥µµ•‘¥…Ñ•}…Ñ¥½¸ ™µÕÐÍÑ…Ñ”°A±…å•ÉÑ¥½¸èé!½±¤(€€€€€€€€€€€€¹•áÁ•Ð ‰¹QÕÉ¸µÕÍÐÉ•Í½±Ù”„µ…¹‘…Ñ½ÉäÉ•ÑÉ•…Ðˆ¤ì((€€€€€€€±•Ð…Ñ½È€ô€™ÍÑ…Ñ”¹Í¥´¹…Í}É•˜ ¤¹•áÁ•Ð ‰Í¥µÕ±…Ñ¥½¸ˆ¤¹…Ñ½ÉÍl™…Ñ½É}¥‘tì(€€€€€€€…ÍÍ•ÉÐ„¡…Ñ½È¹É½ÕÑ•¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡…Ñ½È¹…À°Á‰}½É”èé¥‘ÌèéÀ À¤¤ì(€€€€€€€…ÍÍ•ÉÑ}¹”„ (€€€€€€€€€€€ÍÑ…Ñ”¹Í¥´¹…Í}É•˜ ¤¹•áÁ•Ð ‰Í¥µÕ±…Ñ¥½¸ˆ¤¹…Ñ¥Ù•}…Ñ½È°(€€€€€€€€€€€M½µ”¡…Ñ½É}¥¤(€€€€€€€€¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡ÍÑ…Ñ”¹ÍÉ••¸°…µ•MÉ••¸èé™Ñ•ÉÑ¥½¸¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡ÍÑ…Ñ”¹±…ÍÑ}Ù¥Ñ½Éä°M½µ”¡™…±Í”¤¤ì(€€€€€€€…ÍÍ•ÉÐ„¡ÍÑ…Ñ”¹µ•ÍÍ…”¹½¹Ñ…¥¹Ì ‰•™•…Ðˆ¤¤ì(€€€ô)ô((m™œ¡Ñ•ÍÐ¥t)µ½ÑÕÉ¹}Ñ•ÍÑÌì(€€€ÕÍ”ÍÕÁ•Èèè¨ì(€€€ÕÍ”Á‰}Í¥´èé±½¬èéí‰Õ¥±‘}…Ñ½È°É•¥ÍÑ•É}…Ñ½Éôì((€€€€mÑ•ÍÑt(€€€™¸ÍÉ••¹}Ñ½}Ñ¥±•}…½Õ¹ÑÍ}™½É}Á…¹}…¹‘}é½½´ ¤ì(€€€€€€€±•ÐÑ¥±”€ôÍÉ••¹}Ñ½}Ñ¥±” ÜàÜ¸È°€ØÐà¸À°€ÄÈà¸À°€ÈÐÀ¸À°€Ä¸ÌÔ°€ÄäÈÀ¸À°€ÄÀàÀ¸À¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡Ñ¥±”°Q¥±•adèé¹•Ü Ô°€Ô¤¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸™¥ÑÑ•‘}…µ•É…}­••ÁÍ}•Ù•Éå}Á•É¥µ•Ñ•É}‘¥…µ½¹‘}™Õ±±å}¥¹Í¥‘•}Ñ¡•}Ù¥•ÝÁ½ÉÐ ¤ì(€€€€€€€±•Ð•¹Ñ•É}à€ô€¡I%}=1L…Ì˜ÌÈ€´I%}I=]L…Ì˜ÌÈ¤€¨Q%1}\€¨€À¸ÈÔì(€€€€€€€±•Ð•¹Ñ•É}ä€ô€¡I%}=1L€¬I%}I=]L€´€È¤…Ì˜ÌÈ€¨Q%1} €¨€À¸ÈÔì(€€€€€€€±•Ðµ¥¹}à€ô€´¡I%}I=]L…Ì˜ÌÈ¤€¨Q%1}\€¨€À¸Ôì(€€€€€€€±•Ðµ…á}à€ôI%}=1L…Ì˜ÌÈ€¨Q%1}\€¨€À¸Ôì(€€€€€€€±•Ðµ¥¹}ä€ô€µQ%1} €¨€À¸Ôì(€€€€€€€±•Ðµ…á}ä€ô€¡I%}=1L€¬I%}I=]L€´€Ä¤…Ì˜ÌÈ€¨Q%1} €¨€À¸Ôì((€€€€€€€™½È€¡Ý¥‘Ñ °¡•¥¡Ð¤¥¸l àÀÀ¸À°€ØÀÀ¸À¤°€ ÄÈàÀ¸À°€ÜÈÀ¸À¤°€ ÄäÈÀ¸À°€ÄÀàÀ¸À¥tì(€€€€€€€€€€€±•Ðé½½´€ô™¥ÑÑ•‘}‰…ÑÑ±•}é½½´ Ä¸Ü°Ý¥‘Ñ °¡•¥¡Ð¤ì(€€€€€€€€€€€±•Ð±•™Ð€ôÝ¥‘Ñ €¨€À¸Ô€¬€¡µ¥¹}à€´•¹Ñ•É}à¤€¨é½½´ì(€€€€€€€€€€€±•ÐÉ¥¡Ð€ôÝ¥‘Ñ €¨€À¸Ô€¬€¡µ…á}à€´•¹Ñ•É}à¤€¨é½½´ì(€€€€€€€€€€€±•ÐÑ½À€ô¡•¥¡Ð€¨€À¸Ô€´€¡µ…á}ä€´•¹Ñ•É}ä¤€¨é½½´ì(€€€€€€€€€€€±•Ð‰½ÑÑ½´€ô¡•¥¡Ð€¨€À¸Ô€´€¡µ¥¹}ä€´•¹Ñ•É}ä¤€¨é½½´ì((€€€€€€€€€€€…ÍÍ•ÉÐ„¡±•™Ð€øôI%}}UQQH€´€À¸ÀÄ°€‰íÝ¥‘Ñ¡ô±•™Ðõí±•™Ñôˆ¤ì(€€€€€€€€€€€…ÍÍ•ÉÐ„ (€€€€€€€€€€€€€€€É¥¡Ð€ðôÝ¥‘Ñ €´I%}}UQQH€¬€À¸ÀÄ°(€€€€€€€€€€€€€€€€‰íÝ¥‘Ñ¡ôÉ¥¡ÐõíÉ¥¡Ñôˆ(€€€€€€€€€€€€¤ì(€€€€€€€€€€€…ÍÍ•ÉÐ„¡Ñ½À€øôI%}}UQQH€´€À¸ÀÄ°€‰í¡•¥¡ÑôÑ½ÀõíÑ½Áôˆ¤ì(€€€€€€€€€€€…ÍÍ•ÉÐ„ (€€€€€€€€€€€€€€€‰½ÑÑ½´€ðô¡•¥¡Ð€´I%}}UQQH€¬€À¸ÀÄ°(€€€€€€€€€€€€€€€€‰í¡•¥¡Ñô‰½ÑÑ½´õí‰½ÑÑ½µôˆ(€€€€€€€€€€€€¤ì(€€€€€€€ô(€€€ô((€€€€mÑ•ÍÑt(€€€™¸É•¹‘•É•‘}‘¥…µ½¹‘}•¹Ñ•É}…¹‘}¥¹Ñ•É¥½É}Í•±•Ñ}Ñ¡•}Í…µ•}Ñ¥±” ¤ì(€€€€€€€±•Ð•áÁ•Ñ•€ôQ¥±•adèé¹•Ü Ô°€Ô¤ì(€€€€€€€€¼¼Q¥±”€ Ô°Ô¤É•¹‘•ÉÌ…ÐÝ½É±€ À°ÄØÀ¤°½ÈÍÉ••¸€ ÔÀÀ°ÈÐÀ¤¸(€€€€€€€™½È€¡à°ä¤¥¸l(€€€€€€€€€€€€ ÔÀÀ¸À°€ÈÐÀ¸À¤°(€€€€€€€€€€€€ ÔÀÀ¸À°€ÈÈÔ¸À¤°(€€€€€€€€€€€€ ÔÀÀ¸À°€ÈÔÔ¸À¤°(€€€€€€€€€€€€ ÐØä¸À°€ÈÐÀ¸À¤°(€€€€€€€€€€€€ ÔÌÄ¸À°€ÈÐÀ¸À¤°(€€€€€€€tì(€€€€€€€€€€€…ÍÍ•ÉÑ}•Ä„ (€€€€€€€€€€€€€€€ÍÉ••¹}Ñ½}Ñ¥±”¡à°ä°€À¸À°€À¸À°€Ä¸À°€Å|ÀÀÀ¸À°€àÀÀ¸À¤°(€€€€€€€€€€€€€€€•áÁ•Ñ•°(€€€€€€€€€€€€€€€€‰Á½¥¹Ñ•È€¡íáô±íåô¤µ¥ÍÍ•Ñ¡”É•¹‘•É•‘¥…µ½¹ˆ(€€€€€€€€€€€€¤ì(€€€€€€€ô(€€€ô((€€€€mÑ•ÍÑt(€€€™¸É…¥Í•‘}Ñ¥±•}ÍÕÉ™…•}É•µ…¥¹Í}Á½¥¹Ñ•É}Í•±•Ñ…‰±” ¤ì(€€€€€€€±•ÐµÕÐ…µ”€ô…µ•MÑ…Ñ”èé¹•Ü ¤ì(€€€€€€€±•ÐµÕÐÍ¥´€ôM¥µMÑ…Ñ”èé¹•Ü ÐÈ°€Ä¤ì(€€€€€€€Í¥´¹Ñ¥±•}•±•Ù…Ñ¥½¹Ì¹¥¹Í•ÉÐ¡Q¥±•adèé¹•Ü Ô°€Ô¤°€È¤ì(€€€€€€€…µ”¹Í¥´€ôM½µ”¡Í¥´¤ì(€€€€€€€…µ”¹…µ•É…}é½½´€ô€Ä¸Àì(€€€€€€€±•ÐÙ¥•ÝÁ½ÉÑ}Ý¥‘Ñ €ô€Å|ÀÀÀ¸Àì(€€€€€€€±•ÐÙ¥•ÝÁ½ÉÑ}¡•¥¡Ð€ô€àÀÀ¸Àì(€€€€€€€±•Ðé½½´€ô™¥ÑÑ•‘}‰…ÑÑ±•}é½½´¡…µ”¹…µ•É…}é½½´°Ù¥•ÝÁ½ÉÑ}Ý¥‘Ñ °Ù¥•ÝÁ½ÉÑ}¡•¥¡Ð¤ì(€€€€€€€±•ÐÑ¥±•}Ý½É±‘}à€ô€À¸Àì(€€€€€€€±•ÐÑ¥±•}Ý½É±‘}ä€ô€ÄØÀ¸À€¬€È¸À€¨Á‰}É•¹‘•ÈèéÑ¥±•Ìèé1YQ%=9}MI9}MQ@ì(€€€€€€€±•ÐÁ½¥¹Ñ•É}à€ôÙ¥•ÝÁ½ÉÑ}Ý¥‘Ñ €¨€À¸Ô€¬€¡Ñ¥±•}Ý½É±‘}à€´…µ”¹…µ•É…}à¤€¨é½½´ì(€€€€€€€±•ÐÁ½¥¹Ñ•É}ä€ôÙ¥•ÝÁ½ÉÑ}¡•¥¡Ð€¨€À¸Ô€´€¡Ñ¥±•}Ý½É±‘}ä€´…µ”¹…µ•É…}ä¤€¨é½½´ì((€€€€€€€…ÍÍ•ÉÑ}•Ä„ (€€€€€€€€€€€ÍÉ••¹}Ñ½}Ñ¥±•}¥¹}ÍÑ…Ñ” (€€€€€€€€€€€€€€€€™…µ”°(€€€€€€€€€€€€€€€˜ØÐèé™É½´¡Á½¥¹Ñ•É}à¤°(€€€€€€€€€€€€€€€˜ØÐèé™É½´¡Á½¥¹Ñ•É}ä¤°(€€€€€€€€€€€€€€€Ù¥•ÝÁ½ÉÑ}Ý¥‘Ñ °(€€€€€€€€€€€€€€€Ù¥•ÝÁ½ÉÑ}¡•¥¡Ð°(€€€€€€€€€€€€¤°(€€€€€€€€€€€Q¥±•adèé¹•Ü Ô°€Ô¤(€€€€€€€€¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸É½ÍÍ¥¹}…}‘¥…µ½¹‘}•‘•}Í•±•ÑÍ}Ñ¡•}Ù¥Í¥‰±•}¹•¥¡‰½È ¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„ (€€€€€€€€€€€ÍÉ••¹}Ñ½}Ñ¥±” ÔÌÌ¸À°€ÈÐÀ¸À°€À¸À°€À¸À°€Ä¸À°€Å|ÀÀÀ¸À°€àÀÀ¸À¤°(€€€€€€€€€€€Q¥±•adèé¹•Ü Ø°€Ð¤(€€€€€€€€¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„ (€€€€€€€€€€€ÍÉ••¹}Ñ½}Ñ¥±” ÔÀÀ¸À°€ÈÈÌ¸À°€À¸À°€À¸À°€Ä¸À°€Å|ÀÀÀ¸À°€àÀÀ¸À¤°(€€€€€€€€€€€Q¥±•adèé¹•Ü Ø°€Ø¤(€€€€€€€€¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸Ù¥Í¥‰±•}ÍÁÉ¥Ñ•}¡¥Ñ‰½á}Í•±•ÑÍ}Ñ¡•}…Ñ¥Ù•}…±±ä ¤ì(€€€€€€€±•ÐµÕÐ…µ”€ô…µ•MÑ…Ñ”èé¹•Ü ¤ì(€€€€€€€±•ÐµÕÐÍ¥´€ôM¥µMÑ…Ñ”èé¹•Ü ÐÈ°€Ä¤ì(€€€€€€€±•Ð¥€ôÑ½É% Ä¤ì(€€€€€€€±•ÐµÕÐ…Ñ½È€ô‰Õ¥±‘}…Ñ½È¡¥°€‰A±…å•Èˆ°€Ô°€ÄÀÀ°€ÈÀ°Q¥±•adèé¹•Ü Ô°€Ô¤¤ì(€€€€€€€…Ñ½È¹™…Ñ¥½¹}¥€ô€‰Á±…å•Èˆ¹Ñ½}ÍÑÉ¥¹œ ¤ì(€€€€€€€Í¥´¹…Ñ½ÉÌ¹¥¹Í•ÉÐ¡¥°…Ñ½È¤ì(€€€€€€€Í¥´¹…Ñ¥Ù•}…Ñ½È€ôM½µ”¡¥¤ì(€€€€€€€…µ”¹Í¥´€ôM½µ”¡Í¥´¤ì(€€€€€€€…µ”¹…µ•É…}é½½´€ô€Ä¸Àì(€€€€€€€…µ”¹µ½ÕÍ•}à€ô€ÔÀÀ¸Àì(€€€€€€€€¼¼Q¥±”€ Ô°Ô¤ÁÉ½©•ÑÌÑ¼Ý½É±€ À°ÄØÀ¤ìÑ¡”€àÉÁàÍÁÉ¥Ñ”¥Ì¹½Ü(€€€€€€€€¼¼‰½ÑÑ½´µ…¹¡½É•Ñ¼Ñ¡…ÐÑ¥±”°ÁÕÑÑ¥¹œ¥ÑÌÙ¥ÍÕ…°•¹Ñ•È¹•…ÈäôÈÄÐ(€€€€€€€€¼¼…ÐÑ¡”™¥ÑÑ•€Ä°ÀÀÁààÀÀ‰…ÑÑ±”Ù¥•ÝÁ½ÉÐ¸(€€€€€€€…µ”¹µ½ÕÍ•}ä€ô€ÈÄÐ¸Àì((€€€€€€€…ÍÍ•ÉÐ„¡¡…¹‘±•}½µ‰…Ñ}±¥¬ ™µÕÐ…µ”°½µ‰…ÑA½¥¹Ñ•É	ÕÑÑ½¸èé1•™Ð°€Å|ÀÀÀ°€àÀÀ¤¹¥Í}½¬ ¤¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡…µ”¹Á¡…Í”°%¹Ñ•É…Ñ¥½¹A¡…Í”èéM•±•Ñ•‘Ñ½È¡¥¤¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸•Ù•Éå}…Ñ½É}É•¹‘•É}…¹¡½É}¥Í}Á±…¹Ñ•‘}½¹}½¹•}É¥‘}Ñ¥±” ¤ì(€€€€€€€±•ÐµÕÐ…µ”€ô…µ•MÑ…Ñ”èé¹•Ü ¤ì(€€€€€€€±•ÐµÕÐÍ¥´€ôM¥µMÑ…Ñ”èé¹•Ü ÐÈ°€Ä¤ì(€€€€€€€™½È€¡¥°™…Ñ¥½¸°Á½Í¥Ñ¥½¸¤¥¸l(€€€€€€€€€€€€¡Ñ½É% Ä¤°€‰Á±…å•Èˆ°Q¥±•adèé¹•Ü Ô°€Ô¤¤°(€€€€€€€€€€€€¡Ñ½É% È¤°€‰•¹•µäˆ°Q¥±•adèé¹•Ü à°€Ô¤¤°(€€€€€€€tì(€€€€€€€€€€€±•ÐµÕÐ…Ñ½È€ô‰Õ¥±‘}…Ñ½È¡¥°€‰Ñ½Èˆ°€Ô°€ÄÀÀ°€ÈÀ°Á½Í¥Ñ¥½¸¤ì(€€€€€€€€€€€…Ñ½È¹™…Ñ¥½¹}¥€ô™…Ñ¥½¸¹Ñ½}ÍÑÉ¥¹œ ¤ì(€€€€€€€€€€€Í¥´¹…Ñ½ÉÌ¹¥¹Í•ÉÐ¡¥°…Ñ½È¤ì(€€€€€€€ô(€€€€€€€…µ”¹Í¥´€ôM½µ”¡Í¥´¤ì(€€€€€€€±•ÐÍÁÉ¥Ñ•Ì€ô‰Õ¥±‘}ÍÁÉ¥Ñ•}¥¹ÍÑ…¹•Ì ™…µ”¤ì(€€€€€€€±•ÐµÕÐ¡…É…Ñ•É}ÍÁÉ¥Ñ•Ì€ôÍÁÉ¥Ñ•Ì¹¥Ñ•È ¤¹™¥±Ñ•È¡ñÍÁÉ¥Ñ•ðÍÁÉ¥Ñ”¹ÔÀ€øô€À¸À¤ì((€€€€€€€…ÍÍ•ÉÑ}•Ä„¡¡…É…Ñ•É}ÍÁÉ¥Ñ•Ì¹±½¹” ¤¹½Õ¹Ð ¤°€È¤ì(€€€€€€€…ÍÍ•ÉÐ„¡¡…É…Ñ•É}ÍÁÉ¥Ñ•Ì¹…±°¡ñÍÁÉ¥Ñ•ðÍÁÉ¥Ñ”¹…¹¡½É}‰½ÑÑ½´¤¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸‰…ÑÑ±•}…Ñ¥½¹}¡¥Ñ‰½á•Í}…¹‘}É¥¡Ñ}±¥­}‘¥ÍÁ…Ñ¡}µ½ÕÍ•}½µµ…¹‘Ì ¤ì(€€€€€€€±•ÐµÕÐ…µ”€ô…µ•MÑ…Ñ”èé¹•Ü ¤ì(€€€€€€€±•ÐµÕÐÍ¥´€ôM¥µMÑ…Ñ”èé¹•Ü ÐÈ°€Ä¤ì(€€€€€€€±•ÐÁ±…å•É}¥€ôÑ½É% Ä¤ì(€€€€€€€±•Ð•¹•µå}¥€ôÑ½É% È¤ì(€€€€€€€±•ÐµÕÐÁ±…å•È€ô‰Õ¥±‘}…Ñ½È¡Á±…å•É}¥°€‰A±…å•Èˆ°€Ô°€ÄÀÀ°€ÈÀ°Q¥±•adèé¹•Ü È°€È¤¤ì(€€€€€€€Á±…å•È¹™…Ñ¥½¹}¥€ô€‰Á±…å•Èˆ¹Ñ½}ÍÑÉ¥¹œ ¤ì(€€€€€€€Á±…å•È¹…À€ôÁ‰}½É”èé¥‘ÌèéÀ Ô¤ì(€€€€€€€Á±…å•È¹…À€ôÁ‰}½É”èé¥‘ÌèéÀ ÄÀ¤ì(€€€€€€€±•ÐµÕÐ•¹•µä€ô‰Õ¥±‘}…Ñ½È¡•¹•µå}¥°€‰¹•µäˆ°€Ô°€ÄÀÀ°€ÈÀ°Q¥±•adèé¹•Ü Ì°€È¤¤ì(€€€€€€€•¹•µä¹™…Ñ¥½¹}¥€ô€‰•¹•µäˆ¹Ñ½}ÍÑÉ¥¹œ ¤ì(€€€€€€€Í¥´¹…Ñ½ÉÌ¹¥¹Í•ÉÐ¡Á±…å•É}¥°Á±…å•È¤ì(€€€€€€€Í¥´¹…Ñ½ÉÌ¹¥¹Í•ÉÐ¡•¹•µå}¥°•¹•µä¤ì(€€€€€€€Í¥´¹…Ñ¥Ù•}…Ñ½È€ôM½µ”¡Á±…å•É}¥¤ì(€€€€€€€…µ”¹Í¥´€ôM½µ”¡Í¥´¤ì(€€€€€€€…µ”¹Á¡…Í”€ô%¹Ñ•É…Ñ¥½¹A¡…Í”èéM•±•Ñ•‘Ñ½È¡Á±…å•É}¥¤ì(€€€€€€€…µ”¹…µ•É…}é½½´€ô€Ä¸Àì((€€€€€€€±•Ð€¡…Ñ¥½¸°|°mà°ä°Ý¥‘Ñ °¡•¥¡Ñt¤€ô‰…ÑÑ±•}…Ñ¥½¹}±…å½ÕÐ Å|ÀÀÀ°€àÀÀ¥lÅtì(€€€€€€€…µ”¹µ½ÕÍ•}à€ô˜ØÐèé™É½´¡à€¬Ý¥‘Ñ €¨€À¸Ô¤ì(€€€€€€€…µ”¹µ½ÕÍ•}ä€ô˜ØÐèé™É½´¡ä€¬¡•¥¡Ð€¨€À¸Ô¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„ (€€€€€€€€€€€‰…ÑÑ±•}…Ñ¥½¹}…Ð ™…µ”°€Å|ÀÀÀ°€àÀÀ¤°(€€€€€€€€€€€M½µ”¡	…ÑÑ±•!Õ‘Ñ¥½¸èé¥´¤(€€€€€€€€¤ì(€€€€€€€…ÍÍ•ÉÐ„¡…Ñ¥Ù…Ñ•}‰…ÑÑ±•}…Ñ¥½¸ ™µÕÐ…µ”°…Ñ¥½¸¤¹¥Í}½¬ ¤¤ì(€€€€€€€…ÍÍ•ÉÐ„¡µ…Ñ¡•Ì„ (€€€€€€€€€€€…µ”¹Á¡…Í”°(€€€€€€€€€€€%¹Ñ•É…Ñ¥½¹A¡…Í”èéQ…É•Ñ¥¹œì(€€€€€€€€€€€€€€€…Ñ¥½¸èA±…å•ÉÑ¥½¸èé¥µ•‘M¡½Ð°(€€€€€€€€€€€€€€€€¸¸(€€€€€€€€€€€ô(€€€€€€€€¤¤ì((€€€€€€€€¼¼¹•µä…ÐÑ¥±”€ Ì°È¤è±¥¬Ñ¡”É•¹‘•É•Ñ¥±”•¹Ñ•È…ÐÍÉ••¸(€€€€€€€€¼¼€ ÔÌÈ°ÌÈÀ¤°¹½ÐÑ¡”½±™½ÕÈµ½É¹•È¥¹Ñ•ÉÍ•Ñ¥½¸½ÈÍÁÉ¥Ñ”µ¥‘Á½¥¹Ð¸(€€€€€€€…µ”¹µ½ÕÍ•}à€ô€ÔÌÈ¸Àì(€€€€€€€…µ”¹µ½ÕÍ•}ä€ô€ÌÈÀ¸Àì(€€€€€€€…ÍÍ•ÉÐ„¡¡…¹‘±•}½µ‰…Ñ}±¥¬ ™µÕÐ…µ”°½µ‰…ÑA½¥¹Ñ•É	ÕÑÑ½¸èéI¥¡Ð°€Å|ÀÀÀ°€àÀÀ¤¹¥Í}½¬ ¤¤ì(€€€€€€€…ÍÍ•ÉÐ„¡…µ”(€€€€€€€€€€€€¹‰…ÑÑ±•}•Ù•¹ÑÌ(€€€€€€€€€€€€¹¥Ñ•È ¤(€€€€€€€€€€€€¹…¹ä¡ñ•Ù•¹Ñðµ…Ñ¡•Ì„¡•Ù•¹Ð°Ù•¹Ðèé¥É•ì…Ñ½È°Ñ…É•Ðô¥˜€©…Ñ½È€ôôÁ±…å•É}¥€˜˜€©Ñ…É•Ð€ôô•¹•µå}¥¤¤¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸ÍÅÕ…‘}¹Õµ‰•É}Í•±•Ñ¥½¹}ÕÍ•Í}ÍÑ…‰±•}½µÁ…¹å}½É‘•È ¤ì(€€€€€€€±•ÐµÕÐ…µ”€ô…µ•MÑ…Ñ”èé¹•Ü ¤ì(€€€€€€€±•ÐµÕÐÍ¥´€ôM¥µMÑ…Ñ”èé¹•Ü ÐÈ°€Ä¤ì(€€€€€€€™½È€¡¥°¹…µ”°™…Ñ¥½¸¤¥¸l(€€€€€€€€€€€€¡Ñ½É% à¤°€‰M•½¹ˆ°€‰Á±…å•Èˆ¤°(€€€€€€€€€€€€¡Ñ½É% Ì¤°€‰¹•µäˆ°€‰•¹•µäˆ¤°(€€€€€€€€€€€€¡Ñ½É% È¤°€‰¥ÉÍÐˆ°€‰Á±…å•Èˆ¤°(€€€€€€€tì(€€€€€€€€€€€±•ÐµÕÐ…Ñ½È€ô‰Õ¥±‘}…Ñ½È¡¥°¹…µ”°€Ô°€ÄÀÀ°€ÈÀ°Q¥±•adèé¹•Ü¡¥¸À…Ì¤ÄØ°€È¤¤ì(€€€€€€€€€€€…Ñ½È¹™…Ñ¥½¹}¥€ô™…Ñ¥½¸¹Ñ½}ÍÑÉ¥¹œ ¤ì(€€€€€€€€€€€Í¥´¹…Ñ½ÉÌ¹¥¹Í•ÉÐ¡¥°…Ñ½È¤ì(€€€€€€€ô(€€€€€€€Í¥´¹…Ñ¥Ù•}…Ñ½È€ôM½µ”¡Ñ½É% È¤¤ì(€€€€€€€…µ”¹Í¥´€ôM½µ”¡Í¥´¤ì((€€€€€€€…ÍÍ•ÉÐ„¡Í•±•Ñ}ÍÅÕ…‘}µ•µ‰•È ™µÕÐ…µ”°€Ä¤¹¥Í}½¬ ¤¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡…µ”¹Á¡…Í”°%¹Ñ•É…Ñ¥½¹A¡…Í”èéM•±•Ñ•‘Ñ½È¡Ñ½É% à¤¤¤ì(€€€€€€€…ÍÍ•ÉÐ„¡…µ”¹µ•ÍÍ…”¹½¹Ñ…¥¹Ì ‰%¹ÍÁ•Ñ¥¹œM•½¹ˆ¤¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸Ñ…‰}Ñ…É•Ñ}å±•Í}±¥Ù¥¹}•¹•µ¥•Í}…¹‘}ÝÉ…ÁÌ ¤ì(€€€€€€€±•ÐµÕÐ…µ”€ô…µ•MÑ…Ñ”èé¹•Ü ¤ì(€€€€€€€±•ÐµÕÐÍ¥´€ôM¥µMÑ…Ñ”èé¹•Ü ÐÈ°€Ä¤ì(€€€€€€€™½È€¡¥°¹…µ”°Á½Í¥Ñ¥½¸¤¥¸l(€€€€€€€€€€€€¡Ñ½É% È¤°€‰¥ÉÍÐ¹•µäˆ°Q¥±•adèé¹•Ü Ð°€È¤¤°(€€€€€€€€€€€€¡Ñ½É% Ü¤°€‰M•½¹¹•µäˆ°Q¥±•adèé¹•Ü Ü°€È¤¤°(€€€€€€€tì(€€€€€€€€€€€±•ÐµÕÐ…Ñ½È€ô‰Õ¥±‘}…Ñ½È¡¥°¹…µ”°€Ô°€ÄÀÀ°€ÈÀ°Á½Í¥Ñ¥½¸¤ì(€€€€€€€€€€€…Ñ½È¹™…Ñ¥½¹}¥€ô€‰•¹•µäˆ¹Ñ½}ÍÑÉ¥¹œ ¤ì(€€€€€€€€€€€Í¥´¹…Ñ½ÉÌ¹¥¹Í•ÉÐ¡¥°…Ñ½È¤ì(€€€€€€€ô(€€€€€€€…µ”¹¡½Ù•É•‘}Ñ¥±•}à€ô€Ðì(€€€€€€€…µ”¹¡½Ù•É•‘}Ñ¥±•}ä€ô€Èì(€€€€€€€…µ”¹Í¥´€ôM½µ”¡Í¥´¤ì((€€€€€€€…ÍÍ•ÉÐ„¡å±•}Ñ…É•Ð ™µÕÐ…µ”¤¹¥Í}½¬ ¤¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„ ¡…µ”¹¡½Ù•É•‘}Ñ¥±•}à°…µ”¹¡½Ù•É•‘}Ñ¥±•}ä¤°€ Ý}¤ÄØ°€É}¤ÄØ¤¤ì(€€€€€€€…ÍÍ•ÉÐ„¡å±•}Ñ…É•Ð ™µÕÐ…µ”¤¹¥Í}½¬ ¤¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„ ¡…µ”¹¡½Ù•É•‘}Ñ¥±•}à°…µ”¹¡½Ù•É•‘}Ñ¥±•}ä¤°€ Ñ}¤ÄØ°€É}¤ÄØ¤¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸Å}•}É½Ñ…Ñ¥½¹}ÕÍ•Í}Í¥µ}ÍÑ•Á}…¹‘}ÍÁ•¹‘Í}¹½}…À ¤ì(€€€€€€€±•ÐµÕÐ…µ”€ô…µ•MÑ…Ñ”èé¹•Ü ¤ì(€€€€€€€±•ÐµÕÐÍ¥´€ôM¥µMÑ…Ñ”èé¹•Ü ÐÈ°€Ä¤ì(€€€€€€€±•Ð¥€ôÑ½É% Ä¤ì(€€€€€€€±•ÐµÕÐ…Ñ½È€ô‰Õ¥±‘}…Ñ½È¡¥°€‰A±…å•Èˆ°€Ô°€ÄÀÀ°€ÈÀ°Q¥±•adèé¹•Ü È°€È¤¤ì(€€€€€€€…Ñ½È¹™…Ñ¥½¹}¥€ô€‰Á±…å•Èˆ¹Ñ½}ÍÑÉ¥¹œ ¤ì(€€€€€€€…Ñ½È¹™…¥¹œ€ôÁ‰}½É”èé•½´èé…¥¹œèéM½ÕÑ ì(€€€€€€€…Ñ½È¹…À€ôÁ‰}½É”èé¥‘ÌèéÀ Ø¤ì(€€€€€€€Í¥´¹…Ñ½ÉÌ¹¥¹Í•ÉÐ¡¥°…Ñ½È¤ì(€€€€€€€Í¥´¹…Ñ¥Ù•}…Ñ½È€ôM½µ”¡¥¤ì(€€€€€€€…µ”¹Í¥´€ôM½µ”¡Í¥´¤ì(€€€€€€€…µ”¹Á¡…Í”€ô%¹Ñ•É…Ñ¥½¹A¡…Í”èéM•±•Ñ•‘Ñ½È¡¥¤ì((€€€€€€€…ÍÍ•ÉÐ„¡É½Ñ…Ñ•}Í•±•Ñ•‘}™…¥¹œ ™µÕÐ…µ”°ÑÉÕ”¤¹¥Í}½¬ ¤¤ì(€€€€€€€±•ÐM½µ”¡Í¥´¤€ô…µ”¹Í¥´¹…Í}É•˜ ¤•±Í”ì(€€€€€€€€€€€Á…¹¥Œ„ ‰É½Ñ…Ñ¥½¸µÕÍÐÉ•Ñ…¥¸Í¥µÕ±…Ñ¥½¸ÍÑ…Ñ”ˆ¤ì(€€€€€€€ôì(€€€€€€€±•Ð…Ñ½È€ô€™Í¥´¹…Ñ½ÉÍl™¥‘tì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡…Ñ½È¹™…¥¹œ°Á‰}½É”èé•½´èé…¥¹œèéM½ÕÑ¡]•ÍÐ¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡…Ñ½È¹…À°Á‰}½É”èé¥‘ÌèéÀ Ø¤¤ì(€€€€€€€…ÍÍ•ÉÐ„¡…µ”(€€€€€€€€€€€€¹‰…ÑÑ±•}•Ù•¹ÑÌ(€€€€€€€€€€€€¹¥Ñ•È ¤(€€€€€€€€€€€€¹…¹ä¡ñ•Ù•¹Ñðµ…Ñ¡•Ì„¡•Ù•¹Ð°Ù•¹Ðèé…¥¹¡…¹•ì…Ñ½È°€¸¸ô¥˜€©…Ñ½È€ôô¥¤¤¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸µ½Ù•µ•¹Ñ}ÁÉ•Ù¥•Ý}Í¡…É•Í}½ÍÑ}½Ù•É}…¹‘}½Ù•ÉÝ…Ñ¡}ÉÕ±•Ì ¤ì(€€€€€€€±•ÐµÕÐ…µ”€ô…µ•MÑ…Ñ”èé¹•Ü ¤ì(€€€€€€€±•ÐµÕÐÍ¥´€ôM¥µMÑ…Ñ”èé¹•Ü ÐÈ°€Ä¤ì(€€€€€€€±•ÐÁ±…å•É}¥€ôÑ½É% Ä¤ì(€€€€€€€±•ÐÝ…Ñ¡•É}¥€ôÑ½É% È¤ì(€€€€€€€±•Ð‘•ÍÑ¥¹…Ñ¥½¸€ôQ¥±•adèé¹•Ü Ì°€È¤ì((€€€€€€€±•ÐµÕÐÁ±…å•È€ô‰Õ¥±‘}…Ñ½È¡Á±…å•É}¥°€‰A±…å•Èˆ°€Ô°€ÄÀÀ°€ÈÀ°Q¥±•adèé¹•Ü È°€È¤¤ì(€€€€€€€Á±…å•È¹™…Ñ¥½¹}¥€ô€‰Á±…å•Èˆ¹Ñ½}ÍÑÉ¥¹œ ¤ì(€€€€€€€Á±…å•È¹…À€ôÁ‰}½É”èé¥‘ÌèéÀ Ô¤ì(€€€€€€€±•ÐµÕÐÝ…Ñ¡•È€ô‰Õ¥±‘}…Ñ½È¡Ý…Ñ¡•É}¥°€‰]…Ñ¡•Èˆ°€Ô°€ÄÀÀ°€ÈÀ°Q¥±•adèé¹•Ü Ô°€È¤¤ì(€€€€€€€Ý…Ñ¡•È¹™…Ñ¥½¹}¥€ô€‰•¹•µäˆ¹Ñ½}ÍÑÉ¥¹œ ¤ì(€€€€€€€Ý…Ñ¡•È¹™…¥¹œ€ôÁ‰}½É”èé•½´èé…¥¹œèé]•ÍÐì(€€€€€€€Í¥´¹…Ñ½ÉÌ¹¥¹Í•ÉÐ¡Á±…å•É}¥°Á±…å•È¤ì(€€€€€€€Í¥´¹…Ñ½ÉÌ¹¥¹Í•ÉÐ¡Ý…Ñ¡•É}¥°Ý…Ñ¡•È¤ì(€€€€€€€Í¥´¹‘¥™™¥Õ±Ñ}Ñ¥±•Ì¹¥¹Í•ÉÐ¡‘•ÍÑ¥¹…Ñ¥½¸¤ì(€€€€€€€Í¥´¹½Ù•É}•‘•Ì¹¥¹Í•ÉÐ (€€€€€€€€€€€Á‰}Í¥´èéÍÑ…Ñ”èé½Ù•É‘”ì(€€€€€€€€€€€€€€€Ñ¥±”èQ¥±•adèé¹•Ü È°€È¤°(€€€€€€€€€€€€€€€™…¥¹œèÁ‰}½É”èé•½´èé…¥¹œèé9½ÉÑ °(€€€€€€€€€€€ô°(€€€€€€€€€€€Á‰}Í¥´èéÍÑ…Ñ”èé½Ù•ÉMÑ…Ñ”ì(€€€€€€€€€€€€€€€±•Ù•°èÁ‰}Í¥´èéÍÑ…Ñ”èé½Ù•É1•Ù•°èé!…É°(€€€€€€€€€€€€€€€ÍÑÉ¥­•Ìè€À°(€€€€€€€€€€€€€€€¡…±™}¡•¥¡Ðè™…±Í”°(€€€€€€€€€€€€€€€‰ÕÉ¹¥¹œè™…±Í”°(€€€€€€€€€€€ô°(€€€€€€€€¤ì(€€€€€€€Í¥´¹½Ù•ÉÝ…Ñ ¹¥¹Í•ÉÐ¡Ý…Ñ¡•É}¥¤ì(€€€€€€€Í¥´¹É•…Ñ¥½¹}Á½¥¹ÑÌ¹¥¹Í•ÉÐ¡Ý…Ñ¡•É}¥°€Ì¤ì(€€€€€€€…µ”¹Í¥´€ôM½µ”¡Í¥´¤ì(€€€€€€€…µ”¹Á¡…Í”€ô%¹Ñ•É…Ñ¥½¹A¡…Í”èéM•±•Ñ•‘Ñ½È¡Á±…å•É}¥¤ì(€€€€€€€…µ”¹¡½Ù•É•‘}Ñ¥±•}à€ô‘•ÍÑ¥¹…Ñ¥½¸¹àì(€€€€€€€…µ”¹¡½Ù•É•‘}Ñ¥±•}ä€ô‘•ÍÑ¥¹…Ñ¥½¸¹äì((€€€€€€€…ÍÍ•ÉÑ}•Ä„ (€€€€€€€€€€€µ½Ù•µ•¹Ñ}ÁÉ•Ù¥•Ü ™…µ”¤°(€€€€€€€€€€€M½µ”¡5½Ù•µ•¹ÑAÉ•Ù¥•Üì(€€€€€€€€€€€€€€€…Á}½ÍÐè€È°(€€€€€€€€€€€€€€€É•µ…¥¹¥¹}…Àè€Ì°(€€€€€€€€€€€€€€€‘¥ÍÑ…¹”è€Ä°(€€€€€€€€€€€€€€€•¹‘Í}ÑÕÉ¸è™…±Í”°(€€€€€€€€€€€€€€€±•…Ù•Í}½Ù•ÈèÑÉÕ”°(€€€€€€€€€€€€€€€É½ÍÍ•Í}½Ù•ÉÝ…Ñ èÑÉÕ”°(€€€€€€€€€€€ô¤(€€€€€€€€¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸Í•±•Ñ•‘}…Ñ½É}•áÁ½Í•Í}½µÁ±•Ñ•}±•…±}µ½Ù•}…¹‘}ÍÁÉ¥¹Ñ}É…¹” ¤ì(€€€€€€€±•ÐµÕÐ…µ”€ô…µ•MÑ…Ñ”èé¹•Ü ¤ì(€€€€€€€±•ÐµÕÐÍ¥´€ôM¥µMÑ…Ñ”èé¹•Ü ÐÈ°€Ä¤ì(€€€€€€€±•Ð…Ñ½É}¥€ôÑ½É% Ä¤ì(€€€€€€€±•ÐµÕÐ…Ñ½È€ô‰Õ¥±‘}…Ñ½È¡…Ñ½É}¥°€‰A±…å•Èˆ°€Ô°€ÄÀÀ°€ÈÀ°Q¥±•adèé¹•Ü Ô°€Ô¤¤ì(€€€€€€€…Ñ½È¹™…Ñ¥½¹}¥€ô€‰Á±…å•Èˆ¹Ñ½}ÍÑÉ¥¹œ ¤ì(€€€€€€€…Ñ½È¹…À€ôÁ‰}½É”èé¥‘ÌèéÀ Ô¤ì(€€€€€€€Í¥´¹…Ñ½ÉÌ¹¥¹Í•ÉÐ¡…Ñ½É}¥°…Ñ½È¤ì(€€€€€€€Í¥´¹…Ñ¥Ù•}…Ñ½È€ôM½µ”¡…Ñ½É}¥¤ì(€€€€€€€…µ”¹Í¥´€ôM½µ”¡Í¥´¤ì(€€€€€€€…µ”¹Á¡…Í”€ô%¹Ñ•É…Ñ¥½¹A¡…Í”èéM•±•Ñ•‘Ñ½È¡…Ñ½É}¥¤ì((€€€€€€€±•ÐÉ…¹”€ôµ½Ù•µ•¹Ñ}É…¹” ™…µ”¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„ (€€€€€€€€€€€É…¹”(€€€€€€€€€€€€€€€€¹¥Ñ•È ¤(€€€€€€€€€€€€€€€€¹™¥±Ñ•È¡ð¡|°|°ÁÉ•Ù¥•Ü¥ð€…ÁÉ•Ù¥•Ü¹•¹‘Í}ÑÕÉ¸¤(€€€€€€€€€€€€€€€€¹½Õ¹Ð ¤°(€€€€€€€€€€€€à(€€€€€€€€¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„ (€€€€€€€€€€€É…¹”(€€€€€€€€€€€€€€€€¹¥Ñ•È ¤(€€€€€€€€€€€€€€€€¹™¥±Ñ•È¡ð¡|°|°ÁÉ•Ù¥•Ü¥ðÁÉ•Ù¥•Ü¹•¹‘Í}ÑÕÉ¸¤(€€€€€€€€€€€€€€€€¹½Õ¹Ð ¤°(€€€€€€€€€€€€ÄØ(€€€€€€€€¤ì(€€€€€€€…ÍÍ•ÉÐ„¡É…¹”¹¥Ñ•È ¤¹…±°¡ð¡|°|°ÁÉ•Ù¥•Ü¥ðì(€€€€€€€€€€€€¡ÁÉ•Ù¥•Ü¹‘¥ÍÑ…¹”€ôô€Ä€˜˜ÁÉ•Ù¥•Ü¹…Á}½ÍÐ€ôô€Ä€˜˜ÁÉ•Ù¥•Ü¹É•µ…¥¹¥¹}…À€ôô€Ð¤(€€€€€€€€€€€€€€€ñð€¡ÁÉ•Ù¥•Ü¹‘¥ÍÑ…¹”€ôô€È€˜˜ÁÉ•Ù¥•Ü¹…Á}½ÍÐ€ôô€È€˜˜ÁÉ•Ù¥•Ü¹É•µ…¥¹¥¹}…À€ôô€Ì¤(€€€€€€€ô¤¤ì((€€€€€€€±•Ð½Ù•É±…åÌ€ô‰Õ¥±‘}½Ù•É±…å}Ñ¥±•Ì ™…µ”¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡½Ù•É±…åÌ¹±•¸ ¤°€ÈÐ¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„ (€€€€€€€€€€€½Ù•É±…åÌ(€€€€€€€€€€€€€€€€¹¥Ñ•È ¤(€€€€€€€€€€€€€€€€¹™¥±Ñ•È¡ð¡|°|°|°­¥¹¥ðµ…Ñ¡•Ì„ (€€€€€€€€€€€€€€€€€€€­¥¹°(€€€€€€€€€€€€€€€€€€€=Ù•É±…åQ¥±•-¥¹èé5½Ù•µ•¹ÑI…¹”ì(€€€€€€€€€€€€€€€€€€€€€€€•¹‘Í}ÑÕÉ¸èÑÉÕ”°(€€€€€€€€€€€€€€€€€€€€€€€€¸¸(€€€€€€€€€€€€€€€€€€€ô(€€€€€€€€€€€€€€€€¤¤(€€€€€€€€€€€€€€€€¹½Õ¹Ð ¤°(€€€€€€€€€€€€ÄØ(€€€€€€€€¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸•¹•µå}…¥}™¥¹¥Í¡•Í}¥ÑÍ}…Á}ÑÕÉ¹}‰•™½É•}å¥•±‘¥¹}Ñ½}Á±…å•È ¤ì(€€€€€€€±•ÐµÕÐ…µ”€ô…µ•MÑ…Ñ”èé¹•Ü ¤ì(€€€€€€€±•ÐµÕÐÍ¥´€ôM¥µMÑ…Ñ”èé¹•Ü ÐÈ°€Ä¤ì(€€€€€€€±•ÐÁ±…å•É}¥€ôÑ½É% Ä¤ì(€€€€€€€±•Ð•¹•µå}¥€ôÑ½É% È¤ì((€€€€€€€±•ÐµÕÐÁ±…å•È€ô‰Õ¥±‘}…Ñ½È¡Á±…å•É}¥°€‰A±…å•Èˆ°€Ô°€Å|ÀÀÀ°€ÈÀ°Q¥±•adèé¹•Ü È°€È¤¤ì(€€€€€€€Á±…å•È¹™…Ñ¥½¹}¥€ô€‰Á±…å•Èˆ¹Ñ½}ÍÑÉ¥¹œ ¤ì(€€€€€€€±•ÐµÕÐ•¹•µä€ô‰Õ¥±‘}…Ñ½È¡•¹•µå}¥°€‰¹•µäˆ°€ä°€ÄÀÀ°€ÈÀ°Q¥±•adèé¹•Ü Ô°€È¤¤ì(€€€€€€€•¹•µä¹™…Ñ¥½¹}¥€ô€‰•¹•µäˆ¹Ñ½}ÍÑÉ¥¹œ ¤ì(€€€€€€€É•¥ÍÑ•É}…Ñ½È ™µÕÐÍ¥´°Á±…å•É}¥°Á±…å•È¤ì(€€€€€€€É•¥ÍÑ•É}…Ñ½È ™µÕÐÍ¥´°•¹•µå}¥°•¹•µä¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡…‘Ù…¹•}Ñ½}¹•áÑ}…Ñ½È ™µÕÐÍ¥´¤°M½µ”¡•¹•µå}¥¤¤ì(€€€€€€€…µ”¹Í¥´€ôM½µ”¡Í¥´¤ì((€€€€€€€±•ÐÉ•ÍÕ±Ð€ôÉÕ¹}•¹•µå}…¤ ™µÕÐ…µ”¤ì(€€€€€€€…ÍÍ•ÉÐ„¡É•ÍÕ±Ð¹¥Í}½¬ ¤°€‰$ÑÕÉ¸™…¥±•èíÉ•ÍÕ±Ðèýôˆ¤ì(€€€€€€€±•ÐÍ¥´€ô…µ”¹Í¥´¹…Í}É•˜ ¤ì(€€€€€€€…ÍÍ•ÉÐ„¡Í¥´¹¥Í}Í½µ” ¤¤ì(€€€€€€€¥˜±•ÐM½µ”¡Í¥´¤€ôÍ¥´ì(€€€€€€€€€€€…ÍÍ•ÉÑ}•Ä„¡Í¥´¹…Ñ¥Ù•}…Ñ½È°M½µ”¡Á±…å•É}¥¤¤ì(€€€€€€€€€€€…ÍÍ•ÉÑ}•Ä„¡Í¥´¹…Ñ½ÉÍl™•¹•µå}¥‘t¹…À¸À°€À¤ì(€€€€€€€€€€€…ÍÍ•ÉÑ}•Ä„¡Í¥´¹Ñ¥¬¸À°€À¤ì(€€€€€€€ô(€€€ô((€€€€mÑ•ÍÑt(€€€™¸±¥Ù¥¹}…Ñ½ÉÍ}¹•Ù•É}Í¡…É•}½¹•}™É½é•¹}¥‘±•}Á½Í” ¤ì(€€€€€€€±•Ð™¥ÉÍÐ€ô¥‘±•}‰½‘å}µ½Ñ¥½¸¡Ñ½É% Ä¤°MÑ…¹”èéMÑ…¹‘¥¹œ°€À°™…±Í”°ÑÉÕ”¤ì(€€€€€€€±•Ð±…Ñ•È€ô¥‘±•}‰½‘å}µ½Ñ¥½¸¡Ñ½É% Ä¤°MÑ…¹”èéMÑ…¹‘¥¹œ°€ÌÄ°™…±Í”°ÑÉÕ”¤ì(€€€€€€€±•ÐÍÅÕ…‘µ…Ñ”€ô¥‘±•}‰½‘å}µ½Ñ¥½¸¡Ñ½É% È¤°MÑ…¹”èéMÑ…¹‘¥¹œ°€À°™…±Í”°ÑÉÕ”¤ì(€€€€€€€±•Ð™…±±•¸€ô¥‘±•}‰½‘å}µ½Ñ¥½¸¡Ñ½É% Ä¤°MÑ…¹”èéMÑ…¹‘¥¹œ°€ÌÄ°™…±Í”°™…±Í”¤ì((€€€€€€€…ÍÍ•ÉÑ}¹”„¡™¥ÉÍÐ°±…Ñ•È¤ì(€€€€€€€…ÍÍ•ÉÑ}¹”„¡™¥ÉÍÐ°ÍÅÕ…‘µ…Ñ”¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡™…±±•¸°	½‘å5½Ñ¥½¸èé‘•™…Õ±Ð ¤¤ì(€€€€€€€…ÍÍ•ÉÐ„¡±…Ñ•È¹±¥™Ð¹…‰Ì ¤€ð€Ä¸À¤ì(€€€€€€€…ÍÍ•ÉÐ„¡±…Ñ•È¹Ñ½Á}ÍÝ…ä¹…‰Ì ¤€ð€È¸À¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸ÍÑ…¹‘¥¹}É½Õ¡•‘}…¹‘}ÁÉ½¹•}¡…Ù•}‘¥ÍÑ¥¹Ñ}‰…±…¹•}µ½Ñ¥½¸ ¤ì(€€€€€€€±•ÐÍÑ…¹‘¥¹œ€ô¥‘±•}‰½‘å}µ½Ñ¥½¸¡Ñ½É% Ü¤°MÑ…¹”èéMÑ…¹‘¥¹œ°€äÌ°™…±Í”°ÑÉÕ”¤ì(€€€€€€€±•ÐÉ½Õ¡•€ô¥‘±•}‰½‘å}µ½Ñ¥½¸¡Ñ½É% Ü¤°MÑ…¹”èéÉ½Õ¡•°€äÌ°™…±Í”°ÑÉÕ”¤ì(€€€€€€€±•ÐÁÉ½¹”€ô¥‘±•}‰½‘å}µ½Ñ¥½¸¡Ñ½É% Ü¤°MÑ…¹”èéAÉ½¹”°€äÌ°™…±Í”°ÑÉÕ”¤ì((€€€€€€€…ÍÍ•ÉÑ}¹”„¡ÍÑ…¹‘¥¹œ°É½Õ¡•¤ì(€€€€€€€…ÍÍ•ÉÑ}¹”„¡É½Õ¡•°ÁÉ½¹”¤ì(€€€€€€€…ÍÍ•ÉÐ„¡É½Õ¡•¹É½Ñ…Ñ¥½¸¹…‰Ì ¤€øôÍÑ…¹‘¥¹œ¹É½Ñ…Ñ¥½¸¹…‰Ì ¤¤ì(€€€€€€€…ÍÍ•ÉÐ„¡ÁÉ½¹”¹ÍÝ…ä¹…‰Ì ¤€ðÉ½Õ¡•¹ÍÝ…ä¹…‰Ì ¤¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸µ½Ù•µ•¹Ñ}…¹¥µ…Ñ¥½¹}‘ÕÉ…Ñ¥½¹}Í…±•Í}Ý¥Ñ¡}‘¥ÍÑ…¹•}…¹‘}ÍÁÉ¥¹Ð ¤ì(€€€€€€€±•Ð™É½´€ôQ¥±•adèé¹•Ü Ä°€Ä¤ì(€€€€€€€±•Ð¹•…È€ôQ¥±•adèé¹•Ü È°€Ä¤ì(€€€€€€€±•Ð™…È€ôQ¥±•adèé¹•Ü Ø°€Ä¤ì((€€€€€€€…ÍÍ•ÉÑ}•Ä„¡µ½Ù•µ•¹Ñ}‘ÕÉ…Ñ¥½¹}µÌ¡™É½´°¹•…È°™…±Í”¤°€ÈàÀ¤ì(€€€€€€€…ÍÍ•ÉÐ„¡µ½Ù•µ•¹Ñ}‘ÕÉ…Ñ¥½¹}µÌ¡™É½´°™…È°™…±Í”¤€øµ½Ù•µ•¹Ñ}‘ÕÉ…Ñ¥½¹}µÌ¡™É½´°¹•…È°™…±Í”¤¤ì(€€€€€€€…ÍÍ•ÉÐ„¡µ½Ù•µ•¹Ñ}‘ÕÉ…Ñ¥½¹}µÌ¡™É½´°™…È°ÑÉÕ”¤€ðµ½Ù•µ•¹Ñ}‘ÕÉ…Ñ¥½¹}µÌ¡™É½´°™…È°™…±Í”¤¤ì(€€€€€€€…ÍÍ•ÉÐ„¡µ½Ù•µ•¹Ñ}‘ÕÉ…Ñ¥½¹}µÌ¡™É½´°Q¥±•adèé¹•Ü ÐÀ°€ÐÀ¤°™…±Í”¤€ðô€äÀÀ¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸Ý…±­¥¹}å±•}…¹¥µ…Ñ•Í}±•Í}¡¥ÁÍ}…¹‘}½Õ¹Ñ•É}ÍÝ¥¹¥¹}…ÉµÌ ¤ì(€€€€€€€±•Ð™¥ÉÍÐ€ôÝ…±­}å±•}µ½Ñ¥½¸¡Ñ½É% Ä¤°€À¸ÈÐ°€Ä¸À¤ì(€€€€€€€±•ÐÍ•½¹€ôÝ…±­}å±•}µ½Ñ¥½¸¡Ñ½É% Ä¤°€À¸ÜÐ°€Ä¸À¤ì((€€€€€€€…ÍÍ•ÉÐ„¡™¥ÉÍÐ¹±•}ÍÝ…ä¹…‰Ì ¤€ø€À¸Ä¤ì(€€€€€€€…ÍÍ•ÉÐ„¡™¥ÉÍÐ¹±•}±¥™Ð€ø€À¸À¤ì(€€€€€€€…ÍÍ•ÉÐ„¡™¥ÉÍÐ¹¡¥Á}É½Ñ…Ñ¥½¸¹…‰Ì ¤€ø€À¸ÀÄ¤ì(€€€€€€€…ÍÍ•ÉÐ„¡™¥ÉÍÐ¹…Éµ}ÍÝ¥¹œ¹…‰Ì ¤€ø€À¸ÀÄ¤ì(€€€€€€€…ÍÍ•ÉÐ„¡™¥ÉÍÐ¹…Éµ}ÍÝ¥¹œ¹Í¥¹Õ´ ¤€„ô™¥ÉÍÐ¹¡¥Á}É½Ñ…Ñ¥½¸¹Í¥¹Õ´ ¤¤ì(€€€€€€€…ÍÍ•ÉÑ}¹”„¡™¥ÉÍÐ¹±•}ÍÝ…ä°Í•½¹¹±•}ÍÝ…ä¤ì(€€€€€€€…ÍÍ•ÉÑ}¹”„¡™¥ÉÍÐ¹…Éµ}ÍÝ¥¹œ°Í•½¹¹…Éµ}ÍÝ¥¹œ¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸™¥É¥¹}…¹¥µ…Ñ¥½¹}…‘‘Í}Ñ¡É••}Í½±¥‘}µÕéé±•}™±…Í¡}±…å•ÉÌ ¤ì(€€€€€€€±•ÐµÕÐ…µ”€ô…µ•MÑ…Ñ”èé¹•Ü ¤ì(€€€€€€€±•ÐµÕÐÍ¥´€ôM¥µMÑ…Ñ”èé¹•Ü ÐÈ°€Ä¤ì(€€€€€€€±•Ð…Ñ½É}¥€ôÑ½É% Ä¤ì(€€€€€€€±•ÐÑ…É•Ñ}¥€ôÑ½É% È¤ì(€€€€€€€±•Ð…Ñ½É}Á½Í¥Ñ¥½¸€ôQ¥±•adèé¹•Ü È°€È¤ì(€€€€€€€±•ÐÑ…É•Ñ}Á½Í¥Ñ¥½¸€ôQ¥±•adèé¹•Ü Ô°€È¤ì(€€€€€€€±•ÐµÕÐ…Ñ½È€ô‰Õ¥±‘}…Ñ½È¡…Ñ½É}¥°€‰A±…å•Èˆ°€Ô°€ÄÀÀ°€ÈÀ°…Ñ½É}Á½Í¥Ñ¥½¸¤ì(€€€€€€€…Ñ½È¹™…Ñ¥½¹}¥€ô€‰Á±…å•Èˆ¹Ñ½}ÍÑÉ¥¹œ ¤ì(€€€€€€€±•ÐµÕÐÑ…É•Ð€ô‰Õ¥±‘}…Ñ½È¡Ñ…É•Ñ}¥°€‰¹•µäˆ°€Ô°€ÄÀÀ°€ÈÀ°Ñ…É•Ñ}Á½Í¥Ñ¥½¸¤ì(€€€€€€€Ñ…É•Ð¹™…Ñ¥½¹}¥€ô€‰•¹•µäˆ¹Ñ½}ÍÑÉ¥¹œ ¤ì(€€€€€€€Í¥´¹…Ñ½ÉÌ¹¥¹Í•ÉÐ¡…Ñ½É}¥°…Ñ½È¤ì(€€€€€€€Í¥´¹…Ñ½ÉÌ¹¥¹Í•ÉÐ¡Ñ…É•Ñ}¥°Ñ…É•Ð¤ì(€€€€€€€…µ”¹Í¥´€ôM½µ”¡Í¥´¤ì(€€€€€€€…µ”¹‰…ÑÑ±•}…¹¥µ…Ñ¥½¹Ì¹ÁÕÍ ¡	…ÑÑ±•¹¥µ…Ñ¥½¸ì(€€€€€€€€€€€…Ñ½Èè…Ñ½É}¥°(€€€€€€€€€€€™É½´è…Ñ½É}Á½Í¥Ñ¥½¸°(€€€€€€€€€€€Ñ¼èÑ…É•Ñ}Á½Í¥Ñ¥½¸°(€€€€€€€€€€€ÍÑ…ÉÑ•è%¹ÍÑ…¹Ðèé¹½Ü ¤°(€€€€€€€€€€€‘•±…å}µÌè€À°(€€€€€€€€€€€‘ÕÉ…Ñ¥½¹}µÌè€ÔÈÀ°(€€€€€€€€€€€­¥¹è	…ÑÑ±•¹¥µ…Ñ¥½¹-¥¹èéI•½¥°ì¡¥ÐèÑÉÕ”ô°(€€€€€€€ô¤ì((€€€€€€€±•ÐÍÁÉ¥Ñ•Ì€ô‰Õ¥±‘}ÍÁÉ¥Ñ•}¥¹ÍÑ…¹•Ì ™…µ”¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡ÍÁÉ¥Ñ•Ì¹±•¸ ¤°€ÄÈ¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„ (€€€€€€€€€€€ÍÁÉ¥Ñ•Ì(€€€€€€€€€€€€€€€€¹¥Ñ•È ¤(€€€€€€€€€€€€€€€€¹™¥±Ñ•È¡ñÍÁÉ¥Ñ•ðì(€€€€€€€€€€€€€€€€€€€ÍÁÉ¥Ñ”¹ÔÀ€ð€À¸À(€€€€€€€€€€€€€€€€€€€€€€€€˜˜€¡ÍÁÉ¥Ñ”¹È€´€Ä¸À¤¹…‰Ì ¤€ð˜ÌÈèéAM%1=8(€€€€€€€€€€€€€€€€€€€€€€€€˜˜€¡ÍÁÉ¥Ñ”¹œ€´€À¸ÜÈ¤¹…‰Ì ¤€ð˜ÌÈèéAM%1=8(€€€€€€€€€€€€€€€ô¤(€€€€€€€€€€€€€€€€¹½Õ¹Ð ¤°(€€€€€€€€€€€€Ì(€€€€€€€€¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸½µ‰…Ñ}™••‘‰…­}­••ÁÍ}‘…µ…•}±½…Ñ¥½¹}…¹‘}…¹¥µ…Ñ¥½¹}¥¹}Íå¹Œ ¤ì(€€€€€€€±•ÐÍ¡½½Ñ•É}¥€ôÑ½É% Ä¤ì(€€€€€€€±•ÐÑ…É•Ñ}¥€ôÑ½É% È¤ì(€€€€€€€±•ÐÍ¡½½Ñ•É}Á½Í¥Ñ¥½¸€ôQ¥±•adèé¹•Ü È°€È¤ì(€€€€€€€±•ÐÑ…É•Ñ}Á½Í¥Ñ¥½¸€ôQ¥±•adèé¹•Ü Ô°€È¤ì(€€€€€€€±•ÐµÕÐÍ¥´€ôM¥µMÑ…Ñ”èé¹•Ü ÐÈ°€Ä¤ì(€€€€€€€±•ÐµÕÐÍ¡½½Ñ•È€ô‰Õ¥±‘}…Ñ½È¡Í¡½½Ñ•É}¥°€‰A±…å•Èˆ°€Ô°€ÄÀÀ°€ÈÀ°Í¡½½Ñ•É}Á½Í¥Ñ¥½¸¤ì(€€€€€€€Í¡½½Ñ•È¹™…Ñ¥½¹}¥€ô€‰Á±…å•Èˆ¹Ñ½}ÍÑÉ¥¹œ ¤ì(€€€€€€€±•ÐµÕÐÑ…É•Ð€ô‰Õ¥±‘}…Ñ½È¡Ñ…É•Ñ}¥°€‰¹•µäˆ°€Ô°€ÄÀÀ°€ÈÀ°Ñ…É•Ñ}Á½Í¥Ñ¥½¸¤ì(€€€€€€€Ñ…É•Ð¹™…Ñ¥½¹}¥€ô€‰•¹•µäˆ¹Ñ½}ÍÑÉ¥¹œ ¤ì(€€€€€€€Í¥´¹…Ñ½ÉÌ¹¥¹Í•ÉÐ¡Í¡½½Ñ•É}¥°Í¡½½Ñ•È¤ì(€€€€€€€Í¥´¹…Ñ½ÉÌ¹¥¹Í•ÉÐ¡Ñ…É•Ñ}¥°Ñ…É•Ð¤ì((€€€€€€€±•Ð•Ù•¹ÑÌ€ôÙ•Œ…l(€€€€€€€€€€€Ù•¹Ðèé¥É•ì(€€€€€€€€€€€€€€€…Ñ½ÈèÍ¡½½Ñ•É}¥°(€€€€€€€€€€€€€€€Ñ…É•ÐèÑ…É•Ñ}¥°(€€€€€€€€€€€ô°(€€€€€€€€€€€Ù•¹ÐèéM¡½Ñ!¥Ðì(€€€€€€€€€€€€€€€…Ñ½ÈèÍ¡½½Ñ•É}¥°(€€€€€€€€€€€€€€€Ñ…É•ÐèÑ…É•Ñ}¥°(€€€€€€€€€€€€€€€¡¥ÐèÑÉÕ”°(€€€€€€€€€€€ô°(€€€€€€€€€€€Ù•¹Ðèé!¥Ñ1½…Ñ¥½¸ì(€€€€€€€€€€€€€€€…Ñ½ÈèÑ…É•Ñ}¥°(€€€€€€€€€€€€€€€±½…Ñ¥½¸è!¥Ñ1½…Ñ¥½¹QåÁ”èé!•…°(€€€€€€€€€€€ô°(€€€€€€€€€€€Ù•¹Ðèé…µ…•ÁÁ±¥•ì(€€€€€€€€€€€€€€€…Ñ½ÈèÑ…É•Ñ}¥°(€€€€€€€€€€€€€€€‘…µ…”è€ÄÈ°(€€€€€€€€€€€ô°(€€€€€€€€€€€Ù•¹Ðèé]½Õ¹‘ÁÁ±¥•ì(€€€€€€€€€€€€€€€…Ñ½ÈèÑ…É•Ñ}¥°(€€€€€€€€€€€€€€€Ý½Õ¹è]½Õ¹‘QåÁ”èé	±••‘¥¹œ°(€€€€€€€€€€€ô°(€€€€€€€€€€€Ù•¹ÐèéÉ¥Ñ¥…°ì(€€€€€€€€€€€€€€€…Ñ½ÈèÑ…É•Ñ}¥°(€€€€€€€€€€€€€€€•™™•Ðè€‰ÍÑ…•Èˆ¹Ñ½}ÍÑÉ¥¹œ ¤°(€€€€€€€€€€€ô°(€€€€€€€€€€€Ù•¹ÐèéÑ½É-¥±±•ì…Ñ½ÈèÑ…É•Ñ}¥ô°(€€€€€€€tì((€€€€€€€±•Ð•¹ÑÉ¥•Ì€ô½µ‰…Ñ}±½}•¹ÑÉ¥•Ì ™Í¥´°Í¡½½Ñ•É}¥°M½µ” ‰%5M!=Pˆ¤°€™•Ù•¹ÑÌ¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡•¹ÑÉ¥•Ì¹±•¸ ¤°€Ä¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡•¹ÑÉ¥•ÍlÁt¹Ñ½¹”°É…Ñ”èéÍÑ…Ñ”èé½µ‰…Ñ1½Q½¹”èéÉ¥Ñ¥…°¤ì(€€€€€€€…ÍÍ•ÉÐ„¡•¹ÑÉ¥•ÍlÁt(€€€€€€€€€€€€¹Ñ•áÐ(€€€€€€€€€€€€¹½¹Ñ…¥¹Ì ‰I%PA±…å•È€ø¹•µäè€´ÄÈ!@!€­	1%9=]8ˆ¤¤ì((€€€€€€€±•Ðµ•±••}•¹ÑÉ¥•Ì€ô½µ‰…Ñ}±½}•¹ÑÉ¥•Ì (€€€€€€€€€€€€™Í¥´°(€€€€€€€€€€€Í¡½½Ñ•É}¥°(€€€€€€€€€€€M½µ” ‰51ˆ¤°(€€€€€€€€€€€€™l(€€€€€€€€€€€€€€€Ù•¹Ðèé…µ…•ÁÁ±¥•ì(€€€€€€€€€€€€€€€€€€€…Ñ½ÈèÑ…É•Ñ}¥°(€€€€€€€€€€€€€€€€€€€‘…µ…”è€Ü°(€€€€€€€€€€€€€€€ô°(€€€€€€€€€€€€€€€Ù•¹Ðèé]½Õ¹‘ÁÁ±¥•ì(€€€€€€€€€€€€€€€€€€€…Ñ½ÈèÑ…É•Ñ}¥°(€€€€€€€€€€€€€€€€€€€Ý½Õ¹è]½Õ¹‘QåÁ”èé	±••‘¥¹œ°(€€€€€€€€€€€€€€€ô°(€€€€€€€€€€€€€€€Ù•¹ÐèéÑ½É-¥±±•ì…Ñ½ÈèÑ…É•Ñ}¥ô°(€€€€€€€€€€€t°(€€€€€€€€¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡µ•±••}•¹ÑÉ¥•ÍlÁt¹Ñ½¹”°É…Ñ”èéÍÑ…Ñ”èé½µ‰…Ñ1½Q½¹”èé•™•…Ð¤ì(€€€€€€€…ÍÍ•ÉÐ„¡µ•±••}•¹ÑÉ¥•ÍlÁt(€€€€€€€€€€€€¹Ñ•áÐ(€€€€€€€€€€€€¹½¹Ñ…¥¹Ì ‰A±…å•È€ø¹•µäè€´Ü!@51€­	1%9=]8ˆ¤¤ì((€€€€€€€±•ÐµÕÐ…µ”€ô…µ•MÑ…Ñ”èé¹•Ü ¤ì(€€€€€€€…µ”¹Í¥´€ôM½µ”¡Í¥´¤ì(€€€€€€€ÅÕ•Õ•}•Ù•¹Ñ}…¹¥µ…Ñ¥½¹Ì ™µÕÐ…µ”°€™•Ù•¹ÑÌ°Í¡½½Ñ•É}¥°™…±Í”¤ì(€€€€€€€…ÍÍ•ÉÐ„¡…µ”¹‰…ÑÑ±•}…¹¥µ…Ñ¥½¹Ì¹¥Ñ•È ¤¹…¹ä¡ñ…¹¥µ…Ñ¥½¹ðì(€€€€€€€€€€€…¹¥µ…Ñ¥½¸¹…Ñ½È€ôôÍ¡½½Ñ•É}¥(€€€€€€€€€€€€€€€€˜˜µ…Ñ¡•Ì„¡…¹¥µ…Ñ¥½¸¹­¥¹°	…ÑÑ±•¹¥µ…Ñ¥½¹-¥¹èéI•½¥°ì¡¥ÐèÑÉÕ”ô¤(€€€€€€€ô¤¤ì(€€€€€€€…ÍÍ•ÉÐ„¡…µ”¹‰…ÑÑ±•}…¹¥µ…Ñ¥½¹Ì¹¥Ñ•È ¤¹…¹ä¡ñ…¹¥µ…Ñ¥½¹ðì(€€€€€€€€€€€…¹¥µ…Ñ¥½¸¹…Ñ½È€ôôÑ…É•Ñ}¥(€€€€€€€€€€€€€€€€˜˜µ…Ñ¡•Ì„ (€€€€€€€€€€€€€€€€€€€…¹¥µ…Ñ¥½¸¹­¥¹°(€€€€€€€€€€€€€€€€€€€	…ÑÑ±•¹¥µ…Ñ¥½¹-¥¹èé!¥ÑI•…Ðì(€€€€€€€€€€€€€€€€€€€€€€€‘…µ…”è€ÄÈ°(€€€€€€€€€€€€€€€€€€€€€€€É¥Ñ¥…°èÑÉÕ”°(€€€€€€€€€€€€€€€€€€€€€€€­¥±±•èÑÉÕ”(€€€€€€€€€€€€€€€€€€€ô(€€€€€€€€€€€€€€€€¤(€€€€€€€ô¤¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸¡•…±Ñ¡}µ•Ñ•É}™¥±±}ÑÉ…­Í}‘…µ…•}…¹‘}ÕÍ•Í}É¥Ñ¥…±}½±½È ¤ì(€€€€€€€±•Ðm™É…µ”°ÑÉ…¬°™¥±±t€ô¡•…±Ñ¡}µ•Ñ•É}ÍÁÉ¥Ñ•Ì ÄÀÀ¸À°€àÀ¸À°€Ì¸À°€ÈÔ°€ÄÀÀ°™…±Í”¤ì((€€€€€€€…ÍÍ•ÉÐ„¡™É…µ”¹ˆ€ø™É…µ”¹È°€‰…±±¥•™É…µ”Í¡½Õ±‰”‰±Õ”ˆ¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡ÑÉ…¬¹Ý¥‘Ñ °€Ðà¸À¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡™¥±°¹Ý¥‘Ñ °€ÄÈ¸À¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡™¥±°¹à°€àÈ¸À¤ì(€€€€€€€…ÍÍ•ÉÐ„¡™¥±°¹È€ø™¥±°¹œ°€‰É¥Ñ¥…°¡•…±Ñ Í¡½Õ±‰”É•ˆ¤ì(€€€€€€€…ÍÍ•ÉÐ„¡™¥±°¹Ù¥Í¥‰±”¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸‘…µ…•}…¹‘}‘•…Ñ¡}Õ•Í}™½±±½Ý}Ñ¡•}Ù¥Í¥‰±•}…Ù…Ñ…É}¥‘•¹Ñ¥Ñä ¤ì(€€€€€€€±•Ðµ…±”€ôÑ½É% À¤ì(€€€€€€€±•Ð™•µ…±”€ôÑ½É% Ø¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡‘…µ…•}Í™á}™½É}…Ñ½È¡µ…±”¤°Á‰}…Õ‘¥¼èéM™àèé…µ…•5…±”¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡‘…µ…•}Í™á}™½É}…Ñ½È¡™•µ…±”¤°Á‰}…Õ‘¥¼èéM™àèé…µ…••µ…±”¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡‘•…Ñ¡}Í™á}™½É}…Ñ½È¡µ…±”¤°Á‰}…Õ‘¥¼èéM™àèé•…Ñ¡5…±”¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡‘•…Ñ¡}Í™á}™½É}…Ñ½È¡™•µ…±”¤°Á‰}…Õ‘¥¼èéM™àèé•…Ñ¡•µ…±”¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„ (€€€€€€€€€€€Í™á}™½É}•Ù•¹Ð ™Ù•¹Ðèé…µ…•ÁÁ±¥•ì(€€€€€€€€€€€€€€€…Ñ½Èè™•µ…±”°(€€€€€€€€€€€€€€€‘…µ…”è€Ô°(€€€€€€€€€€€ô¤°(€€€€€€€€€€€M½µ”¡Á‰}…Õ‘¥¼èéM™àèé…µ…••µ…±”¤(€€€€€€€€¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„ (€€€€€€€€€€€Í™á}™½É}•Ù•¹Ð ™Ù•¹ÐèéÑ½É-¥±±•ì…Ñ½Èèµ…±”ô¤°(€€€€€€€€€€€M½µ”¡Á‰}…Õ‘¥¼èéM™àèé•…Ñ¡5…±”¤(€€€€€€€€¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„ (€€€€€€€€€€€Í™á}™½É}•Ù•¹Ð ™Ù•¹Ðèé…µ…•ÁÁ±¥•ì(€€€€€€€€€€€€€€€…Ñ½Èèµ…±”°(€€€€€€€€€€€€€€€‘…µ…”è€À°(€€€€€€€€€€€ô¤°(€€€€€€€€€€€9½¹”(€€€€€€€€¤ì(€€€ô)ô(