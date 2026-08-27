//! Deterministic presentation-only environmental props.
//!
//! Props are derived from canonical terrain and cover state. They never enter
//! the simulation hash and cannot affect pathing, cover, or combat results.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use pb_core::geom::{Facing, TileXY};

use crate::device::RenderDevice;
use crate::sprites::{SpriteInstance, SpriteSystem};

const PROP_ATLAS: &[u8] = include_bytes!("../../../assets/props/frontier_prop_atlas.png");

/// A renderer for presentation-only frontier props.
#[allow(missing_debug_implementations)]
pub struct PropSystem(SpriteSystem);

impl PropSystem {
    /// Build the prop system from presentation instances derived from state.
    pub fn new(
        device: &Arc<RenderDevice>,
        props: &[SpriteInstance],
        camera_matrix_bytes: &[u8; 64],
    ) -> Self {
        Self(SpriteSystem::new_with_atlas(
            device,
            props,
            camera_matrix_bytes,
            PROP_ATLAS,
            "frontier environmental prop atlas",
        ))
    }

    /// Build props with a pipeline matching the destination target.
    pub fn new_with_format(
        device: &Arc<RenderDevice>,
        props: &[SpriteInstance],
        camera_matrix_bytes: &[u8; 64],
        target_format: wgpu::TextureFormat,
    ) -> Self {
        Self(SpriteSystem::new_with_atlas_and_format(
            device,
            props,
            camera_matrix_bytes,
            PROP_ATLAS,
            "frontier environmental prop atlas",
            target_format,
        ))
    }

    /// Update the tactical camera without recreating the prop atlas/pipeline.
    pub fn update_camera(&self, device: &Arc<RenderDevice>, camera_matrix_bytes: &[u8; 64]) {
        self.0.update_camera(device, camera_matrix_bytes);
    }

    /// Refresh state-derived props without recreating their atlas/pipeline.
    pub fn update(
        &mut self,
        device: &Arc<RenderDevice>,
        props: &[SpriteInstance],
        camera_matrix_bytes: &[u8; 64],
    ) {
        self.0.update(device, props, camera_matrix_bytes);
    }

    /// Draw all props inside an existing render pass.
    pub fn render<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>) {
        self.0.render(rpass);
    }
}

#[derive(Debug, Clone, Copy)]
struct PropPlacement {
    prop: u8,
    rotation: f32,
}

