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

use pb_content::load;
use pb_content::schema::Content;
use pb_core::geom::TileXY;
use pb_core::ids::ActorId;
use pb_render::camera::IsoCamera;
use pb_render::device::RenderDevice;
use pb_render::overlay::{OverlaySystem, OverlayTileKind};
use pb_render::smoke::{SmokeSystem, SmokeTile};
use pb_render::sprites::{SpriteInstance, SpriteSystem};
use pb_render::tiles::{TileSystem, TileVisual};
use pb_sim::action::{step, Action, Command};
use pb_sim::clock::advance_to_next_actor;
use pb_sim::state::{ActorState, SimState, Stance};

use crate::state::{GameScreen, GameState, InteractionPhase, PlayerAction};
use pb_core::event::Event;

// ── Constants ─────────────────────────────────────────────────────────────

/// Default battlefield grid dimensions.
const GRID_COLS: u32 = 20;
const GRID_ROWS: u32 = 12;

/// Isometric tile dimensions in pixels (at zoom = 1.0).
const TILE_W: f32 = 64.0;
const TILE_H: f32 = 32.0;

// ── Faction helpers ───────────────────────────────────────────────────────

/// Return true if the actor's name indicates it is an ally (player side).
pub fn is_ally(actor: &ActorState) -> bool {
    actor.name.starts_with("e_ally_")
        || actor.name.starts_with("c_")
        || (actor.name.contains("ally") && !actor.name.contains("enemy"))
}

/// Return true if the actor's name indicates it is an enemy.
pub fn is_enemy(actor: &ActorState) -> bool {
    actor.name.starts_with("e_enemy_")
        || actor.name.contains("enemy")
        || actor.name.contains("Enemy")
}

// ── Coordinate conversion ─────────────────────────────────────────────────

