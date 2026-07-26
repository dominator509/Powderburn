//! Headless frame capture.
//!
//! Renders an isometric battlefield frame to an offscreen texture and writes it as a PNG file.
//! This is the basis for LF-08 and for all deterministic visual proofs.

use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use sha2::Digest;

use super::device::RenderDevice;
use crate::{CaptureMeta, RenderConfig};

/// Render a single frame headlessly and capture it to a PNG file.
///
/// Renders isometric tiles and sprites on a battlefield.
pub async fn capture_frame(
    device: Arc<RenderDevice>,
    config: &RenderConfig,
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
    let camera = crate::camera::IsoCamera::new(config.width, config.height);
    let camera_bytes = camera.ortho_matrix_bytes();

    // Build tile system - a 16x12 grid
    let cols = 16u32;
    let rows = 12u32;
    let mut tiles = Vec::new();
    for y in 0..rows {
        for x in 0..cols {
            let shade = 0.3 + ((x + y) % 3) as f32 * 0.1;
            let elevation = if (x + y) % 4 == 0 { 1 } else { 0 };
            tiles.push(crate::tiles::TileVisual::new(0.2, shade, 0.15, elevation));
        }
    }
    let tile_system = crate::tiles::TileSystem::new(&device, cols, rows, &tiles, &camera_bytes);

    // Build sprites
    let sprites = vec![
        crate::sprites::SpriteInstance::new(0.0, 0.0, 2.0),
        crate::sprites::SpriteInstance::new(3.0, 2.0, 2.0),
        crate::sprites::SpriteInstance::new(-2.0, 4.0, 2.0),
        crate::sprites::SpriteInstance::new(1.0, -3.0, 2.0),
    ];
    let sprite_system = crate::sprites::SpriteSystem::new(&device, &sprites, &camera_bytes);

    // Build smoke overlay - simulate some smoke density near the center
    let mut smoke_tiles = Vec::new();
    for y in 0..rows {
        for x in 0..cols {
            let dist = ((x as i32 - 8).abs() + (y as i32 - 6).abs()) as u8;
            let density = if dist < 3 {
                (4 - dist) as u8
            } else if dist < 5 {
                1
            } else {
                0
            };
            smoke_tiles.push(crate::smoke::SmokeTile::new(density));
        }
    }
    let smoke_system =
        crate::smoke::SmokeSystem::new(&device, cols, rows, &smoke_tiles, &camera_bytes);

    // Build overlay highlights - simulate movement range and attackable tiles
    let overlay_tiles = vec![
        (
            7u32,
            5u32,
            crate::overlay::OverlayTileKind::Movable { ap_cost: 2 },
        ),
        (
            8u32,
            5u32,
            crate::overlay::OverlayTileKind::Movable { ap_cost: 3 },
        ),
        (
            9u32,
            5u32,
            crate::overlay::OverlayTileKind::Movable { ap_cost: 4 },
        ),
        (
            8u32,
            6u32,
            crate::overlay::OverlayTileKind::Movable { ap_cost: 2 },
        ),
        (
            9u32,
            6u32,
            crate::overlay::OverlayTileKind::Movable { ap_cost: 3 },
        ),
        (
            8u32,
            7u32,
            crate::overlay::OverlayTileKind::Attackable { hit_chance: 65 },
        ),
        (
            9u32,
            7u32,
            crate::overlay::OverlayTileKind::Attackable { hit_chance: 45 },
        ),
        (
            10u32,
            6u32,
            crate::overlay::OverlayTileKind::Cover { hard: true },
        ),
    ];
    let overlay_system = crate::overlay::OverlaySystem::new(&device, &overlay_tiles, &camera_bytes);

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
