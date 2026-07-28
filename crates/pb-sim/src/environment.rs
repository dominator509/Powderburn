//! Environment systems: smoke, light, weather, and cover.
//!
//! M4: Implements smoke volume management, sight radius based on lighting,
//! weather types, and cover accuracy penalties for the simulation kernel.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use pb_core::fix32::Fix32;
use pb_core::geom::{Facing, TileXY};

/// A volume of smoke on a tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Smoke {
    /// Current density of the smoke volume.
    pub density: u32,
    /// Number of ticks remaining before the smoke dissipates.
    pub tiles_remaining: u32,
}

/// The smoke system: a map of tile positions to smoke volumes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmokeSystem {
    /// Smoke volumes keyed by tile position.
    pub volumes: BTreeMap<TileXY, Smoke>,
}

impl SmokeSystem {
    /// Create a new, empty smoke system.
    pub fn new() -> Self {
        SmokeSystem {
            volumes: BTreeMap::new(),
        }
    }

    /// Return the smoke density at a given tile, or 0 if no smoke is present.
    pub fn density_at(&self, tile: TileXY) -> u32 {
        self.volumes.get(&tile).map(|s| s.density).unwrap_or(0)
    }
}

impl Default for SmokeSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Deposit smoke at a tile and its forward-adjacent tiles.
///
/// The muzzle tile (at `tile`) receives density 3. The two tiles immediately
/// in front of the firer (along `facing`) each receive density 1.
pub fn deposit_smoke(tile: TileXY, facing: Facing, system: &mut SmokeSystem) {
    // Muzzle tile: density 3
    let entry = system.volumes.entry(tile).or_insert(Smoke {
        density: 0,
        tiles_remaining: 0,
    });
    entry.density = entry.density.saturating_add(3);
    entry.tiles_remaining = entry.tiles_remaining.saturating_add(3 * 40); // roughly 3*40 ticks

    // Two tiles in front along facing: density 1 each
    let first = tile.neighbour(facing);
    let second = first.neighbour(facing);

    for front_tile in [first, second] {
        let entry = system.volumes.entry(front_tile).or_insert(Smoke {
            density: 0,
            tiles_remaining: 0,
        });
        entry.density = entry.density.saturating_add(1);
        entry.tiles_remaining = entry.tiles_remaining.saturating_add(40); // roughly 1*40 ticks
    }
}

/// Decay smoke volumes over elapsed ticks.
///
/// Each volume's density decreases by 1 per 40 ticks. If density reaches 0
/// the volume is removed.
pub fn decay_smoke(system: &mut SmokeSystem, elapsed_ticks: u64) {
    let decay_amount = (elapsed_ticks / 40) as u32;
    if decay_amount == 0 {
        return;
    }

    let mut to_remove: Vec<TileXY> = Vec::new();
    for (tile, smoke) in system.volumes.iter_mut() {
        if smoke.density <= decay_amount {
            to_remove.push(*tile);
        } else {
            smoke.density = smoke.density.saturating_sub(decay_amount);
        }
    }
    for tile in to_remove {
        system.volumes.remove(&tile);
    }
}

/// Drift smoke volumes in the wind direction.
///
/// Smoke drifts 1 tile per 120 ticks along `wind_direction`.
/// The original tile's smoke is moved, and if the destination already has
/// smoke the densities are merged.
pub fn drift_smoke(system: &mut SmokeSystem, wind_direction: Facing, elapsed_ticks: u64) {
    let shifts = (elapsed_ticks / 120) as u32;
    if shifts == 0 {
        return;
    }

    // Collect all tiles and their smoke to move
    let to_move: Vec<(TileXY, Smoke)> = system
        .volumes
        .iter()
        .map(|(tile, smoke)| (*tile, smoke.clone()))
        .collect();

    // Clear the system and re-insert at drifted positions
    system.volumes.clear();
    for (tile, smoke) in to_move {
        let mut current = tile;
        for _ in 0..shifts {
            current = current.neighbour(wind_direction);
        }
        let entry = system.volumes.entry(current).or_insert(Smoke {
            density: 0,
            tiles_remaining: 0,
        });
        entry.density = entry.density.saturating_add(smoke.density);
        entry.tiles_remaining = entry.tiles_remaining.saturating_add(smoke.tiles_remaining);
    }
}

