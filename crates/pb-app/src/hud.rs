//! HUD overlay for POWDERBURN combat.
//!
//! Renders on top of the combat screen: turn indicator, health bars,
//! AP display, action menu, selection info, and instructions.
//! Uses the `pb_render::text` bitmap font renderer and a simple
//! coloured-rectangle pipeline for backgrounds / health bars.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use pb_core::event::HitLocationType;
use pb_core::metrics::MetricsRegistry;
use pb_render::device::RenderDevice;
use pb_render::text::{fit_text, layout_wrapped_text, BitmapFont, TextRenderer, GLYPH_H};
use pb_render::ui_contract::{
    HUD_ACTION_SELECTED, HUD_ACTION_TARGETING, HUD_INSTRUCTION_IDLE, HUD_INSTRUCTION_SELECTED,
    HUD_INSTRUCTION_TARGETING,
};

use crate::combat::compute_hit_chance_for_hover;
use crate::state::{GameScreen, GameState, InteractionPhase};

// ── Layout constants (all in pixel coords) ─────────────────────────────

/// Y-offset of the HUD bar from the bottom edge.
const HUD_BOTTOM: f32 = 100.0;
/// Height of the bottom HUD bar.
const BAR_H: f32 = 90.0;
/// Text scale (1.0 = 8×8 px).
const TXT_SCALE: f32 = 2.0;
/// Small text scale.
const TXT_SCALE_SMALL: f32 = 1.5;
/// Line height in scaled pixels.
const LINE_H: f32 = 20.0;
/// Left margin.
const MARGIN: f32 = 12.0;
/// Width of the health bar.
const HP_BAR_W: f32 = 120.0;
/// Height of the health bar.
const HP_BAR_H: f32 = 10.0;

// ═════════════════════════════════════════════════════════════════════════
// Rectangle rendering (background bars, health bars)
// ═════════════════════════════════════════════════════════════════════════

/// A coloured rectangle in pixel coords.
#[derive(Debug, Clone, Copy)]
struct HudRect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: [f32; 4],
}

/// Convert a pixel-coord rect to two triangles in NDC.
fn rect_to_ndc(rect: &HudRect, sw: f32, sh: f32) -> (Vec<RectVertex>, Vec<u16>) {
    let left = 2.0 * rect.x / sw - 1.0;
    let right = 2.0 * (rect.x + rect.w) / sw - 1.0;
    let top = 1.0 - 2.0 * rect.y / sh;
    let bottom = 1.0 - 2.0 * (rect.y + rect.h) / sh;

    let v = [
        RectVertex {
            position: [left, top, 0.0],
            color: rect.color,
        },
        RectVertex {
            position: [right, top, 0.0],
            color: rect.color,
        },
        RectVertex {
            position: [right, bottom, 0.0],
            color: rect.color,
        },
        RectVertex {
            position: [left, bottom, 0.0],
            color: rect.color,
        },
    ];
    let idx: [u16; 6] = [0, 1, 2, 2, 3, 0];
    (v.to_vec(), idx.to_vec())
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct RectVertex {
    position: [f32; 3],
    color: [f32; 4],
}

/// Pipeline for drawing filled coloured rectangles (no texture).
#[allow(missing_debug_implementations)]
struct RectRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
}

impl RectRenderer {
    fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hud rect shader"),
            source: wgpu::ShaderSource::Wgsl(RECT_SHADER_SOURCE.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hud rect pipeline layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hud rect pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: size_of::<RectVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hud rect vertex buffer"),
            size: 4096,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hud rect index buffer"),
            size: 4096,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
        }
    }

    fn render_rects<'a>(
        &'a self,
        queue: &wgpu::Queue,
        rpass: &mut wgpu::RenderPass<'a>,
        rects: &[HudRect],
        sw: f32,
        sh: f32,
    ) {
        if rects.is_empty() {
            return;
        }

        // Collect all vertices and indices.
        let mut all_verts = Vec::with_capacity(rects.len() * 4);
        let mut all_idx = Vec::with_capacity(rects.len() * 6);

        for rect in rects {
            let (verts, idx) = rect_to_ndc(rect, sw, sh);
            let base = all_verts.len() as u16;
            all_verts.extend(verts);
            all_idx.extend(idx.iter().map(|i| i + base));
        }

        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&all_verts));
        queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&all_idx));

        rpass.set_pipeline(&self.pipeline);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..all_idx.len() as u32, 0, 0..1);
    }
}

