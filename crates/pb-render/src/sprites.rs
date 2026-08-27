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
    /// Clockwise rotation in radians around the sprite center, or around the
    /// planted foot point when `anchor_bottom` is enabled.
    pub rotation: f32,
    /// When true, `x/y` is the sprite's planted foot point instead of its
    /// visual center. Rotation, recoil, and body sway then preserve the tile
    /// anchor rather than swinging the actor across neighboring cells.
    pub anchor_bottom: bool,
    /// Presentation-only horizontal bend applied progressively toward the head.
    pub top_sway: f32,
    /// Presentation-only upper-body width multiplier; feet remain anchored.
    pub top_scale_x: f32,
    /// Presentation-only lower-body translation used by the walk cycle.
    pub leg_sway: f32,
    /// Presentation-only lower-body lift used by the walk cycle.
    pub leg_lift: f32,
    /// Presentation-only hip rotation used by the walk cycle.
    pub hip_rotation: f32,
    /// Presentation-only upper-body/arm counter-swing used by the walk cycle.
    pub arm_swing: f32,
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

/// Presentation identity used to keep combat vocalizations aligned with the
/// visible atlas cell. This is not a simulation rule or a character trait.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvatarGender {
    Male,
    Female,
}

const CHARACTER_COUNT: u32 = 8;
const FEMALE_CHARACTER_INDEX: u32 = 6;

/// Return the stable atlas identity selected by an actor ID.
pub const fn character_atlas_index(index: u32) -> u32 {
    index % CHARACTER_COUNT
}

/// Return the presentation gender of the selected company-atlas identity.
pub const fn character_gender(index: u32) -> AvatarGender {
    if character_atlas_index(index) == FEMALE_CHARACTER_INDEX {
        AvatarGender::Female
    } else {
        AvatarGender::Male
    }
}

impl SpriteInstance {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            x,
            y,
            z,
            width: 32.0,
            height: 32.0,
            rotation: 0.0,
            anchor_bottom: false,
            top_sway: 0.0,
            top_scale_x: 1.0,
            leg_sway: 0.0,
            leg_lift: 0.0,
            hip_rotation: 0.0,
            arm_swing: 0.0,
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
        let index = character_atlas_index(index);
        self.set_atlas_cell((index % 4) as u8, (index / 4) as u8);
    }

    /// Render this instance as a solid-color quad instead of sampling the atlas.
    pub fn set_solid_color(&mut self) {
        self.u0 = -1.0;
        self.v0 = -1.0;
        self.u1 = -1.0;
        self.v1 = -1.0;
    }
}

const SPRITE_ROWS: [f32; 4] = [0.0, 0.34, 0.62, 1.0];

