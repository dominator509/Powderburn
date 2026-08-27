//! Isometric tile rendering system.
//!
//! Renders the battlefield as a grid of textured isometric tiles using wgpu.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use pb_core::geom::TileXY;
use pb_sim::state::SimState;

use crate::device::RenderDevice;

/// The authored tactical cell footprint at zoom = 1.0.
///
/// Every presentation system uses this contract: terrain, smoke, overlays,
/// props, actor anchors, and pointer picking. Keeping the dimensions here
/// prevents a second projection from drifting onto tile intersections.
pub const TILE_WIDTH: f32 = 64.0;
pub const TILE_HEIGHT: f32 = 32.0;
pub const TILE_HALF_WIDTH: f32 = TILE_WIDTH * 0.5;
pub const TILE_HALF_HEIGHT: f32 = TILE_HEIGHT * 0.5;

/// Overlay-only gutter in world pixels. The terrain still occupies the full
/// diamond; the gutter keeps adjacent highlighted cells visually distinct.
pub const TILE_HIGHLIGHT_GUTTER_X: f32 = 1.5;
pub const TILE_HIGHLIGHT_GUTTER_Y: f32 = 0.75;

/// Vertical screen-space relief represented by one authored elevation level.
pub const ELEVATION_SCREEN_STEP: f32 = 12.0;

/// Project an unsigned grid coordinate to the center of exactly one tile.
pub fn iso_tile_center(x: u32, y: u32) -> [f32; 2] {
    iso_world_center(x as f32, y as f32)
}

/// Project continuous presentation coordinates. Movement animation uses this
/// form so interpolation never snaps back to an integer tile between frames.
pub fn iso_world_center(x: f32, y: f32) -> [f32; 2] {
    [(x - y) * TILE_HALF_WIDTH, (x + y) * TILE_HALF_HEIGHT]
}

/// Project a signed gameplay coordinate to the center of exactly one tile.
pub fn tile_center(tile: TileXY) -> [f32; 2] {
    iso_world_center(f32::from(tile.x), f32::from(tile.y))
}

/// Project a tile center including its authored elevation.
pub fn tile_center_with_elevation(tile: TileXY, elevation: i32) -> [f32; 2] {
    let [x, y] = tile_center(tile);
    [x, y + elevation as f32 * ELEVATION_SCREEN_STEP]
}

/// Project continuous presentation coordinates including interpolated
/// elevation, preserving a moving actor's exact world position.
pub fn iso_world_center_with_elevation(x: f32, y: f32, elevation: f32) -> [f32; 2] {
    let [iso_x, iso_y] = iso_world_center(x, y);
    [iso_x, iso_y + elevation * ELEVATION_SCREEN_STEP]
}

/// Return the normalized Manhattan distance from a world point to a tile's
/// isometric diamond. A value at or below one is inside the tile footprint.
pub fn tile_diamond_distance(world_x: f32, world_y: f32, tile: TileXY, elevation: i32) -> f32 {
    let [center_x, center_y] = tile_center_with_elevation(tile, elevation);
    (world_x - center_x).abs() / TILE_HALF_WIDTH + (world_y - center_y).abs() / TILE_HALF_HEIGHT
}

/// Test whether a world point belongs to one rendered tile footprint.
pub fn tile_diamond_contains(world_x: f32, world_y: f32, tile: TileXY, elevation: i32) -> bool {
    tile_diamond_distance(world_x, world_y, tile, elevation) <= 1.001
}

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

/// Stable presentation seed for a grid cell.
///
/// This is intentionally separate from the simulation RNG. Battlefield
/// dressing must not consume combat randomness or change a replay outcome.
pub fn stable_tile_seed(tile: TileXY, scenario_id: u32) -> u32 {
    u32::try_from(tile.x).unwrap_or_default().wrapping_mul(31)
        ^ u32::try_from(tile.y).unwrap_or_default().wrapping_mul(17)
        ^ scenario_id.wrapping_mul(13)
}

