//! Geometry primitives for the simulation kernel.
//!
//! Provides `TileXY`, `Facing`, and a Bresenham line walker.
//! All arithmetic uses only integer operations — no floating point.

#![forbid(unsafe_code)]

use core::fmt;

/// A tile coordinate on the game grid.
///
/// Uses signed 16-bit coordinates, sufficient for maps up to 32767 tiles in
/// each direction.  The Chebyshev distance metric matches the 8-directional
/// movement model used by the tactical engine.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct TileXY {
    pub x: i16,
    pub y: i16,
}

impl TileXY {
    /// The origin tile.
    pub const ZERO: TileXY = TileXY { x: 0, y: 0 };

    /// Create a new tile coordinate.
    pub const fn new(x: i16, y: i16) -> Self {
        TileXY { x, y }
    }

    /// Chebyshev distance: `max(|dx|, |dy|)`.
    ///
    /// This is the natural distance metric for an 8-directional grid since it
    /// reflects the minimum number of king-move steps between two tiles.
    pub fn chebyshev_distance(self, other: TileXY) -> i16 {
        let dx = self.x.abs_diff(other.x) as i16;
        let dy = self.y.abs_diff(other.y) as i16;
        dx.max(dy)
    }

    /// The eight principal directions from this tile (including diagonals).
    pub const FACINGS: [Facing; 8] = [
        Facing::North,
        Facing::NorthEast,
        Facing::East,
        Facing::SouthEast,
        Facing::South,
        Facing::SouthWest,
        Facing::West,
        Facing::NorthWest,
    ];

    /// Return the neighbour in the given direction (wrapping on overflow).
    pub fn neighbour(self, facing: Facing) -> TileXY {
        let (dx, dy) = facing.delta();
        TileXY {
            x: self.x.wrapping_add(dx),
            y: self.y.wrapping_add(dy),
        }
    }

    /// Walk all tiles on the Bresenham line from `self` to `target`, inclusive
    /// of both endpoints.
    ///
    /// Uses the classic integer Bresenham algorithm — no floating point.
    /// Symmetric: the same set of tiles is returned regardless of direction.
    pub fn line_to(self, target: TileXY) -> Vec<TileXY> {
        let mut tiles = Vec::new();

        let dx = (target.x as i32) - (self.x as i32);
        let dy = (target.y as i32) - (self.y as i32);

        let abs_dx = dx.abs();
        let abs_dy = dy.abs();

        let sx: i32 = if dx >= 0 { 1 } else { -1 };
        let sy: i32 = if dy >= 0 { 1 } else { -1 };

        let mut x = self.x as i32;
        let mut y = self.y as i32;

        if abs_dx >= abs_dy {
            // Shallow: step in x, accumulate error in y.
            let mut err = 0;
            let e_step = abs_dy;
            for _ in 0..=abs_dx {
                tiles.push(TileXY::new(x as i16, y as i16));
                err += e_step;
                if err * 2 >= abs_dx {
                    y += sy;
                    err -= abs_dx;
                }
                x += sx;
            }
        } else {
            // Steep: step in y, accumulate error in x.
            let mut err = 0;
            let e_step = abs_dx;
            for _ in 0..=abs_dy {
                tiles.push(TileXY::new(x as i16, y as i16));
                err += e_step;
                if err * 2 >= abs_dy {
                    x += sx;
                    err -= abs_dy;
                }
                y += sy;
            }
        }

        tiles
    }
}

impl fmt::Display for TileXY {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

/// The eight cardinal and intercardinal facings.
///
/// Ordering corresponds to clockwise rotation starting from North.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Facing {
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}

impl Facing {
    /// Return the (dx, dy) delta for this facing.
    ///
    /// Positive y is south (screen-space convention).
    pub const fn delta(self) -> (i16, i16) {
        match self {
            Facing::North => (0, -1),
            Facing::NorthEast => (1, -1),
            Facing::East => (1, 0),
            Facing::SouthEast => (1, 1),
            Facing::South => (0, 1),
            Facing::SouthWest => (-1, 1),
            Facing::West => (-1, 0),
            Facing::NorthWest => (-1, -1),
        }
    }

    /// Convert a zero-based index (0 = North, 1 = NorthEast, …, 7 = NorthWest)
    /// to a `Facing`.  Panics if `i >= 8`.
    pub fn from_index(i: usize) -> Self {
        match i % 8 {
            0 => Facing::North,
            1 => Facing::NorthEast,
            2 => Facing::East,
            3 => Facing::SouthEast,
            4 => Facing::South,
            5 => Facing::SouthWest,
            6 => Facing::West,
            7 => Facing::NorthWest,
            _ => unreachable!(),
        }
    }

