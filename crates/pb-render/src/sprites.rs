//! Sprite rendering system for wgpu.
//!
//! Renders sprites on top of the isometric tile grid.
//! Each sprite is a textured quad with tint and alpha.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::device::RenderDevice;

/// A single vertex for the sprite mesh.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SpriteVertex {
    /// Position in world space (x, y, z)
    pub position: [f32; 3],
    /// Texture coordinate (u, v)
    pub tex_coord: [f32; 2],
    /// Tint color (r, g, b, a)
    pub color: [f32; 4],
}

impl SpriteVertex {
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
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 20,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// A sprite instance to render.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpriteInstance {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub width: f32,
    pub height: f32,
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
    pub visible: bool,
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
}

impl SpriteInstance {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            x,
            y,
            z,
            width: 32.0,
            height: 32.0,
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
            visible: true,
            u0: 0.0,
            v0: 0.0,
            u1: 0.25,
            v1: 0.5,
        }
    }

    /// Select one cell from the bundled 4x2 frontier company atlas.
    pub fn set_atlas_cell(&mut self, column: u8, row: u8) {
        let column = column.min(3) as f32;
        let row = row.min(1) as f32;
        self.u0 = column * 0.25;
        self.v0 = row * 0.5;
        self.u1 = self.u0 + 0.25;
        self.v1 = self.v0 + 0.5;
    }

    /// Select one of the eight company identities by a stable zero-based index.
    pub fn set_character(&mut self, index: u32) {
        let index = index % 8;
        self.set_atlas_cell((index % 4) as u8, (index / 4) as u8);
    }
}

/// System for rendering sprites.
#[allow(missing_debug_implementations)]
pub struct SpriteSystem {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
}

impl SpriteSystem {
    /// Create a new sprite system.
    pub fn new(
        device: &Arc<RenderDevice>,
        sprites: &[SpriteInstance],
        camera_matrix_bytes: &[u8; 64],
    ) -> Self {
        Self::new_with_atlas(
            device,
            sprites,
            camera_matrix_bytes,
            include_bytes!("../../../assets/sprites/frontier_company_atlas_v2.png"),
            "frontier company atlas",
        )
    }

    /// Create a sprite system backed by a caller-selected PNG atlas.
    ///
    /// This keeps actor and environmental-prop rendering on one GPU path while
    /// allowing each presentation layer to use its own atlas.
    pub fn new_with_atlas(
        device: &Arc<RenderDevice>,
        sprites: &[SpriteInstance],
        camera_matrix_bytes: &[u8; 64],
        atlas_bytes: &[u8],
        atlas_label: &str,
    ) -> Self {
        let decoded_atlas =
            match image::load_from_memory_with_format(atlas_bytes, image::ImageFormat::Png) {
                Ok(image) => image.to_rgba8(),
                Err(_) => image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 255, 255])),
            };
        // Keep atlas cells at least twice their normal display size while
        // bounding texture working sets for llvmpipe and low-memory GPUs.
        let atlas = if decoded_atlas.width() > 768 || decoded_atlas.height() > 512 {
            let scale =
                (768.0 / decoded_atlas.width() as f32).min(512.0 / decoded_atlas.height() as f32);
            let width = (decoded_atlas.width() as f32 * scale).round().max(1.0) as u32;
            let height = (decoded_atlas.height() as f32 * scale).round().max(1.0) as u32;
            image::imageops::resize(
                &decoded_atlas,
                width,
                height,
                image::imageops::FilterType::Triangle,
            )
        } else {
            decoded_atlas
        };
        let (atlas_width, atlas_height) = atlas.dimensions();
        let texture = device.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(atlas_label),
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
            label: Some("sprite sampler"),
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

        // Build sprite geometry
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for sprite in sprites {
            if !sprite.visible {
                continue;
            }
            let half_w = sprite.width * 0.5;
            let half_h = sprite.height * 0.5;

            let vtx = |dx: f32, dy: f32, u: f32, v: f32| SpriteVertex {
                position: [sprite.x + dx, sprite.y + dy, sprite.z],
                tex_coord: [u, v],
                color: [sprite.r, sprite.g, sprite.b, sprite.a],
            };

            let base = vertices.len() as u32;
            // World-space +Y points up, while image V=0 is the atlas top.
            vertices.push(vtx(-half_w, -half_h, sprite.u0, sprite.v1));
            vertices.push(vtx(half_w, -half_h, sprite.u1, sprite.v1));
            vertices.push(vtx(half_w, half_h, sprite.u1, sprite.v0));
            vertices.push(vtx(-half_w, half_h, sprite.u0, sprite.v0));
            indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
        }

        let num_indices = indices.len() as u32;

        // Buffers
        let vertex_buffer = device
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("sprite vertex buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let index_buffer = device
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("sprite index buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        // Uniform buffer
        let uniform_buffer = device
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("sprite uniform"),
                contents: camera_matrix_bytes,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        // Bind group layout
        let bind_group_layout =
            device
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("sprite bind group layout"),
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
            label: Some("sprite bind group"),
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

        // Shader
        let shader = device
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("sprite shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/sprite.wgsl").into()),
            });

        // Pipeline
        let pipeline_layout =
            device
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("sprite pipeline layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        let pipeline = device
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("sprite pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[SpriteVertex::desc()],
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
            texture,
            texture_view,
            sampler,
        }
    }

    /// Draw all sprites. Must be called inside a render pass.
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