/// Convert screen pixel coordinates to tile coordinates using the isometric
/// projection inverse.
pub fn screen_to_tile(
    mouse_x: f64,
    mouse_y: f64,
    camera_x: f32,
    camera_y: f32,
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
    let sx = mouse_x as f32 - viewport_width * 0.5 + camera_x;
    let sy = mouse_y as f32 - viewport_height * 0.5 + camera_y;

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

// ── Public API ────────────────────────────────────────────────────────────

/// Initialize combat: load content, create SimState, put game into combat
/// screen.
pub fn init_combat(
    game_state: &mut GameState,
    content_root: &Path,
) -> Result<(), String> {
    // Load content
    let content: Content = load::load_all(content_root)
        .map_err(|e| format!("content load failed: {e}"))?;

    // Pick the first scenario (or m01_elk_creek if available)
    let scenario_id = if content.scenarios.contains_key("scn_m01_elk_creek") {
        "scn_m01_elk_creek"
    } else {
        content
            .scenarios
            .keys()
            .next()
            .ok_or_else(|| "no scenarios in content".to_string())?
            .as_str()
    };

    let scenario = content
        .scenarios
        .get(scenario_id)
        .ok_or_else(|| format!("scenario {scenario_id} not found"))?;

    // Build SimState
    let seed: u64 = 42; // deterministic for now
    let mut sim = SimState::new(seed, 1);

    // Register actors
    let mut next_id = 1u32;
    for actor_data in &scenario.actors {
        let actor_id = ActorId(next_id);
        next_id += 1;

        let ap_val = if actor_data.ap > 0 {
            actor_data.ap
        } else {
            actor_data.ap_max
        };

        let actor_state = ActorState {
            ap: pb_core::ids::Ap(ap_val),
            position: TileXY::new(actor_data.pos.x, actor_data.pos.y),
            facing: pb_core::geom::Facing::from_index(actor_data.facing as usize % 8),
            sequence: actor_data.sequence,
            hit_points: if actor_data.is_dead { 0 } else { actor_data.hp },
            max_hp: actor_data.hp_max,
            name: actor_data.id.clone(),
            alive: !actor_data.is_dead,
            wounds: Vec::new(),
            sand: actor_data.sand,
            max_sand: actor_data.sand_max,
            stance: parse_stance(&actor_data.stance),
        };

        sim.actors.insert(actor_id, actor_state);
        sim.sequence_clock.insert(actor_id, 0);
    }

    // Grant initial AP to the first actor via advance_to_next_actor
    advance_to_next_actor(&mut sim);

    game_state.sim = Some(sim);
    game_state.screen = GameScreen::Combat;
    game_state.phase = InteractionPhase::Idle;
    game_state.tick = 0;
    game_state.message = format!("{} loaded — click an ally to act", scenario.display_name);

    Ok(())
}

/// Render one frame of the combat screen.
///
/// Builds all pb-render systems from scratch each frame, draws them in
/// z-order: tiles → smoke → overlay → sprites.
pub fn render_combat_frame(
    game_state: &GameState,
    render_device: &Arc<RenderDevice>,
    view: &wgpu::TextureView,
    _surface_format: wgpu::TextureFormat,
    viewport_width: u32,
    viewport_height: u32,
) {
    // ── Camera ──────────────────────────────────────────────────────────
    let camera = IsoCamera {
        center_x: game_state.camera_x,
        center_y: game_state.camera_y,
        zoom: 1.0,
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
    let smoke_system =
        SmokeSystem::new(render_device, GRID_COLS, GRID_ROWS, &smoke_tiles, &camera_bytes);

    // ── Overlay (hovered tile, selected actor highlight) ────────────────
    let overlay_tiles = build_overlay_tiles(game_state);
    let overlay_system = OverlaySystem::new(render_device, &overlay_tiles, &camera_bytes);

    // ── Actor sprites ───────────────────────────────────────────────────
    let sprites = build_sprite_instances(game_state);
    let sprite_system = SpriteSystem::new(render_device, &sprites, &camera_bytes);

    // ── Command encoder & render pass ───────────────────────────────────
    let mut encoder = render_device
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

        // Draw in z-order: terrain → smoke → overlay → sprites
        tile_system.render(&mut rpass);
        smoke_system.render(&mut rpass);
        overlay_system.render(&mut rpass);
        sprite_system.render(&mut rpass);
    }

    render_device.queue.submit(std::iter::once(encoder.finish()));
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
                    if actor.alive && is_ally(actor) {
                        // Switch selection to different ally
                        game_state.phase = InteractionPhase::SelectedActor(id);
                        game_state.message = format!(
                            "Selected {} (HP: {}/{}, AP: {})",
                            actor.name, actor.hit_points, actor.max_hp, actor.ap.0
                        );
                    }
                }
            }
            // Clicking empty tile while selected → could add move-to later
        }
        InteractionPhase::Targeting { actor: selected_id, action: player_action } => {
            // Check if the clicked tile has a valid enemy target
            if let Some((&target_id, target_actor)) = actor_at {
                if target_id != selected_id && target_actor.alive && !is_ally(target_actor) {
                    // Valid target! Keep the phase and let execute_player_action read it
                    game_state.message = format!(
                        "Executing {:?} on {}",
                        player_action,
                        target_actor.name
                    );

                    // Execute the player action
                    execute_player_action(game_state)?;

                    // Run enemy AI for all alive enemies
                    run_enemy_ai(game_state)?;

                    // Check win/lose conditions
                    check_victory_conditions(game_state);

                    return Ok(());
                }
            }
            // No valid target — cancel back to selected
            game_state.phase = InteractionPhase::SelectedActor(selected_id);
            game_state.message = "Targeting cancelled".to_string();
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

    // Find the enemy actor at the hovered tile
    let target_id = sim
        .actors
        .iter()
        .find(|(_, a)| a.position == hovered && a.alive && !is_ally(a))
        .map(|(id, _)| *id)
        .ok_or_else(|| "no valid enemy target at hovered tile".to_string())?;

    // Convert player action to sim action
    let action = match player_action {
        PlayerAction::SnapShot => Action::SnapShot(target_id),
        PlayerAction::AimedShot => Action::AimedShot(target_id),
        PlayerAction::CalledShot(loc) => Action::CalledShot(target_id, loc),
        _ => return Err("action cannot target an enemy".to_string()),
    };

    let cmd = Command { actor_id, action };

    // Play pistol shot sound
    if let Some(ref audio) = gs.audio {
        audio.play(pb_audio::Sfx::PistolShot);
    }

    // Execute via sim step
    let events = step(sim, cmd).map_err(|e| format!("action failed: {e:?}"))?;

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
        format!("Action executed (AP remaining: {:?})",
            sim.actors.get(&actor_id).map(|a| a.ap.0).unwrap_or(0))
    } else {
        summary
    };

    gs.phase = InteractionPhase::Executing;
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
        _ => return Err("not an immediate action".to_string()),
    };

    let cmd = Command {
        actor_id,
        action: sim_action,
    };

    let events = step(sim, cmd).map_err(|e| format!("action failed: {e:?}"))?;

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

    // After immediate action, run enemy AI
    gs.phase = InteractionPhase::Executing;
    run_enemy_ai(gs)?;
    check_victory_conditions(gs);

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
/// For each alive enemy, finds the nearest player character and attempts a
/// SnapShot. If that fails (AP/reason), falls back to Hold.
///
/// TODO: Use `pb_ai::utility::decide_action` properly once the ActorId
/// mapping issue is resolved in the AI pipeline (the AI uses 1-based
/// index IDs that don't match sim ActorIds).
pub fn run_enemy_ai(gs: &mut GameState) -> Result<(), String> {
    let sim = gs.sim.as_mut().ok_or("no simulation loaded")?;

    // Collect alive enemy IDs
    let alive_enemy_ids: Vec<ActorId> = sim
        .actors
        .iter()
        .filter(|(_, a)| a.alive && is_enemy(a))
        .map(|(id, _)| *id)
        .collect();

    if alive_enemy_ids.is_empty() {
        gs.message = "No enemies left to act".to_string();
        gs.phase = InteractionPhase::Idle;
        return Ok(());
    }

    for eid in &alive_enemy_ids {
        // Get actor state (clone to avoid borrow issues)
        let actor = match sim.actors.get(eid) {
            Some(a) if a.alive => a.clone(),
            _ => continue,
        };

        // Collect player-side actors (the enemy's "enemies")
        let player_states: Vec<(ActorId, ActorState)> = sim
            .actors
            .iter()
            .filter(|(_, a)| a.alive && !is_enemy(a))
            .map(|(id, a)| (*id, a.clone()))
            .collect();

        // Try to use pb_ai utility AI
        let cmd = if !player_states.is_empty() {
            let enemy_side: Vec<ActorState> = sim
                .actors
                .iter()
                .filter(|(id, a)| a.alive && **id != *eid && is_enemy(a))
                .map(|(_, a)| a.clone())
                .collect();

            let player_only: Vec<ActorState> =
                player_states.iter().map(|(_, a)| a.clone()).collect();

            // Use the AI to decide
            let mut ai_cmd =
                pb_ai::utility::decide_action(&actor, &enemy_side, &player_only);
            ai_cmd.actor_id = *eid;

            // Fix target IDs: map from 1-based index back to real ActorId
            ai_cmd.action = resolve_ai_target_id(ai_cmd.action, &player_states);
            ai_cmd
        } else {
            // No targets — just hold
            Command {
                actor_id: *eid,
                action: Action::Hold,
            }
        };

        // Check if this is a shooting action (before cmd is moved)
        let is_fire = matches!(cmd.action, Action::SnapShot(_) | Action::AimedShot(_) | Action::CalledShot(..));

        // Execute the AI decision
        match step(sim, cmd) {
            Ok(events) => {
                for ev in &events {
                    println!("[AI {}] {}", actor.name, ev);
                }
                // Play AI audio
                if let Some(ref audio) = gs.audio {
                    // Rifle shot for enemy fire
                    if is_fire {
                        audio.play(pb_audio::Sfx::RifleShot);
                    }
                    for ev in &events {
                        match ev {
                            Event::ShotHit { hit: true, .. } => audio.play(pb_audio::Sfx::Hit),
                            Event::ShotHit { hit: false, .. } => audio.play(pb_audio::Sfx::Miss),
                            Event::ActorKilled { .. } => audio.play(pb_audio::Sfx::Death),
                            _ => {}
                        }
                    }
                }
            }
            Err(e) => {
                println!(
                    "[AI {}] action failed ({:?}), falling back to Hold",
                    actor.name, e
                );
                // Fall back to Hold
                let _ = step(
                    sim,
                    Command {
                        actor_id: *eid,
                        action: Action::Hold,
                    },
                );
            }
        }
    }

    // Advance the sequence clock to the next actor
    advance_to_next_actor(sim);

    gs.tick = sim.tick.0;
    gs.phase = InteractionPhase::Idle;

    Ok(())
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
        gs.phase = InteractionPhase::Idle;
        gs.screen = crate::state::GameScreen::AfterAction;
    } else if allies_alive == 0 {
        gs.message = "💀 Defeat! All allies have fallen.".to_string();
        gs.phase = InteractionPhase::Idle;
        gs.screen = crate::state::GameScreen::AfterAction;
    }
}