/// Return the terrain material the player should see for one cell.
///
/// Scenario files author the mechanically meaningful special cells. The
/// remaining cells still need a legible biome so an otherwise valid map does
/// not render as a flat untextured slab. These inferred materials are
/// presentation-only: difficult terrain, cover, elevation, and pathing stay
/// sourced from the canonical simulation state.
pub fn visual_material_for_tile(state: &SimState, tile: TileXY) -> u8 {
    if let Some(terrain) = state.terrain_tiles.get(&tile) {
        return material_for_terrain(terrain);
    }

    let seed = stable_tile_seed(tile, state.scenario_id);
    let region = TileXY::new(tile.x.div_euclid(3), tile.y.div_euclid(3));
    let region_seed = stable_tile_seed(region, state.scenario_id);
    let authored_materials = state
        .terrain_tiles
        .values()
        .map(|terrain| material_for_terrain(terrain))
        .collect::<Vec<_>>();
    let authored_has = |material: u8| authored_materials.contains(&material);

    if state.weather == pb_sim::environment::Weather::Snow || authored_has(5) {
        return if seed % 9 == 0 { 3 } else { 5 };
    }

    if authored_has(7) {
        // Keep rail-grade dressing as a readable route without turning the
        // entire map into a single grey material.
        return if (tile.x.unsigned_abs() + tile.y.unsigned_abs()) % 6 == 0 {
            7
        } else if region_seed % 7 == 0 {
            1
        } else {
            0
        };
    }
    if authored_has(6) {
        return if region_seed % 17 == 0 {
            6
        } else if region_seed % 7 == 0 {
            1
        } else {
            0
        };
    }
    if authored_has(4) {
        return if region_seed % 31 == 0 {
            4
        } else if region_seed % 9 == 0 {
            3
        } else {
            0
        };
    }
    if authored_has(3) {
        return if region_seed % 7 == 0 { 3 } else { 0 };
    }
    if authored_has(2) {
        return if region_seed % 19 == 0 {
            2
        } else if region_seed % 7 == 0 {
            3
        } else {
            0
        };
    }

    // Empty proving maps and legacy maps still receive a distinct frontier
    // identity. The scenario hash makes the look stable per battlefield.
    let center_x = (state.smoke_cols.max(1) / 2) as i16;
    let center_y = (state.smoke_rows.max(1) / 2) as i16;
    match state.scenario_id % 4 {
        0 if tile.y == center_y && tile.x > 1 && tile.x + 2 < state.smoke_cols as i16 => 1,
        1 if tile.y == center_y => 7,
        2 if tile.y == center_y || tile.x == center_x => 1,
        3 if tile.x == center_x && tile.y > 1 && tile.y + 2 < state.smoke_rows as i16 => 2,
        0 => match seed % 9 {
            0 if region_seed % 3 == 0 => 1,
            1 if region_seed % 5 == 0 => 3,
            _ => 0,
        },
        1 => {
            if region_seed % 23 == 0 {
                1
            } else if region_seed % 11 == 0 {
                3
            } else {
                0
            }
        }
        2 => match seed % 11 {
            0 if region_seed % 17 == 0 => 6,
            1 if region_seed % 7 == 0 => 1,
            _ => 0,
        },
        _ => match seed % 10 {
            0 if region_seed % 37 == 0 => 4,
            1 if region_seed % 7 == 0 => 3,
            _ => 0,
        },
    }
}

/// Build one textured tile from the shared battlefield presentation contract.
pub fn visual_tile_for_state(state: &SimState, tile: TileXY) -> TileVisual {
    let material = visual_material_for_tile(state, tile);
    let seed = stable_tile_seed(tile, state.scenario_id);
    let variation = (seed % 7) as f32 * 0.014;
    let base = match material {
        2 => 0.90,
        5 => 0.97,
        6 => 0.91,
        7 => 0.87,
        _ => 0.92,
    };
    let elevation = state.tile_elevations.get(&tile).copied().unwrap_or(0);
    let shade = (base + variation).clamp(0.78, 1.05);
    TileVisual::new(shade, shade, shade, elevation).with_material(material)
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
        let half_w = TILE_HALF_WIDTH;
        let half_h = TILE_HALF_HEIGHT;

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
                let [iso_x, base_iso_y] = iso_tile_center(x, y);
                let iso_y = base_iso_y + tile.elevation as f32 * ELEVATION_SCREEN_STEP;
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
    fn shared_projection_centers_and_elevation_are_consistent() {
        assert_eq!(iso_tile_center(5, 5), [0.0, 160.0]);
        assert_eq!(tile_center(TileXY::new(6, 5)), [32.0, 176.0]);
        assert_eq!(
            tile_center_with_elevation(TileXY::new(5, 5), 2),
            [0.0, 184.0]
        );
    }

    #[test]
    fn each_cell_has_one_full_diamond_footprint() {
        let tile = TileXY::new(5, 5);
        let [center_x, center_y] = tile_center(tile);

        assert_eq!(tile_diamond_distance(center_x, center_y, tile, 0), 0.0);
        assert!(tile_diamond_contains(center_x, center_y, tile, 0));
        assert!(tile_diamond_contains(
            center_x + TILE_HALF_WIDTH - 0.01,
            center_y,
            tile,
            0
        ));
        assert!(!tile_diamond_contains(
            center_x + TILE_HALF_WIDTH + 0.10,
            center_y,
            tile,
            0
        ));
    }

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
