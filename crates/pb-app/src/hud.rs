//! HUD overlay for POWDERBURN combat.
//!
//! Renders on top of the combat screen: turn indicator, health bars,
//! AP display, action menu, selection info, and instructions.
//! Uses the `pb_render::text` bitmap font renderer and a simple
//! coloured-rectangle pipeline for backgrounds / health bars.

use std::sync::Arc;

use pb_render::device::RenderDevice;
use pb_render::text::{BitmapFont, TextRenderer};

use crate::state::{GameScreen, GameState, InteractionPhase};

// ── Layout constants (all in pixel coords) ─────────────────────────────

/// Y-offset of the HUD bar from the bottom edge.
const HUD_BOTTOM: f32 = 100.0;
/// Height of the bottom HUD bar.
const BAR_H: f32 = 90.0;
/// Text scale (1.0 = 8×8 px).
const TXT_SCALE: f32 = 2.0;
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
        if game_state.screen != GameScreen::Combat {
            return;
        }

        let sw = screen_w as f32;
        let sh = screen_h as f32;

        // ── Build HUD content ──────────────────────────────────────────

        let has_sim = game_state.sim.is_some();

        // Turn indicator at top-left.
        let turn_text = if has_sim {
            "YOUR TURN"
        } else {
            "NO SIMULATION"
        };

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
            InteractionPhase::SelectedActor(_) => {
                "Actions: [F]ire  [A]imed  [1-7]Called  [H]old  [R]eload"
            }
            InteractionPhase::Targeting { .. } => {
                "Click an enemy to target | Right-click to cancel"
            }
            _ => "",
        };

        let instructions = match game_state.phase {
            InteractionPhase::Idle => "Click an ally to select | Press key for action",
            InteractionPhase::SelectedActor(_) => {
                "Press F: fire  A: aimed  1-7: called shot  H: hold  R: reload"
            }
            InteractionPhase::Targeting { .. } => "Click on an enemy to execute the action",
            InteractionPhase::Executing => "Executing action...",
        };

        // Message text (one-line from game state).
        let message = &game_state.message;

        // ── Background bar ─────────────────────────────────────────────

        let mut rects = Vec::new();

        // Bottom bar background.
        let bar_y = sh - HUD_BOTTOM;
        rects.push(HudRect {
            x: 0.0,
            y: bar_y,
            w: sw,
            h: BAR_H,
            color: [0.0, 0.0, 0.0, 0.65],
        });

        // Top bar background (thin strip for turn indicator + selection).
        rects.push(HudRect {
            x: 0.0,
            y: 0.0,
            w: sw,
            h: 44.0,
            color: [0.0, 0.0, 0.0, 0.65],
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
                let hp_bar_y = bar_y + 36.0;

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
                    [0.2, 0.75, 0.2, 1.0]
                } else if hp_ratio > 0.3 {
                    [0.9, 0.7, 0.1, 1.0]
                } else {
                    [0.9, 0.15, 0.15, 1.0]
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

            // 2. Turn indicator (top-left).
            let turn_mesh = font.render_text(
                turn_text,
                MARGIN,
                8.0,
                TXT_SCALE,
                [1.0, 0.9, 0.4, 1.0],
                sw,
                sh,
            );
            self.text_renderer
                .render(&render_device.queue, &mut rpass, &turn_mesh);

            // 3. Selection info (top area, right side).
            if let Some(ref info) = selected_info {
                let info_x = MARGIN + HP_BAR_W + 30.0;
                let info_mesh =
                    font.render_text(info, info_x, 8.0, TXT_SCALE, [1.0, 1.0, 1.0, 1.0], sw, sh);
                self.text_renderer
                    .render(&render_device.queue, &mut rpass, &info_mesh);
            }

            // 4. Action menu (bottom bar, line 1).
            if !action_menu.is_empty() {
                let menu_mesh = font.render_text(
                    action_menu,
                    MARGIN,
                    bar_y + 8.0,
                    TXT_SCALE,
                    [0.8, 0.8, 1.0, 1.0],
                    sw,
                    sh,
                );
                self.text_renderer
                    .render(&render_device.queue, &mut rpass, &menu_mesh);
            }

            // 5. Instructions (bottom bar, line 2).
            let instr_mesh = font.render_text(
                instructions,
                MARGIN,
                bar_y + 8.0 + LINE_H,
                TXT_SCALE,
                [0.7, 0.7, 0.7, 1.0],
                sw,
                sh,
            );
            self.text_renderer
                .render(&render_device.queue, &mut rpass, &instr_mesh);

            // 6. Message (bottom bar, line 3).
            if !message.is_empty() {
                let msg_mesh = font.render_text(
                    message,
                    MARGIN,
                    bar_y + 8.0 + 2.0 * LINE_H,
                    TXT_SCALE,
                    [1.0, 1.0, 0.6, 1.0],
                    sw,
                    sh,
                );
                self.text_renderer
                    .render(&render_device.queue, &mut rpass, &msg_mesh);
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
    ) {
        let sw = screen_w as f32;
        let sh = screen_h as f32;

        // ── Dim overlay rect (full-screen semi-transparent black) ─────
        let rects = vec![HudRect {
            x: 0.0,
            y: 0.0,
            w: sw,
            h: sh,
            color: [0.0, 0.0, 0.0, 0.55],
        }];

        // ── Build text meshes ─────────────────────────────────────────
        let title_mesh = font.render_text(
            title_line,
            60.0,
            sh * 0.3,
            3.5,
            [1.0, 1.0, 0.4, 1.0],
            sw,
            sh,
        );
        let text_meshes: Vec<_> = lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                font.render_text(
                    line,
                    60.0,
                    sh * 0.3 + 50.0 + i as f32 * 28.0,
                    2.0,
                    [0.8, 0.8, 0.8, 1.0],
                    sw,
                    sh,
                )
            })
            .collect();

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

    /// Render the pause menu overlay (dimmed combat + pause text).
    pub fn render_pause_overlay(
        &self,
        font: &BitmapFont,
        render_device: &Arc<RenderDevice>,
        view: &wgpu::TextureView,
        screen_w: u32,
        screen_h: u32,
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
        );
    }

    /// Render the slot-selection overlay (dimmed combat + slot instructions).
    pub fn render_slot_overlay(
        &self,
        font: &BitmapFont,
        render_device: &Arc<RenderDevice>,
        view: &wgpu::TextureView,
        screen_w: u32,
        screen_h: u32,
        title: &str,
    ) {
        self.render_dim_overlay(
            font,
            render_device,
            view,
            screen_w,
            screen_h,
            title,
            &["Press 1-5 to select a slot", "Press ESC to cancel"],
        );
    }
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