fn smoothstep01(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Apply the presentation-only lower-body and upper-body walk deformation to
/// one local point, then apply the sprite's whole-body transform.
fn transformed_sprite_point(
    sprite: &SpriteInstance,
    half_h: f32,
    dx: f32,
    dy: f32,
    pivot_y: f32,
) -> [f32; 2] {
    let top_weight = if half_h > f32::EPSILON {
        ((dy + half_h) / (half_h * 2.0)).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // The feet stay planted while the shin/hip region bends above them. The
    // second smoothstep fades the lower deformation out before the torso.
    let leg_weight =
        smoothstep01(top_weight / 0.16) * (1.0 - smoothstep01((top_weight - 0.62) / 0.20));
    let leg_pivot_y = -half_h + half_h * 1.12;
    let leg_angle = sprite.hip_rotation * leg_weight;
    let leg_sin = leg_angle.sin();
    let leg_cos = leg_angle.cos();
    let leg_relative_y = dy - leg_pivot_y;
    let leg_rotated_x = dx * leg_cos - leg_relative_y * leg_sin;
    let leg_rotated_y = leg_relative_y * leg_cos + leg_pivot_y;
    let mut shaped_x = dx + (leg_rotated_x - dx) * leg_weight + sprite.leg_sway * leg_weight;
    let mut shaped_y = dy + (leg_rotated_y - dy) * leg_weight + sprite.leg_lift * leg_weight;

    // The upper section carries the arm swing and counter-rotates against the
    // hips, producing a readable walk silhouette on the subdivided quad.
    let arm_weight = smoothstep01((top_weight - 0.34) / 0.40);
    let arm_pivot_y = -half_h + half_h * 1.12;
    let arm_angle = sprite.arm_swing * arm_weight;
    let arm_sin = arm_angle.sin();
    let arm_cos = arm_angle.cos();
    let arm_relative_y = shaped_y - arm_pivot_y;
    let arm_rotated_x = shaped_x * arm_cos - arm_relative_y * arm_sin;
    let arm_rotated_y = arm_relative_y * arm_cos + arm_pivot_y;
    shaped_x += (arm_rotated_x - shaped_x) * arm_weight;
    shaped_y += (arm_rotated_y - shaped_y) * arm_weight;

    let upper_scale = 1.0 + (sprite.top_scale_x - 1.0) * top_weight;
    shaped_x = shaped_x * upper_scale + sprite.top_sway * top_weight;
    let relative_y = shaped_y - pivot_y;
    let sin = sprite.rotation.sin();
    let cos = sprite.rotation.cos();
    let rotated_x = shaped_x * cos - relative_y * sin;
    let rotated_y = shaped_x * sin + relative_y * cos;
    [rotated_x, rotated_y]
}

fn sprite_vertex(sprite: &SpriteInstance, dx: f32, dy: f32, u: f32, v: f32) -> SpriteVertex {
    let half_h = sprite.height * 0.5;
    let pivot_y = if sprite.anchor_bottom { -half_h } else { 0.0 };
    let [rotated_x, rotated_y] = transformed_sprite_point(sprite, half_h, dx, dy, pivot_y);
    SpriteVertex {
        position: [sprite.x + rotated_x, sprite.y + rotated_y, sprite.z],
        tex_coord: [u, v],
        color: [sprite.r, sprite.g, sprite.b, sprite.a],
    }
}

fn append_sprite_geometry(
    vertices: &mut Vec<SpriteVertex>,
    indices: &mut Vec<u32>,
    sprite: &SpriteInstance,
) {
    let half_w = sprite.width * 0.5;
    let half_h = sprite.height * 0.5;
    let base = vertices.len() as u32;
    for row in SPRITE_ROWS {
        let dy = -half_h + half_h * 2.0 * row;
        let v = sprite.v1 + (sprite.v0 - sprite.v1) * row;
        vertices.push(sprite_vertex(sprite, -half_w, dy, sprite.u0, v));
        vertices.push(sprite_vertex(sprite, half_w, dy, sprite.u1, v));
    }
    for row in 0..(SPRITE_ROWS.len() - 1) as u32 {
        let left = base + row * 2;
        let right = left + 1;
        let next_left = left + 2;
        let next_right = right + 2;
        indices.extend_from_slice(&[left, right, next_right, next_right, next_left, left]);
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

    /// Create a sprite system whose pipeline matches the destination target.
    pub fn new_with_format(
        device: &Arc<RenderDevice>,
        sprites: &[SpriteInstance],
        camera_matrix_bytes: &[u8; 64],
        target_format: wgpu::TextureFormat,
    ) -> Self {
        Self::new_with_atlas_and_format(
            device,
            sprites,
            camera_matrix_bytes,
            include_bytes!("../../../assets/sprites/frontier_company_atlas_v2.png"),
            "frontier company atlas",
            target_format,
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
        Self::new_with_atlas_and_format(
            device,
            sprites,
            camera_matrix_bytes,
            atlas_bytes,
            atlas_label,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        )
    }

    /// Create an atlas-backed sprite system for a specific target format.
    pub fn new_with_atlas_and_format(
        device: &Arc<RenderDevice>,
        sprites: &[SpriteInstance],
        camera_matrix_bytes: &[u8; 64],
        atlas_bytes: &[u8],
        atlas_label: &str,
        target_format: wgpu::TextureFormat,
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
            append_sprite_geometry(&mut vertices, &mut indices, sprite);
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
            texture,
            texture_view,
            sampler,
        }
    }

    /// Refresh dynamic sprite geometry and the camera while retaining the
    /// decoded atlas, bind group, shader, and render pipeline.
    pub fn update(
        &mut self,
        device: &Arc<RenderDevice>,
        sprites: &[SpriteInstance],
        camera_matrix_bytes: &[u8; 64],
    ) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        for sprite in sprites {
            if !sprite.visible {
                continue;
            }
            append_sprite_geometry(&mut vertices, &mut indices, sprite);
        }
        self.num_indices = indices.len() as u32;
        self.vertex_buffer = device
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("sprite vertex buffer dynamic"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        self.index_buffer = device
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("sprite index buffer dynamic"),
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

#[cfg(test)]
mod tests {
    use super::{
        character_atlas_index, character_gender, transformed_sprite_point, AvatarGender,
        SpriteInstance, SPRITE_ROWS,
    };

    #[test]
    fn atlas_identity_and_voice_gender_are_stable() {
        assert_eq!(character_atlas_index(14), 6);
        assert_eq!(character_gender(6), AvatarGender::Female);
        assert_eq!(character_gender(14), AvatarGender::Female);
        assert_eq!(character_gender(0), AvatarGender::Male);
        assert_eq!(character_gender(7), AvatarGender::Male);
    }

    #[test]
    fn walk_deformation_keeps_feet_planted_and_moves_body_regions() {
        let mut sprite = SpriteInstance::new(100.0, 200.0, 1.0);
        sprite.width = 58.0;
        sprite.height = 82.0;
        sprite.anchor_bottom = true;
        sprite.leg_sway = 2.4;
        sprite.leg_lift = 1.6;
        sprite.hip_rotation = 0.16;
        sprite.arm_swing = -0.11;

        let half_h = sprite.height * 0.5;
        let pivot_y = -half_h;
        let bottom_left =
            transformed_sprite_point(&sprite, half_h, -sprite.width * 0.5, -half_h, pivot_y);
        let lower = transformed_sprite_point(&sprite, half_h, 0.0, -half_h * 0.10, pivot_y);
        let upper = transformed_sprite_point(&sprite, half_h, 0.0, half_h * 0.72, pivot_y);

        assert!(
            (bottom_left[0] + sprite.width * 0.5).abs() < 0.001,
            "bottom-left x moved: {bottom_left:?}"
        );
        assert!(
            bottom_left[1].abs() < 0.001,
            "bottom-left y moved: {bottom_left:?}"
        );
        assert!(lower[0].abs() > 0.05 || (lower[1] + half_h * 0.10).abs() > 0.05);
        assert!(upper[0].abs() > 0.05 || (upper[1] - half_h * 0.72).abs() > 0.05);
        assert_eq!(SPRITE_ROWS.len(), 4);
    }
}
