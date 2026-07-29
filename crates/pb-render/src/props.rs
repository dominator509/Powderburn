//! Deterministic presentation-only environmental props.
//!
//! Props are derived from canonical terrain and cover state. They never enter
//! the simulation hash and cannot affect pathing, cover, or combat results.

use std::collections::BTreeMap;
use std::sync::Arc;

use pb_core::geom::TileXY;

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

/// Derive a stable, sparse set of prop sprites from terrain and cover.
pub fn prop_instances_from_state(state: &pb_sim::state::SimState) -> Vec<SpriteInstance> {
    let cols = state.smoke_cols.max(1);
    let rows = state.smoke_rows.max(1);
    let mut cells = BTreeMap::<TileXY, u8>::new();

    // Terrain establishes the ambient layer. Sparse difficult-ground fallback
    // prevents authored maps with little explicit material data from appearing
    // empty without inventing gameplay-affecting obstacles.
    for (tile, terrain) in &state.terrain_tiles {
        if !in_bounds(*tile, cols, rows) {
            continue;
        }
        let material = terrain.to_ascii_lowercase();
        let prop = if material.contains("rail") || material.contains("ballast") {
            Some(10)
        } else if material.contains("timber")
            || material.contains("cottonwood")
            || material.contains("woodland")
        {
            Some(7)
        } else if material.contains("brush")
            || material.contains("sage")
            || material.contains("scrub")
        {
            Some(11)
        } else if material.contains("rock") || material.contains("sandstone") {
            Some(12)
        } else {
            None
        };
        if let Some(prop) = prop {
            cells.entry(*tile).or_insert(prop);
        }
    }

    for tile in &state.difficult_tiles {
        if in_bounds(*tile, cols, rows) && stable_cell(*tile, state.scenario_id) % 7 == 0 {
            cells.entry(*tile).or_insert(11);
        }
    }

    // Authored cover has priority over ambient decoration so what the player
    // sees corresponds to the canonical cover topology.
    for (edge, cover) in &state.cover_edges {
        if !in_bounds(edge.tile, cols, rows) {
            continue;
        }
        let variant = stable_cell(edge.tile, state.scenario_id);
        let prop = if cover.level >= pb_sim::state::CoverLevel::Hard {
            if variant % 2 == 0 {
                6
            } else {
                1
            }
        } else if variant % 3 == 0 {
            15
        } else {
            0
        };
        cells.insert(edge.tile, prop);
    }

    let mut ordered: Vec<_> = cells.into_iter().collect();
    ordered.sort_by_key(|(tile, _)| (tile.x + tile.y, tile.y, tile.x));
    ordered
        .into_iter()
        .map(|(tile, prop)| {
            let elevation = state.tile_elevations.get(&tile).copied().unwrap_or(0);
            prop_sprite(tile, prop, elevation)
        })
        .collect()
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

fn prop_sprite(tile: TileXY, prop: u8, elevation: i32) -> SpriteInstance {
    let grid_x = f32::from(tile.x);
    let grid_y = f32::from(tile.y);
    let iso_x = (grid_x - grid_y) * 32.0;
    let tile_y = (grid_x + grid_y) * 16.0 + elevation as f32 * crate::tiles::ELEVATION_SCREEN_STEP;
    let (width, height) = match prop {
        3 | 4 | 7 | 8 => (112.0, 112.0),
        9 | 10 => (104.0, 88.0),
        11 | 12 => (82.0, 76.0),
        _ => (92.0, 82.0),
    };
    let mut sprite = SpriteInstance::new(iso_x, tile_y - height * 0.5 + 14.0, 8.0 + grid_y);
    sprite.width = width;
    sprite.height = height;
    let column = f32::from(prop % 4);
    let row = f32::from((prop / 4).min(3));
    sprite.u0 = column * 0.25;
    sprite.v0 = row * 0.25;
    sprite.u1 = sprite.u0 + 0.25;
    sprite.v1 = sprite.v0 + 0.25;
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
        assert_eq!(first.len(), 2);
        assert_eq!(before, pb_sim::hash::compute_state_hash(&state));
    }
}
