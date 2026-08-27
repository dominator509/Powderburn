//! Smoke rendering for the isometric battlefield.
//!
//! Reads per-tile smoke density from SimState (or from a density grid) and
//! renders it as a translucent overlay. Smoke is the signature mechanic:
//! black powder smoke degrades line of sight and makes distant shots harder.

use std::sync::Arc;

use wgpu::util::DeviceExt;

use crate::device::RenderDevice;

/// A single smoke density tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmokeTile {
    pub density: u8, // 0-6, where 0 = no smoke, 6 = maximum
}

impl SmokeTile {
    pub const fn new(density: u8) -> Self {
        Self {
            density: if density > 6 { 6 } else { density },
        }
    }
}

/// System for rendering the smoke overlay.
#[allow(missing_debug_implementations)]
pub struct SmokeSystem {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
}

impl SmokeSystem {
    /// Create a smoke overlay system from a grid of smoke density tiles.
    pub fn new(
        device: &Arc<RenderDevice>,
        cols: u32,
        rows: u32,
        smoke_grid: &[SmokeTile],
        camera_matrix_bytes: &[u8; 64],
    ) -> Self {
        Self::new_with_format(
            device,
            cols,
            rows,
            smoke_grid,
            camera_matrix_bytes,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        )
    }

    /// Create a smoke system whose pipeline matches the destination target.
    pub fn new_with_format(
        device: &Arc<RenderDevice>,
        cols: u32,
        rows: u32,
        smoke_grid: &[SmokeTile],
        camera_matrix_bytes: &[u8; 64],
        target_format: wgpu::TextureFormat,
    ) -> Self {
        let half_w = crate::tiles::TILE_HALF_WIDTH;
        let half_h = crate::tiles::TILE_HALF_HEIGHT;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for y in 0..rows {
            for x in 0..cols {
                let idx = (y * cols + x) as usize;
                let tile = smoke_grid.get(idx).copied().unwrap_or(SmokeTile::new(0));
                if tile.density == 0 {
                    continue;
                }

                let [iso_x, iso_y] = crate::tiles::iso_tile_center(x, y);
                let z = 10.0; // smoke renders above tiles and units

                // Preserve the kernel's 0-6 density as a readable opacity.
                let normalized = f32::from(tile.density) / 6.0;
                let alpha = 0.12 + normalized * 0.58;
                let gray = 0.78 + normalized * 0.16;

                #[repr(C)]
                #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
                struct SmokeVert {
                    position: [f32; 3],
                    color: [f32; 4],
                }

                let vtx = |dx: f32, dy: f32| SmokeVert {
                    position: [iso_x + dx, iso_y + dy, z],
                    color: [gray, gray, gray, alpha],
                };

                let base = vertices.len() as u32;
                vertices.push(vtx(-half_w, 0.0));
                vertices.push(vtx(0.0, -half_h));
                vertices.push(vtx(half_w, 0.0));
                vertices.push(vtx(0.0, half_h));
                indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
            }
        }

        let num_indices = indices.len() as u32;

        let vertex_buffer = device
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("smoke vertex buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let index_buffer = device
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("smoke index buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        let uniform_buffer = device
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("smoke uniform"),
                contents: camera_matrix_bytes,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let bind_group_layout =
            device
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("smoke bind group layout"),
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
            label: Some("smoke bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let shader = device
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("smoke shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/smoke.wgsl").into()),
            });

        let pipeline_layout =
            device
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("smoke pipeline layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        let pipeline = device
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("smoke pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: 28, // 3 f32 pos + 4 f32 color = 28 bytes
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

    /// Refresh dynamic smoke geometry while retaining the shader pipeline.
    pub fn update(
        &mut self,
        device: &Arc<RenderDevice>,
        cols: u32,
        rows: u32,
        smoke_grid: &[SmokeTile],
        camera_matrix_bytes: &[u8; 64],
    ) {
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct SmokeVert {
            position: [f32; 3],
            color: [f32; 4],
        }

        let half_w = crate::tiles::TILE_HALF_WIDTH;
        let half_h = crate::tiles::TILE_HALF_HEIGHT;
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        for y in 0..rows {
            for x in 0..cols {
                let idx = (y * cols + x) as usize;
                let tile = smoke_grid.get(idx).copied().unwrap_or(SmokeTile::new(0));
                if tile.density == 0 {
                    continue;
                }
                let [iso_x, iso_y] = crate::tiles::iso_tile_center(x, y);
                let normalized = f32::from(tile.density) / 6.0;
                let alpha = 0.12 + normalized * 0.58;
                let gray = 0.78 + normalized * 0.16;
                let vtx = |dx: f32, dy: f32| SmokeVert {
                    position: [iso_x + dx, iso_y + dy, 10.0],
                    color: [gray, gray, gray, alpha],
                };
                let base = vertices.len() as u32;
                vertices.push(vtx(-half_w, 0.0));
                vertices.push(vtx(0.0, -half_h));
                vertices.push(vtx(half_w, 0.0));
                vertices.push(vtx(0.0, half_h));
                indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
            }
        }
        self.num_indices = indices.len() as u32;
        self.vertex_buffer = device
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("smoke vertex buffer dynamic"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        self.index_buffer = device
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("smoke index buffer dynamic"),
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

    /// Draw the smoke overlay.
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
