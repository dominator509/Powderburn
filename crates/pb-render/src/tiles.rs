//! Isometric tile rendering system.
//!
//! Renders the battlefield as a grid of isometric tiles using wgpu.
//! Each tile is a flat-color quad (2 triangles) with z-sorting for depth.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::device::RenderDevice;

/// A single vertex for the tile mesh.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct TileVertex {
    /// Position in world space (x, y, z)
    pub position: [f32; 3],
    /// Color (r, g, b, a)
    pub color: [f32; 4],
}

impl TileVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
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
        }
    }
}

/// Per-tile visual state.
#[derive(Debug, Clone, Copy)]
pub struct TileVisual {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
    pub visible: bool,
    pub elevation: i32,
}

impl TileVisual {
    pub fn new(r: f32, g: f32, b: f32, elevation: i32) -> Self {
        Self {
            r,
            g,
            b,
            a: 1.0,
            visible: true,
            elevation,
        }
    }
}

/// The tile quad index buffer shared across all tiles.
fn quad_indices(start_vertex: u32) -> [u32; 6] {
    [
        start_vertex,
        start_vertex + 1,
        start_vertex + 2,
        start_vertex + 2,
        start_vertex + 3,
        start_vertex,
    ]
}

/// System for rendering a grid of isometric tiles.
#[allow(missing_debug_implementations)]
pub struct TileSystem {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
}

impl TileSystem {
    /// Create a new tile system for a grid of `cols x rows` tiles.
    pub fn new(
        device: &Arc<RenderDevice>,
        cols: u32,
        rows: u32,
        tiles: &[TileVisual],
        camera_matrix_bytes: &[u8; 64],
    ) -> Self {
        // Build vertices for each visible tile
        let tile_w = 64.0;
        let tile_h = 32.0;
        let half_w = tile_w * 0.5;
        let half_h = tile_h * 0.5;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for y in 0..rows {
            for x in 0..cols {
                let idx = (y * cols + x) as usize;
                let tile = tiles
                    .get(idx)
                    .copied()
                    .unwrap_or(TileVisual::new(0.3, 0.5, 0.3, 0));
                if !tile.visible {
                    continue;
                }

                // Isometric tile positions
                let iso_x = (x as f32 - y as f32) * half_w;
                let iso_y = (x as f32 + y as f32) * half_h;
                let z = tile.elevation as f32;

                let vtx = |dx: f32, dy: f32| TileVertex {
                    position: [iso_x + dx, iso_y + dy, z],
                    color: [tile.r, tile.g, tile.b, tile.a],
                };

                let base = vertices.len() as u32;
                vertices.push(vtx(-half_w, 0.0));
                vertices.push(vtx(0.0, -half_h));
                vertices.push(vtx(half_w, 0.0));
                vertices.push(vtx(0.0, half_h));
                indices.extend_from_slice(&quad_indices(base));
            }
        }

        let num_indices = indices.len() as u32;

        // Create vertex buffer
        let vertex_buffer = device
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tile vertex buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        // Create index buffer
        let index_buffer = device
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tile index buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        // Create uniform buffer
        let uniform_buffer = device
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tile uniform"),
                contents: camera_matrix_bytes,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        // Create bind group
        let bind_group_layout =
            device
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("tile bind group layout"),
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
            label: Some("tile bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Shaders
        let shader = device
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("tile shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/tile.wgsl").into()),
            });

        // Pipeline layout
        let pipeline_layout =
            device
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("tile pipeline layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        // Render pipeline
        let pipeline = device
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("tile pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[TileVertex::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent::OVER,
                            alpha: wgpu::BlendComponent::OVER,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
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

    /// Draw all tiles. Must be called inside a render pass.
    pub fn render<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>) {
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        rpass.draw_indexed(0..self.num_indices, 0, 0..1);
    }
}
