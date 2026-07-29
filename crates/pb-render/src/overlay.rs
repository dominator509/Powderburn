//! Overlay rendering for the isometric battlefield.
//!
//! Draws movement range indicators, AP costs, cover state, and hit chance
//! breakdowns as semi-transparent overlays on the tile grid.
//! Uses line rendering for movement paths and colored highlights for ranges.

use std::sync::Arc;

use wgpu::util::DeviceExt;

use crate::device::RenderDevice;

/// Grid coordinate, authored elevation, and semantic overlay style.
pub type OverlayTile = (u32, u32, i32, OverlayTileKind);

/// A highlighted tile for the overlay (movement range, cover state, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayTileKind {
    /// A legal destination in the selected actor's complete movement range.
    MovementRange {
        ap_cost: u8,
        ends_turn: bool,
    },
    Movable {
        ap_cost: u8,
    },
    Attackable {
        hit_chance: u8,
    },
    Cover {
        hard: bool,
    },
    /// Movement leaves the actor's current cover; rendered with crosshatch.
    LeavesCover {
        ap_cost: u8,
    },
    /// Movement enters at least one armed enemy facing cone; rendered hatched.
    Overwatch {
        ap_cost: u8,
    },
}

impl OverlayTileKind {
    /// One example of every semantically distinct shipped overlay.
    pub const ACCESSIBILITY_SAMPLES: [Self; 8] = [
        Self::MovementRange {
            ap_cost: 1,
            ends_turn: false,
        },
        Self::MovementRange {
            ap_cost: 2,
            ends_turn: true,
        },
        Self::Movable { ap_cost: 1 },
        Self::Attackable { hit_chance: 50 },
        Self::Cover { hard: false },
        Self::Cover { hard: true },
        Self::LeavesCover { ap_cost: 1 },
        Self::Overwatch { ap_cost: 1 },
    ];

    /// Pattern identifier consumed by the production shader.
    pub const fn pattern_id(self) -> u8 {
        match self {
            Self::LeavesCover { .. } => 1,
            Self::Overwatch { .. } => 2,
            Self::Movable { .. } => 3,
            Self::Attackable { .. } => 4,
            Self::Cover { hard: false } => 5,
            Self::Cover { hard: true } => 6,
            Self::MovementRange {
                ends_turn: false, ..
            } => 7,
            Self::MovementRange {
                ends_turn: true, ..
            } => 8,
        }
    }

    /// Human-readable non-color cue exposed in the HUD/help contract.
    pub const fn non_color_cue(self) -> &'static str {
        match self {
            Self::MovementRange {
                ends_turn: false, ..
            } => "blue filled legal-move diamond with perimeter",
            Self::MovementRange {
                ends_turn: true, ..
            } => "red hatched sprint diamond that ends the turn",
            Self::Movable { .. } => "filled movement diamond with dotted border plus AP text",
            Self::Attackable { .. } => {
                "filled target diamond with aiming-ring border plus hit-chance text"
            }
            Self::Cover { hard: false } => "horizontal-stripe soft-cover tile",
            Self::Cover { hard: true } => "checker hard-cover tile",
            Self::LeavesCover { .. } => "crosshatched leaves-cover tile plus warning text",
            Self::Overwatch { .. } => "diagonal-hatched overwatch tile plus warning text",
        }
    }
}

const TILE_HALF_WIDTH: f32 = 32.0;
const TILE_HALF_HEIGHT: f32 = 16.0;

fn iso_tile_center(x: u32, y: u32) -> [f32; 2] {
    [
        (x as f32 - y as f32) * TILE_HALF_WIDTH,
        (x as f32 + y as f32) * TILE_HALF_HEIGHT,
    ]
}

/// System for rendering overlay highlights on the battlefield.
#[allow(missing_debug_implementations)]
pub struct OverlaySystem {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
}

impl OverlaySystem {
    /// Create a new overlay system from a set of highlighted tiles.
    pub fn new(
        device: &Arc<RenderDevice>,
        tiles: &[OverlayTile],
        camera_matrix_bytes: &[u8; 64],
    ) -> Self {
        Self::new_with_format(
            device,
            tiles,
            camera_matrix_bytes,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        )
    }

