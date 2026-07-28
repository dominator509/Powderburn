//! Combat screen — renders the isometric battlefield from SimState.
//!
//! Builds pb-render systems each frame to display tiles, actors, overlays,
//! and smoke. Handles tile hover/selection and AI stepping.
//!
//! Also provides the full interactive combat loop: player clicks to select,
//! keyboard to choose actions, clicks to target, AI runs for enemies.

#![allow(dead_code)]

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use pb_content::load;
use pb_content::schema::Content;
use pb_core::geom::TileXY;
use pb_core::ids::ActorId;
use pb_core::metrics::MetricsRegistry;
use pb_render::camera::IsoCamera;
use pb_render::device::RenderDevice;
use pb_render::overlay::{OverlaySystem, OverlayTileKind};
use pb_render::props::{prop_instances_from_state, PropSystem};
use pb_render::smoke::{SmokeSystem, SmokeTile};
use pb_render::sprites::{SpriteInstance, SpriteSystem};
use pb_render::tiles::{TileSystem, TileVisual};
use pb_sim::action::{effective_action_cost, overwatch_threats_at, step, Action, Command};
use pb_sim::clock::advance_to_next_actor;
use pb_sim::shot::{compute_hit_chance_breakdown, HitChanceBreakdown};
use pb_sim::state::{ActorState, SimError, SimState, Stance};

use crate::state::{GameState, InteractionPhase, PlayerAction};
use pb_core::event::Event;

// ── Constants ─────────────────────────────────────────────────────────────

/// Default battlefield grid dimensions.
const GRID_COLS: u32 = 20;
const GRID_ROWS: u32 = 12;

/// Isometric tile dimensions in pixels (at zoom = 1.0).
const TILE_W: f32 = 64.0;
const TILE_H: f32 = 32.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MovementPreview {
    pub ap_cost: u8,
    pub leaves_cover: bool,
    pub crosses_overwatch: bool,
}

// ── Faction helpers ───────────────────────────────────────────────────────

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
            "Selected {} — HP {}/{}, AP {}, facing {}",
            actor.name, actor.hit_points, actor.max_hp, actor.ap.0, actor.facing
        )
    } else {
        let active_name = sim
            .active_actor
            .and_then(|active| sim.actors.get(&active))
            .map_or("no one", |active| active.name.as_str());
        format!(
            "Inspecting {} — facing {}; {active_name} has the turn",
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
        format!("Target: {name} — hit chance {}%", breakdown.total)
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
    game_state.message = format!("Facing {next} — changing facing costs 0 AP");
    Ok(())
}

/// Return the exact movement consequences for the selected actor and hovered
/// tile, sharing AP and overwatch rules with command execution.
pub fn movement_preview(game_state: &GameState) -> Option<MovementPreview> {
    let sim = game_state.sim.as_ref()?;
    let actor_id = match game_state.phase {
        InteractionPhase::SelectedActor(id) => id,
        _ => return None,
    };
    let actor = sim.actors.get(&actor_id)?;
    let destination = TileXY::new(game_state.hovered_tile_x, game_state.hovered_tile_y);
    let distance = actor.position.chebyshev_distance(destination);
    let action = match distance {
        1 => Action::Move(destination),
        2 if actor.stance == Stance::Standing => Action::Sprint(destination),
        _ => return None,
    };
    let cost = effective_action_cost(sim, actor_id, &action).ok()?.0;
    let ap_cost = u8::try_from(cost.max(0)).unwrap_or(u8::MAX);
    let has_cover = |tile| {
        sim.cover_edges.iter().any(|(edge, cover)| {
            edge.tile == tile && cover.level != pb_sim::state::CoverLevel::None
        })
    };
    Some(MovementPreview {
        ap_cost,
        leaves_cover: has_cover(actor.position) && !has_cover(destination),
        crosses_overwatch: !overwatch_threats_at(sim, actor_id, destination).is_empty(),
    })
}

// ── Coordinate conversion ─────────────────────────────────────────────────

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
    let sy = (mouse_y as f32 - viewport_height * 0.5) / zoom + camera_y;

    let half_w = TILE_W * 0.5;
    let half_h = TILE_H * 0.5;

    if half_w.abs() < f32::EPSILON || half_h.abs() < f32::EPSILON {
        return TileXY::new(0, 0);
    }

    let a = sx / half_w;
    let b = sy / half_h;

    let tile_x = ((a + b) * 0.5).floor() as i16;
    let tile_y = ((b - a) * 0.5).floor() as i16;

    TileXY::new(tile_x, tile_y)
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

// ── Public API ────────────────────────────────────────────────────────────

/// Load the selected scenario and create its deterministic simulation state.
/// Screen transitions remain the caller's responsibility.
pub fn init_combat(game_state: &mut GameState, content_root: &Path) -> Result<(), String> {
    game_state.battle_events.clear();
    game_state.pending_ledger_writes.clear();
    game_state.ledger_write_cursor = 0;
    let content: Content =
        load::load_all(content_root).map_err(|e| format!("content load failed: {e}"))?;

    let mission_id = game_state
        .current_mission
        .as_deref()
        .ok_or_else(|| "no campaign mission selected".to_string())?;
    let scenario_id = content
        .campaign_nodes
        .get(mission_id)
        .and_then(|node| node.scenario_id.as_deref())
        .ok_or_else(|| format!("campaign mission {mission_id} has no scenario"))?;

    let scenario = content
        .scenarios
        .get(scenario_id)
        .ok_or_else(|| format!("scenario {scenario_id} not found"))?;

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

    advance_to_next_actor(&mut sim);

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
    game_state.message = format!("{} loaded — click an ally to act", scenario.display_name);

    Ok(())
}

/// Render one frame of the combat screen.
///
/// Builds all pb-render systems from scratch each frame, draws them in
/// z-order: tiles → smoke → overlay → props → sprites.
pub fn render_combat_frame(
    game_state: &GameState,
    render_device: &Arc<RenderDevice>,
    view: &wgpu::TextureView,
    _surface_format: wgpu::TextureFormat,
    viewport_width: u32,
    viewport_height: u32,
) {
    let _timer = Instant::now();

    // ── Camera ──────────────────────────────────────────────────────────
    let camera = IsoCamera {
        center_x: game_state.camera_x,
        center_y: game_state.camera_y,
        zoom: game_state.camera_zoom,
        viewport_width: viewport_width as f32,
        viewport_height: viewport_height as f32,
        tile_w: TILE_W,
        tile_h: TILE_H,
    };
    let camera_bytes = camera.ortho_matrix_bytes();

    // ── Tile grid ───────────────────────────────────────────────────────
    let tiles = build_tile_visuals(game_state);
    let tile_system = TileSystem::new(render_device, GRID_COLS, GRID_ROWS, &tiles, &camera_bytes);

    // ── Smoke overlay ───────────────────────────────────────────────────
    let smoke_tiles = build_smoke_grid(game_state);
    let smoke_system = SmokeSystem::new(
        render_device,
        GRID_COLS,
        GRID_ROWS,
        &smoke_tiles,
        &camera_bytes,
    );

    // ── Overlay (hovered tile, selected actor highlight) ────────────────
    let overlay_tiles = build_overlay_tiles(game_state);
    let overlay_system = OverlaySystem::new(render_device, &overlay_tiles, &camera_bytes);

    // ── Presentation-only environmental props ──────────────────────────
    let props = game_state
        .sim
        .as_ref()
        .map(prop_instances_from_state)
        .unwrap_or_default();
    let prop_system = PropSystem::new(render_device, &props, &camera_bytes);

    // ── Actor sprites ───────────────────────────────────────────────────
    let sprites = build_sprite_instances(game_state);
    let sprite_system = SpriteSystem::new(render_device, &sprites, &camera_bytes);

    // ── Command encoder & render pass ───────────────────────────────────
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

        // Draw in z-order: terrain → smoke → overlay → props → sprites
        tile_system.render(&mut rpass);
        smoke_system.render(&mut rpass);
        overlay_system.render(&mut rpass);
        prop_system.render(&mut rpass);
        sprite_system.render(&mut rpass);
    }

    render_device
        .queue
        .submit(std::iter::once(encoder.finish()));

    let elapsed_ms = match u64::try_from(_timer.elapsed().as_millis()) {
        Ok(value) => value,
        Err(_) => u64::MAX,
    };
    MetricsRegistry::global().record_render_frame(elapsed_ms);
}

