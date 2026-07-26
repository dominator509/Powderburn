//! Explosives: dynamite throwing, blast damage, and fuse management.
//!
//! M4: Implements dynamite scatter on throw, area-of-effect blast damage
//! with falloff, and fuse tick counting.

#![forbid(unsafe_code)]

use pb_core::fix32::Fix32;
use pb_core::geom::TileXY;
use pb_rng::{PbRng, StreamTag};

/// A stick of dynamite with a burning fuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dynamite {
    /// Number of ticks remaining on the fuse before detonation.
    pub fuse_ticks: u64,
}

impl Dynamite {
    /// Create a new dynamite stick with the given fuse length.
    pub fn new(fuse_ticks: u64) -> Self {
        Dynamite { fuse_ticks }
    }
}

/// Throw a stick of dynamite at a target tile.
///
/// The dynamite scatters from the target position using the `Scatter` RNG
/// stream. Scatter displacement is up to 3 tiles in a random direction
/// (uniform over x ∈ [-3, 3], y ∈ [-3, 3]).
///
/// Returns `(landing_tile, remaining_fuse)` where `remaining_fuse` is the
/// fuse duration before detonation.
pub fn throw_dynamite(
    seed: u64,
    scenario_id: u32,
    tick: u64,
    actor_id: u32,
    target: TileXY,
) -> (TileXY, u64) {
    // Scatter displacement: up to 3 tiles in both x and y
    let dx = PbRng::draw(seed, scenario_id, tick, actor_id, StreamTag::Scatter, -3, 3);
    let dy = PbRng::draw(
        seed,
        scenario_id,
        tick,
        actor_id.wrapping_add(1),
        StreamTag::Scatter,
        -3,
        3,
    );

    let landing = TileXY {
        x: target.x.wrapping_add(dx as i16),
        y: target.y.wrapping_add(dy as i16),
    };

    // Standard fuse: 120 ticks (about 3 seconds at 40 ticks/sec)
    let fuse = 120u64;
    (landing, fuse)
}

/// Compute blast damage at a tile from a blast centered at `blast_center`.
///
/// Radius 3. Damage falls by 1/3 per tile of Chebyshev distance from center.
/// Returns the damage amount (0 if out of range).
///
/// Base damage is 24 (representing a standard dynamite stick).
pub fn blast_damage(tile: TileXY, blast_center: TileXY) -> i32 {
    let dist = tile.chebyshev_distance(blast_center);

    if dist > 3 {
        return 0;
    }

    // Base damage 24, falls by 1/3 per tile of distance
    let base_damage = Fix32::from_int(24);
    // Multiplier = (3 - dist) / 3
    let multiplier = Fix32::from_int(3 - dist as i32) / Fix32::from_int(3);
    let damage = base_damage * multiplier;
    damage.to_int_floor().max(0)
}

/// Check whether a fuse has expired after elapsed ticks.
///
/// Returns `true` when `elapsed >= fuse`, meaning the dynamite detonates.
pub fn fuse_tick(fuse: u64, elapsed: u64) -> bool {
    elapsed >= fuse
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Dynamite scatter
    // -----------------------------------------------------------------------

    #[test]
    fn throw_dynamite_returns_tile_and_fuse() {
        let (tile, fuse) = throw_dynamite(42, 1, 100, 5, TileXY::new(10, 10));
        // Tile was displaced but should be within reasonable range
        let dx = (tile.x as i32) - 10;
        let dy = (tile.y as i32) - 10;
        assert!(dx.abs() <= 3);
        assert!(dy.abs() <= 3);
        assert_eq!(fuse, 120);
    }

    #[test]
    fn throw_dynamite_deterministic() {
        let (tile_a, _) = throw_dynamite(42, 1, 100, 5, TileXY::new(10, 10));
        let (tile_b, _) = throw_dynamite(42, 1, 100, 5, TileXY::new(10, 10));
        assert_eq!(tile_a, tile_b);
    }

    #[test]
    fn throw_dynamite_scatter_within_bounds_over_many_seeds() {
        for seed in 0..1000u64 {
            let (tile, _) = throw_dynamite(seed, 1, 100, 5, TileXY::new(50, 50));
            let dx = (tile.x as i32) - 50;
            let dy = (tile.y as i32) - 50;
            assert!(dx.abs() <= 3, "seed {} dx={} exceeds 3", seed, dx);
            assert!(dy.abs() <= 3, "seed {} dy={} exceeds 3", seed, dy);
        }
    }

    // -----------------------------------------------------------------------
    // Blast damage
    // -----------------------------------------------------------------------

    #[test]
    fn blast_damage_center_is_24() {
        let tile = TileXY::new(5, 5);
        let center = TileXY::new(5, 5);
        assert_eq!(blast_damage(tile, center), 24);
    }

    #[test]
    fn blast_damage_distance_1_is_15() {
        let center = TileXY::new(5, 5);
        let tile = TileXY::new(6, 5); // distance 1
        assert_eq!(blast_damage(tile, center), 15); // 24 * 2/3 truncated = 15
    }

    #[test]
    fn blast_damage_distance_2_is_7() {
        let center = TileXY::new(5, 5);
        let tile = TileXY::new(7, 5); // distance 2
        assert_eq!(blast_damage(tile, center), 7); // 24 * 1/3 truncated = 7
    }

    #[test]
    fn blast_damage_distance_3_is_0() {
        let center = TileXY::new(5, 5);
        let tile = TileXY::new(8, 5); // distance 3
        assert_eq!(blast_damage(tile, center), 0);
    }

    #[test]
    fn blast_damage_out_of_range() {
        let center = TileXY::new(5, 5);
        let tile = TileXY::new(9, 5); // distance 4
        assert_eq!(blast_damage(tile, center), 0);
    }

    #[test]
    fn blast_damage_diagonal_falloff() {
        let center = TileXY::new(5, 5);
        let tile = TileXY::new(6, 6); // Chebyshev distance 1
        assert_eq!(blast_damage(tile, center), 15);
    }

    // -----------------------------------------------------------------------
    // Fuse
    // -----------------------------------------------------------------------

    #[test]
    fn fuse_not_expired_before_duration() {
        assert!(!fuse_tick(120, 0));
        assert!(!fuse_tick(120, 119));
    }

    #[test]
    fn fuse_expired_at_duration() {
        assert!(fuse_tick(120, 120));
    }

    #[test]
    fn fuse_expired_after_duration() {
        assert!(fuse_tick(120, 200));
    }
}
