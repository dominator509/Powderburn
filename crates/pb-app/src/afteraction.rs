//! After-action report screen for POWDERBURN.
//!
//! Rendered after combat ends (victory or defeat). Shows a summary of
//! the battle: mission outcome, kills, and losses.  Press Enter to
//! return to the title screen.

#![forbid(unsafe_code)]

use std::sync::Arc;

use pb_render::backdrop::BackdropSystem;
use pb_render::device::RenderDevice;
use pb_render::text::{BitmapFont, TextRenderer};

use crate::combat::{is_ally, is_enemy};
use crate::state::GameState;

/// After-action report renderer, created once and reused each frame.
#[allow(missing_debug_implementations)]
pub struct AfterActionRenderer {
    backdrop: BackdropSystem,
    text_renderer: TextRenderer,
}

impl AfterActionRenderer {
    /// Create a new after-action renderer.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        font: &BitmapFont,
        surface_format: wgpu::TextureFormat,
    ) -> Result<Self, String> {
        let backdrop = BackdropSystem::from_png_bytes(
            device,
            queue,
            surface_format,
            include_bytes!("../../../assets/art/companion_portraits.png"),
        )?;
        Ok(Self {
            backdrop,
            text_renderer: TextRenderer::new(device, font, surface_format),
        })
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

        let (enemies_killed, allies_lost, fouling_total) =
            gs.sim.as_ref().map_or((0, 0, 0), |sim| {
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
                let fouling = sim.actors.values().map(|actor| actor.fouling).sum();
                (ek, al, fouling)
            });
        let shots = gs
            .battle_events
            .iter()
            .filter(|event| matches!(event, pb_core::event::Event::Fired { .. }))
            .count();
        let sand_spent: i32 = gs
            .battle_events
            .iter()
            .filter_map(|event| match event {
                pb_core::event::Event::SandLost { amount, .. } => Some(*amount),
                _ => None,
            })
            .sum();
        let sand_recovered: i32 = gs
            .battle_events
            .iter()
            .filter_map(|event| match event {
                pb_core::event::Event::SandGained { amount, .. } => Some(*amount),
                _ => None,
            })
            .sum();
        let wounds = gs
            .battle_events
            .iter()
            .filter(|event| matches!(event, pb_core::event::Event::WoundApplied { .. }))
            .count();

        // ── Build text meshes ────────────────────────────────────────────
        let outcome = if is_victory {
            "MISSION COMPLETE"
        } else {
            "MISSION FAILED"
        };
        let mut report_lines = vec![
            format!("Enemies killed: {enemies_killed}    Allies lost: {allies_lost}"),
            format!("Ammunition expended: {shots}    Wounds: {wounds}    Fouling: {fouling_total}"),
            format!("Sand spent: {sand_spent}    Sand recovered: {sand_recovered}"),
        ];
        for event in gs.battle_events.iter().rev().take(4).rev() {
            report_lines.push(event_as_prose(event, gs));
        }
        if let Some(entry) = gs.pending_ledger_writes.get(gs.ledger_write_cursor) {
            report_lines.push(format!(
                "LEDGER {}/{} — {} at {}, {}",
                gs.ledger_write_cursor + 1,
                gs.pending_ledger_writes.len(),
                entry.name,
                entry.place,
                entry.date
            ));
            for (index, line) in entry.lines.iter().enumerate() {
                let selected = if index == usize::from(entry.selected_index) {
                    ">"
                } else {
                    " "
                };
                report_lines.push(format!("{selected} {}. {line}", index + 1));
            }
            report_lines.push(
                "Press 1-3 to choose; arrows review deaths; ENTER accepts defaults.".to_string(),
            );
        } else {
            report_lines.push("No named death awaits the Ledger. Press ENTER.".to_string());
        }

        let fg_color: [f32; 4] = if is_victory {
            [0.3, 0.9, 0.3, 1.0]
        } else {
            [0.9, 0.2, 0.2, 1.0]
        };

        let title_mesh = font.render_text(outcome, 60.0, 80.0, 4.0, fg_color, sw, sh);
        let line_meshes = report_lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                font.render_text(
                    line,
                    60.0,
                    155.0 + index as f32 * 32.0,
                    2.0,
                    [0.96, 0.91, 0.78, 1.0],
                    sw,
                    sh,
                )
            })
            .collect::<Vec<_>>();

        // ── Render pass ──────────────────────────────────────────────────
        self.backdrop
            .render(&render_device.device, &render_device.queue, view);
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
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // Text overlay on top
            self.text_renderer
                .render(&render_device.queue, &mut rpass, &title_mesh);
            for mesh in &line_meshes {
                self.text_renderer
                    .render(&render_device.queue, &mut rpass, mesh);
            }
        }

        render_device
            .queue
            .submit(std::iter::once(encoder.finish()));
    }
}

fn event_as_prose(event: &pb_core::event::Event, gs: &GameState) -> String {
    let actor_name = |id: pb_core::ids::ActorId| {
        gs.sim
            .as_ref()
            .and_then(|sim| sim.actors.get(&id))
            .map_or_else(|| format!("Actor {}", id.0), |actor| actor.name.clone())
    };
    match event {
        pb_core::event::Event::Fired { actor, target } => {
            format!("{} fired on {}.", actor_name(*actor), actor_name(*target))
        }
        pb_core::event::Event::Missed { actor, target } => {
            format!("{} missed {}.", actor_name(*actor), actor_name(*target))
        }
        pb_core::event::Event::DamageApplied { actor, damage } => {
            format!("{} took {damage} damage.", actor_name(*actor))
        }
        pb_core::event::Event::WoundApplied { actor, wound } => {
            format!("{} carried a {wound} wound.", actor_name(*actor))
        }
        pb_core::event::Event::ActorKilled { actor } => {
            format!("{} died on the field.", actor_name(*actor))
        }
        pb_core::event::Event::SmokeDeposited { density, .. } => {
            format!("Black-powder smoke thickened by {density}.")
        }
        _ => event.to_string().replacen("event: ", "", 1),
    }
}