/// Handle a mouse click during combat.
///
/// Selects/deselects actors, fires on targets, and orchestrates the combat
/// turn cycle.
pub fn handle_combat_click(game_state: &mut GameState) -> Result<(), String> {
    let Some(ref sim) = game_state.sim else {
        return Err("no simulation loaded".to_string());
    };

    let hovered = TileXY::new(game_state.hovered_tile_x, game_state.hovered_tile_y);

    // Check if an actor is at the hovered tile
    let actor_at = sim.actors.iter().find(|(_, a)| a.position == hovered);

    match game_state.phase {
        InteractionPhase::Idle => {
            if let Some((&id, actor)) = actor_at {
                if actor.alive && is_ally(actor) {
                    if sim.active_actor != Some(id) {
                        let active_name = sim
                            .active_actor
                            .and_then(|active| sim.actors.get(&active))
                            .map_or("another actor", |active| active.name.as_str());
                        game_state.message =
                            format!("It is {active_name}'s turn, not {}'s", actor.name);
                        return Ok(());
                    }
                    game_state.phase = InteractionPhase::SelectedActor(id);
                    if let Some(ref audio) = game_state.audio {
                        audio.play(pb_audio::Sfx::Select);
                    }
                    if let Some(a) = sim.actors.get(&id) {
                        game_state.message = format!(
                            "Selected {} (HP: {}/{}, AP: {}) — press F(fire), R(reload), H(hold)",
                            a.name, a.hit_points, a.max_hp, a.ap.0
                        );
                    }
                } else if actor.alive {
                    game_state.message = format!("Cannot select enemy {}", actor.name);
                } else {
                    game_state.message = "That actor is dead".to_string();
                }
            } else {
                game_state.message = "Click on an ally to select".to_string();
            }
        }
        InteractionPhase::SelectedActor(selected_id) => {
            // If clicking the same actor, deselect
            if let Some((&id, _)) = actor_at {
                if id == selected_id {
                    game_state.phase = InteractionPhase::Idle;
                    game_state.message = "Deselected".to_string();
                } else if let Some(actor) = sim.actors.get(&id) {
                    if actor.alive && is_ally(actor) && sim.active_actor == Some(id) {
                        // Switch selection to different ally
                        game_state.phase = InteractionPhase::SelectedActor(id);
                        game_state.message = format!(
                            "Selected {} (HP: {}/{}, AP: {})",
                            actor.name, actor.hit_points, actor.max_hp, actor.ap.0
                        );
                    }
                }
            } else {
                let distance = sim
                    .actors
                    .get(&selected_id)
                    .map(|actor| actor.position.chebyshev_distance(hovered))
                    .unwrap_or(0);
                if matches!(distance, 1 | 2) {
                    execute_move_to(game_state, selected_id, hovered, distance == 2)?;
                    check_victory_conditions(game_state);
                    if game_state.screen != crate::state::GameScreen::Battle {
                        return Ok(());
                    }
                    let player_continues = game_state
                        .sim
                        .as_ref()
                        .is_some_and(|state| state.active_actor == Some(selected_id));
                    if player_continues {
                        game_state.phase = InteractionPhase::SelectedActor(selected_id);
                    } else {
                        run_enemy_ai(game_state)?;
                        check_victory_conditions(game_state);
                    }
                } else {
                    game_state.message =
                        "Move one tile, or click exactly two tiles away to Sprint".to_string();
                }
            }
        }
        InteractionPhase::Targeting {
            actor: selected_id,
            action: player_action,
        } => {
            game_state.message = format!("Executing {player_action:?}");
            if let Err(error) = execute_player_action(game_state) {
                game_state.phase = InteractionPhase::SelectedActor(selected_id);
                game_state.message = error;
                return Ok(());
            }
            check_victory_conditions(game_state);
            if game_state.screen != crate::state::GameScreen::Battle {
                return Ok(());
            }
            let player_continues = game_state
                .sim
                .as_ref()
                .is_some_and(|sim| sim.active_actor == Some(selected_id));
            if player_continues {
                game_state.phase = InteractionPhase::SelectedActor(selected_id);
            } else {
                run_enemy_ai(game_state)?;
                check_victory_conditions(game_state);
            }
        }
        InteractionPhase::Executing => {
            game_state.message = "Action in progress...".to_string();
        }
    }

    Ok(())
}