/// Map ActorIds returned by `pb_ai::utility::decide_action` (1-based indices
/// into the enemies slice) back to real ActorIds from the simulation state.
fn resolve_ai_target_id(
    action: Action,
    player_states: &[(ActorId, ActorState)],
) -> Action {
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

/// Run one AI decision for an enemy actor (legacy stub — use run_enemy_ai).
pub fn run_ai_step(game_state: &mut GameState) -> Result<(), String> {
    run_enemy_ai(game_state)
}

// ── Internal rendering helpers ────────────────────────────────────────────

/// Build per-tile visuals from the simulation state.
fn build_tile_visuals(game_state: &GameState) -> Vec<TileVisual> {
    let mut tiles = Vec::with_capacity((GRID_COLS * GRID_ROWS) as usize);

    let actor_positions: Vec<(i16, i16)> = game_state
        .sim
        .as_ref()
        .map(|sim| {
            sim.actors
                .iter()
                .map(|(_, a)| (a.position.x, a.position.y))
                .collect()
        })
        .unwrap_or_default();

    for y in 0..GRID_ROWS {
        for x in 0..GRID_COLS {
            let actor_at = actor_positions
                .iter()
                .any(|&(ax, ay)| ax == x as i16 && ay == y as i16);

            let alive_actor_at = game_state.sim.as_ref().is_some_and(|sim| {
                sim.actors
                    .iter()
                    .any(|(_, a)| a.position.x == x as i16 && a.position.y == y as i16 && a.alive)
            });

            let dead_actor_at = game_state.sim.as_ref().is_some_and(|sim| {
                sim.actors.iter().any(|(_, a)| {
                    a.position.x == x as i16 && a.position.y == y as i16 && !a.alive
                })
            });

            let elevation: i32 = if actor_at { 1 } else { 0 };

            let (r, g, b) = if dead_actor_at {
                (0.45, 0.20, 0.15)
            } else if alive_actor_at {
                (0.35, 0.55, 0.25)
            } else {
                let shade = 0.30 + ((x + y) % 3) as f32 * 0.10;
                (0.25, shade, 0.20)
            };

            tiles.push(TileVisual::new(r, g, b, elevation));
        }
    }

    tiles
}

/// Build a smoke density grid from the simulation (currently a stub).
fn build_smoke_grid(game_state: &GameState) -> Vec<SmokeTile> {
    let _ = game_state;
    vec![SmokeTile::new(0); (GRID_COLS * GRID_ROWS) as usize]
}

/// Build overlay tile highlights based on hovered tile and selected actor.
fn build_overlay_tiles(game_state: &GameState) -> Vec<(u32, u32, OverlayTileKind)> {
    let mut overlays = Vec::new();

    let hx = game_state.hovered_tile_x.max(0).min(GRID_COLS as i16 - 1) as u32;
    let hy = game_state.hovered_tile_y.max(0).min(GRID_ROWS as i16 - 1) as u32;
    overlays.push((hx, hy, OverlayTileKind::Movable { ap_cost: 2 }));

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
        sprite.width = 28.0;
        sprite.height = 28.0;

        if !actor.alive {
            sprite.r = 0.5;
            sprite.g = 0.1;
            sprite.b = 0.1;
            sprite.a = 0.4;
        } else if Some(*id) == selected_id {
            sprite.r = 1.0;
            sprite.g = 1.0;
            sprite.b = 0.4;
            sprite.a = 1.0;
        } else if is_enemy(actor) {
            sprite.r = 0.9;
            sprite.g = 0.3;
            sprite.b = 0.25;
            sprite.a = 1.0;
        } else {
            sprite.r = 0.3;
            sprite.g = 0.6;
            sprite.b = 0.9;
            sprite.a = 1.0;
        }

        sprites.push(sprite);
    }

    sprites
}