    /// Return the zero-based index of this facing (0 = North, …, 7 = NorthWest).
    pub const fn to_index(self) -> usize {
        match self {
            Facing::North => 0,
            Facing::NorthEast => 1,
            Facing::East => 2,
            Facing::SouthEast => 3,
            Facing::South => 4,
            Facing::SouthWest => 5,
            Facing::West => 6,
            Facing::NorthWest => 7,
        }
    }

    /// Return the opposite facing.
    pub const fn opposite(self) -> Facing {
        match self {
            Facing::North => Facing::South,
            Facing::NorthEast => Facing::SouthWest,
            Facing::East => Facing::West,
            Facing::SouthEast => Facing::NorthWest,
            Facing::South => Facing::North,
            Facing::SouthWest => Facing::NorthEast,
            Facing::West => Facing::East,
            Facing::NorthWest => Facing::SouthEast,
        }
    }
}

impl fmt::Display for Facing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Facing::North => write!(f, "N"),
            Facing::NorthEast => write!(f, "NE"),
            Facing::East => write!(f, "E"),
            Facing::SouthEast => write!(f, "SE"),
            Facing::South => write!(f, "S"),
            Facing::SouthWest => write!(f, "SW"),
            Facing::West => write!(f, "W"),
            Facing::NorthWest => write!(f, "NW"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // TileXY
    // -----------------------------------------------------------------------

    #[test]
    fn tile_chebyshev_distance_cardinal() {
        let a = TileXY::new(0, 0);
        let b = TileXY::new(5, 0);
        assert_eq!(a.chebyshev_distance(b), 5);
    }

    #[test]
    fn tile_chebyshev_distance_diagonal() {
        let a = TileXY::new(0, 0);
        let b = TileXY::new(3, 4);
        // max(|3|, |4|) = 4
        assert_eq!(a.chebyshev_distance(b), 4);
    }

    #[test]
    fn tile_chebyshev_distance_same_tile() {
        let a = TileXY::new(7, -3);
        assert_eq!(a.chebyshev_distance(a), 0);
    }

    #[test]
    fn tile_chebyshev_distance_negative_coords() {
        let a = TileXY::new(-10, -5);
        let b = TileXY::new(-3, -1);
        assert_eq!(a.chebyshev_distance(b), 7); // max(|-7|, |-4|) = 7
    }

    #[test]
    fn tile_neighbour_north() {
        let t = TileXY::new(5, 5);
        assert_eq!(t.neighbour(Facing::North), TileXY::new(5, 4));
    }

    #[test]
    fn tile_neighbour_south() {
        let t = TileXY::new(5, 5);
        assert_eq!(t.neighbour(Facing::South), TileXY::new(5, 6));
    }

    #[test]
    fn tile_neighbour_east() {
        let t = TileXY::new(5, 5);
        assert_eq!(t.neighbour(Facing::East), TileXY::new(6, 5));
    }

    #[test]
    fn tile_neighbour_west() {
        let t = TileXY::new(5, 5);
        assert_eq!(t.neighbour(Facing::West), TileXY::new(4, 5));
    }

    #[test]
    fn tile_neighbour_northwest() {
        let t = TileXY::new(5, 5);
        assert_eq!(t.neighbour(Facing::NorthWest), TileXY::new(4, 4));
    }

    #[test]
    fn tile_neighbour_southeast() {
        let t = TileXY::new(5, 5);
        assert_eq!(t.neighbour(Facing::SouthEast), TileXY::new(6, 6));
    }

    // -----------------------------------------------------------------------
    // Bresenham line walking
    // -----------------------------------------------------------------------

    #[test]
    fn line_horizontal() {
        let a = TileXY::new(0, 0);
        let b = TileXY::new(4, 0);
        let line = a.line_to(b);
        assert_eq!(line.len(), 5);
        assert_eq!(line[0], TileXY::new(0, 0));
        assert_eq!(line[4], TileXY::new(4, 0));
    }

    #[test]
    fn line_vertical() {
        let a = TileXY::new(2, 0);
        let b = TileXY::new(2, 3);
        let line = a.line_to(b);
        assert_eq!(line.len(), 4);
        assert_eq!(line[0], TileXY::new(2, 0));
        assert_eq!(line[3], TileXY::new(2, 3));
    }

    #[test]
    fn line_diagonal() {
        let a = TileXY::new(0, 0);
        let b = TileXY::new(3, 3);
        let line = a.line_to(b);
        assert_eq!(line.len(), 4);
        for (i, p) in line.iter().enumerate() {
            assert_eq!(*p, TileXY::new(i as i16, i as i16));
        }
    }

    #[test]
    fn line_shallow_slope() {
        let a = TileXY::new(0, 0);
        let b = TileXY::new(5, 1);
        let line = a.line_to(b);
        assert_eq!(line[0], TileXY::new(0, 0));
        assert_eq!(line[line.len() - 1], TileXY::new(5, 1));
        // All y values should be 0 or 1.
        for p in &line {
            assert!(p.y == 0 || p.y == 1);
        }
    }

    #[test]
    fn line_steep_slope() {
        let a = TileXY::new(0, 0);
        let b = TileXY::new(1, 5);
        let line = a.line_to(b);
        assert_eq!(line[0], TileXY::new(0, 0));
        assert_eq!(line[line.len() - 1], TileXY::new(1, 5));
    }

    #[test]
    fn line_same_point() {
        let a = TileXY::new(3, 7);
        let line = a.line_to(a);
        assert_eq!(line.len(), 1);
        assert_eq!(line[0], a);
    }

    #[test]
    fn line_symmetry() {
        let a = TileXY::new(0, 0);
        let b = TileXY::new(3, 1);
        let forward = a.line_to(b);
        let backward = b.line_to(a);
        // The sets should be the same, just reversed.
        assert_eq!(forward.len(), backward.len());
        for i in 0..forward.len() {
            assert_eq!(forward[i], backward[forward.len() - 1 - i]);
        }
    }

    #[test]
    fn line_negative_coordinates() {
        let a = TileXY::new(-3, -2);
        let b = TileXY::new(1, 2);
        let line = a.line_to(b);
        assert_eq!(line[0], TileXY::new(-3, -2));
        assert_eq!(line[line.len() - 1], TileXY::new(1, 2));
        // Should monotonically increase in both x and y.
        for w in line.windows(2) {
            assert!(w[0].x <= w[1].x);
            assert!(w[0].y <= w[1].y);
        }
    }

    // -----------------------------------------------------------------------
    // Facing
    // -----------------------------------------------------------------------

    #[test]
    fn facing_delta_north() {
        assert_eq!(Facing::North.delta(), (0, -1));
    }

    #[test]
    fn facing_delta_south() {
        assert_eq!(Facing::South.delta(), (0, 1));
    }

    #[test]
    fn facing_delta_east() {
        assert_eq!(Facing::East.delta(), (1, 0));
    }

    #[test]
    fn facing_delta_west() {
        assert_eq!(Facing::West.delta(), (-1, 0));
    }

    #[test]
    fn facing_delta_northeast() {
        assert_eq!(Facing::NorthEast.delta(), (1, -1));
    }

    #[test]
    fn facing_delta_northwest() {
        assert_eq!(Facing::NorthWest.delta(), (-1, -1));
    }

    #[test]
    fn facing_index_roundtrip() {
        for i in 0..8 {
            let facing = Facing::from_index(i);
            assert_eq!(facing.to_index(), i);
        }
    }

    #[test]
    fn facing_opposite() {
        assert_eq!(Facing::North.opposite(), Facing::South);
        assert_eq!(Facing::South.opposite(), Facing::North);
        assert_eq!(Facing::East.opposite(), Facing::West);
        assert_eq!(Facing::West.opposite(), Facing::East);
        assert_eq!(Facing::NorthEast.opposite(), Facing::SouthWest);
        assert_eq!(Facing::SouthWest.opposite(), Facing::NorthEast);
        assert_eq!(Facing::NorthWest.opposite(), Facing::SouthEast);
        assert_eq!(Facing::SouthEast.opposite(), Facing::NorthWest);
    }

    #[test]
    fn facing_display() {
        assert_eq!(format!("{}", Facing::North), "N");
        assert_eq!(format!("{}", Facing::NorthEast), "NE");
        assert_eq!(format!("{}", Facing::East), "E");
        assert_eq!(format!("{}", Facing::SouthEast), "SE");
        assert_eq!(format!("{}", Facing::South), "S");
        assert_eq!(format!("{}", Facing::SouthWest), "SW");
        assert_eq!(format!("{}", Facing::West), "W");
        assert_eq!(format!("{}", Facing::NorthWest), "NW");
    }

    #[test]
    fn tile_all_eight_facings() {
        let t = TileXY::new(10, 10);
        for facing in &TileXY::FACINGS {
            let n = t.neighbour(*facing);
            let diff_x = (n.x as i32) - (t.x as i32);
            let diff_y = (n.y as i32) - (t.y as i32);
            assert!(diff_x.abs() <= 1);
            assert!(diff_y.abs() <= 1);
            assert!(diff_x != 0 || diff_y != 0);
        }
    }
}