/// Process a player's intended action against the hovered target.
///
/// Reads the current `InteractionPhase::Targeting` fields and the hovered
/// tile to build a `Command` and step the simulation.
pub fn execute_player_action(gs: &mut GameState) -> Result<(), String> {
    let (actor_id, player_action) = match gs.phase {
        InteractionPhase::Targeting { actor, action } => (actor, action),
        _ => return Err("not in targeting phase".to_string()),
    };

    let sim = gs.sim.as_mut().ok_or("no simulation loaded")?;

    let hovered = TileXY::new(gs.hovered_tile_x, gs.hovered_tile_y);

    let actor_at_tile = sim
        .actors
        .iter()
        .find(|(_, actor)| actor.position == hovered)
        .map(|(id, actor)| (*id, actor.alive, is_ally(actor)));
    let enemy = || {
        actor_at_tile
            .filter(|(_, alive, ally)| *alive && !*ally)
            .map(|(id, _, _)| id)
            .ok_or_else(|| "No living enemy at the selected tile".to_string())
    };
    let ally = || {
        actor_at_tile
            .filter(|(_, alive, ally)| *alive && *ally)
            .map(|(id, _, _)| id)
            .ok_or_else(|| "No living ally at the selected tile".to_string())
    };
    let any_actor = || {
        actor_at_tile
            .map(|(id, _, _)| id)
            .ok_or_else(|| "No actor at the selected tile".to_string())
    };

    let action = match player_action {
        PlayerAction::SnapShot => Action::SnapShot(enemy()?),
        PlayerAction::AimedShot => Action::AimedShot(enemy()?),
        PlayerAction::CalledShot(loc) => Action::CalledShot(enemy()?, loc),
        PlayerAction::FanHammer => Action::FanHammer(enemy()?),
        PlayerAction::Volley => Action::Volley(enemy()?),
        PlayerAction::LeftHandDraw => Action::LeftHandDraw(enemy()?),
        PlayerAction::Melee => Action::Melee(enemy()?),
        PlayerAction::Bandage => Action::Bandage(ally()?),
        PlayerAction::Rally => Action::Rally(ally()?),
        PlayerAction::Loot => Action::Loot(any_actor()?),
        PlayerAction::ThrowDynamite => Action::ThrowDynamite(hovered),
        PlayerAction::CatchDynamite => Action::CatchDynamite(hovered),
        PlayerAction::RethrowDynamite => Action::RethrowDynamite(hovered),
        _ => return Err("action does not use a tile target".to_string()),
    };

    let cmd = Command { actor_id, action };

    if matches!(
        player_action,
        PlayerAction::SnapShot
            | PlayerAction::AimedShot
            | PlayerAction::CalledShot(_)
            | PlayerAction::FanHammer
            | PlayerAction::Volley
            | PlayerAction::LeftHandDraw
    ) {
        if let Some(ref audio) = gs.audio {
            audio.play(pb_audio::Sfx::PistolShot);
        }
    }

    // Execute via sim step
    let events = step(sim, cmd).map_err(|e| format!("action failed: {e:?}"))?;
    gs.battle_events.extend(events.iter().cloned());

    // Play hit/miss/death sounds from events
    play_sfx_from_events(&mut gs.audio, &events);

    // Log events to console and update message
    for ev in &events {
        println!("{}", ev);
    }

    // Build a summary message from events
    let summary = events
        .iter()
        .map(|e| format!("{}", e))
        .collect::<Vec<_>>()
        .join("; ");
    gs.message = if summary.is_empty() {
        format!(
            "Action executed (AP remaining: {:?})",
            sim.actors.get(&actor_id).map(|a| a.ap.0).unwrap_or(0)
        )
    } else {
        summary
    };

    gs.phase = InteractionPhase::Executing;
    gs.tick = sim.tick.0;

    Ok(())
}

fn execute_move_to(
    gs: &mut GameState,
    actor_id: ActorId,
    target: TileXY,
    sprint: bool,
) -> Result<(), String> {
    let sim = gs.sim.as_mut().ok_or("no simulation loaded")?;
    let action = if sprint {
        Action::Sprint(target)
    } else {
        Action::Move(target)
    };
    let events = step(sim, Command { actor_id, action })
        .map_err(|error| format!("Move failed: {error:?}"))?;
    gs.battle_events.extend(events.iter().cloned());
    if let Some(audio) = &gs.audio {
        audio.play(pb_audio::Sfx::Move);
    }
    gs.message = events
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ");
    gs.tick = sim.tick.0;
    Ok(())
}