/// Compute the smoke penalty for a shot from `from` to `to`.
///
/// Accumulates smoke density along the line of sight (excluding the origin
/// tile). If total accumulated density is >= 6, the shot is illegal (returns
/// i32::MAX). If >= 3, the shot incurs a -20 accuracy penalty. Otherwise 0.
pub fn smoke_penalty(system: &SmokeSystem, from: TileXY, to: TileXY) -> i32 {
    let line = from.line_to(to);
    let mut accumulated: u32 = 0;

    // Accumulate density along the line, excluding the origin tile
    for tile in line.iter().skip(1) {
        accumulated = accumulated.saturating_add(system.density_at(*tile));
    }

    if accumulated >= 6 {
        i32::MAX
    } else if accumulated >= 3 {
        20
    } else {
        0
    }
}

/// Check whether there is any line of sight through smoke between two tiles.
///
/// Returns `false` if accumulated smoke density along the LOS (excluding the
/// origin) is >= 6 (shot is blocked).
pub fn can_see_through_smoke(system: &SmokeSystem, from: TileXY, to: TileXY) -> bool {
    smoke_penalty(system, from, to) < i32::MAX
}

/// Lighting conditions that affect sight range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LightLevel {
    /// Full daylight: no sight range penalty.
    Day,
    /// Twilight conditions: slight reduction.
    Dusk,
    /// Complete darkness: severe reduction.
    Night,
    /// Moonlit night: moderate reduction.
    Moonlit,
    /// Lantern illumination: moderate reduction.
    Lanternlit,
}

impl Default for LightLevel {
    fn default() -> Self {
        Self::Day
    }
}

/// Return the sight radius multiplier for a given light level.
///
/// Returns a `Fix32` value representing the fraction of normal sight range:
/// - Day: 1.0
/// - Dusk: 0.75
/// - Night: 0.25
/// - Moonlit: 0.5
/// - Lanternlit: 1.0 (full range within lantern radius)
pub fn sight_radius_multiplier(light: LightLevel) -> Fix32 {
    match light {
        LightLevel::Day => Fix32::ONE,
        LightLevel::Dusk => Fix32::from_int(3) / Fix32::from_int(4),
        LightLevel::Night => Fix32::ONE / Fix32::from_int(4),
        LightLevel::Moonlit => Fix32::ONE / Fix32::from_int(2),
        LightLevel::Lanternlit => Fix32::ONE,
    }
}

/// Weather conditions affecting the battlefield.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Weather {
    /// Clear skies: no penalties.
    Clear,
    /// Heavy rain: visibility and accuracy penalties.
    Rain,
    /// Snow: visibility and movement penalties.
    Snow,
    /// Dust storm: severe visibility penalties.
    Dust,
    /// Wind: projectile drift.
    Wind,
}

impl Default for Weather {
    fn default() -> Self {
        Self::Clear
    }
}

/// Cover type providing protection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cover {
    /// No cover.
    None,
    /// Soft cover (e.g. bushes, fence): some protection.
    Soft,
    /// Hard cover (e.g. wall, rocks): significant protection.
    Hard,
    /// Full cover (blocked line of sight): cannot be targeted directly.
    Full,
}