/// Derive a stable set of readable battlefield dressing from terrain and cover.
///
/// Authored cover always wins at its tile. The ambient layer is deliberately
/// presentation-only and skips occupied cells, so it gives empty/legacy maps
/// a composed frontier battlefield without creating invisible blockers or
/// changing the simulation's AP, LOS, or damage rules.
pub fn prop_instances_from_state(state: &pb_sim::state::SimState) -> Vec<SpriteInstance> {
    let cols = state.smoke_cols.max(1);
    let rows = state.smoke_rows.max(1);
    let occupied = state
        .actors
        .values()
        .map(|actor| actor.position)
        .collect::<BTreeSet<_>>();
    let mut cells = BTreeMap::<TileXY, PropPlacement>::new();

    // A small amount of intentional density is what makes a tactical map read
    // as a place instead of a spreadsheet. The seed is stable per cell and
    // independent of the combat RNG, so every replay sees the same dressing.
    for y in 0..rows {
        for x in 0..cols {
            let tile = TileXY::new(x as i16, y as i16);
            if occupied.contains(&tile) {
                continue;
            }
            let seed = stable_cell(tile, state.scenario_id);
            let material = crate::tiles::visual_material_for_tile(state, tile);
            if let Some((prop, rotation)) = ambient_prop(material, seed) {
                cells.insert(tile, PropPlacement { prop, rotation });
            }
        }
    }

    // Preserve a visual signature for explicitly authored terrain even when
    // the ambient density roll lands empty on that cell.
    for (tile, terrain) in &state.terrain_tiles {
        if !in_bounds(*tile, cols, rows) || occupied.contains(tile) {
            continue;
        }
        let terrain_name = terrain.to_ascii_lowercase();
        let prop = if terrain_name.contains("rail") || terrain_name.contains("ballast") {
            Some(10)
        } else if terrain_name.contains("timber")
            || terrain_name.contains("cottonwood")
            || terrain_name.contains("woodland")
        {
            Some(7)
        } else if terrain_name.contains("brush")
            || terrain_name.contains("sage")
            || terrain_name.contains("scrub")
        {
            Some(11)
        } else if terrain_name.contains("creek")
            || terrain_name.contains("rock")
            || terrain_name.contains("sandstone")
        {
            Some(12)
        } else {
            None
        };
        if let Some(prop) = prop {
            let rotation = if stable_cell(*tile, state.scenario_id) & 1 == 0 {
                std::f32::consts::FRAC_PI_4
            } else {
                -std::f32::consts::FRAC_PI_4
            };
            cells
                .entry(*tile)
                .or_insert(PropPlacement { prop, rotation });
        }
    }

    // Four composition anchors give each battlefield a readable silhouette:
    // a landmark at the four approaches, with the exact set chosen from the
    // authored biome. These are scenery, not cover, and are moved off an
    // occupied/covered cell rather than hiding a tactical signal.
    let landmark_props = match battlefield_style(state) {
        0 => [4, 7, 14, 0], // open range: wagon, cottonwood, fire, fence
        1 => [9, 8, 1, 10], // rail: gun cart, telegraph, freight, rail
        2 => [6, 3, 1, 14], // settlement: wall, tent, crates, fire
        _ => [7, 4, 12, 0], // creek/brush: tree, wagon, rock, fence
    };
    let anchors = [
        TileXY::new(2, 2),
        TileXY::new(cols.saturating_sub(3) as i16, 2),
        TileXY::new(2, rows.saturating_sub(3) as i16),
        TileXY::new(cols.saturating_sub(3) as i16, rows.saturating_sub(3) as i16),
    ];
    for (slot, (&anchor, &prop)) in anchors.iter().zip(&landmark_props).enumerate() {
        for attempt in 0..8_i16 {
            let candidate = TileXY::new(
                anchor.x.saturating_add((attempt % 3) - 1),
                anchor.y.saturating_add((attempt / 3) - 1),
            );
            let has_cover = state.cover_edges.iter().any(|(edge, cover)| {
                edge.tile == candidate && cover.level != pb_sim::state::CoverLevel::None
            });
            if !in_bounds(candidate, cols, rows)
                || occupied.contains(&candidate)
                || has_cover
                || cells.contains_key(&candidate)
            {
                continue;
            }
            let rotation =
                if (slot + (stable_cell(candidate, state.scenario_id) & 1) as usize) % 2 == 0 {
                    std::f32::consts::FRAC_PI_4
                } else {
                    -std::f32::consts::FRAC_PI_4
                };
            cells.insert(candidate, PropPlacement { prop, rotation });
            break;
        }
    }

    // Authored cover has priority over ambient decoration so what the player
    // sees corresponds to the canonical cover topology. Orienting each prop
    // from its actual cover edge keeps the visual barrier on the same facing
    // as the simulation's cover record.
    for (edge, cover) in &state.cover_edges {
        if !in_bounds(edge.tile, cols, rows) || occupied.contains(&edge.tile) {
            continue;
        }
        if cover.level == pb_sim::state::CoverLevel::None {
            continue;
        }
        let seed = stable_cell(edge.tile, state.scenario_id);
        let prop = if cover.level >= pb_sim::state::CoverLevel::Hard {
            if cover.half_height || seed % 3 == 0 {
                15
            } else {
                6
            }
        } else if cover.level == pb_sim::state::CoverLevel::Soft {
            if seed % 3 == 0 {
                11
            } else {
                0
            }
        } else if seed % 3 == 0 {
            15
        } else {
            11
        };
        cells.insert(
            edge.tile,
            PropPlacement {
                prop,
                rotation: cover_rotation(edge.facing),
            },
        );
    }

    let mut ordered: Vec<_> = cells.into_iter().collect();
    ordered.sort_by_key(|(tile, _)| (tile.x + tile.y, tile.y, tile.x));
    ordered
        .into_iter()
        .map(|(tile, placement)| {
            let elevation = state.tile_elevations.get(&tile).copied().unwrap_or(0);
            prop_sprite(tile, placement.prop, elevation, placement.rotation)
        })
        .collect()
}

fn ambient_prop(material: u8, seed: u32) -> Option<(u8, f32)> {
    let diagonal = if seed & 1 == 0 {
        std::f32::consts::FRAC_PI_4
    } else {
        -std::f32::consts::FRAC_PI_4
    };
    match material {
        // Grassland: low rocks and sage create readable micro-landmarks.
        0 => match seed % 17 {
            0 => Some((12, diagonal)),
            1 => Some((11, -diagonal)),
            _ => None,
        },
        // Road/dirt: a wagon, wheel, or supply cache breaks up the route.
        1 => match seed % 19 {
            0 => Some((4, diagonal)),
            1 => Some((5, -diagonal)),
            2 => Some((1, 0.0)),
            _ => None,
        },
        // Water/creek: exposed stones and bank brush, never a blocking wall.
        2 => match seed % 5 {
            0 => Some((12, diagonal)),
            1 => Some((11, -diagonal)),
            _ => None,
        },
        // Brush/scrub: dense enough to read at tactical zoom, not a solid wall.
        3 => match seed % 4 {
            0 => Some((11, diagonal)),
            1 if seed % 13 == 0 => Some((12, -diagonal)),
            _ => None,
        },
        // Timber: tree clusters use a lower frequency so lanes stay visible.
        4 => match seed % 7 {
            0 => Some((7, diagonal)),
            1 if seed % 11 == 0 => Some((12, -diagonal)),
            _ => None,
        },
        // Snow: rocks and sparse brush preserve contrast against the white.
        5 => match seed % 13 {
            0 => Some((12, diagonal)),
            1 => Some((11, -diagonal)),
            _ => None,
        },
        // Adobe/settlement: supplies, tents, walls, and one warm focal point.
        6 => match seed % 17 {
            0 => Some((1, 0.0)),
            1 => Some((2, diagonal)),
            2 if seed % 29 == 0 => Some((3, 0.0)),
            3 if seed % 31 == 0 => Some((14, 0.0)),
            _ => None,
        },
        // Rail/ballast: keep the line legible and place occasional rail-side
        // freight/equipment rather than flooding the board with repeated rail.
        7 => match seed % 13 {
            0 => Some((10, 0.0)),
            1 => Some((1, diagonal)),
            _ => None,
        },
        _ => None,
    }
}