use std::mem::size_of;

// ═════════════════════════════════════════════════════════════════════════
// HUD renderer – owned by the game loop
// ═════════════════════════════════════════════════════════════════════════

/// Top-level HUD renderer, created once and reused every frame.
#[allow(missing_debug_implementations)]
pub struct HudRenderer {
    text_renderer: TextRenderer,
    rect_renderer: RectRenderer,
}

impl HudRenderer {
    pub fn new(
        device: &wgpu::Device,
        font: &BitmapFont,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            text_renderer: TextRenderer::new(device, font, surface_format),
            rect_renderer: RectRenderer::new(device, surface_format),
        }
    }

    /// Render the full HUD overlay for a single frame.
    ///
    /// Must be called **after** the combat frame has been drawn and
    /// submitted, with a *new* encoder that uses `LoadOp::Load`.
    pub fn render(
        &self,
        font: &BitmapFont,
        game_state: &GameState,
        render_device: &Arc<RenderDevice>,
        view: &wgpu::TextureView,
        screen_w: u32,
        screen_h: u32,
    ) {
        if game_state.screen != GameScreen::Battle {
            return;
        }

        let sw = screen_w as f32;
        let sh = screen_h as f32;
        let text_multiplier = game_state.settings.text_scale as f32 / 100.0;
        let txt_scale = TXT_SCALE * text_multiplier;
        let txt_scale_small = TXT_SCALE_SMALL * text_multiplier;
        let palette = game_state.settings.palette();

        // ── Build HUD content ──────────────────────────────────────────

        let has_sim = game_state.sim.is_some();

        // Turn indicator at top-left.
        let turn_text = if has_sim {
            "YOUR TURN"
        } else {
            "NO SIMULATION"
        };

        // ── Sequence strip (top of screen) ────────────────────────────
        let sequence_text = build_sequence_strip(game_state);

        // ── Selected actor info ────────────────────────────────────────
        let selected_info = match game_state.phase {
            InteractionPhase::SelectedActor(id) | InteractionPhase::Targeting { actor: id, .. } => {
                game_state.sim.as_ref().and_then(|sim| {
                    sim.actors.get(&id).map(|a| {
                        format!(
                            "{}  HP: {}/{}  AP: {}/{}",
                            a.name, a.hit_points, a.max_hp, a.ap.0, a.max_hp
                        )
                    })
                })
            }
            _ => None,
        };

        let action_menu = match game_state.phase {
            InteractionPhase::SelectedActor(_) => HUD_ACTION_SELECTED,
            InteractionPhase::Targeting { .. } => HUD_ACTION_TARGETING,
            _ => "",
        };

        let instructions = match game_state.phase {
            InteractionPhase::Idle => HUD_INSTRUCTION_IDLE,
            InteractionPhase::SelectedActor(_) => HUD_INSTRUCTION_SELECTED,
            InteractionPhase::Targeting { .. } => HUD_INSTRUCTION_TARGETING,
            InteractionPhase::Executing => "Executing action...",
        };

        // Message text (one-line from game state).
        let message = &game_state.message;

        // ── Modifier breakdown (while targeting) ──────────────────────
        let modifier_text = if matches!(game_state.phase, InteractionPhase::Targeting { .. }) {
            build_modifier_breakdown_text(game_state)
        } else {
            String::new()
        };

        // ── Weapon status ─────────────────────────────────────────────
        let weapon_text = build_weapon_status_text(game_state);

        // ── Wound doll ────────────────────────────────────────────────
        let wound_info = build_wound_doll_text(game_state);

        // The production word-wrapper is shared with the accessibility gate.
        let text_width = (sw - 2.0 * MARGIN).max(1.0);
        let action_lines = layout_wrapped_text(action_menu, txt_scale, text_width);
        let instruction_lines = layout_wrapped_text(instructions, txt_scale, text_width);
        let message_lines = layout_wrapped_text(message, txt_scale, text_width);
        let line_height = GLYPH_H as f32 * txt_scale + 6.0;
        let content_line_count = action_lines.len() + instruction_lines.len() + message_lines.len();
        let dynamic_bar_h = (content_line_count as f32 * line_height + 28.0)
            .max(BAR_H)
            .min(sh * 0.45);

        let mut rects = Vec::new();

        // Bottom bar background.
        let bar_y = sh - dynamic_bar_h - (HUD_BOTTOM - BAR_H);
        rects.push(HudRect {
            x: 0.0,
            y: bar_y,
            w: sw,
            h: dynamic_bar_h,
            color: palette.background,
        });

        // Top bar background (thin strip for turn indicator + selection).
        rects.push(HudRect {
            x: 0.0,
            y: 0.0,
            w: sw,
            h: 44.0,
            color: palette.background,
        });

        // Health bar for selected actor (bottom-left of the HUD bar).
        if selected_info.is_some() {
            // Parse HP values from the info string
            if let Some(actor) = game_state.sim.as_ref().and_then(|sim| {
                let id = match game_state.phase {
                    InteractionPhase::SelectedActor(id)
                    | InteractionPhase::Targeting { actor: id, .. } => Some(id),
                    _ => None,
                };
                id.and_then(|id| sim.actors.get(&id))
            }) {
                let hp_ratio = if actor.max_hp > 0 {
                    (actor.hit_points as f32 / actor.max_hp as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                let hp_bar_x = MARGIN;
                let hp_bar_y = bar_y + dynamic_bar_h - 16.0;

                // Background (dark red).
                rects.push(HudRect {
                    x: hp_bar_x,
                    y: hp_bar_y,
                    w: HP_BAR_W,
                    h: HP_BAR_H,
                    color: [0.35, 0.08, 0.08, 0.9],
                });
                // Foreground (green → yellow → red gradient).
                let hp_color = if hp_ratio > 0.6 {
                    palette.ally
                } else if hp_ratio > 0.3 {
                    palette.warning
                } else {
                    palette.enemy
                };
                let fill_w = HP_BAR_W * hp_ratio;
                if fill_w > 1.0 {
                    rects.push(HudRect {
                        x: hp_bar_x,
                        y: hp_bar_y,
                        w: fill_w,
                        h: HP_BAR_H,
                        color: hp_color,
                    });
                }
            }
        }

        // ── Called shot wheel overlay ─────────────────────────────────
        if game_state.called_shot_active {
            // Dim overlay for called shot wheel
            let wheel_rects = build_called_shot_wheel_rects();
            rects.extend(wheel_rects);
        }
        if game_state.debug_metrics_visible {
            rects.push(HudRect {
                x: sw - 310.0,
                y: 52.0,
                w: 298.0,
                h: 220.0,
                color: [0.02, 0.03, 0.02, 0.88],
            });
        }

        // ── Render pass ────────────────────────────────────────────────

        let mut encoder =
            render_device
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("hud encoder"),
                });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hud pass"),
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

            // 1. Background rectangles.
            self.rect_renderer
                .render_rects(&render_device.queue, &mut rpass, &rects, sw, sh);

            // 2. Turn indicator (top-left, small).
            let turn_mesh = font.render_text(
                turn_text,
                MARGIN,
                8.0,
                txt_scale_small,
                palette.accent,
                sw,
                sh,
            );
            self.text_renderer
                .render(&render_device.queue, &mut rpass, &turn_mesh);

            // 3. Sequence strip (top, next to turn indicator)
            if !sequence_text.is_empty() {
                let sequence_text =
                    fit_text(&sequence_text, txt_scale_small, (sw - 420.0).max(120.0));
                let seq_mesh = font.render_text(
                    &sequence_text,
                    MARGIN + 90.0,
                    8.0,
                    txt_scale_small,
                    palette.ally,
                    sw,
                    sh,
                );
                self.text_renderer
                    .render(&render_device.queue, &mut rpass, &seq_mesh);
            }

            // 4. Selection info (top area, right side).
            if let Some(ref info) = selected_info {
                let info_x = MARGIN + HP_BAR_W + 30.0;
                let info = fit_text(info, txt_scale, (sw - info_x - 320.0).max(120.0));
                let info_mesh =
                    font.render_text(&info, info_x, 8.0, txt_scale, palette.text, sw, sh);
                self.text_renderer
                    .render(&render_device.queue, &mut rpass, &info_mesh);
            }

            // 5. Weapon status (top bar, right side)
            if !weapon_text.is_empty() {
                let weapon_text = fit_text(&weapon_text, txt_scale_small, 288.0);
                let weapon_mesh = font.render_text(
                    &weapon_text,
                    sw - 300.0,
                    8.0,
                    txt_scale_small,
                    palette.warning,
                    sw,
                    sh,
                );
                self.text_renderer
                    .render(&render_device.queue, &mut rpass, &weapon_mesh);
            }

            // 6-8. Wrapped action, instruction, and message copy.
            let mut text_y = bar_y + 8.0;
            for (lines, color) in [
                (&action_lines, palette.accent),
                (&instruction_lines, palette.text),
                (&message_lines, palette.warning),
            ] {
                for line in lines {
                    let mesh =
                        font.render_text(&line.text, MARGIN, text_y, txt_scale, color, sw, sh);
                    self.text_renderer
                        .render(&render_device.queue, &mut rpass, &mesh);
                    text_y += line_height;
                }
            }

            // Speaker-labeled subtitle bus. Subtitles are on by default and
            // remain independent from simulation timing.
            if game_state.settings.subtitles {
                if let Some(subtitle) = game_state
                    .audio
                    .as_ref()
                    .and_then(pb_audio::AudioSystem::current_subtitle)
                {
                    let subtitle_mesh = font.render_text(
                        &subtitle.to_string(),
                        MARGIN,
                        bar_y - 2.0 * LINE_H,
                        txt_scale,
                        palette.text,
                        sw,
                        sh,
                    );
                    self.text_renderer
                        .render(&render_device.queue, &mut rpass, &subtitle_mesh);
                }
            }

            // 9. Modifier breakdown (below instructions, while targeting)
            if !modifier_text.is_empty() {
                let mod_mesh = font.render_text(
                    &modifier_text,
                    MARGIN,
                    bar_y + 8.0 - LINE_H,
                    txt_scale_small,
                    palette.warning,
                    sw,
                    sh,
                );
                self.text_renderer
                    .render(&render_device.queue, &mut rpass, &mod_mesh);
            }

            // 10. Wound doll (bottom bar, right side)
            if !wound_info.is_empty() {
                let wound_mesh = font.render_text(
                    &wound_info,
                    sw - 250.0,
                    bar_y + 8.0,
                    txt_scale_small,
                    palette.enemy,
                    sw,
                    sh,
                );
                self.text_renderer
                    .render(&render_device.queue, &mut rpass, &wound_mesh);
            }

            // 11. Called shot wheel text overlay
            if game_state.called_shot_active {
                let wheel_text = build_called_shot_wheel_text(game_state);
                let wmesh = font.render_text(
                    &wheel_text,
                    sw * 0.1,
                    sh * 0.25,
                    2.5 * text_multiplier,
                    palette.text,
                    sw,
                    sh,
                );
                self.text_renderer
                    .render(&render_device.queue, &mut rpass, &wmesh);

                // Hint text at bottom
                let hint = font.render_text(
                    "Press G to confirm | TAB to cycle | ESC to cancel",
                    sw * 0.1,
                    sh * 0.25 + 260.0,
                    txt_scale,
                    palette.text,
                    sw,
                    sh,
                );
                self.text_renderer
                    .render(&render_device.queue, &mut rpass, &hint);
            }

            if game_state.debug_metrics_visible {
                let metrics = build_metrics_overlay_text();
                let metrics_mesh = font.render_text(
                    &metrics,
                    sw - 300.0,
                    62.0,
                    txt_scale_small,
                    palette.ally,
                    sw,
                    sh,
                );
                self.text_renderer
                    .render(&render_device.queue, &mut rpass, &metrics_mesh);
            }
        }

        render_device
            .queue
            .submit(std::iter::once(encoder.finish()));
    }

    /// Render a dimming overlay with centred text (for pause menu or slot selection).
    #[allow(clippy::too_many_arguments)]
    fn render_dim_overlay(
        &self,
        font: &BitmapFont,
        render_device: &Arc<RenderDevice>,
        view: &wgpu::TextureView,
        screen_w: u32,
        screen_h: u32,
        title_line: &str,
        lines: &[&str],
        settings: &crate::settings::Settings,
    ) {
        let sw = screen_w as f32;
        let sh = screen_h as f32;
        let multiplier = settings.text_scale as f32 / 100.0;
        let title_scale = 3.5 * multiplier;
        let body_scale = 2.0 * multiplier;
        let palette = settings.palette();

        // ── Dim overlay rect (full-screen semi-transparent black) ─────
        let rects = vec![HudRect {
            x: 0.0,
            y: 0.0,
            w: sw,
            h: sh,
            color: palette.background,
        }];

        // ── Build text meshes ─────────────────────────────────────────
        let title_line = fit_text(title_line, title_scale, (sw - 120.0).max(1.0));
        let title_mesh = font.render_text(
            &title_line,
            60.0,
            sh * 0.3,
            title_scale,
            palette.accent,
            sw,
            sh,
        );
        let mut text_meshes = Vec::new();
        let mut y = sh * 0.3 + 50.0 * multiplier;
        let line_height = GLYPH_H as f32 * body_scale + 10.0;
        for line in lines {
            if line.is_empty() {
                y += line_height;
                continue;
            }
            for wrapped in layout_wrapped_text(line, body_scale, (sw - 120.0).max(1.0)) {
                text_meshes.push(font.render_text(
                    &wrapped.text,
                    60.0,
                    y,
                    body_scale,
                    palette.text,
                    sw,
                    sh,
                ));
                y += line_height;
            }
        }

        let mut encoder =
            render_device
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("overlay encoder"),
                });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("overlay pass"),
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

            self.rect_renderer
                .render_rects(&render_device.queue, &mut rpass, &rects, sw, sh);
            self.text_renderer
                .render(&render_device.queue, &mut rpass, &title_mesh);
            for mesh in &text_meshes {
                self.text_renderer
                    .render(&render_device.queue, &mut rpass, mesh);
            }
        }

        render_device
            .queue
            .submit(std::iter::once(encoder.finish()));
    }

    /// Render the title treatment over the cinematic backdrop.
    pub fn render_title_overlay(
        &self,
        font: &BitmapFont,
        render_device: &Arc<RenderDevice>,
        view: &wgpu::TextureView,
        screen_w: u32,
        screen_h: u32,
        settings: &crate::settings::Settings,
    ) {
        self.render_dim_overlay(
            font,
            render_device,
            view,
            screen_w,
            screen_h,
            "POWDERBURN",
            &[
                "THE LEDGER OF ELK CREEK",
                "",
                "ENTER  BEGIN THE CAMPAIGN",
                "ESC    QUIT",
            ],
            settings,
        );
    }

    /// Render the pause menu overlay (dimmed combat + pause text).
    pub fn render_pause_overlay(
        &self,
        font: &BitmapFont,
        render_device: &Arc<RenderDevice>,
        view: &wgpu::TextureView,
        screen_w: u32,
        screen_h: u32,
        settings: &crate::settings::Settings,
    ) {
        self.render_dim_overlay(
            font,
            render_device,
            view,
            screen_w,
            screen_h,
            "PAUSED",
            &[
                "Press ESC to resume",
                "Press S to save",
                "Press L to load",
                "Press Q to quit",
            ],
            settings,
        );
    }

    /// Render an authored non-battle screen over the cinematic backdrop.
    #[allow(clippy::too_many_arguments)]
    pub fn render_screen_overlay(
        &self,
        font: &BitmapFont,
        render_device: &Arc<RenderDevice>,
        view: &wgpu::TextureView,
        screen_size: (u32, u32),
        title: &str,
        lines: &[&str],
        settings: &crate::settings::Settings,
    ) {
        self.render_dim_overlay(
            font,
            render_device,
            view,
            screen_size.0,
            screen_size.1,
            title,
            lines,
            settings,
        );
    }

    /// Render the slot-selection overlay (dimmed combat + slot instructions).
    #[allow(clippy::too_many_arguments)]
    pub fn render_slot_overlay(
        &self,
        font: &BitmapFont,
        render_device: &Arc<RenderDevice>,
        view: &wgpu::TextureView,
        screen_w: u32,
        screen_h: u32,
        title: &str,
        settings: &crate::settings::Settings,
    ) {
        self.render_dim_overlay(
            font,
            render_device,
            view,
            screen_w,
            screen_h,
            title,
            &["Press 1-5 to select a slot", "Press ESC to cancel"],
            settings,
        );
    }
}

