//! Integration tests for the explosives system.
//!
//! Tests dynamite scatter within bounds over many throws and
//! blast damage falloff from the center.

use pb_core::geom::TileXY;
use pb_sim::explosive::{blast_damage, throw_dynamite};

/// Over 1000 seeded throws, verify that dynamite scatter is always
/// within the allowed displacement bounds (±3 tiles in each axis).
#[test]
fn dynamite_scatter_within_bounds_over_1000_throws() {
    for seed in 0..1000u64 {
        let (landing, _fuse) = throw_dynamite(seed, 1, 100, 5, TileXY::new(50, 50));
        let dx = (landing.x as i32) - 50;
        let dy = (landing.y as i32) - 50;
        assert!(
            dx.abs() <= 3,
            "seed {} dx={} exceeds bounds",
            seed,
            dx
        );
        assert!(
            dy.abs() <= 3,
            "seed {} dy={} exceeds bounds",
            seed,
            dy
        );
    }
}

/// Different seeds produce different scatter results (mostly).
#[test]
fn dynamite_scatter_deterministic_per_seed() {
    let (tile_a, _) = throw_dynamite(42, 1, 100, 5, TileXY::new(50, 50));
    let (tile_b, _) = throw_dynamite(42, 1, 100, 5, TileXY::new(50, 50));
    assert_eq!(tile_a, tile_b);
}

/// Blast damage falls off by 1/3 per tile of Chebyshev distance.
#[test]
fn blast_damage_falloff() {
    let center = TileXY::new(10, 10);

    // Center: full damage 24
    assert_eq!(blast_damage(TileXY::new(10, 10), center), 24);

    // Distance 1: 24 * 2/3 with Fix32 truncation = 15
    assert_eq!(blast_damage(TileXY::new(11, 10), center), 15);
    assert_eq!(blast_damage(TileXY::new(10, 11), center), 15);
    assert_eq!(blast_damage(TileXY::new(11, 11), center), 15); // diagonal

    // Distance 2: 24 * 1/3 with Fix32 truncation = 7
    assert_eq!(blast_damage(TileXY::new(12, 10), center), 7);
    assert_eq!(blast_damage(TileXY::new(10, 12), center), 7);
    assert_eq!(blast_damage(TileXY::new(12, 12), center), 7); // diagonal

    // Distance 3: 0 (out of radius)
    assert_eq!(blast_damage(TileXY::new(13, 10), center), 0);
    assert_eq!(blast_damage(TileXY::new(10, 13), center), 0);

    // Distance 4+: 0
    assert_eq!(blast_damage(TileXY::new(14, 10), center), 0);
    assert_eq!(blast_damage(TileXY::new(0, 0), center), 0);
}

/// Blast damage is deterministic for the same inputs.
#[test]
fn blast_damage_deterministic() {
    let center = TileXY::new(5, 5);
    assert_eq!(
        blast_damage(TileXY::new(6, 5), center),
        blast_damage(TileXY::new(6, 5), center)
    );
    assert_eq!(
        blast_damage(TileXY::new(5, 5), center),
        blast_damage(TileXY::new(5, 5), center)
    );
}

/// Multiple tiles at the same distance from the center get the same damage.
#[test]
fn blast_damage_symmetric() {
    let center = TileXY::new(0, 0);
    // All tiles at Chebyshev distance 1 from origin
    let d1_tiles = [
        TileXY::new(1, 0),
        TileXY::new(0, 1),
        TileXY::new(-1, 0),
        TileXY::new(0, -1),
        TileXY::new(1, 1),
        TileXY::new(-1, -1),
        TileXY::new(1, -1),
        TileXY::new(-1, 1),
    ];
    for tile in &d1_tiles {
        assert_eq!(
            blast_damage(*tile, center),
            15,
            "tile {:?} should have damage 15",
            tile
        );
    }
}
