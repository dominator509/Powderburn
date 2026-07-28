//! Benchmark commands for pbcli.

#![allow(clippy::float_arithmetic)]

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use pb_content::load::load_all;
use pb_content::schema::ActorData;
use pb_core::ids::ActorId;
use pb_render::device::RenderDevice;
use pb_sim::action::{Action, Command};
use pb_sim::clock::{advance_to_next_actor, register_actor};
use pb_sim::state::{ActorState, SimState};

use crate::args::Args;
use crate::cmd_sim::actor_from_data;
use crate::output;

/// Run the `bench turn` subcommand.
pub fn run_bench(args: &Args) -> Result<(), String> {
    let content_root = args
        .content_root
        .as_deref()
        .unwrap_or_else(|| Path::new("content"));
    let content = load_all(content_root).map_err(|e| format!("content load error: {}", e))?;

    let scenario_id = scenario_key(args.bench_scenario.as_deref().unwrap_or("prov_full_battle"));
    let scenario = content
        .scenarios
        .get(&scenario_id)
        .ok_or_else(|| format!("scenario '{}' not found", scenario_id))?;

    let iterations = args.iterations.unwrap_or(10) as usize;
    let seed = args.seed.unwrap_or(42);

    let mut worst_turn_ms: u128 = 0;
    let mut worst_step_ms: u128 = 0;

    for _ in 0..iterations {
        let mut state = SimState::new(seed, hash_string(&scenario_id));

        // Register actors
        for actor_data in &scenario.actors {
            let actor_id = actor_data_id(actor_data);
            let actor = actor_from_data(actor_data, &content);
            register_actor(&mut state, actor_id, actor);
        }

        // Run several turns
        for _ in 0..20 {
            let turn_start = Instant::now();

            let ai_elapsed;
            let step_elapsed;
            {
                let ai_start = Instant::now();
                step_elapsed = run_one_ai_actor_turn(&mut state)?;
                ai_elapsed = ai_start.elapsed().as_millis();
            }
            if ai_elapsed > worst_turn_ms {
                worst_turn_ms = ai_elapsed;
            }
            if step_elapsed > worst_step_ms {
                worst_step_ms = step_elapsed;
            }
            let _whole_iteration_elapsed = turn_start.elapsed();
        }
    }

    // --emit-budget: only emit budget lines if explicitly requested
    if args.emit_budget {
        println!("{}{}", output::WORST_AI_TURN, worst_turn_ms);
        println!("{}{}", output::WORST_SIM_STEP, worst_step_ms);
    }
    Ok(())
}

/// Run a complete utility-AI turn for the next scheduled actor.
///
/// Candidate generation, scoring, deterministic target translation, legality,
/// and state mutation are all included. Illegal preferences fall back to Hold
/// exactly as the interactive client does. Returns the worst individual
/// simulation-step duration in milliseconds.
pub(crate) fn run_one_ai_actor_turn(state: &mut SimState) -> Result<u128, String> {
    let Some(actor_id) = advance_to_next_actor(state) else {
        return Ok(0);
    };
    let mut worst_step_ms = 0;

    for _ in 0..64 {
        let actor = state
            .actors
            .get(&actor_id)
            .cloned()
            .ok_or_else(|| format!("active actor {} disappeared", actor_id.0))?;
        let allies: Vec<ActorState> = state
            .actors
            .iter()
            .filter(|(id, candidate)| {
                **id != actor_id && candidate.alive && candidate.faction_id == actor.faction_id
            })
            .map(|(_, candidate)| candidate.clone())
            .collect();
        let targets: Vec<(ActorId, ActorState)> = state
            .actors
            .iter()
            .filter(|(_, candidate)| candidate.alive && candidate.faction_id != actor.faction_id)
            .map(|(id, candidate)| (*id, candidate.clone()))
            .collect();
        let target_states: Vec<ActorState> = targets
            .iter()
            .map(|(_, candidate)| candidate.clone())
            .collect();

        let mut command = pb_ai::utility::decide_action(actor_id, &actor, &allies, &target_states);
        command.action = resolve_ai_target(command.action, &targets);

        let step_start = Instant::now();
        if let Err(error) = pb_sim::action::step(state, command) {
            let fallback = if matches!(error, pb_sim::state::SimError::MustRetreat(_)) {
                pb_sim::action::choose_retreat_tile(state, actor_id)
                    .map(Action::Move)
                    .unwrap_or(Action::Hold)
            } else {
                Action::Hold
            };
            pb_sim::action::step(
                state,
                Command {
                    actor_id,
                    action: fallback,
                },
            )
            .map_err(|fallback_error| {
                format!(
                    "AI actor {} failed {error:?} and fallback failed: {fallback_error:?}",
                    actor_id.0
                )
            })?;
        }
        worst_step_ms = worst_step_ms.max(step_start.elapsed().as_millis());

        if state.active_actor != Some(actor_id) {
            return Ok(worst_step_ms);
        }
    }

    Err(format!(
        "AI actor {} exceeded 64 commands without ending its turn",
        actor_id.0
    ))
}