fn build_metrics_overlay_text() -> String {
    let registry = MetricsRegistry::global();
    [
        format!("metric: sim.step.ms {}", registry.sim_step_ms.summary().1),
        format!("metric: ai.turn.ms {}", registry.ai_turn_ms.summary().1),
        format!(
            "metric: render.frame.ms {}",
            registry.render_frame_ms.summary().1
        ),
        format!(
            "metric: sim.events.per_turn {}",
            registry.sim_events_per_turn.summary().1
        ),
        format!(
            "metric: content.load.ms {}",
            registry.content_load_ms.load(Ordering::Relaxed)
        ),
        format!(
            "metric: save.write.ms {}",
            registry.save_write_ms.load(Ordering::Relaxed)
        ),
        format!(
            "metric: save.size.bytes {}",
            registry.save_size_bytes.load(Ordering::Relaxed)
        ),
        format!(
            "metric: smoke.volumes.live {}",
            registry.smoke_volumes_live.load(Ordering::Relaxed)
        ),
        format!(
            "metric: rng.draws.per_turn {}",
            registry.rng_draws.load(Ordering::Relaxed)
        ),
    ]
    .join("\n")
}

// ═════════════════════════════════════════════════════════════════════════
// HUD content builders
// ═════════════════════════════════════════════════════════════════════════

