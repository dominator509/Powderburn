//! Bitmap font text rendering for wgpu.
//!
//! Loads a font atlas PNG and renders text as textured quads with a
//! dedicated wgpu pipeline. Designed for the POWDERBURN HUD overlay.
//!
//! Font atlas layout: 128×48 PNG with 96 ASCII glyphs (32–127) in a 16×6
//! grid. Each glyph is 8×8 pixels, white on transparent.

// ── Font atlas constants ────────────────────────────────────────────────

/// Width of each glyph in pixels.
pub const GLYPH_W: u32 = 8;
/// Height of each glyph in pixels.
pub const GLYPH_H: u32 = 8;
/// Number of columns in the atlas grid.
pub const ATLAS_COLS: u32 = 16;
/// Number of rows in the atlas grid.
pub const ATLAS_ROWS: u32 = 6;
/// Total atlas width in pixels.
pub const ATLAS_W: u32 = 128;
/// Total atlas height in pixels.
pub const ATLAS_H: u32 = 48;
/// First ASCII code stored in the atlas.
pub const FIRST_CHAR: u8 = 32; // space
/// Last ASCII code (exclusive) stored in the atlas.
pub const LAST_CHAR: u8 = 127;

// ═════════════════════════════════════════════════════════════════════════
// Vertex & Mesh types
// ═════════════════════════════════════════════════════════════════════════

/// A single vertex in a text mesh.
///
/// Position is expected in normalised device coordinates (NDC),
/// UV samples the font atlas, and color is multiplied with the
/// sampled texel.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

/// A mesh containing one or more character quads.
#[derive(Debug)]
pub struct TextMesh {
    pub vertices: Vec<TextVertex>,
    pub indices: Vec<u16>,
}

// ═════════════════════════════════════════════════════════════════════════
// BitmapFont – loads and holds the font texture
// ═════════════════════════════════════════════════════════════════════════

/// A bitmap font loaded from a PNG atlas.
#[allow(missing_debug_implementations)]
pub struct BitmapFont {
    /// The GPU texture containing the atlas image.
    pub texture: wgpu::Texture,
    /// Default view of the atlas texture.
    pub view: wgpu::TextureView,
    /// Sampler (nearest-neighbour) for crisp pixel text.
    pub sampler: wgpu::Sampler,
}

impl BitmapFont {
    /// Load a font atlas from raw PNG bytes.
    ///
    /// Expects a 128×48 RGBA PNG with 96 ASCII glyphs (codes 32–127) laid
    /// out in a 16×6 grid, each glyph 8×8 pixels, white on transparent.
    pub fn from_png_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        png_bytes: &[u8],
    ) -> Result<Self, String> {
        let img = image::load_from_memory(png_bytes)
            .map_err(|e| format!("failed to load font PNG: {e}"))?
            .into_rgba8();

        let (w, h) = img.dimensions();
        if w != ATLAS_W || h != ATLAS_H {
            return Err(format!(
                "font atlas must be {ATLAS_W}×{ATLAS_H}, got {w}×{h}"
            ));
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("font atlas"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &img,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * w),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("font sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Ok(Self {
            texture,
            view,
            sampler,
        })
    }

    /// Compute UV coordinates `[[u0,v0],[u1,v1]]` for a character.
    ///
    /// Returns `None` for characters outside ASCII 32–126.
    pub fn char_uv(&self, c: char) -> Option<[[f32; 2]; 2]> {
        let code = c as u8;
        if !(FIRST_CHAR..LAST_CHAR).contains(&code) {
            return None;
        }
        let idx = code - FIRST_CHAR;
        let col = f32::from(idx % 16);
        let row = f32::from(idx / 16);

        let u0 = col / ATLAS_COLS as f32;
        let v0 = row / ATLAS_ROWS as f32;
        let u1 = (col + 1.0) / ATLAS_COLS as f32;
        let v1 = (row + 1.0) / ATLAS_ROWS as f32;

        Some([[u0, v0], [u1, v1]])
    }

    /// Build a `TextMesh` from a string, with screen-space input coords.
    ///
    /// * `text` – the string to render (non-ASCII chars are skipped).
    /// * `x`, `y` – top-left corner in **pixel** coordinates (origin at
    ///   top-left of the viewport).
    /// * `scale` – multiplier for glyph size (1.0 → 8×8 px).
    /// * `color` – tint applied to each glyph (RGBA, 0–1).
    /// * `screen_w`, `screen_h` – viewport dimensions for NDC conversion.
    #[allow(clippy::too_many_arguments)]
    pub fn render_text(
        &self,
        text: &str,
        x: f32,
        y: f32,
        scale: f32,
        color: [f32; 4],
        screen_w: f32,
        screen_h: f32,
    ) -> TextMesh {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut char_idx: u32 = 0;

        let gw = GLYPH_W as f32 * scale;
        let gh = GLYPH_H as f32 * scale;

        for c in text.chars() {
            let Some(uv) = self.char_uv(c) else {
                // Skip characters not in the atlas; still consume the slot
                // so positioning stays consistent.
                char_idx += 1;
                continue;
            };

            let cx = x + char_idx as f32 * gw;
            let cy = y;

            // Pixel → NDC conversion.
            // wgpu NDC: x ∈ [-1, 1] left→right, y ∈ [-1, 1] bottom→top.
            // Screen:   x ∈ [0, w] left→right,   y ∈ [0, h] top→bottom.
            let left = 2.0 * cx / screen_w - 1.0;
            let right = 2.0 * (cx + gw) / screen_w - 1.0;
            let top = 1.0 - 2.0 * cy / screen_h;
            let bottom = 1.0 - 2.0 * (cy + gh) / screen_h;

            let base = vertices.len() as u16;

            // Triangle strip: TL, TR, BR, BL → two triangles
            vertices.push(TextVertex {
                position: [left, top, 0.0],
                uv: [uv[0][0], uv[0][1]],
                color,
            });
            vertices.push(TextVertex {
                position: [right, top, 0.0],
                uv: [uv[1][0], uv[0][1]],
                color,
            });
            vertices.push(TextVertex {
                position: [right, bottom, 0.0],
                uv: [uv[1][0], uv[1][1]],
                color,
            });
            vertices.push(TextVertex {
                position: [left, bottom, 0.0],
                uv: [uv[0][0], uv[1][1]],
                color,
            });

            indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
            char_idx += 1;
        }

        TextMesh { vertices, indices }
    }
}

