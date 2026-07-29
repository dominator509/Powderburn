//! Isometric tile rendering system.
//!
//! Renders the battlefield as a grid of textured isometric tiles using wgpu.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::device::RenderDevice;

/// Vertical screen-space relief represented by one authored elevation level.
pub const ELEVATION_SCREEN_STEP: f32 = 12.0;

/// A single vertex for the tile mesh.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct TileVertex {
    /// Position in world space (x, y, z)
    pub position: [f32; 3],
    /// Color (r, g, b, a)
    pub color: [f32; 4],
    /// Texture coordinate into the 4x2 frontier terrain atlas.
    pub tex_coord: [f32; 2],
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
                wgpu::VertexAttribute {
                    offset: 28,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
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
    pub material: u8,
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
            material: 0,
        }
    }

    pub fn with_material(mut self, material: u8) -> Self {
        self.material = material.min(7);
        self
    }
}

/// Map authored terrain vocabulary onto the eight atlas materials.
pub fn material_for_terrain(terrain: &str) -> u8 {
    match terrain.to_ascii_lowercase().as_str() {
        "road" | "dirt" | "trail" | "mud" | "scree" | "rubble" | "sandstone" => 1,
        "creek" | "stream" | "water" | "river" | "ford" => 2,
        "brush" | "scrub" | "sagebrush" | "sage" => 3,
        "timber" | "cottonwood" | "woods" | "woodland" | "forest" => 4,
        "snow" | "deepsnow" | "winter" | "ice" => 5,
        "floor" | "adobe" | "courtyard" | "settlement" | "station" | "depot" => 6,
        "rail" | "railroad" | "ballast" | "rail_grade" => 7,
        _ => 0,
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
    _texture: wgpu::Texture,
    _texture_view: wgpu::TextureView,
    _sampler: wgpu::Sampler,
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
        Self::new_with_format(
            device,
            cols,
            rows,
            tiles,
            camera_matrix_bytes,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        )
    }

    /// Create a tile system whose pipeline matches the destination target.
    pub fn new_with_format(
        device: &Arc<RenderDevice>,
        cols: u32,
        rows: u32,
        tiles: &[TileVisual],
        camera_matrix_bytes: &[u8; 64],
        target_format: wgpu::TextureFormat,
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
                let iso_y =
                    (x as f32 + y as f32) * half_h + tile.elevation as f32 * ELEVATION_SCREEN_STEP;
                let z = tile.elevation as f32;

                let material = tile.material.min(7);
                let column = f32::from(material % 4);
                let row = f32::from(material / 4);
                let inset = 0.001;
                let u0 = column * 0.25 + inset;
                let v0 = row * 0.5 + inset;
                let u1 = (column + 1.0) * 0.25 - inset;
                let v1 = (row + 1.0) * 0.5 - inset;
                let vtx = |dx: f32, dy: f32, u: f32, v: f32| TileVertex {
                    position: [iso_x + dx, iso_y + dy, z],
                    color: [tile.r, tile.g, tile.b, tile.a],
                    tex_coord: [u, v],
                };

                let base = vertices.len() as u32;
                vertices.push(vtx(-half_w, 0.0, u0, v0));
                vertices.push(vtx(0.0, -half_h, u1, v0));
                vertices.push(vtx(half_w, 0.0, u1, v1));
                vertices.push(vtx(0.0, half_h, u0, v1));
                indices.extend_from_slice(&quad_indices(base));

                // Raised terrain needs visible banks; otherwise elevation only
                // changes depth ordering and the battlefield still reads flat.
                // The two downward-facing isometric edges are the visible
                // cliff faces from the production camera.
                let neighbor_elevation = |nx: u32, ny: u32| {
                    tiles
                        .get((ny * cols + nx) as usize)
                        .map_or(0, |neighbor| neighbor.elevation)
                };
                let side_color = [tile.r * 0.50, tile.g * 0.44, tile.b * 0.36, tile.a];
                if x + 1 < cols {
                    let lower = neighbor_elevation(x + 1, y);
                    if tile.elevation > lower {
                        let drop = (tile.elevation - lower) as f32 * ELEVATION_SCREEN_STEP;
                        let face_base = vertices.len() as u32;
                        vertices.extend_from_slice(&[
                            TileVertex {
                                position: [iso_x + half_w, iso_y, z],
                                color: side_color,
                                tex_coord: [u1, v0],
                            },
                            TileVertex {
                                position: [iso_x, iso_y + half_h, z],
                                color: side_color,
                                tex_coord: [u0, v1],
                            },
                            TileVertex {
                                position: [iso_x, iso_y + half_h - drop, lower as f32],
                                color: side_color,
                                tex_coord: [u0, v1],
                            },
                            TileVertex {
                                position: [iso_x + half_w, iso_y - drop, lower as f32],
                                color: side_color,
                                tex_coord: [u1, v0],
                            },
                        ]);
                        indices.extend_from_slice(&quad_indices(face_base));
                    }
                }
                if y + 1 < rows {
                    let lower = neighbor_elevation(x, y + 1);
                    if tile.elevation > lower {
                        let drop = (tile.elevation - lower) as f32 * ELEVATION_SCREEN_STEP;
                        let face_base = vertices.len() as u32;
                        vertices.extend_from_slice(&[
                            TileVertex {
                                position: [iso_x, iso_y + half_h, z],
                                color: side_color,
                                tex_coord: [u1, v1],
                            },
                            TileVertex {
                                position: [iso_x - half_w, iso_y, z],
                                color: side_color,
                                tex_coord: [u0, v0],
                            },
                            TileVertex {
                                position: [iso_x - half_w, iso_y - drop, lower as f32],
                                color: side_color,
                                tex_coord: [u0, v0],
                            },
                            TileVertex {
                                position: [iso_x, iso_y + half_h - drop, lower as f32],
                                color: side_color,
                                tex_coord: [u1, v1],
                            },
                        ]);
                        indices.extend_from_slice(&quad_indices(face_base));
                    }
                }
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

        let decoded_atlas = image::load_from_memory_with_format(
            include_bytes!("../../../assets/art/frontier_terrain_atlas.png"),
            image::ImageFormat::Png,
        )
        .map_or_else(
            |_| image::RgbaImage::from_pixel(1, 1, image::Rgba([96, 112, 72, 255])),
            |image| image.to_rgba8(),
        );
        // Authored source cells are far larger than their on-screen footprint.
        // Uploading a 512x256 presentation copy retains at least 2x sampling
        // headroom per tile and avoids cache-thrashing software adapters.
        let atlas = if decoded_atlas.width() > 512 || decoded_atlas.height() > 256 {
            image::imageops::resize(
                &decoded_atlas,
                512,
                256,
                image::imageops::FilterType::Triangle,
            )
        } else {
            decoded_atlas
        };
        let (atlas_width, atlas_height) = atlas.dimensions();
        let texture = device.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("frontier terrain atlas"),
            size: wgpu::Extent3d {
                width: atlas_width,
                height: atlas_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("terrain sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        device.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            atlas.as_raw(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(atlas_width * 4),
                rows_per_image: Some(atlas_height),
            },
            wgpu::Extent3d {
                width: atlas_width,
                height: atlas_height,
                depth_or_array_layers: 1,
            },
        );

        // Create bind group
        let bind_group_layout =
            device
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("tile bind group layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });

        let bind_group = device.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tile bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
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
                        format: target_format,
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
            _texture: texture,
            _texture_view: texture_view,
            _sampler: sampler,
        }
    }

    /// Update the tactical camera without rebuilding terrain GPU resources.
    pub fn update_camera(&self, device: &Arc<RenderDevice>, camera_matrix_bytes: &[u8; 64]) {
        device
            .queue
            .write_buffer(&self.uniform_buffer, 0, camera_matrix_bytes);
    }

    /// Draw all tiles. Must be called inside a render pass.
    pub fn render<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>) {
        if self.num_indices == 0 {
            return;
        }
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        rpass.draw_indexed(0..self.num_indices, 0, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_frontier_vocabulary_reaches_distinct_atlas_materials() {
        assert_eq!(material_for_terrain("Mud"), 1);
        assert_eq!(material_for_terrain("Creek"), 2);
        assert_eq!(material_for_terrain("Sagebrush"), 3);
        assert_eq!(material_for_terrain("Cottonwood"), 4);
        assert_eq!(material_for_terrain("DeepSnow"), 5);
        assert_eq!(material_for_terrain("Adobe"), 6);
        assert_eq!(material_for_terrain("Ballast"), 7);
    }
}