/// Build the modifier breakdown text for the HUD.
fn build_modifier_breakdown_text(game_state: &GameState) -> String {
    let Some(breakdown) = compute_hit_chance_for_hover(game_state) else {
        return String::new();
    };
    let mut parts = Vec::new();
    for m in &breakdown.modifiers {
        let sign = if m.value >= 0 { "+" } else { "" };
        parts.push(format!("{}{}", sign, m.value));
    }
    let joined = parts.join(" ");
    format!("Hit chance: {}%  ({})", breakdown.total, joined)
}

/// Build the sequence strip text (next 8 actors in turn order).
fn build_sequence_strip(game_state: &GameState) -> String {
    let Some(ref sim) = game_state.sim else {
        return String::new();
    };
    // Collect alive actors with their next_act_at
    let mut actors: Vec<(u64, &pb_sim::state::ActorState, ActorId)> = sim
        .sequence_clock
        .iter()
        .filter_map(|(id, tick)| {
            let actor = sim.actors.get(id)?;
            if !actor.alive {
                return None;
            }
            Some((*tick, actor, *id))
        })
        .collect();

    // Sort by next_act_at, tie-break by sequence desc, then ActorId asc
    actors.sort_by(|(ta, aa, ida), (tb, ab, idb)| {
        ta.cmp(tb)
            .then_with(|| ab.sequence.cmp(&aa.sequence))
            .then_with(|| ida.cmp(idb))
    });

    let count = actors.len().min(8);
    let mut parts = Vec::with_capacity(count);
    for (tick, actor, _id) in actors.iter().take(count) {
        let name = if actor.name.len() > 12 {
            &actor.name[..12]
        } else {
            &actor.name
        };
        parts.push(format!("{}@{}", name, tick));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("Seq: {}", parts.join(" │ "))
    }
}
use pb_core::ids::ActorId;