fn battlefield_style(state: &pb_sim::state::SimState) -> u8 {
    if state.weather == pb_sim::environment::Weather::Snow {
        return 3;
    }
    let materials = state
        .terrain_tiles
        .values()
        .map(|terrain| crate::tiles::material_for_terrain(terrain))
        .collect::<BTreeSet<_>>();
    if materials.contains(&7) {
        1
    } else if materials.contains(&6) {
        2
    } else if materials.contains(&2) {
        3
    } else if materials.contains(&4) {
        0
    } else {
        (state.scenario_id % 4) as u8
    }
}

fn cover_rotation(facing: Facing) -> f32 {
    match facing {
        Facing::North | Facing::South => 0.0,
        Facing::East | Facing::West => std::f32::consts::FRAC_PI_2,
        Facing::NorthEast | Facing::SouthWest => -std::f32::consts::FRAC_PI_4,
        Facing::SouthEast | Facing::NorthWest => std::f32::consts::FRAC_PI_4,
    }
}

fn in_bounds(tile: TileXY, cols: u32, rows: u32) -> bool {
    tile.x >= 0
        && tile.y >= 0
        && u32::try_from(tile.x).is_ok_and(|x| x < cols)
        && u32::try_from(tile.y).is_ok_and(|y| y < rows)
}

fn stable_cell(tile: TileXY, scenario_id: u32) -> u32 {
    u32::try_from(tile.x).unwrap_or_default().wrapping_mul(31)
        ^ u32::try_from(tile.y).unwrap_or_default().wrapping_mul(17)
        ^ scenario_id.wrapping_mul(13)
}

fn prop_sprite(tile: TileXY, prop: u8, elevation: i32, rotation: f32) -> SpriteInstance {
    let [iso_x, tile_y] = crate::tiles::tile_center_with_elevation(tile, elevation);
    let grid_y = f32::from(tile.y);
    let (width, height) = match prop {
        3 | 4 | 7 | 8 => (92.0, 94.0),
        9 | 10 => (88.0, 76.0),
        11 | 12 => (68.0, 64.0),
        _ => (78.0, 70.0),
    };
    let mut sprite = SpriteInstance::new(
        iso_x,
        tile_y - height * 0.5 + 14.0,
        8.0 + grid_y + elevation as f32 * 0.1,
    );
    sprite.width = width;
    sprite.height = height;
    sprite.rotation = rotation;
    let column = f32::from(prop % 4);
    let row = f32::from((prop / 4).min(3));
    // Keep linear filtering inside the selected atlas cell. Sampling across
    // the cell boundary is what produces the saturated backing-color fringe.
    let inset = 0.004;
    sprite.u0 = column * 0.25 + inset;
    sprite.v0 = row * 0.25 + inset;
    sprite.u1 = (column + 1.0) * 0.25 - inset;
    sprite.v1 = (row + 1.0) * 0.25 - inset;
    sprite
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prop_derivation_is_stable_and_presentation_only() {
        let mut state = pb_sim::state::SimState::new(1867, 3);
        state.smoke_cols = 12;
        state.smoke_rows = 8;
        state
            .terrain_tiles
            .insert(TileXY::new(2, 3), "Rail".to_string());
        state
            .terrain_tiles
            .insert(TileXY::new(7, 1), "Sagebrush".to_string());
        let before = pb_sim::hash::compute_state_hash(&state);

        let first = prop_instances_from_state(&state);
        let second = prop_instances_from_state(&state);

        assert_eq!(first, second);
        assert!(first.len() >= 2);
        assert_eq!(before, pb_sim::hash::compute_state_hash(&state));
    }

    #[test]
    fn empty_battlefield_receives_composed_dressing_without_state_mutation() {
        let mut state = pb_sim::state::SimState::new(1867, 7);
        state.smoke_cols = 20;
        state.smoke_rows = 12;
        let before = pb_sim::hash::compute_state_hash(&state);

        let props = prop_instances_from_state(&state);

        assert!(
            props.len() >= 12,
            "empty board should not render barren: {}",
            props.len()
        );
        assert!(props.iter().any(|sprite| sprite.u0 >= 0.0));
        assert_eq!(before, pb_sim::hash::compute_state_hash(&state));
    }
}
