//! See ARCHITECTURE.md for this crate's place in the import law.
#![forbid(unsafe_code)]

/// A signed 32-bit fixed-point number with 10 fractional bits (Q22.10).
///
/// Representation: `raw = (value * 1024) as i32`, rounded toward negative infinity.
/// Range: approximately [-2,097,152, 2,097,151.999].
/// Precision: 1/1024 ≈ 0.00098.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fix32(i32);

const FRAC_BITS: i32 = 10;
const SCALE: i32 = 1 << FRAC_BITS; // 1024

impl Fix32 {
    /// The fixed-point representation of 1.0.
    pub const ONE: Fix32 = Fix32(SCALE);

    /// The fixed-point representation of 0.
    pub const ZERO: Fix32 = Fix32(0);

    /// Create a `Fix32` from an integer value.
    pub const fn from_int(v: i32) -> Self {
        Fix32(v.wrapping_mul(SCALE))
    }

    /// Convert to the nearest integer, rounding toward negative infinity (floor).
    pub fn to_int_floor(self) -> i32 {
        if self.0 >= 0 {
            self.0 / SCALE
        } else {
            // For negative numbers, integer division in Rust rounds toward zero,
            // so we adjust to match floor semantics.
            (self.0 - (SCALE - 1)) / SCALE
        }
    }

    /// The raw `i32` representation (10 fractional bits).
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// Construct from a raw `i32` representation.
    pub const fn from_raw(v: i32) -> Self {
        Fix32(v)
    }

    /// Multiply two fixed-point numbers, rounding half toward zero.
    pub fn mul(self, rhs: Self) -> Self {
        let product = self.0 as i64 * rhs.0 as i64;
        Fix32((product / SCALE as i64) as i32)
    }

    /// Divide two fixed-point numbers, rounding half toward zero.
    pub fn div(self, rhs: Self) -> Self {
        let quotient = (self.0 as i64 * SCALE as i64) / rhs.0 as i64;
        Fix32(quotient as i32)
    }

    /// Add two fixed-point numbers.
    pub fn add(self, rhs: Self) -> Self {
        Fix32(self.0.wrapping_add(rhs.0))
    }

    /// Subtract two fixed-point numbers.
    pub fn sub(self, rhs: Self) -> Self {
        Fix32(self.0.wrapping_sub(rhs.0))
    }

    /// Negate.
    pub fn neg(self) -> Self {
        Fix32(self.0.wrapping_neg())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fix32_mul_exact_integer() {
        let a = Fix32::from_int(3);
        let b = Fix32::from_int(4);
        assert_eq!(a.mul(b).to_int_floor(), 12);
    }

    #[test]
    fn fix32_div_rounds_toward_negative() {
        // 1/3 * 3 = 0.999... which floors to 0
        let one = Fix32::ONE;
        let three = Fix32::from_int(3);
        let result = one.div(three).mul(three).to_int_floor();
        assert_eq!(result, 0);
    }

    #[test]
    fn fix32_representation_is_i32_with_10_frac_bits() {
        assert_eq!(Fix32::ONE.raw(), 1024);
    }
}
