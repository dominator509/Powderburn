//! Headless frame capture.
//!
//! Renders an isometric battlefield frame to an offscreen texture and writes it as a PNG file.
//! This is the basis for LF-08 and for all deterministic visual proofs.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use sha2::Digest;

use super::device::RenderDevice;
use crate::{CaptureMeta, RenderConfig};

/// Immutable presentation data derived from one simulation state.
#[derive(Debug, Clone)]
pub struct CaptureScene {
    pub cols: u32,
    pub rows: u32,
    pub tiles: Vec<crate::tiles::TileVisual>,
    pub props: Vec<crate::sprites::SpriteInstance>,
    pub sprites: Vec<crate::sprites::SpriteInstance>,
    pub smoke: Vec<crate::smoke::SmokeTile>,
    pub overlays: Vec<crate::overlay::OverlayTile>,
}

impl CaptureScene {
    pub fn from_state(state: &pb_sim::state::SimState) -> Self {
        let cols = state.smoke_cols.max(1);
        let rows = state.smoke_rows.max(1);
        let mut tiles = Vec::with_capacity((cols * rows) as usize);
        for y in 0..rows {
            for x in 0..cols {
                let variation = ((x * 17 + y * 29 + state.scenario_id) % 7) as f32 * 0.012;
                let tile = pb_core::geom::TileXY::new(x as i16, y as i16);
                let terrain = state
                    .terrain_tiles
                    .get(&tile)
                    .map(String::as_str)
                    .unwrap_or_else(|| {
                        if state.weather == pb_sim::environment::Weather::Snow {
                            "Snow"
                        } else {
                            "Grass"
                        }
                    });
                let material = crate::tiles::material_for_terrain(terrain);
                let elevation = state.tile_elevations.get(&tile).copied().unwrap_or(0);
                let tint = if state.difficult_tiles.contains(&tile) {
                    0.88 + variation
                } else {
                    0.94 + variation
                };
                tiles.push(
                    crate::tiles::TileVisual::new(tint, tint, tint, elevation)
                        .with_material(material),
                );
            }
        }

        let mut ordered_actors: Vec<_> = state.actors.iter().collect();
        ordered_actors.sort_by_key(|(id, actor)| {
            (
                actor.position.x + actor.position.y,
                actor.position.y,
                actor.position.x,
                id.0,
            )
        });
        let mut sprites = Vec::with_capacity(ordered_actors.len());
        for (id, actor) in ordered_actors {
            let grid_x = f32::from(actor.position.x);
            let grid_y = f32::from(actor.position.y);
            let iso_x = (grid_x - grid_y) * 32.0;
            let elevation = state
                .tile_elevations
                .get(&actor.position)
                .copied()
                .unwrap_or(0);
            let iso_y = (grid_x + grid_y) * 16.0
                + elevation as f32 * crate::tiles::ELEVATION_SCREEN_STEP
                - 38.0;
            let mut sprite = crate::sprites::SpriteInstance::new(iso_x, iso_y, 20.0 + grid_y);
            sprite.width = 104.0;
            sprite.height = 104.0;
            sprite.set_character(id.0);
            if actor.faction_id == "player" {
                (sprite.r, sprite.g, sprite.b) = (0.90, 0.96, 1.0);
            } else {
                (sprite.r, sprite.g, sprite.b) = (1.0, 0.82, 0.78);
            }
            if !actor.alive {
                (sprite.r, sprite.g, sprite.b, sprite.a) = (0.45, 0.22, 0.18, 0.48);
            }
            if actor.routed {
                sprite.a = 0.55;
            }
            sprites.push(sprite);
        }

        let smoke = (0..(cols * rows) as usize)
            .map(|index| {
                crate::smoke::SmokeTile::new(state.smoke_grid.get(index).copied().unwrap_or(0))
            })
            .collect();
        let props = crate::props::prop_instances_from_state(state);
        let mut covered = BTreeSet::new();
        let mut overlays = Vec::new();
        for (edge, cover) in &state.cover_edges {
            if edge.tile.x < 0 || edge.tile.y < 0 {
                continue;
            }
            let x = edge.tile.x as u32;
            let y = edge.tile.y as u32;
            if x >= cols || y >= rows || !covered.insert((x, y)) {
                continue;
            }
            overlays.push((
                x,
                y,
                state.tile_elevations.get(&edge.tile).copied().unwrap_or(0),
                crate::overlay::OverlayTileKind::Cover {
                    hard: cover.level >= pb_sim::state::CoverLevel::Hard,
                },
            ));
        }
        Self {
            cols,
            rows,
            tiles,
            props,
            sprites,
            smoke,
            overlays,
        }
    }

    fn demo() -> Self {
        Self::from_state(&pb_sim::state::SimState::new(42, 1))
    }
}

/// Render a single frame headlessly and capture it to a PNG file.
///
/// Renders isometric tiles and sprites on a battlefield.
pub async fn capture_frame(
    device: Arc<RenderDevice>,
    config: &RenderConfig,
    output_path: &Path,
) -> Result<CaptureMeta, String> {
    let scene = CaptureScene::demo();
    capture_scene_frame(device, config, &scene, output_path).await
}

/// Render the actual actor, smoke, cover, and terrain state.
pub async fn capture_state_frame(
    device: Arc<RenderDevice>,
    config: &RenderConfig,
    state: &pb_sim::state::SimState,
    output_path: &Path,
) -> Result<CaptureMeta, String> {
    let scene = CaptureScene::from_state(state);
    capture_scene_frame(device, config, &scene, output_path).await
}