/// Build weapon status text for the selected actor.
fn build_weapon_status_text(game_state: &GameState) -> String {
    let actor_id = match game_state.phase {
        InteractionPhase::SelectedActor(id) | InteractionPhase::Targeting { actor: id, .. } => id,
        _ => return String::new(),
    };
    let Some(ref sim) = game_state.sim else {
        return String::new();
    };
    let Some(actor) = sim.actors.get(&actor_id) else {
        return String::new();
    };
    let jam_str = if actor.jammed { " JAMMED!" } else { "" };
    format!(
        "{} {}/{} f{}{}",
        actor.weapon, actor.loaded_rounds, actor.weapon_capacity, actor.fouling, jam_str
    )
}

/// Build wound doll text for the selected actor.
fn build_wound_doll_text(game_state: &GameState) -> String {
    let actor_id = match game_state.phase {
        InteractionPhase::SelectedActor(id) | InteractionPhase::Targeting { actor: id, .. } => id,
        _ => return String::new(),
    };
    let Some(ref sim) = game_state.sim else {
        return String::new();
    };
    let Some(actor) = sim.actors.get(&actor_id) else {
        return String::new();
    };

    // Build wound status for each of the 7 hit locations
    let all_locs = [
        HitLocationType::Head,
        HitLocationType::Eyes,
        HitLocationType::Torso,
        HitLocationType::Vitals,
        HitLocationType::GunArm,
        HitLocationType::OffArm,
        HitLocationType::Legs,
    ];
    let mut parts = Vec::new();
    for loc in &all_locs {
        let wounded = actor
            .wounds
            .iter()
            .any(|w| pb_rules::tables::location_to_wound(*loc) == *w);
        let mark = if wounded { "✗" } else { "○" };
        parts.push(format!("{}{}", mark, abbrev_location(*loc)));
    }
    format!("Wounds: {}", parts.join(" "))
}