/// Execute a non-targeted player action (Hold, Reload) directly from the
/// SelectedActor phase.
pub fn execute_immediate_action(gs: &mut GameState, action: PlayerAction) -> Result<(), String> {
    let actor_id = match gs.phase {
        InteractionPhase::SelectedActor(id) => id,
        _ => return Err("no actor selected".to_string()),
    };

    let sim = gs.sim.as_mut().ok_or("no simulation loaded")?;

    let sim_action = match action {
        PlayerAction::Hold => Action::Hold,
        PlayerAction::Reload => {
            // Play reload sound
            if let Some(ref audio) = gs.audio {
                audio.play(pb_audio::Sfx::Reload);
            }
            Action::Reload
        }
        PlayerAction::Crouch => Action::StanceCrouch,
        PlayerAction::Prone => {
            if sim
                .actors
                .get(&actor_id)
                .is_some_and(|actor| actor.stance == pb_sim::state::Stance::Prone)
            {
                Action::RiseFromProne
            } else {
                Action::StanceProne
            }
        }
        PlayerAction::DrawBead => Action::DrawBead(actor_id),
        PlayerAction::UseItem => Action::UseItem,
        PlayerAction::CapAndBallReload => Action::CapAndBallReload,
        PlayerAction::ClearJam => Action::ClearJam,
        _ => return Err("not an immediate action".to_string()),
    };

    let cmd = Command {
        actor_id,
        action: sim_action,
    };

    let events = step(sim, cmd).map_err(|e| format!("action failed: {e:?}"))?;
    gs.battle_events.extend(events.iter().cloned());

    let summary = events
        .iter()
        .map(|e| format!("{}", e))
        .collect::<Vec<_>>()
        .join("; ");
    gs.message = format!(
        "{:?} done. {}",
        action,
        if summary.is_empty() {
            format!(
                "AP remaining: {}",
                sim.actors.get(&actor_id).map(|a| a.ap.0).unwrap_or(0)
            )
        } else {
            summary
        }
    );

    check_victory_conditions(gs);
    if gs.screen != crate::state::GameScreen::Battle {
        return Ok(());
    }
    let player_continues = gs
        .sim
        .as_ref()
        .is_some_and(|sim| sim.active_actor == Some(actor_id));
    if player_continues {
        gs.phase = InteractionPhase::SelectedActor(actor_id);
    } else {
        gs.phase = InteractionPhase::Executing;
        run_enemy_ai(gs)?;
        check_victory_conditions(gs);
    }

    Ok(())
}

/// Play sound effects based on simulation events.
fn play_sfx_from_events(audio: &mut Option<pb_audio::AudioSystem>, events: &[Event]) {
    let Some(ref audio) = *audio else { return };
    for ev in events {
        match ev {
            Event::ShotHit { hit: true, .. } => audio.play(pb_audio::Sfx::Hit),
            Event::ShotHit { hit: false, .. } => audio.play(pb_audio::Sfx::Miss),
            Event::ActorKilled { .. } => audio.play(pb_audio::Sfx::Death),
            _ => {}
        }
    }
}

/// Run AI for all alive enemies in the sim.
///
/// Uses `pb_ai::utility::decide_action` to generate, score, and select the
/// best action for each enemy. AI target IDs (1-based slice indices) are
/// mapped back to real simulation `ActorId`s via `resolve_ai_target_id`.
/// Falls back to `Hold` if no player targets exist or if the action fails.
pub fn run_enemy_ai(gs: &mut GameState) -> Result<(), String> {
    let timer = Instant::now();
    let mut commands = 0_u16;

    loop {
        commands = commands.saturating_add(1);
        if commands > 1_024 {
            return Err(
                "E-SIM-AI-TURN: exceeded 1024 commands without reaching a player".to_string(),
            );
        }

        let (actor_id, actor, player_states, enemy_side) = {
            let sim = gs.sim.as_mut().ok_or("no simulation loaded")?;
            let Some(actor_id) = advance_to_next_actor(sim) else {
                break;
            };
            let actor = sim
                .actors
                .get(&actor_id)
                .cloned()
                .ok_or_else(|| format!("active actor {} is absent", actor_id.0))?;
            if !is_enemy(&actor) {
                break;
            }
            let player_states: Vec<(ActorId, ActorState)> = sim
                .actors
                .iter()
                .filter(|(_, candidate)| candidate.alive && is_ally(candidate))
                .map(|(id, candidate)| (*id, candidate.clone()))
                .collect();
            let enemy_side: Vec<ActorState> = sim
                .actors
                .iter()
                .filter(|(id, candidate)| {
                    candidate.alive && **id != actor_id && is_enemy(candidate)
                })
                .map(|(_, candidate)| candidate.clone())
                .collect();
            (actor_id, actor, player_states, enemy_side)
        };

        let player_only: Vec<ActorState> = player_states
            .iter()
            .map(|(_, actor)| actor.clone())
            .collect();
        let mut command = if player_only.is_empty() {
            Command {
                actor_id,
                action: Action::Hold,
            }
        } else {
            pb_ai::utility::decide_action(actor_id, &actor, &enemy_side, &player_only)
        };
        command.action = resolve_ai_target_id(command.action, &player_states);
        let is_fire = matches!(
            command.action,
            Action::SnapShot(_) | Action::AimedShot(_) | Action::CalledShot(..)
        );

        let events = {
            let sim = gs.sim.as_mut().ok_or("no simulation loaded")?;
            match step(sim, command) {
                Ok(events) => events,
                Err(error) => {
                    println!(
                        "[AI {}] action failed ({error:?}), falling back to Hold",
                        actor.name
                    );
                    let fallback = if matches!(error, SimError::MustRetreat(_)) {
                        pb_sim::action::choose_retreat_tile(sim, actor_id)
                            .map(Action::Move)
                            .unwrap_or(Action::Hold)
                    } else {
                        Action::Hold
                    };
                    step(
                        sim,
                        Command {
                            actor_id,
                            action: fallback,
                        },
                    )
                    .map_err(|hold_error| {
                        format!(
                            "E-SIM-AI-TURN: {} could neither act ({error:?}) nor fall back ({hold_error:?})",
                            actor.name
                        )
                    })?
                }
            }
        };
        gs.battle_events.extend(events.iter().cloned());

        for event in &events {
            println!("[AI {}] {}", actor.name, event);
        }
        if let Some(ref audio) = gs.audio {
            if is_fire {
                audio.play(pb_audio::Sfx::RifleShot);
            }
            for event in &events {
                match event {
                    Event::ShotHit { hit: true, .. } => audio.play(pb_audio::Sfx::Hit),
                    Event::ShotHit { hit: false, .. } => audio.play(pb_audio::Sfx::Miss),
                    Event::ActorKilled { .. } => audio.play(pb_audio::Sfx::Death),
                    _ => {}
                }
            }
        }

        if player_states.is_empty() {
            break;
        }
    }

    if let Some(sim) = gs.sim.as_ref() {
        gs.tick = sim.tick.0;
    }
    gs.phase = InteractionPhase::Idle;

    // Record AI turn timing
    let elapsed_ms = match u64::try_from(timer.elapsed().as_millis()) {
        Ok(value) => value,
        Err(_) => u64::MAX,
    };
    MetricsRegistry::global().record_ai_turn(elapsed_ms);

    // Update combat metrics (alive actors, XP total)
    update_combat_metrics(gs);

    Ok(())
}

