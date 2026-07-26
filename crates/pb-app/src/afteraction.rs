//! After-action report screen for POWDERBURN.
//!
//! Rendered after combat ends (victory or defeat). Shows a summary of
//! the battle: mission outcome, kills, and losses.  Press Enter to
//! return to the title screen.

#![forbid(unsafe_code)]

use std::sync::Arc;

use pb_render::device::RenderDevice;
use pb_render::overlay::{OverlaySystem, OverlayTileKind};
use pb_render::text::{BitmapFont, TextRenderer};
use pb_render::tiles::{TileSystem, TileVisual};

use crate::combat::{is_ally, is_enemy};
use crate::state::GameState;

/// After-action report renderer, created once and reused each frame.
#[allow(missing_debug_implementations)]
pub struct AfterActionRenderer {
    text_renderer: TextRenderer,
}

impl AfterActionRenderer {
    /// Create a new after-action renderer.
    pub fn new(
        device: &wgpu::Device,
        font: &BitmapFont,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            text_renderer: TextRenderer::new(device, font, surface_format),
        }
    }

    /// Render one frame of the after-action report.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        gs: &GameState,
        font: &BitmapFont,
        render_device: &Arc<RenderDevice>,
        view: &wgpu::TextureView,
        _format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) {
        let sw = width as f32;
        let sh = height as f32;

        // ── Determine mission outcome ────────────────────────────────────
        let is_victory = gs.sim.as_ref().is_some_and(|sim| {
            sim.actors
                .values()
                .filter(|a| a.alive && is_enemy(a))
                .count()
                == 0
        });

        let (enemies_killed, allies_lost) = gs.sim.as_ref().map_or((0, 0), |sim| {
            let ek = sim
                .actors
                .values()
                .filter(|a| !a.alive && is_enemy(a))
                .count();
            let al = sim
                .actors
                .values()
                .filter(|a| !a.alive && is_ally(a))
                .count();
            (ek, al)
        });

        // ── Build a simple decorative tile grid ───────────────────────────
        let cols = 16u32;
        let rows = 12u32;
        let mut tiles = Vec::with_capacity((cols * rows) as usize);
        for y in 0..rows {
            for x in 0..cols {
                let shade = 0.3 + ((x + y) % 3) as f32 * 0.1;
                let (r, g, b) = if is_victory {
                    (0.15, shade * 0.8, 0.12)
                } else {
                    (shade * 0.9, 0.10, 0.08)
                };
                tiles.push(TileVisual::new(r, g, b, 0));
            }
        }

        let camera = pb_render::camera::IsoCamera::new(width, height);
        let camera_bytes = camera.ortho_matrix_bytes();

        let tile_system = TileSystem::new(render_device, cols, rows, &tiles, &camera_bytes);

        // ── Overlay: highlight centre area ────────────────────────────────
        let cx = cols / 2;
        let cy = rows / 2;
        let overlay_tiles: Vec<(u32, u32, OverlayTileKind)> = (0..4)
            .flat_map(|i| {
                let r = i;
                vec![
                    (
                        cx as i32 + r,
                        cy as i32,
                        OverlayTileKind::Movable { ap_cost: 0 },
                    ),
                    (
                        cx as i32 - r,
                        cy as i32,
                        OverlayTileKind::Movable { ap_cost: 0 },
                    ),
                    (
                        cx as i32,
                        cy as i32 + r,
                        OverlayTileKind::Movable { ap_cost: 0 },
                    ),
                    (
                        cx as i32,
                        cy as i32 - r,
                        OverlayTileKind::Movable { ap_cost: 0 },
                    ),
                ]
            })
            .filter(|(x, y, _)| *x >= 0 && *x < cols as i32 && *y >= 0 && *y < rows as i32)
            .map(|(x, y, k)| (x as u32, y as u32, k))
            .collect();
        let overlay_system = OverlaySystem::new(render_device, &overlay_tiles, &camera_bytes);

        // ── Build text meshes ────────────────────────────────────────────
        let outcome = if is_victory {
            "MISSION COMPLETE"
        } else {
            "MISSION FAILED"
        };
        let summary = format!(
            "Enemies killed: {}    Allies lost: {}",
            enemies_killed, allies_lost
        );
        let prompt = "Press ENTER to continue";

        let fg_color: [f32; 4] = if is_victory {
            [0.3, 0.9, 0.3, 1.0]
        } else {
            [0.9, 0.2, 0.2, 1.0]
        };

        let title_mesh = font.render_text(outcome, 60.0, 80.0, 4.0, fg_color, sw, sh);
        let summary_mesh =
            font.render_text(&summary, 60.0, 160.0, 2.5, [1.0, 1.0, 1.0, 1.0], sw, sh);
        let prompt_mesh = font.render_text(prompt, 60.0, 220.0, 2.0, [0.7, 0.7, 0.7, 1.0], sw, sh);

        // ── Render pass ──────────────────────────────────────────────────
        let mut encoder =
            render_device
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("afteraction encoder"),
                });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("afteraction pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: if is_victory { 0.08 } else { 0.18 },
                            g: if is_victory { 0.18 } else { 0.05 },
                            b: if is_victory { 0.06 } else { 0.04 },
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

            // Text overlay on top
            self.text_renderer
                .render(&render_device.queue, &mut rpass, &title_mesh);
            self.text_renderer
                .render(&render_device.queue, &mut rpass, &summary_mesh);
            self.text_renderer
                .render(&render_device.queue, &mut rpass, &prompt_mesh);
        }

        render_device
            .queue
            .submit(std::iter::once(encoder.finish()));
    }
}
