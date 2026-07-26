//! Title screen rendering.
//!
//! Renders the POWDERBURN title screen with a decorative isometric pattern
//! and serves as the game's entry point before combat begins.

use std::sync::Arc;

use pb_render::camera::IsoCamera;
use pb_render::device::RenderDevice;
use pb_render::overlay::{OverlaySystem, OverlayTileKind};
use pb_render::tiles::{TileSystem, TileVisual};

/// Render the title screen.
///
/// Draws a dark-green isometric grid with a centred cross pattern, creating
/// a simple visual title card until the player presses Enter to start.
pub fn render_title(
    render_device: &Arc<RenderDevice>,
    view: &wgpu::TextureView,
    _format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) {
    let camera = IsoCamera::new(width, height);
    let camera_bytes = camera.ortho_matrix_bytes();

    // Decorative isometric grid pattern (16 × 12 tiles)
    let cols = 16u32;
    let rows = 12u32;
    let mut tiles = Vec::with_capacity((cols * rows) as usize);
    for y in 0..rows {
        for x in 0..cols {
            let shade = 0.3 + ((x + y) % 3) as f32 * 0.1;
            tiles.push(TileVisual::new(0.2, shade, 0.15, 0));
        }
    }
    let tile_system = TileSystem::new(render_device, cols, rows, &tiles, &camera_bytes);

    // Centred cross highlight using overlay
    let cx = cols / 2;
    let cy = rows / 2;
    let overlay_tiles: Vec<(u32, u32, OverlayTileKind)> = vec![
        (cx - 1, cy, OverlayTileKind::Movable { ap_cost: 0 }),
        (cx, cy - 1, OverlayTileKind::Movable { ap_cost: 0 }),
        (cx, cy, OverlayTileKind::Movable { ap_cost: 0 }),
        (cx, cy + 1, OverlayTileKind::Movable { ap_cost: 0 }),
        (cx + 1, cy, OverlayTileKind::Movable { ap_cost: 0 }),
    ];
    let overlay_system = OverlaySystem::new(render_device, &overlay_tiles, &camera_bytes);

    let mut encoder = render_device
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("title encoder"),
        });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("title pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.05,
                        g: 0.08,
                        b: 0.04,
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
        overlay_system.render(&mut rpass);
    }
    render_device
        .queue
        .submit(std::iter::once(encoder.finish()));
}