/// Update the `sim.actors.alive` and `progression.xp.total` metric gauges
/// based on the current game state simulation.
fn update_combat_metrics(gs: &GameState) {
    let Some(ref sim) = gs.sim else { return };
    let alive_count = sim.actors.values().filter(|a| a.alive).count() as u64;
    let total_xp: u64 = sim.actors.values().map(|a| a.progression.xp).sum();
    let registry = MetricsRegistry::global();
    registry.set_sim_actors_alive(alive_count);
    registry.set_progression_xp_total(total_xp);
}

/// Check win/lose conditions.
pub fn check_victory_conditions(gs: &mut GameState) {
    let Some(ref sim) = gs.sim else { return };

    let allies_alive = sim
        .actors
        .values()
        .filter(|a| a.alive && is_ally(a))
        .count();
    let enemies_alive = sim
        .actors
        .values()
        .filter(|a| a.alive && is_enemy(a))
        .count();

    if enemies_alive == 0 {
        if let Some(ref audio) = gs.audio {
            audio.play(pb_audio::Sfx::Victory);
        }
        gs.message = "🎉 Victory! All enemies eliminated.".to_string();
        gs.last_victory = Some(true);
        gs.phase = InteractionPhase::Idle;
        gs.screen = crate::state::GameScreen::AfterAction;
    } else if allies_alive == 0 {
        gs.message = "💀 Defeat! All allies have fallen.".to_string();
        gs.last_victory = Some(false);
        gs.phase = InteractionPhase::Idle;
        gs.screen = crate::state::GameScreen::AfterAction;
    }
}

/// Map ActorIds returned by `pb_ai::utility::decide_action` (1-based indices
/// into the enemies slice) back to real ActorIds from the simulation state.
fn resolve_ai_target_id(action: Action, player_states: &[(ActorId, ActorState)]) -> Action {
    // Given a fake 1-based index from the AI, find the real ActorId.
    let real_id = |fake: ActorId| -> ActorId {
        let idx = fake.0.saturating_sub(1) as usize;
        player_states
            .get(idx)
            .map(|(real, _)| *real)
            .unwrap_or(fake)
    };

    match action {
        Action::SnapShot(fake) => Action::SnapShot(real_id(fake)),
        Action::AimedShot(fake) => Action::AimedShot(real_id(fake)),
        Action::CalledShot(fake, loc) => Action::CalledShot(real_id(fake), loc),
        Action::Melee(fake) => Action::Melee(real_id(fake)),
        Action::Bandage(fake) => Action::Bandage(real_id(fake)),
        other => other,
    }
}

/// Compatibility entry point for one enemy decision.
#[allow(clippy::too_many_arguments)]
pub fn run_ai_step(game_state: &mut GameState) -> Result<(), String> {
    run_enemy_ai(game_state)
}

// ── Public hit chance breakdown ───────────────────────────────────────────

/// Compute the hit chance breakdown for the selected actor vs a hovered target.
pub fn compute_hit_chance_for_hover(game_state: &GameState) -> Option<HitChanceBreakdown> {
    let sim = game_state.sim.as_ref()?;
    let (actor_id, aimed, called) = match game_state.phase {
        InteractionPhase::Targeting { actor, action } => {
            let aimed = matches!(action, PlayerAction::AimedShot);
            let called = match action {
                PlayerAction::CalledShot(loc) => Some(loc),
                _ => None,
            };
            (actor, aimed, called)
        }
        _ => return None,
    };
    let hovered = TileXY::new(game_state.hovered_tile_x, game_state.hovered_tile_y);
    let (target_id, _) = sim
        .actors
        .iter()
        .find(|(_, a)| a.position == hovered && a.alive && !is_ally(a))?;
    compute_hit_chance_breakdown(sim, actor_id, *target_id, aimed, called, 0).ok()
}

// ── Internal rendering helpers ────────────────────────────────────────────