// ═════════════════════════════════════════════════════════════════════════
// TextRenderer – wgpu pipeline for drawing text meshes
// ═════════════════════════════════════════════════════════════════════════

/// Renders `TextMesh` instances to a wgpu render pass.
///
/// Holds the pipeline, bind group (font sampler + texture), and
/// pre-allocated vertex/index buffers for efficient streaming.
#[allow(missing_debug_implementations)]
pub struct TextRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
}

// Pre-allocated buffer capacities.
const MAX_VERTS: usize = 65536;
const MAX_INDS: usize = 65536;

impl TextRenderer {
    /// Create a new text renderer for the given font and surface format.
    pub fn new(
        device: &wgpu::Device,
        font: &BitmapFont,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("text shader"),
            source: wgpu::ShaderSource::Wgsl(TEXT_SHADER_SOURCE.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("text bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
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
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&font.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&font.view),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("text pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("text pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: size_of::<TextVertex>() as wgpu::BufferAddress,
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

        // Pre-allocate vertex / index buffers for streaming.
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text vertex buffer"),
            size: (MAX_VERTS * size_of::<TextVertex>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text index buffer"),
            size: (MAX_INDS * size_of::<u16>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group,
            vertex_buffer,
            index_buffer,
        }
    }

    /// Draw a single `TextMesh` into the current render pass.
    ///
    /// Writes mesh data to the pre-allocated GPU buffers via the queue,
    /// then issues a draw call.  Safe to call multiple times per frame.
    pub fn render<'a>(
        &'a self,
        queue: &wgpu::Queue,
        rpass: &mut wgpu::RenderPass<'a>,
        mesh: &TextMesh,
    ) {
        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            return;
        }

        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&mesh.vertices));
        queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&mesh.indices));

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..mesh.indices.len() as u32, 0, 0..1);
    }
}

use std::mem::size_of;

// ═════════════════════════════════════════════════════════════════════════
// WGSL shaders (embedded)
// ═════════════════════════════════════════════════════════════════════════

const TEXT_SHADER_SOURCE: &str = r#"
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 1.0);
    output.uv = input.uv;
    output.color = input.color;
    return output;
}

@group(0) @binding(0) var font_sampler: sampler;
@group(0) @binding(1) var font_texture: texture_2d<f32>;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(font_texture, font_sampler, input.uv);
    return texel * input.color;
}
"#;