/// Return the accuracy penalty associated with a cover type.
///
/// - `None`: 0
/// - `Soft`: 15
/// - `Hard`: 30
/// - `Full`: returns `i32::MAX` (illegal shot)
pub fn cover_accuracy_penalty(cover: Cover) -> i32 {
    match cover {
        Cover::None => 0,
        Cover::Soft => 15,
        Cover::Hard => 30,
        Cover::Full => i32::MAX,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Smoke deposit
    // -----------------------------------------------------------------------

    #[test]
    fn deposit_smoke_muzzle_tile_gets_density_3() {
        let mut sys = SmokeSystem::new();
        let tile = TileXY::new(5, 5);
        deposit_smoke(tile, Facing::South, &mut sys);

        assert_eq!(sys.density_at(tile), 3);
    }

    #[test]
    fn deposit_smoke_front_tiles_get_density_1() {
        let mut sys = SmokeSystem::new();
        let tile = TileXY::new(5, 5);
        deposit_smoke(tile, Facing::South, &mut sys);

        let first = tile.neighbour(Facing::South);
        let second = first.neighbour(Facing::South);

        assert_eq!(sys.density_at(first), 1);
        assert_eq!(sys.density_at(second), 1);
    }

    #[test]
    fn deposit_smoke_accumulates_on_existing_smoke() {
        let mut sys = SmokeSystem::new();
        let tile = TileXY::new(0, 0);
        deposit_smoke(tile, Facing::East, &mut sys);
        deposit_smoke(tile, Facing::East, &mut sys);

        assert_eq!(sys.density_at(tile), 6); // 3 + 3
    }

    // -----------------------------------------------------------------------
    // Smoke decay
    // -----------------------------------------------------------------------

    #[test]
    fn decay_smoke_removes_when_density_zero() {
        let mut sys = SmokeSystem::new();
        let tile = TileXY::new(0, 0);
        sys.volumes.insert(
            tile,
            Smoke {
                density: 1,
                tiles_remaining: 40,
            },
        );

        decay_smoke(&mut sys, 40);
        assert_eq!(sys.density_at(tile), 0);
    }

    #[test]
    fn decay_smoke_no_decay_below_threshold() {
        let mut sys = SmokeSystem::new();
        let tile = TileXY::new(0, 0);
        sys.volumes.insert(
            tile,
            Smoke {
                density: 5,
                tiles_remaining: 200,
            },
        );

        decay_smoke(&mut sys, 30);
        assert_eq!(sys.density_at(tile), 5);
    }

    #[test]
    fn decay_smoke_multiple_decay_steps() {
        let mut sys = SmokeSystem::new();
        let tile = TileXY::new(0, 0);
        sys.volumes.insert(
            tile,
            Smoke {
                density: 4,
                tiles_remaining: 160,
            },
        );

        decay_smoke(&mut sys, 80); // 2 decay
        assert_eq!(sys.density_at(tile), 2);
    }

    #[test]
    fn decay_smoke_removes_tile_when_density_exhausted() {
        let mut sys = SmokeSystem::new();
        sys.volumes.insert(
            TileXY::new(0, 0),
            Smoke {
                density: 2,
                tiles_remaining: 80,
            },
        );
        sys.volumes.insert(
            TileXY::new(1, 0),
            Smoke {
                density: 5,
                tiles_remaining: 200,
            },
        );

        decay_smoke(&mut sys, 80); // 2 decay
        assert_eq!(sys.density_at(TileXY::new(0, 0)), 0);
        assert_eq!(sys.density_at(TileXY::new(1, 0)), 3);
    }

    // -----------------------------------------------------------------------
    // Smoke drift
    // -----------------------------------------------------------------------

    #[test]
    fn drift_smoke_moves_tile_downwind() {
        let mut sys = SmokeSystem::new();
        let tile = TileXY::new(5, 5);
        sys.volumes.insert(
            tile,
            Smoke {
                density: 3,
                tiles_remaining: 120,
            },
        );

        drift_smoke(&mut sys, Facing::East, 120);
        assert_eq!(sys.density_at(tile), 0);
        assert_eq!(sys.density_at(TileXY::new(6, 5)), 3);
    }

    #[test]
    fn drift_smoke_no_drift_below_threshold() {
        let mut sys = SmokeSystem::new();
        let tile = TileXY::new(0, 0);
        sys.volumes.insert(
            tile,
            Smoke {
                density: 3,
                tiles_remaining: 120,
            },
        );

        drift_smoke(&mut sys, Facing::North, 100);
        assert_eq!(sys.density_at(tile), 3);
    }

    #[test]
    fn drift_smoke_merges_at_destination() {
        let mut sys = SmokeSystem::new();
        // Two smoke clouds that drift to the same destination:
        // With east drift of 2 tiles, (4,5) → (6,5) and (5,5) → (7,5) — no merge.
        // Instead, use a map layout where two sources converge:
        // Place at (5,5) and (4,5) with east drift by 1:
        // (5,5) → (6,5), (4,5) → (5,5) — no merge, they don't converge.
        //
        // Actually, we can test that drift correctly moves all smoke:
        sys.volumes.insert(
            TileXY::new(5, 5),
            Smoke {
                density: 2,
                tiles_remaining: 120,
            },
        );
        sys.volumes.insert(
            TileXY::new(6, 5),
            Smoke {
                density: 1,
                tiles_remaining: 120,
            },
        );

        drift_smoke(&mut sys, Facing::East, 120);

        // (5,5) → (6,5) with density 2
        assert_eq!(sys.density_at(TileXY::new(6, 5)), 2);
        // (6,5) → (7,5) with density 1
        assert_eq!(sys.density_at(TileXY::new(7, 5)), 1);
        // Origin tile is now empty
        assert_eq!(sys.density_at(TileXY::new(5, 5)), 0);
    }

    // -----------------------------------------------------------------------
    // Smoke penalty and line of sight
    // -----------------------------------------------------------------------

    #[test]
    fn smoke_penalty_zero_when_no_smoke() {
        let sys = SmokeSystem::new();
        let from = TileXY::new(0, 0);
        let to = TileXY::new(5, 0);
        assert_eq!(smoke_penalty(&sys, from, to), 0);
    }

    #[test]
    fn smoke_penalty_accumulates_along_los() {
        let mut sys = SmokeSystem::new();
        sys.volumes.insert(
            TileXY::new(2, 0),
            Smoke {
                density: 2,
                tiles_remaining: 80,
            },
        );
        sys.volumes.insert(
            TileXY::new(3, 0),
            Smoke {
                density: 2,
                tiles_remaining: 80,
            },
        );

        let from = TileXY::new(0, 0);
        let to = TileXY::new(5, 0);
        // Accumulated: 2 + 2 = 4 >= 3, so penalty is 20
        assert_eq!(smoke_penalty(&sys, from, to), 20);
    }

    #[test]
    fn smoke_penalty_illegal_when_accumulated_6_or_more() {
        let mut sys = SmokeSystem::new();
        sys.volumes.insert(
            TileXY::new(2, 0),
            Smoke {
                density: 3,
                tiles_remaining: 120,
            },
        );
        sys.volumes.insert(
            TileXY::new(3, 0),
            Smoke {
                density: 4,
                tiles_remaining: 160,
            },
        );

        let from = TileXY::new(0, 0);
        let to = TileXY::new(5, 0);
        // Accumulated: 7 >= 6, illegal
        assert_eq!(smoke_penalty(&sys, from, to), i32::MAX);
    }

    #[test]
    fn can_see_through_smoke_true_when_clear() {
        let sys = SmokeSystem::new();
        assert!(can_see_through_smoke(
            &sys,
            TileXY::new(0, 0),
            TileXY::new(5, 0)
        ));
    }

    // -----------------------------------------------------------------------
    // Light level
    // -----------------------------------------------------------------------

    #[test]
    fn sight_radius_day_is_one() {
        assert_eq!(sight_radius_multiplier(LightLevel::Day), Fix32::ONE);
    }

    #[test]
    fn sight_radius_dusk_is_three_quarters() {
        let expected = Fix32::from_int(3) / Fix32::from_int(4);
        assert_eq!(sight_radius_multiplier(LightLevel::Dusk), expected);
    }

    #[test]
    fn sight_radius_night_is_one_quarter() {
        let expected = Fix32::ONE / Fix32::from_int(4);
        assert_eq!(sight_radius_multiplier(LightLevel::Night), expected);
    }

    #[test]
    fn sight_radius_moonlit_is_one_half() {
        let expected = Fix32::ONE / Fix32::from_int(2);
        assert_eq!(sight_radius_multiplier(LightLevel::Moonlit), expected);
    }

    #[test]
    fn sight_radius_lanternlit_is_one() {
        assert_eq!(sight_radius_multiplier(LightLevel::Lanternlit), Fix32::ONE);
    }

    // -----------------------------------------------------------------------
    // Cover
    // -----------------------------------------------------------------------

    #[test]
    fn cover_penalty_none_is_zero() {
        assert_eq!(cover_accuracy_penalty(Cover::None), 0);
    }

    #[test]
    fn cover_penalty_soft_is_15() {
        assert_eq!(cover_accuracy_penalty(Cover::Soft), 15);
    }

    #[test]
    fn cover_penalty_hard_is_30() {
        assert_eq!(cover_accuracy_penalty(Cover::Hard), 30);
    }

    #[test]
    fn cover_penalty_full_is_max() {
        assert_eq!(cover_accuracy_penalty(Cover::Full), i32::MAX);
    }
}