/// Build per-tile visuals from the simulation state.
fn build_tile_visuals(game_state: &GameState) -> Vec<TileVisual> {
    let mut tiles = Vec::with_capacity((GRID_COLS * GRID_ROWS) as usize);

    for y in 0..GRID_ROWS {
        for x in 0..GRID_COLS {
            let tile_position = TileXY::new(x as i16, y as i16);
            let alive_actor_at = game_state.sim.as_ref().is_some_and(|sim| {
                sim.actors
                    .iter()
                    .any(|(_, a)| a.position.x == x as i16 && a.position.y == y as i16 && a.alive)
            });

            let dead_actor_at = game_state.sim.as_ref().is_some_and(|sim| {
                sim.actors
                    .iter()
                    .any(|(_, a)| a.position.x == x as i16 && a.position.y == y as i16 && !a.alive)
            });

            let elevation = game_state
                .sim
                .as_ref()
                .and_then(|sim| sim.tile_elevations.get(&tile_position).copied())
                .unwrap_or(0);
            let material = game_state
                .sim
                .as_ref()
                .and_then(|sim| sim.terrain_tiles.get(&tile_position))
                .map_or(0, |terrain| pb_render::tiles::material_for_terrain(terrain));

            let (r, g, b) = if dead_actor_at {
                (0.90, 0.56, 0.52)
            } else if alive_actor_at {
                (0.78, 0.98, 0.78)
            } else {
                let shade = 0.92 + ((x + y) % 3) as f32 * 0.025;
                (shade, shade, shade)
            };

            tiles.push(TileVisual::new(r, g, b, elevation).with_material(material));
        }
    }

    tiles
}

/// Build a smoke density grid from the simulation state.
fn build_smoke_grid(game_state: &GameState) -> Vec<SmokeTile> {
    let Some(ref sim) = game_state.sim else {
        return vec![SmokeTile::new(0); (GRID_COLS * GRID_ROWS) as usize];
    };
    let mut tiles = Vec::with_capacity((GRID_COLS * GRID_ROWS) as usize);
    for y in 0..GRID_ROWS {
        for x in 0..GRID_COLS {
            let idx = y as usize * sim.smoke_cols as usize + x as usize;
            let density = sim.smoke_grid.get(idx).copied().unwrap_or(0);
            tiles.push(SmokeTile::new(density));
        }
    }
    tiles
}

/// Build overlay tile highlights based on hovered tile and selected actor.
fn build_overlay_tiles(game_state: &GameState) -> Vec<(u32, u32, OverlayTileKind)> {
    let mut overlays = Vec::new();

    let hx = game_state.hovered_tile_x.max(0).min(GRID_COLS as i16 - 1) as u32;
    let hy = game_state.hovered_tile_y.max(0).min(GRID_ROWS as i16 - 1) as u32;
    let hovered_kind = if let Some(preview) = movement_preview(game_state) {
        if preview.crosses_overwatch {
            OverlayTileKind::Overwatch {
                ap_cost: preview.ap_cost,
            }
        } else if preview.leaves_cover {
            OverlayTileKind::LeavesCover {
                ap_cost: preview.ap_cost,
            }
        } else {
            OverlayTileKind::Movable {
                ap_cost: preview.ap_cost,
            }
        }
    } else if let Some(hit) = compute_hit_chance_for_hover(game_state) {
        OverlayTileKind::Attackable {
            hit_chance: u8::try_from(hit.total.clamp(0, 100)).unwrap_or(0),
        }
    } else {
        OverlayTileKind::Movable { ap_cost: 0 }
    };
    overlays.push((hx, hy, hovered_kind));

    // Highlight selected actor's tile
    if let InteractionPhase::SelectedActor(id) = game_state.phase {
        if let Some(ref sim) = game_state.sim {
            if let Some(actor) = sim.actors.get(&id) {
                let ax = actor.position.x.max(0).min(GRID_COLS as i16 - 1) as u32;
                let ay = actor.position.y.max(0).min(GRID_ROWS as i16 - 1) as u32;
                if ax != hx || ay != hy {
                    overlays.push((ax, ay, OverlayTileKind::Movable { ap_cost: 0 }));
                }
            }
        }
    }

    overlays
}

