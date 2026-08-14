//! Bitmap font text rendering for wgpu.
//!
//! Loads a font atlas PNG and renders text as textured quads with a
//! dedicated wgpu pipeline. Designed for the POWDERBURN HUD overlay.
//!
//! Font atlas layout: 128×48 PNG with 96 ASCII glyphs (32–127) in a 16×6
//! grid. Each glyph is 8×8 pixels, white on transparent.

use ab_glyph::{point, Font, FontRef, Glyph, PxScale, ScaleFont};

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
const VECTOR_FONT_PX: f32 = 20.0;
const VECTOR_CELL_W: u32 = 28;
const VECTOR_CELL_H: u32 = 28;
const VECTOR_PADDING: f32 = 3.0;
const VECTOR_RENDER_SCALE: f32 = 0.5;

struct RasterizedFont {
    pixels: Vec<u8>,
    advances: [f32; (LAST_CHAR - FIRST_CHAR) as usize],
}

fn rasterize_ttf_atlas(ttf_bytes: &[u8]) -> Result<RasterizedFont, String> {
    let font = FontRef::try_from_slice(ttf_bytes)
        .map_err(|error| format!("invalid TTF font: {error:?}"))?;
    let px_scale = PxScale::from(VECTOR_FONT_PX);
    let scaled = font.as_scaled(px_scale);
    let atlas_w = VECTOR_CELL_W * ATLAS_COLS;
    let atlas_h = VECTOR_CELL_H * ATLAS_ROWS;
    let mut pixels = vec![0_u8; (atlas_w * atlas_h * 4) as usize];
    let mut advances = [0.0; (LAST_CHAR - FIRST_CHAR) as usize];

    for code in FIRST_CHAR..LAST_CHAR {
        let character = char::from(code);
        let glyph_id = font.glyph_id(character);
        let index = usize::from(code - FIRST_CHAR);
        advances[index] = scaled.h_advance(glyph_id).max(1.0);
        let glyph = Glyph {
            id: glyph_id,
            scale: px_scale,
            position: point(VECTOR_PADDING, VECTOR_PADDING + scaled.ascent()),
        };
        let Some(outlined) = font.outline_glyph(glyph) else {
            continue;
        };
        let bounds = outlined.px_bounds();
        let cell_x = u32::from((code - FIRST_CHAR) % ATLAS_COLS as u8) * VECTOR_CELL_W;
        let cell_y = u32::from((code - FIRST_CHAR) / ATLAS_COLS as u8) * VECTOR_CELL_H;
        outlined.draw(|x, y, coverage| {
            let pixel_x = cell_x as i32 + bounds.min.x.floor() as i32 + x as i32;
            let pixel_y = cell_y as i32 + bounds.min.y.floor() as i32 + y as i32;
            if pixel_x < cell_x as i32
                || pixel_y < cell_y as i32
                || pixel_x >= (cell_x + VECTOR_CELL_W) as i32
                || pixel_y >= (cell_y + VECTOR_CELL_H) as i32
            {
                return;
            }
            let offset = ((pixel_y as u32 * atlas_w + pixel_x as u32) * 4) as usize;
            let alpha = (coverage.clamp(0.0, 1.0) * 255.0).round() as u8;
            pixels[offset..offset + 4].copy_from_slice(&[255, 255, 255, alpha]);
        });
    }

    Ok(RasterizedFont { pixels, advances })
}

/// Convert authored UI copy to glyphs that exist in the shipped bitmap atlas.
///
/// Common typographic punctuation receives a readable ASCII equivalent.
/// Everything else becomes `?` instead of being skipped or accidentally
/// truncated to an unrelated glyph.
pub fn normalize_bitmap_text(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            ' '..='~' => normalized.push(character),
            '\u{00a0}' => normalized.push(' '),
            '–' | '—' | '−' => normalized.push('-'),
            '‘' | '’' | '‚' => normalized.push('\''),
            '“' | '”' | '„' => normalized.push('"'),
            '…' => normalized.push_str("..."),
            '•' => normalized.push('*'),
            '✗' | '×' => normalized.push('X'),
            '○' | '●' => normalized.push('O'),
            '→' | '▶' => normalized.push('>'),
            '←' | '◀' => normalized.push('<'),
            '─' | '━' => normalized.push('-'),
            '\t' | '\r' | '\n' => normalized.push(' '),
            _ => normalized.push('?'),
        }
    }
    normalized
}