    /// Create an overlay system whose pipeline matches the destination target.
    pub fn new_with_format(
        device: &Arc<RenderDevice>,
        tiles: &[OverlayTile],
        camera_matrix_bytes: &[u8; 64],
        target_format: wgpu::TextureFormat,
    ) -> Self {
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct OverlayVert {
            position: [f32; 3],
            color: [f32; 4],
            local: [f32; 2],
            pattern: f32,
        }

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for &(x, y, elevation, kind) in tiles {
            let [iso_x, iso_y] = iso_tile_center(x, y);
            let iso_y = iso_y + elevation as f32 * crate::tiles::ELEVATION_SCREEN_STEP;
            let z = 5.0 + elevation as f32; // overlays above terrain, below sprites

            let color = match kind {
                OverlayTileKind::MovementRange { ap_cost, ends_turn } => {
                    let intensity = 1.0 - (ap_cost as f32) * 0.08;
                    if ends_turn {
                        [0.86 * intensity, 0.16, 0.10, 0.30]
                    } else {
                        [0.10, 0.42 * intensity, 0.92 * intensity, 0.25]
                    }
                }
                OverlayTileKind::Movable { ap_cost } => {
                    let intensity = 1.0 - (ap_cost as f32) * 0.15;
                    [0.2 * intensity, 0.8 * intensity, 0.2 * intensity, 0.44]
                }
                OverlayTileKind::Attackable { hit_chance } => {
                    let intensity = hit_chance as f32 / 100.0;
                    [0.8 * intensity, 0.2 * intensity, 0.2 * intensity, 0.48]
                }
                OverlayTileKind::Cover { hard } => {
                    if hard {
                        [0.6, 0.6, 0.2, 0.16]
                    } else {
                        [0.2, 0.6, 0.6, 0.16]
                    }
                }
                OverlayTileKind::LeavesCover { ap_cost } => {
                    let intensity = 1.0 - (ap_cost as f32) * 0.12;
                    [0.95 * intensity, 0.67 * intensity, 0.16, 0.30]
                }
                OverlayTileKind::Overwatch { ap_cost } => {
                    let intensity = 1.0 - (ap_cost as f32) * 0.10;
                    [0.92 * intensity, 0.18, 0.16, 0.34]
                }
            };

            let pattern = f32::from(kind.pattern_id());
            let vtx = |dx: f32, dy: f32, local: [f32; 2]| OverlayVert {
                position: [iso_x + dx, iso_y + dy, z],
                color,
                local,
                pattern,
            };

            let base = vertices.len() as u32;
            vertices.push(vtx(-TILE_HALF_WIDTH, 0.0, [0.0, 0.5]));
            vertices.push(vtx(0.0, -TILE_HALF_HEIGHT, [0.5, 0.0]));
            vertices.push(vtx(TILE_HALF_WIDTH, 0.0, [1.0, 0.5]));
            vertices.push(vtx(0.0, TILE_HALF_HEIGHT, [0.5, 1.0]));
            indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
        }

        let num_indices = indices.len() as u32;

        let vertex_buffer = device
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("overlay vertex buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let index_buffer = device
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("overlay index buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        let uniform_buffer = device
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("overlay uniform"),
                contents: camera_matrix_bytes,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let bind_group_layout =
            device
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("overlay bind group layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let bind_group = device.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("overlay bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let shader = device
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("overlay shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/overlay.wgsl").into()),
            });

        let pipeline_layout =
            device
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("overlay pipeline layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        let pipeline = device
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("overlay pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: 40,
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
                            wgpu::VertexAttribute {
                                offset: 28,
                                shader_location: 2,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                            wgpu::VertexAttribute {
                                offset: 36,
                                shader_location: 3,
                                format: wgpu::VertexFormat::Float32,
                            },
                        ],
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target_format,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::SrcAlpha,
                                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                operation: wgpu::BlendOperation::Add,
                            },
                            alpha: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::One,
                                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                operation: wgpu::BlendOperation::Add,
                            },
                        }),
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

        Self {
            vertex_buffer,
            index_buffer,
            num_indices,
            pipeline,
            bind_group,
            uniform_buffer,
        }
    }

    /// Refresh pointer/action highlights while retaining the shader pipeline.
    pub fn update(
        &mut self,
        device: &Arc<RenderDevice>,
        tiles: &[OverlayTile],
        camera_matrix_bytes: &[u8; 64],
    ) {
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct OverlayVert {
            position: [f32; 3],
            color: [f32; 4],
            local: [f32; 2],
            pattern: f32,
        }

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        for &(x, y, elevation, kind) in tiles {
            let [iso_x, iso_y] = iso_tile_center(x, y);
            let iso_y = iso_y + elevation as f32 * crate::tiles::ELEVATION_SCREEN_STEP;
            let color = match kind {
                OverlayTileKind::MovementRange { ap_cost, ends_turn } => {
                    let intensity = 1.0 - (ap_cost as f32) * 0.08;
                    if ends_turn {
                        [0.86 * intensity, 0.16, 0.10, 0.30]
                    } else {
                        [0.10, 0.42 * intensity, 0.92 * intensity, 0.25]
                    }
                }
                OverlayTileKind::Movable { ap_cost } => {
                    let intensity = 1.0 - (ap_cost as f32) * 0.15;
                    [0.2 * intensity, 0.8 * intensity, 0.2 * intensity, 0.44]
                }
                OverlayTileKind::Attackable { hit_chance } => {
                    let intensity = hit_chance as f32 / 100.0;
                    [0.8 * intensity, 0.2 * intensity, 0.2 * intensity, 0.48]
                }
                OverlayTileKind::Cover { hard } => {
                    if hard {
                        [0.6, 0.6, 0.2, 0.16]
                    } else {
                        [0.2, 0.6, 0.6, 0.16]
                    }
                }
                OverlayTileKind::LeavesCover { ap_cost } => {
                    let intensity = 1.0 - (ap_cost as f32) * 0.12;
                    [0.95 * intensity, 0.67 * intensity, 0.16, 0.30]
                }
                OverlayTileKind::Overwatch { ap_cost } => {
                    let intensity = 1.0 - (ap_cost as f32) * 0.10;
                    [0.92 * intensity, 0.18, 0.16, 0.34]
                }
            };
            let pattern = f32::from(kind.pattern_id());
            let vtx = |dx: f32, dy: f32, local: [f32; 2]| OverlayVert {
                position: [iso_x + dx, iso_y + dy, 5.0 + elevation as f32],
                color,
                local,
                pattern,
            };
            let base = vertices.len() as u32;
            vertices.push(vtx(-TILE_HALF_WIDTH, 0.0, [0.0, 0.5]));
            vertices.push(vtx(0.0, -TILE_HALF_HEIGHT, [0.5, 0.0]));
            vertices.push(vtx(TILE_HALF_WIDTH, 0.0, [1.0, 0.5]));
            vertices.push(vtx(0.0, TILE_HALF_HEIGHT, [0.5, 1.0]));
            indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
        }
        self.num_indices = indices.len() as u32;
        self.vertex_buffer = device
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("overlay vertex buffer dynamic"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        self.index_buffer = device
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("overlay index buffer dynamic"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        device
            .queue
            .write_buffer(&self.uniform_buffer, 0, camera_matrix_bytes);
    }

    /// Update only the camera transform.
    pub fn update_camera(&self, device: &Arc<RenderDevice>, camera_matrix_bytes: &[u8; 64]) {
        device
            .queue
            .write_buffer(&self.uniform_buffer, 0, camera_matrix_bytes);
    }

    /// Draw all overlay tiles.
    pub fn render<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>) {
        if self.num_indices == 0 {
            return;
        }
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        if self.num_indices > 0 {
            rpass.draw_indexed(0..self.num_indices, 0, 0..1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_centers_follow_one_terrain_diamond_per_grid_coordinate() {
        assert_eq!(iso_tile_center(0, 0), [0.0, 0.0]);
        assert_eq!(iso_tile_center(5, 5), [0.0, 160.0]);
        assert_eq!(iso_tile_center(6, 5), [32.0, 176.0]);
        assert_eq!(iso_tile_center(5, 6), [-32.0, 176.0]);
    }

    #[test]
    fn action_overlay_shader_keeps_a_full_tile_fill() {
        let shader = include_str!("../shaders/overlay.wgsl");

        assert!(shader.contains("input.color.a * mix(0.72, 1.0, ink)"));
        assert!(shader.contains("input.color.a * mix(0.76, 1.0, ink)"));
        assert!(OverlayTileKind::Movable { ap_cost: 1 }
            .non_color_cue()
            .contains("filled movement diamond"));
        assert!(OverlayTileKind::Attackable { hit_chance: 75 }
            .non_color_cue()
            .contains("filled target diamond"));
        assert_eq!(
            OverlayTileKind::MovementRange {
                ap_cost: 1,
                ends_turn: false,
            }
            .pattern_id(),
            7
        );
        assert_eq!(
            OverlayTileKind::MovementRange {
                ap_cost: 2,
                ends_turn: true,
            }
            .pattern_id(),
            8
        );
        assert!(OverlayTileKind::MovementRange {
            ap_cost: 2,
            ends_turn: true,
        }
        .non_color_cue()
        .contains("ends the turn"));
    }
}