/// Build sprite instances from all actors in the simulation.
fn build_sprite_instances(game_state: &GameState) -> Vec<SpriteInstance> {
    let Some(ref sim) = game_state.sim else {
        return Vec::new();
    };

    let selected_id = match game_state.phase {
        InteractionPhase::SelectedActor(id) => Some(id),
        InteractionPhase::Targeting { actor, .. } => Some(actor),
        _ => None,
    };

    let mut sprites = Vec::with_capacity(sim.actors.len());

    for (id, actor) in &sim.actors {
        let half_w = TILE_W * 0.5;
        let half_h = TILE_H * 0.5;
        let wx = (actor.position.x as f32 - actor.position.y as f32) * half_w;
        let wy = (actor.position.x as f32 + actor.position.y as f32) * half_h;
        let z = if actor.alive { 2.0 } else { 0.5 };

        let mut sprite = SpriteInstance::new(wx, wy, z);
        sprite.width = 58.0;
        sprite.height = 82.0;
        sprite.y -= 24.0;
        sprite.set_character(id.0);

        if !actor.alive {
            sprite.r = 0.5;
            sprite.g = 0.24;
            sprite.b = 0.2;
            sprite.a = 0.48;
        } else {
            match actor.stance {
                Stance::Standing => {}
                Stance::Crouched => {
                    sprite.height *= 0.82;
                    sprite.y -= 5.0;
                }
                Stance::Prone => {
                    sprite.height *= 0.58;
                    sprite.width *= 1.18;
                    sprite.y -= 12.0;
                }
            }
            if Some(*id) == selected_id {
                sprite.r = 1.0;
                sprite.g = 1.0;
                sprite.b = 0.72;
                sprite.a = 1.0;
            } else if is_enemy(actor) {
                sprite.r = 1.0;
                sprite.g = 0.82;
                sprite.b = 0.78;
                sprite.a = 1.0;
            } else {
                sprite.r = 0.90;
                sprite.g = 0.96;
                sprite.b = 1.0;
                sprite.a = 1.0;
            }
        }

        sprites.push(sprite);
    }

    sprites
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod coverage_tests {
    use super::*;
    use crate::state::GameScreen;
    use std::path::PathBuf;

    fn content_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("content")
    }

    fn authored_battle() -> GameState {
        let root = content_root();
        let mut state = GameState::new();
        crate::campaign::start_new(&mut state, &root, 90210).expect("new campaign");
        let mission =
            crate::campaign::select_next(&mut state, &root).expect("opening mission selected");
        assert_eq!(mission, "m01_elk_creek");
        state.screen = GameScreen::Battle;
        init_combat(&mut state, &root).expect("combat initialized");
        state
    }

    fn active_ally(state: &GameState) -> ActorId {
        state
            .sim
            .as_ref()
            .and_then(|sim| sim.active_actor)
            .expect("active actor")
    }

    #[test]
    fn authored_combat_initializes_every_presentation_projection() {
        let mut state = authored_battle();
        let sim = state.sim.as_ref().expect("simulation");
        assert!(sim.actors.values().any(is_ally));
        assert!(sim.actors.values().any(is_enemy));
        assert!(!build_tile_visuals(&state).is_empty());
        assert_eq!(build_sprite_instances(&state).len(), sim.actors.len());
        assert!(!build_smoke_grid(&state).is_empty());
        assert!(!build_overlay_tiles(&state).is_empty());

        select_squad_member(&mut state, 0).expect("select first squad member");
        assert!(matches!(state.phase, InteractionPhase::SelectedActor(_)));
        let selected = active_ally(&state);
        state.phase = InteractionPhase::SelectedActor(selected);
        cycle_target(&mut state).expect("cycle target");
        let _ = compute_hit_chance_for_hover(&state);
        rotate_selected_facing(&mut state, true).expect("rotate clockwise");
        rotate_selected_facing(&mut state, false).expect("rotate counter-clockwise");
        let actor = state
            .sim
            .as_ref()
            .and_then(|sim| sim.actors.get(&selected))
            .expect("selected actor");
        state.hovered_tile_x = actor.position.x.saturating_add(1);
        state.hovered_tile_y = actor.position.y;
        assert!(movement_preview(&state).is_some());
        let _ = screen_to_tile(
            640.0,
            360.0,
            state.camera_x,
            state.camera_y,
            state.camera_zoom,
            1280.0,
            720.0,
        );
    }

    #[test]
    fn click_targeting_and_immediate_actions_drive_the_real_kernel() {
        let mut state = authored_battle();
        let actor_id = active_ally(&state);
        state.phase = InteractionPhase::SelectedActor(actor_id);
        execute_immediate_action(&mut state, PlayerAction::Crouch).expect("crouch");
        state.phase = InteractionPhase::SelectedActor(actor_id);
        execute_immediate_action(&mut state, PlayerAction::Prone).expect("prone");
        state.phase = InteractionPhase::SelectedActor(actor_id);
        execute_immediate_action(&mut state, PlayerAction::Prone).expect("rise");

        let mut state = authored_battle();
        let actor_id = active_ally(&state);
        let (enemy_id, target_position, actor_position) = {
            let sim = state.sim.as_ref().expect("simulation");
            let enemy_id = sim
                .actors
                .iter()
                .find_map(|(id, actor)| is_enemy(actor).then_some(*id))
                .expect("enemy");
            (
                enemy_id,
                sim.actors.get(&enemy_id).expect("enemy actor").position,
                sim.actors.get(&actor_id).expect("active actor").position,
            )
        };
        {
            let sim = state.sim.as_mut().expect("simulation");
            sim.actors
                .retain(|id, _| *id == actor_id || *id == enemy_id);
            sim.sequence_clock
                .retain(|id, _| *id == actor_id || *id == enemy_id);
            sim.active_actor = Some(actor_id);
            sim.actors.get_mut(&actor_id).expect("active actor").ap.0 = 20;
            let enemy = sim.actors.get_mut(&enemy_id).expect("enemy actor");
            enemy.position = actor_position.neighbour(pb_core::geom::Facing::East);
            enemy.alive = true;
            enemy.hit_points = enemy.max_hp;
        }
        state.phase = InteractionPhase::Targeting {
            actor: actor_id,
            action: PlayerAction::SnapShot,
        };
        let enemy_position = state
            .sim
            .as_ref()
            .and_then(|sim| sim.actors.get(&enemy_id))
            .expect("enemy")
            .position;
        state.hovered_tile_x = enemy_position.x;
        state.hovered_tile_y = enemy_position.y;
        execute_player_action(&mut state).expect("snap shot");
        assert!(!state.battle_events.is_empty());

        state.phase = InteractionPhase::Idle;
        state.hovered_tile_x = target_position.x;
        state.hovered_tile_y = target_position.y;
        handle_combat_click(&mut state).expect("enemy click handled");
        assert!(!state.message.is_empty());
    }

    #[test]
    fn hold_runs_enemy_ai_and_victory_moves_to_after_action() {
        let mut state = authored_battle();
        let actor_id = active_ally(&state);
        state.phase = InteractionPhase::SelectedActor(actor_id);
        execute_immediate_action(&mut state, PlayerAction::Hold).expect("hold and enemy AI");
        assert!(!state.message.is_empty());

        {
            let sim = state.sim.as_mut().expect("simulation");
            for actor in sim.actors.values_mut().filter(|actor| is_enemy(actor)) {
                actor.alive = false;
                actor.hit_points = 0;
            }
        }
        state.screen = GameScreen::Battle;
        check_victory_conditions(&mut state);
        assert_eq!(state.screen, GameScreen::AfterAction);
        assert_eq!(state.last_victory, Some(true));
    }
}

#[cfg(test)]
mod turn_tests {
    use super::*;
    use pb_sim::clock::{build_actor, register_actor};

    #[test]
    fn screen_to_tile_accounts_for_pan_and_zoom() {
        let tile = screen_to_tile(787.2, 432.0, 128.0, 240.0, 1.35, 1920.0, 1080.0);
        assert_eq!(tile, TileXY::new(5, 5));
    }