/// One measured line returned by the production word-wrapping algorithm.
#[derive(Debug, Clone, PartialEq)]
pub struct TextLineLayout {
    pub text: String,
    pub width: f32,
    pub height: f32,
}

/// Wrap text to the largest whole-glyph line that fits `max_width`.
///
/// The renderer consumes the returned strings and measurements directly;
/// the accessibility gate calls the same function at 200 percent scale.
pub fn layout_wrapped_text(text: &str, scale: f32, max_width: f32) -> Vec<TextLineLayout> {
    if text.is_empty() || scale <= 0.0 || max_width <= 0.0 {
        return Vec::new();
    }
    let text = normalize_bitmap_text(text);
    let glyph_width = GLYPH_W as f32 * scale;
    let glyph_height = GLYPH_H as f32 * scale;
    let max_chars = (max_width / glyph_width).floor().max(1.0) as usize;
    let mut output = Vec::new();
    let mut current = String::new();

    let push_line = |line: String, output: &mut Vec<TextLineLayout>| {
        let char_count = line.chars().count() as f32;
        output.push(TextLineLayout {
            text: line,
            width: char_count * glyph_width,
            height: glyph_height,
        });
    };

    for word in text.split_whitespace() {
        let word_len = word.chars().count();
        if word_len > max_chars {
            if !current.is_empty() {
                push_line(std::mem::take(&mut current), &mut output);
            }
            let chars: Vec<_> = word.chars().collect();
            for chunk in chars.chunks(max_chars) {
                push_line(chunk.iter().collect(), &mut output);
            }
            continue;
        }
        let separator = usize::from(!current.is_empty());
        if current.chars().count() + separator + word_len > max_chars {
            push_line(std::mem::take(&mut current), &mut output);
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        push_line(current, &mut output);
    }
    output
}

/// Fit a single-line label to a measured width, adding an ellipsis when
/// content cannot fit. This is used for the compact top status strip.
pub fn fit_text(text: &str, scale: f32, max_width: f32) -> String {
    if scale <= 0.0 || max_width <= 0.0 {
        return String::new();
    }
    let text = normalize_bitmap_text(text);
    let max_chars = (max_width / (GLYPH_W as f32 * scale)).floor() as usize;
    if text.chars().count() <= max_chars {
        return text;
    }
    if max_chars <= 3 {
        return ".".repeat(max_chars);
    }
    let mut fitted: String = text.chars().take(max_chars - 3).collect();
    fitted.push_str("...");
    fitted
}

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

impl TextMesh {
    /// Append another mesh while rebasing its indices for one GPU upload.
    ///
    /// Returns `false` when the combined mesh would exceed the renderer's
    /// fixed streaming buffers or the `u16` index range.
    pub fn append(&mut self, other: &Self) -> bool {
        let base = self.vertices.len();
        let new_vertex_count = base.saturating_add(other.vertices.len());
        let new_index_count = self.indices.len().saturating_add(other.indices.len());
        if new_vertex_count > MAX_VERTS
            || new_index_count > MAX_INDS
            || base > usize::from(u16::MAX)
            || other
                .indices
                .iter()
                .any(|index| base + usize::from(*index) > usize::from(u16::MAX))
        {
            return false;
        }
        self.vertices.extend_from_slice(&other.vertices);
        self.indices
            .extend(other.indices.iter().map(|index| *index + base as u16));
        true
    }
}

// ═════════════════════════════════════════════════════════════════════════
// BitmapFont – loads and holds the font texture
// ═════════════════════════════════════════════════════════════════════════

/// A GPU font atlas with per-glyph advances.
#[allow(missing_debug_implementations)]
pub struct BitmapFont {
    /// The GPU texture containing the atlas image.
    pub texture: wgpu::Texture,
    /// Default view of the atlas texture.
    pub view: wgpu::TextureView,
    /// Sampler used for the font texture.
    pub sampler: wgpu::Sampler,
    cell_width: f32,
    cell_height: f32,
    render_scale: f32,
    padding: f32,
    advances: [f32; (LAST_CHAR - FIRST_CHAR) as usize],
}

impl BitmapFont {
    #[allow(clippy::too_many_arguments)]
    fn from_rgba(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pixels: &[u8],
        width: u32,
        height: u32,
        cell_width: u32,
        cell_height: u32,
        render_scale: f32,
        padding: f32,
        advances: [f32; (LAST_CHAR - FIRST_CHAR) as usize],
        filter: wgpu::FilterMode,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("font atlas"),
            size: wgpu::Extent3d {
                width,
                height,
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
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("font sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: filter,
            min_filter: filter,
            mipmap_filter: filter,
            ..Default::default()
        });
        Self {
            texture,
            view,
            sampler,
            cell_width: cell_width as f32,
            cell_height: cell_height as f32,
            render_scale,
            padding,
            advances,
        }
    }

    /// Rasterize a proportional TrueType face into the GPU atlas.
    pub fn from_ttf_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        ttf_bytes: &[u8],
    ) -> Result<Self, String> {
        let rasterized = rasterize_ttf_atlas(ttf_bytes)?;
        Ok(Self::from_rgba(
            device,
            queue,
            &rasterized.pixels,
            VECTOR_CELL_W * ATLAS_COLS,
            VECTOR_CELL_H * ATLAS_ROWS,
            VECTOR_CELL_W,
            VECTOR_CELL_H,
            VECTOR_RENDER_SCALE,
            VECTOR_PADDING,
            rasterized.advances,
            wgpu::FilterMode::Linear,
        ))
    }

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

        Ok(Self::from_rgba(
            device,
            queue,
            img.as_raw(),
            w,
            h,
            GLYPH_W,
            GLYPH_H,
            1.0,
            0.0,
            [GLYPH_W as f32; (LAST_CHAR - FIRST_CHAR) as usize],
            wgpu::FilterMode::Nearest,
        ))
    }

    /// Compute UV coordinates `[[u0,v0],[u1,v1]]` for a character.
    ///
    /// Returns `None` for characters outside ASCII 32–126.
    pub fn char_uv(&self, c: char) -> Option<[[f32; 2]; 2]> {
        let code = u32::from(c);
        if !(u32::from(FIRST_CHAR)..u32::from(LAST_CHAR)).contains(&code) {
            return None;
        }
        let idx = code as u8 - FIRST_CHAR;
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
    /// * `text` – the string to render (typographic punctuation is normalized).
    /// * `x`, `y` – top-left corner in **pixel** coordinates (origin at
    ///   top-left of the viewport).
    /// * `scale` – logical multiplier; vector faces preserve the legacy UI size.
    /// * `color` – tint applied to each glyph (RGBA, 0–1).
    /// * `screen_w`, `screen_h` – viewport dimensions for NDC conversion.
    pub fn text_width(&self, text: &str, scale: f32) -> f32 {
        let factor = scale * self.render_scale;
        normalize_bitmap_text(text)
            .chars()
            .filter_map(|character| {
                let code = character as u32;
                if (u32::from(FIRST_CHAR)..u32::from(LAST_CHAR)).contains(&code) {
                    Some(self.advances[code as usize - usize::from(FIRST_CHAR)] * factor)
                } else {
                    None
                }
            })
            .sum()
    }

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
        let factor = scale * self.render_scale;
        let gw = self.cell_width * factor;
        let gh = self.cell_height * factor;
        let mut caret_x = x;

        for c in normalize_bitmap_text(text).chars() {
            let Some(uv) = self.char_uv(c) else {
                continue;
            };

            let index = c as usize - usize::from(FIRST_CHAR);
            let cx = caret_x - self.padding * factor;
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
            caret_x += self.advances[index] * factor;
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

    /// Batch several text meshes into one upload and one draw call.
    ///
    /// Every text element in a render pass must use this method together.
    /// Repeated calls to [`Self::render`] in one unsubmitted pass would
    /// overwrite the shared streaming buffer before the GPU consumes it.
    pub fn render_many<'pass, 'mesh>(
        &'pass self,
        queue: &wgpu::Queue,
        rpass: &mut wgpu::RenderPass<'pass>,
        meshes: impl IntoIterator<Item = &'mesh TextMesh>,
    ) {
        let mut combined = TextMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
        };
        for mesh in meshes {
            if !combined.append(mesh) {
                eprintln!("text batch exceeds the renderer's streaming capacity");
                return;
            }
        }
        self.render(queue, rpass, &combined);
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

#[cfg(test)]
mod layout_tests {
    use super::*;
    use crate::ui_contract::{
        HUD_ACTION_SELECTED, HUD_INSTRUCTION_IDLE, HUD_INSTRUCTION_SELECTED,
        HUD_INSTRUCTION_TARGETING,
    };

    #[test]
    fn shipped_hud_copy_fits_at_two_hundred_percent() {
        let scale = 4.0;
        let max_width = 1920.0 - 24.0;
        let strings = [
            HUD_ACTION_SELECTED,
            HUD_INSTRUCTION_IDLE,
            HUD_INSTRUCTION_SELECTED,
            HUD_INSTRUCTION_TARGETING,
        ];
        let lines: Vec<_> = strings
            .into_iter()
            .flat_map(|text| layout_wrapped_text(text, scale, max_width))
            .collect();
        assert!(lines.iter().all(|line| line.width <= max_width));
        let total_height: f32 = lines.iter().map(|line| line.height + 6.0).sum();
        assert!(total_height <= 1080.0 * 0.45);
    }

    #[test]
    fn long_unbroken_words_are_hard_wrapped() {
        let lines = layout_wrapped_text("abcdefghijkl", 1.0, 32.0);
        assert_eq!(lines.len(), 3);
        assert!(lines.iter().all(|line| line.width <= 32.0));
    }

    #[test]
    fn typographic_copy_maps_to_shipped_ascii_glyphs() {
        assert_eq!(
            normalize_bitmap_text("Elk Creek — “ready”… •"),
            "Elk Creek - \"ready\"... *"
        );
        assert_eq!(normalize_bitmap_text("Kiowa ł"), "Kiowa ?");
        assert!(normalize_bitmap_text("Line\nBreak").is_ascii());
    }

    #[test]
    fn text_mesh_batch_rebases_indices_without_overwriting_prior_geometry() {
        let vertex = TextVertex {
            position: [0.0; 3],
            uv: [0.0; 2],
            color: [1.0; 4],
        };
        let mut combined = TextMesh {
            vertices: vec![vertex; 4],
            indices: vec![0, 1, 2, 2, 3, 0],
        };
        let second = TextMesh {
            vertices: vec![vertex; 4],
            indices: vec![0, 1, 2, 2, 3, 0],
        };
        assert!(combined.append(&second));
        assert_eq!(&combined.indices[6..], &[4, 5, 6, 6, 7, 4]);
    }

    #[test]
    fn embedded_serif_font_rasterizes_with_proportional_advances() {
        let bytes = include_bytes!("../../../assets/fonts/DejaVuSerif.ttf");
        let Ok(rasterized) = rasterize_ttf_atlas(bytes) else {
            panic!("embedded DejaVu Serif must rasterize");
        };
        let i = usize::from(b'I' - FIRST_CHAR);
        let w = usize::from(b'W' - FIRST_CHAR);
        assert!(rasterized.advances[w] > rasterized.advances[i]);
        assert!(rasterized.pixels.chunks_exact(4).any(|pixel| pixel[3] > 0));
    }
}