fn resolve_ai_target(action: Action, targets: &[(ActorId, ActorState)]) -> Action {
    let real_id = |fake: ActorId| {
        targets
            .get(fake.0.saturating_sub(1) as usize)
            .map(|(id, _)| *id)
            .unwrap_or(fake)
    };
    match action {
        Action::SnapShot(target) => Action::SnapShot(real_id(target)),
        Action::AimedShot(target) => Action::AimedShot(real_id(target)),
        Action::CalledShot(target, location) => Action::CalledShot(real_id(target), location),
        Action::Melee(target) => Action::Melee(real_id(target)),
        other => other,
    }
}

fn hash_string(s: &str) -> u32 {
    let h = pb_core::hash::hash_state(s.as_bytes());
    u32::from_le_bytes([h[0], h[1], h[2], h[3]])
}

fn actor_data_id(actor: &ActorData) -> pb_core::ids::ActorId {
    let h = pb_core::hash::hash_state(actor.id.as_bytes());
    pb_core::ids::ActorId(u32::from_le_bytes([h[0], h[1], h[2], h[3]]))
}

fn scenario_key(value: &str) -> String {
    Path::new(value)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(value)
        .to_string()
}

/// Run the `bench frame` subcommand.
///
/// Creates a headless render device, sets up representative scene geometry,
/// then renders N frames measuring each frame's GPU submission time.
/// Reports the 95th percentile frame time in milliseconds.
pub fn run_bench_frame(args: &Args) -> Result<(), String> {
    let num_frames = args.frames.unwrap_or(600) as usize;

    // ── Device setup ────────────────────────────────────────────────
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("failed to create tokio runtime: {}", e))?;

    let device: Arc<RenderDevice> = rt
        .block_on(RenderDevice::new_headless())
        .map_err(|e| format!("failed to create headless device: {}", e))?;

    let config = pb_render::RenderConfig {
        width: 1920,
        height: 1080,
        adapter_name: args.adapter.clone(),
    };

    // ── Scene setup from the authored crowded proving scenario ──────
    let content_root = args
        .content_root
        .as_deref()
        .unwrap_or_else(|| Path::new("content"));
    let scenario_id = scenario_key(
        args.bench_scenario
            .as_deref()
            .unwrap_or("prov_sixty_actors"),
    );
    let state = crate::cmd_sim::construct_scenario_state(
        content_root,
        &scenario_id,
        args.seed.unwrap_or(4),
    )?;
    let scene = pb_render::capture::CaptureScene::from_state(&state);
    let cols = scene.cols;
    let rows = scene.rows;
    let mut camera = pb_render::camera::IsoCamera::new(config.width, config.height);
    camera.center_x = (cols as f32 - rows as f32) * 16.0;
    camera.center_y = (cols.saturating_add(rows).saturating_sub(2) as f32) * 8.0;
    let camera_bytes = camera.ortho_matrix_bytes();
    let tile_system =
        pb_render::tiles::TileSystem::new(&device, cols, rows, &scene.tiles, &camera_bytes);
    let sprite_system =
        pb_render::sprites::SpriteSystem::new(&device, &scene.sprites, &camera_bytes);
    let smoke_system =
        pb_render::smoke::SmokeSystem::new(&device, cols, rows, &scene.smoke, &camera_bytes);
    let overlay_system =
        pb_render::overlay::OverlaySystem::new(&device, &scene.overlays, &camera_bytes);
    let prop_system = pb_render::props::PropSystem::new(&device, &scene.props, &camera_bytes);

    // ── Offscreen render target ─────────────────────────────────────
    let texture_size = wgpu::Extent3d {
        width: config.width,
        height: config.height,
        depth_or_array_layers: 1,
    };
    let texture = device.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bench frame target"),
        size: texture_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    // We also need a readback buffer for the last-frame capture
    let buffer_size = (config.width * config.height * 4) as u64;
    let _readback_buffer = device.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bench readback buffer"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // ── Benchmark loop ──────────────────────────────────────────────
    let mut frame_times_ms: Vec<f64> = Vec::with_capacity(num_frames);

    for _i in 0..num_frames {
        let frame_start = Instant::now();

        let mut encoder = device
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("bench encoder"),
            });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bench render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &texture_view,
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

            tile_system.render(&mut rpass);
            smoke_system.render(&mut rpass);
            overlay_system.render(&mut rpass);
            prop_system.render(&mut rpass);
            sprite_system.render(&mut rpass);
        }

        device.queue.submit(std::iter::once(encoder.finish()));

        // Wait for GPU to finish (required for accurate timing)
        device.device.poll(wgpu::Maintain::Wait);

        let elapsed_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
        frame_times_ms.push(elapsed_ms);
    }

    // ── Compute p95 ─────────────────────────────────────────────────
    frame_times_ms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p95_idx = ((num_frames as f64) * 0.95).ceil() as usize;
    let p95_idx = p95_idx.clamp(1, num_frames) - 1; // 0-based
    let p95_ms = frame_times_ms[p95_idx];

    // Report
    println!("{}{:.3}", output::P95_FRAME_MS, p95_ms);

    if p95_ms > 16.0 {
        eprintln!(
            "WARNING: p95 frame time {:.3}ms exceeds 16ms budget",
            p95_ms
        );
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod direct_bench_tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn authored_sixty_actor_turn_runs_the_real_ai_budget_path() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let raw = [
            "pbcli",
            "bench",
            "turn",
            "--content-root",
            root.to_str().expect("UTF-8 root"),
            "--bench-scenario",
            "content/scenarios/prov_sixty_actors.ron",
            "--seed",
            "4",
            "--iterations",
            "1",
            "--emit-budget",
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        let args = Args::parse(raw).expect("benchmark arguments");
        run_bench(&args).expect("turn benchmark");
    }

    #[test]
    fn target_translation_covers_every_targeted_ai_action() {
        let mut target = pb_sim::clock::build_actor(
            ActorId(77),
            "target",
            5,
            100,
            20,
            pb_core::geom::TileXY::new(1, 1),
        );
        target.faction_id = "enemy".to_string();
        let targets = vec![(ActorId(77), target)];
        for action in [
            Action::SnapShot(ActorId(1)),
            Action::AimedShot(ActorId(1)),
            Action::CalledShot(ActorId(1), pb_core::event::HitLocationType::GunArm),
            Action::Melee(ActorId(1)),
        ] {
            let resolved = resolve_ai_target(action, &targets);
            assert!(match resolved {
                Action::SnapShot(id)
                | Action::AimedShot(id)
                | Action::CalledShot(id, _)
                | Action::Melee(id) => id == ActorId(77),
                _ => false,
            });
        }
        assert!(matches!(
            resolve_ai_target(Action::Hold, &targets),
            Action::Hold
        ));
        assert_eq!(scenario_key("path/to/prov.ron"), "prov");
    }
}
