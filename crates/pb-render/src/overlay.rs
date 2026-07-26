//! Overlay rendering for the isometric battlefield.
//!
//! Draws movement range indicators, AP costs, cover state, and hit chance
//! breakdowns as semi-transparent overlays on the tile grid.
//! Uses line rendering for movement paths and colored highlights for ranges.

use std::sync::Arc;

use wgpu::util::DeviceExt;

use crate::device::RenderDevice;

/// A highlighted tile for the overlay (movement range, cover state, etc.).
#[derive(Debug, Clone, Copy)]
pub enum OverlayTileKind {
    Movable { ap_cost: u8 },
    Attackable { hit_chance: u8 },
    Cover { hard: bool },
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
        tiles: &[(u32, u32, OverlayTileKind)],
        camera_matrix_bytes: &[u8; 64],
    ) -> Self {
        let tile_w = 64.0;
        let tile_h = 32.0;
        let half_w = tile_w * 0.5;
        let half_h = tile_h * 0.5;

        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct OverlayVert {
            position: [f32; 3],
            color: [f32; 4],
        }

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for &(x, y, kind) in tiles {
            let iso_x = (x as f32 - y as f32) * half_w;
            let iso_y = (x as f32 + y as f32) * half_h;
            let z = 5.0; // overlays above terrain, below sprites

            let color = match kind {
                OverlayTileKind::Movable { ap_cost } => {
                    let intensity = 1.0 - (ap_cost as f32) * 0.15;
                    [0.2 * intensity, 0.8 * intensity, 0.2 * intensity, 0.3]
                }
                OverlayTileKind::Attackable { hit_chance } => {
                    let intensity = hit_chance as f32 / 100.0;
                    [0.8 * intensity, 0.2 * intensity, 0.2 * intensity, 0.3]
                }
                OverlayTileKind::Cover { hard } => {
                    if hard {
                        [0.6, 0.6, 0.2, 0.3]
                    } else {
                        [0.2, 0.6, 0.6, 0.3]
                    }
                }
            };

            let vtx = |dx: f32, dy: f32| OverlayVert {
                position: [iso_x + dx, iso_y + dy, z],
                color,
            };

            let base = vertices.len() as u32;
            vertices.push(vtx(-half_w, 0.0));
            vertices.push(vtx(0.0, -half_h));
            vertices.push(vtx(half_w, 0.0));
            vertices.push(vtx(0.0, half_h));
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
                        array_stride: 28,
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
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
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

    /// Draw all overlay tiles.
    pub fn render<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>) {
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        if self.num_indices > 0 {
            rpass.draw_indexed(0..self.num_indices, 0, 0..1);
        }
    }
}
