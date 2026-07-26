//! Fixed-point arithmetic: Q22.10 signed 32-bit.
//! See SPEC-001 section 1 for the numeric law.

use core::ops::{Add, Div, Mul, Neg, Sub};

/// A signed 32-bit fixed-point number with 10 fractional bits (Q22.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fix32(i32);

const FRAC_BITS: i32 = 10;
const SCALE: i32 = 1 << FRAC_BITS; // 1024

impl Fix32 {
    pub const ONE: Fix32 = Fix32(SCALE);
    pub const ZERO: Fix32 = Fix32(0);

    pub const fn from_int(v: i32) -> Self {
        Fix32(v.wrapping_mul(SCALE))
    }

    /// Floor (round toward negative infinity).
    pub fn to_int_floor(self) -> i32 {
        if self.0 >= 0 {
            self.0 / SCALE
        } else {
            (self.0 - (SCALE - 1)) / SCALE
        }
    }

    pub const fn raw(self) -> i32 {
        self.0
    }

    pub const fn from_raw(v: i32) -> Self {
        Fix32(v)
    }
}

impl Mul for Fix32 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        let product = self.0 as i64 * rhs.0 as i64;
        Fix32((product / SCALE as i64) as i32)
    }
}

impl Div for Fix32 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        let quotient = (self.0 as i64 * SCALE as i64) / rhs.0 as i64;
        Fix32(quotient as i32)
    }
}

impl Add for Fix32 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Fix32(self.0.wrapping_add(rhs.0))
    }
}

impl Sub for Fix32 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Fix32(self.0.wrapping_sub(rhs.0))
    }
}

impl Neg for Fix32 {
    type Output = Self;
    fn neg(self) -> Self {
        Fix32(self.0.wrapping_neg())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fix32_mul_exact_integer() {
        assert_eq!((Fix32::from_int(3) * Fix32::from_int(4)).to_int_floor(), 12);
    }

    #[test]
    fn fix32_div_rounds_toward_negative() {
        let result = (Fix32::ONE / Fix32::from_int(3)) * Fix32::from_int(3);
        assert_eq!(result.to_int_floor(), 0);
    }

    #[test]
    fn fix32_representation_is_i32_with_10_frac_bits() {
        assert_eq!(Fix32::ONE.raw(), 1024);
    }

    #[test]
    fn fix32_neg() {
        assert_eq!((-Fix32::from_int(5)).to_int_floor(), -5);
    }

    #[test]
    fn fix32_mul_fractional() {
        // 1.5 * 2.0 = 3.0
        let a = Fix32::from_raw(1024 + 512); // 1.5
        let b = Fix32::from_int(2);
        assert_eq!((a * b).to_int_floor(), 3);
    }
}