    #[test]
    fn squad_number_selection_uses_stable_company_order() {
        let mut game = GameState::new();
        let mut sim = SimState::new(42, 1);
        for (id, name, faction) in [
            (ActorId(8), "Second", "player"),
            (ActorId(3), "Enemy", "enemy"),
            (ActorId(2), "First", "player"),
        ] {
            let mut actor = build_actor(id, name, 5, 100, 20, TileXY::new(id.0 as i16, 2));
            actor.faction_id = faction.to_string();
            sim.actors.insert(id, actor);
        }
        sim.active_actor = Some(ActorId(2));
        game.sim = Some(sim);

        assert!(select_squad_member(&mut game, 1).is_ok());
        assert_eq!(game.phase, InteractionPhase::SelectedActor(ActorId(8)));
        assert!(game.message.contains("Inspecting Second"));
    }

    #[test]
    fn tab_target_cycles_living_enemies_and_wraps() {
        let mut game = GameState::new();
        let mut sim = SimState::new(42, 1);
        for (id, name, position) in [
            (ActorId(2), "First Enemy", TileXY::new(4, 2)),
            (ActorId(7), "Second Enemy", TileXY::new(7, 2)),
        ] {
            let mut actor = build_actor(id, name, 5, 100, 20, position);
            actor.faction_id = "enemy".to_string();
            sim.actors.insert(id, actor);
        }
        game.hovered_tile_x = 4;
        game.hovered_tile_y = 2;
        game.sim = Some(sim);

        assert!(cycle_target(&mut game).is_ok());
        assert_eq!((game.hovered_tile_x, game.hovered_tile_y), (7_i16, 2_i16));
        assert!(cycle_target(&mut game).is_ok());
        assert_eq!((game.hovered_tile_x, game.hovered_tile_y), (4_i16, 2_i16));
    }

    #[test]
    fn q_e_rotation_uses_sim_step_and_spends_no_ap() {
        let mut game = GameState::new();
        let mut sim = SimState::new(42, 1);
        let id = ActorId(1);
        let mut actor = build_actor(id, "Player", 5, 100, 20, TileXY::new(2, 2));
        actor.faction_id = "player".to_string();
        actor.facing = pb_core::geom::Facing::South;
        actor.ap = pb_core::ids::Ap(6);
        sim.actors.insert(id, actor);
        sim.active_actor = Some(id);
        game.sim = Some(sim);
        game.phase = InteractionPhase::SelectedActor(id);

        assert!(rotate_selected_facing(&mut game, true).is_ok());
        let Some(sim) = game.sim.as_ref() else {
            panic!("rotation must retain simulation state");
        };
        let actor = &sim.actors[&id];
        assert_eq!(actor.facing, pb_core::geom::Facing::SouthWest);
        assert_eq!(actor.ap, pb_core::ids::Ap(6));
        assert!(game
            .battle_events
            .iter()
            .any(|event| matches!(event, Event::FacingChanged { actor, .. } if *actor == id)));
    }

    #[test]
    fn movement_preview_shares_cost_cover_and_overwatch_rules() {
        let mut game = GameState::new();
        let mut sim = SimState::new(42, 1);
        let player_id = ActorId(1);
        let watcher_id = ActorId(2);
        let destination = TileXY::new(3, 2);

        let mut player = build_actor(player_id, "Player", 5, 100, 20, TileXY::new(2, 2));
        player.faction_id = "player".to_string();
        let mut watcher = build_actor(watcher_id, "Watcher", 5, 100, 20, TileXY::new(5, 2));
        watcher.faction_id = "enemy".to_string();
        watcher.facing = pb_core::geom::Facing::West;
        sim.actors.insert(player_id, player);
        sim.actors.insert(watcher_id, watcher);
        sim.difficult_tiles.insert(destination);
        sim.cover_edges.insert(
            pb_sim::state::CoverEdge {
                tile: TileXY::new(2, 2),
                facing: pb_core::geom::Facing::North,
            },
            pb_sim::state::CoverState {
                level: pb_sim::state::CoverLevel::Hard,
                strikes: 0,
                half_height: false,
                burning: false,
            },
        );
        sim.overwatch.insert(watcher_id);
        sim.reaction_points.insert(watcher_id, 3);
        game.sim = Some(sim);
        game.phase = InteractionPhase::SelectedActor(player_id);
        game.hovered_tile_x = destination.x;
        game.hovered_tile_y = destination.y;

        assert_eq!(
            movement_preview(&game),
            Some(MovementPreview {
                ap_cost: 2,
                leaves_cover: true,
                crosses_overwatch: true,
            })
        );
    }

    #[test]
    fn enemy_ai_finishes_its_ap_turn_before_yielding_to_player() {
        let mut game = GameState::new();
        let mut sim = SimState::new(42, 1);
        let player_id = ActorId(1);
        let enemy_id = ActorId(2);

        let mut player = build_actor(player_id, "Player", 5, 1_000, 20, TileXY::new(2, 2));
        player.faction_id = "player".to_string();
        let mut enemy = build_actor(enemy_id, "Enemy", 9, 100, 20, TileXY::new(5, 2));
        enemy.faction_id = "enemy".to_string();
        register_actor(&mut sim, player_id, player);
        register_actor(&mut sim, enemy_id, enemy);
        assert_eq!(advance_to_next_actor(&mut sim), Some(enemy_id));
        game.sim = Some(sim);

        let result = run_enemy_ai(&mut game);
        assert!(result.is_ok(), "AI turn failed: {result:?}");
        let sim = game.sim.as_ref();
        assert!(sim.is_some());
        if let Some(sim) = sim {
            assert_eq!(sim.active_actor, Some(player_id));
            assert_eq!(sim.actors[&enemy_id].ap.0, 0);
            assert_eq!(sim.tick.0, 0);
        }
    }
}
