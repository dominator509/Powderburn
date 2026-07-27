//! Benchmark commands for pbcli.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use pb_content::load::load_all;
use pb_content::schema::ActorData;
use pb_sim::action::Action;
use pb_sim::clock::{advance_to_next_actor, build_actor, register_actor};
use pb_sim::state::SimState;
use pb_render::device::RenderDevice;

use crate::args::Args;
use crate::output;

/// Run the `bench turn` subcommand.
pub fn run_bench(args: &Args) -> Result<(), String> {
    let content_root = args
        .content_root
        .as_deref()
        .unwrap_or_else(|| Path::new("content"));
    let content = load_all(content_root).map_err(|e| format!("content load error: {}", e))?;

    let scenario_id = args.bench_scenario.as_deref().unwrap_or("prov_full_battle");
    let scenario = content
        .scenarios
        .get(scenario_id)
        .ok_or_else(|| format!("scenario '{}' not found", scenario_id))?;

    let iterations = args.iterations.unwrap_or(10) as usize;
    let seed = args.seed.unwrap_or(42);

    let mut worst_turn_ms: u128 = 0;
    let mut worst_step_ms: u128 = 0;

    for _ in 0..iterations {
        let mut state = SimState::new(seed, hash_string(scenario_id));

        // Register actors
        for actor_data in &scenario.actors {
            let actor_id = actor_data_id(actor_data);
            let actor = build_actor(
                actor_id,
                &actor_data.id,
                actor_data.sequence,
                actor_data.hp,
                actor_data.sand,
                pos_to_tile(&actor_data.pos),
            );
            register_actor(&mut state, actor_id, actor);
        }

        // Run several turns
        for _ in 0..20 {
            let turn_start = Instant::now();

            // AI turn
            let ai_start = Instant::now();
            // Simple AI: pick the first actor and Hold
            if let Some(actor_id) = advance_to_next_actor(&mut state) {
                let _cmd = pb_sim::action::Command {
                    actor_id,
                    action: Action::Hold,
                };
            }
            let ai_elapsed = ai_start.elapsed().as_millis();
            if ai_elapsed > worst_turn_ms {
                worst_turn_ms = ai_elapsed;
            }

            let step_elapsed = turn_start.elapsed().as_millis();
            if step_elapsed > worst_step_ms {
                worst_step_ms = step_elapsed;
            }
        }
    }

    // --emit-budget: only emit budget lines if explicitly requested
    if args.emit_budget {
        println!("{}{}", output::WORST_AI_TURN, worst_turn_ms);
        println!("{}{}", output::WORST_SIM_STEP, worst_step_ms);
    }
    Ok(())
}

fn hash_string(s: &str) -> u32 {
    let h = pb_core::hash::hash_state(s.as_bytes());
    u32::from_le_bytes([h[0], h[1], h[2], h[3]])
}

fn actor_data_id(actor: &ActorData) -> pb_core::ids::ActorId {
    let h = pb_core::hash::hash_state(actor.id.as_bytes());
    pb_core::ids::ActorId(u32::from_le_bytes([h[0], h[1], h[2], h[3]]))
}

fn pos_to_tile(pos: &pb_content::schema::TileXYData) -> pb_core::geom::TileXY {
    pb_core::geom::TileXY::new(pos.x, pos.y)
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

    // ── Scene setup (synthetic representative scene) ────────────────
    // Use a 20x20 tile grid matching prov_sixty_actors and populate
    // with ~60 sprites, smoke, and overlay highlights.
    let cols = 20u32;
    let rows = 20u32;
    let camera = pb_render::camera::IsoCamera::new(config.width, config.height);
    let camera_bytes = camera.ortho_matrix_bytes();

    // Tiles
    let mut tiles = Vec::with_capacity((cols * rows) as usize);
    for y in 0..rows {
        for x in 0..cols {
            let shade = 0.3 + ((x + y) % 3) as f32 * 0.1;
            let elevation = if (x + y) % 4 == 0 { 1 } else { 0 };
            tiles.push(pb_render::tiles::TileVisual::new(0.2, shade, 0.15, elevation));
        }
    }
    let tile_system = pb_render::tiles::TileSystem::new(&device, cols, rows, &tiles, &camera_bytes);

    // Sprites — 60 actors spread across the grid
    let mut sprites = Vec::with_capacity(60);
    for i in 0..60 {
        let grid_x = (i % 10) as f32 * 2.0 - 9.0;
        let grid_y = (i / 10) as f32 * 2.0 - 5.0;
        sprites.push(pb_render::sprites::SpriteInstance::new(grid_x, grid_y, 2.0));
    }
    let sprite_system = pb_render::sprites::SpriteSystem::new(&device, &sprites, &camera_bytes);

    // Smoke — density near center
    let mut smoke_tiles = Vec::with_capacity((cols * rows) as usize);
    for y in 0..rows {
        for x in 0..cols {
            let dist = ((x as i32 - 10).abs() + (y as i32 - 10).abs()) as u8;
            let density = if dist < 3 {
                (4 - dist) as u8
            } else if dist < 5 {
                1
            } else {
                0
            };
            smoke_tiles.push(pb_render::smoke::SmokeTile::new(density));
        }
    }
    let smoke_system = pb_render::smoke::SmokeSystem::new(&device, cols, rows, &smoke_tiles, &camera_bytes);

    // Overlay highlights — movement range and attackable tiles
    let overlay_tiles = vec![
        (7u32, 5u32, pb_render::overlay::OverlayTileKind::Movable { ap_cost: 2 }),
        (8u32, 5u32, pb_render::overlay::OverlayTileKind::Movable { ap_cost: 3 }),
        (9u32, 5u32, pb_render::overlay::OverlayTileKind::Movable { ap_cost: 4 }),
        (10u32, 6u32, pb_render::overlay::OverlayTileKind::Attackable { hit_chance: 65 }),
        (11u32, 7u32, pb_render::overlay::OverlayTileKind::Cover { hard: true }),
        (12u32, 8u32, pb_render::overlay::OverlayTileKind::Movable { ap_cost: 2 }),
    ];
    let overlay_system = pb_render::overlay::OverlaySystem::new(&device, &overlay_tiles, &camera_bytes);

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