/// Abbreviate a hit location to 2-4 chars.
fn abbrev_location(loc: HitLocationType) -> &'static str {
    match loc {
        HitLocationType::Head => "Hd",
        HitLocationType::Eyes => "Ey",
        HitLocationType::Torso => "To",
        HitLocationType::Vitals => "Vi",
        HitLocationType::GunArm => "GA",
        HitLocationType::OffArm => "OA",
        HitLocationType::Legs => "Lg",
    }
}

/// Build the called shot wheel background rectangles.
fn build_called_shot_wheel_rects() -> Vec<HudRect> {
    vec![
        // Full-screen dim
        HudRect {
            x: 0.0,
            y: 0.0,
            w: 2000.0,
            h: 2000.0,
            color: [0.0, 0.0, 0.0, 0.65],
        },
        // Center panel
        HudRect {
            x: 50.0,
            y: 100.0,
            w: 700.0,
            h: 300.0,
            color: [0.1, 0.1, 0.15, 0.9],
        },
    ]
}

/// Build the called shot wheel overlay text.
fn build_called_shot_wheel_text(game_state: &GameState) -> String {
    let mut lines = vec!["── CALLED SHOT WHEEL ──".to_string()];
    for (i, entry) in game_state.called_shot_entries.iter().enumerate() {
        let marker = if i as u8 == game_state.called_shot_index {
            "▶"
        } else {
            " "
        };
        // Compute effective hit chance for this location
        let chance_str = compute_called_shot_chance(game_state, entry.location)
            .map(|c| format!("{}%", c))
            .unwrap_or_else(|| "--".to_string());
        lines.push(format!(
            "{} [{}] {:<8} -{}% hit  {}  chance: {}",
            marker,
            entry.key,
            format!("{:?}", entry.location),
            entry.penalty,
            entry.crit_effect,
            chance_str
        ));
    }
    lines.join("\n")
}