/// Render one real simulation frame and return the completed GPU frame time.
///
/// Scene and pipeline construction happen before the timer. PNG encoding and
/// readback are deliberately excluded because `render.frame.ms` measures the
/// interactive frame boundary, not screenshot capture.
pub fn render_state_frame_ms(
    device: &Arc<RenderDevice>,
    config: &RenderConfig,
    state: &pb_sim::state::SimState,
) -> Result<u64, String> {
    let scene = CaptureScene::from_state(state);
    let texture = device.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("metric frame target"),
        size: wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut camera = crate::camera::IsoCamera::new(config.width, config.height);
    camera.center_x = (scene.cols as f32 - scene.rows as f32) * 16.0;
    camera.center_y = (scene.cols.saturating_add(scene.rows).saturating_sub(2) as f32) * 8.0;
    camera.zoom = 1.35;
    let camera_bytes = camera.ortho_matrix_bytes();
    let tile_system =
        crate::tiles::TileSystem::new(device, scene.cols, scene.rows, &scene.tiles, &camera_bytes);
    let sprite_system = crate::sprites::SpriteSystem::new(device, &scene.sprites, &camera_bytes);
    let prop_system = crate::props::PropSystem::new(device, &scene.props, &camera_bytes);
    let smoke_system =
        crate::smoke::SmokeSystem::new(device, scene.cols, scene.rows, &scene.smoke, &camera_bytes);
    let overlay_system = crate::overlay::OverlaySystem::new(device, &scene.overlays, &camera_bytes);

    let started = std::time::Instant::now();
    let mut encoder = device
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("metric frame encoder"),
        });
    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("metric frame render pass"),
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
        tile_system.render(&mut render_pass);
        smoke_system.render(&mut render_pass);
        overlay_system.render(&mut render_pass);
        prop_system.render(&mut render_pass);
        sprite_system.render(&mut render_pass);
    }
    device.queue.submit(std::iter::once(encoder.finish()));
    device.device.poll(wgpu::Maintain::Wait);
    u64::try_from(started.elapsed().as_millis())
        .map_err(|_| "render frame duration exceeded u64".to_string())
}

async fn capture_scene_frame(
    device: Arc<RenderDevice>,
    config: &RenderConfig,
    scene: &CaptureScene,
    output_path: &Path,
) -> Result<CaptureMeta, String> {
    let texture_size = wgpu::Extent3d {
        width: config.width,
        height: config.height,
        depth_or_array_layers: 1,
    };

    let texture = device.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("capture target"),
        size: texture_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    // Create camera
    let mut camera = crate::camera::IsoCamera::new(config.width, config.height);
    camera.center_x = (scene.cols as f32 - scene.rows as f32) * 16.0;
    camera.center_y = (scene.cols.saturating_add(scene.rows).saturating_sub(2) as f32) * 8.0;
    camera.zoom = 1.35;
    let camera_bytes = camera.ortho_matrix_bytes();

    let tile_system =
        crate::tiles::TileSystem::new(&device, scene.cols, scene.rows, &scene.tiles, &camera_bytes);
    let sprite_system = crate::sprites::SpriteSystem::new(&device, &scene.sprites, &camera_bytes);
    let prop_system = crate::props::PropSystem::new(&device, &scene.props, &camera_bytes);
    let smoke_system = crate::smoke::SmokeSystem::new(
        &device,
        scene.cols,
        scene.rows,
        &scene.smoke,
        &camera_bytes,
    );
    let overlay_system =
        crate::overlay::OverlaySystem::new(&device, &scene.overlays, &camera_bytes);

    // Create buffer to read back
    let buffer_size = (config.width * config.height * 4) as u64;
    let buffer = device.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("capture buffer"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // Render
    let mut encoder = device
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("capture encoder"),
        });

    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render pass"),
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

        // Draw tiles first (back to front is handled by z-order in vertex data)
        tile_system.render(&mut rpass);

        // Draw smoke overlay (semi-transparent, above terrain)
        smoke_system.render(&mut rpass);

        // Draw overlay highlights (movement range, attackable targets)
        overlay_system.render(&mut rpass);

        // Draw presentation-only environmental props behind actors.
        prop_system.render(&mut rpass);

        // Draw sprites on top
        sprite_system.render(&mut rpass);
    }

    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(config.width * 4),
                rows_per_image: Some(config.height),
            },
        },
        texture_size,
    );

    device.queue.submit(std::iter::once(encoder.finish()));

    // Read back
    let buffer_slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.device.poll(wgpu::Maintain::Wait);

    let result = rx
        .recv()
        .map_err(|_| "channel receive failed".to_string())?;
    result.map_err(|e| format!("buffer map failed: {}", e))?;

    let data = buffer_slice.get_mapped_range();
    let pixels: Vec<u8> = data.to_vec();
    drop(data);
    buffer.unmap();

    image::save_buffer(
        output_path,
        &pixels,
        config.width,
        config.height,
        image::ColorType::Rgba8,
    )
    .map_err(|e| format!("failed to write PNG: {}", e))?;

    let checksum = file_sha256(output_path).unwrap_or_else(|| "unknown".to_string());

    Ok(CaptureMeta {
        width: config.width,
        height: config.height,
        checksum,
    })
}

fn file_sha256(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let hash = hasher.finalize();
    Some(format!("{:x}", hash))
}