/// Compute the hit chance for a specific called shot location.
fn compute_called_shot_chance(game_state: &GameState, location: HitLocationType) -> Option<i32> {
    use pb_sim::shot::compute_hit_chance_breakdown;
    let sim = game_state.sim.as_ref()?;
    let (actor_id, aimed) = match game_state.phase {
        InteractionPhase::SelectedActor(id) => (id, true),
        InteractionPhase::Targeting { actor, .. } => (actor, true),
        _ => return None,
    };
    // Use the first enemy as a dummy target for display
    let (target_id, _) = sim
        .actors
        .iter()
        .find(|(_, a)| a.alive && !crate::combat::is_ally(a))?;
    compute_hit_chance_breakdown(sim, actor_id, *target_id, aimed, Some(location), 0)
        .ok()
        .map(|breakdown| breakdown.total)
}

// ═════════════════════════════════════════════════════════════════════════
// WGSL shaders for coloured rectangles
// ═════════════════════════════════════════════════════════════════════════

const RECT_SHADER_SOURCE: &str = r#"
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 1.0);
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
"#;

#[cfg(test)]
mod metric_overlay_tests {
    use super::build_metrics_overlay_text;

    #[test]
    fn overlay_renders_the_exact_locked_metric_set_once() {
        let text = build_metrics_overlay_text();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 9);
        for name in [
            "sim.step.ms",
            "ai.turn.ms",
            "render.frame.ms",
            "sim.events.per_turn",
            "content.load.ms",
            "save.write.ms",
            "save.size.bytes",
            "smoke.volumes.live",
            "rng.draws.per_turn",
        ] {
            assert_eq!(
                lines
                    .iter()
                    .filter(|line| line.starts_with(&format!("metric: {name} ")))
                    .count(),
                1,
                "{name} missing or duplicated"
            );
        }
    }
}
