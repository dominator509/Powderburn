//! A signed 32-bit fixed-point number with 10 fractional bits.
//!
//! Provides deterministic, integer-only arithmetic for the simulation kernel.
//! All operations are pure integer math, fully deterministic across runs.

use std::ops;

/// A 32-bit fixed-point number with 10 fractional bits.
///
/// One unit is exactly 1/1024.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fix32(i32);

/// The number of fractional bits.
pub const FRAC_BITS: i32 = 10;
/// The multiplier to convert between Fix32 and integer (2^10).
pub const SCALE: i32 = 1024;

impl Fix32 {
    /// The zero value.
    pub const ZERO: Fix32 = Fix32(0);
    /// The one value.
    pub const ONE: Fix32 = Fix32(SCALE);
    /// The smallest positive value.
    pub const EPSILON: Fix32 = Fix32(1);

    /// Create a Fix32 from a raw internal representation.
    pub const fn from_raw(raw: i32) -> Self {
        Fix32(raw)
    }

    /// Create a Fix32 from an integer.
    pub const fn from_int(val: i32) -> Self {
        Fix32(val * SCALE)
    }

    /// Create a Fix32 from an integer ratio, truncating toward zero.
    pub fn from_ratio(numerator: i32, denominator: i32) -> Self {
        assert!(denominator != 0, "Fix32 ratio denominator is zero");
        let scaled = i64::from(numerator) << FRAC_BITS;
        let raw = scaled / i64::from(denominator);
        Fix32(raw.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32)
    }

    /// Get the raw internal representation.
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// Truncate to integer (toward zero).
    pub fn trunc(self) -> i32 {
        self.0 / SCALE
    }

    /// Floor to integer (toward negative infinity).
    pub fn floor(self) -> i32 {
        let trunc = self.trunc();
        if self.0 >= 0 || self.0 % SCALE == 0 {
            trunc
        } else {
            trunc - 1
        }
    }

    /// Alias for floor, used by the damage system.
    pub fn to_int_floor(self) -> i32 {
        self.floor()
    }

    /// Round to nearest integer.
    pub fn round(self) -> i32 {
        let rem = self.0 % SCALE;
        let abs_rem = rem.abs();
        if abs_rem >= SCALE / 2 {
            if self.0 >= 0 {
                self.0 / SCALE + 1
            } else {
                self.0 / SCALE - 1
            }
        } else {
            self.0 / SCALE
        }
    }

    /// Absolute value.
    pub fn abs(self) -> Self {
        Fix32(self.0.abs())
    }

    /// Checked multiplication that returns None on overflow.
    pub fn checked_mul(self, rhs: Self) -> Option<Self> {
        let a = self.0 as i64;
        let b = rhs.0 as i64;
        let prod = a * b;
        // Arithmetic right shift rounds negative products toward negative infinity.
        let result = prod >> FRAC_BITS;
        // Check for overflow when converting back to i32
        if result > i32::MAX as i64 || result < i32::MIN as i64 {
            None
        } else {
            Some(Fix32(result as i32))
        }
    }

    /// Checked division that returns None on overflow or division by zero.
    pub fn checked_div(self, rhs: Self) -> Option<Self> {
        if rhs.0 == 0 {
            return None;
        }
        let a = self.0 as i64;
        let b = rhs.0 as i64;
        // Shift the numerator up by the fixed-point precision.
        let scaled = a << FRAC_BITS;
        let result = scaled / b;
        if result > i32::MAX as i64 || result < i32::MIN as i64 {
            None
        } else {
            Some(Fix32(result as i32))
        }
    }

    /// Saturating addition.
    pub fn saturating_add(self, rhs: Self) -> Self {
        Fix32(self.0.saturating_add(rhs.0))
    }

    /// Saturating subtraction.
    pub fn saturating_sub(self, rhs: Self) -> Self {
        Fix32(self.0.saturating_sub(rhs.0))
    }

    /// Saturating multiplication.
    pub fn saturating_mul(self, rhs: Self) -> Self {
        let a = self.0 as i64;
        let b = rhs.0 as i64;
        let prod = a * b;
        let result = prod >> FRAC_BITS;
        if result > i32::MAX as i64 {
            Fix32(i32::MAX)
        } else if result < i32::MIN as i64 {
            Fix32(i32::MIN)
        } else {
            Fix32(result as i32)
        }
    }

    /// Linear interpolation: self + (target - self) * t
    pub fn lerp(self, target: Self, t: Self) -> Self {
        self + (target - self) * t
    }

    /// Clamp value between min and max.
    pub fn clamp(self, min: Self, max: Self) -> Self {
        if self < min {
            min
        } else if self > max {
            max
        } else {
            self
        }
    }
}

// Arithmetic operators

impl ops::Add for Fix32 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Fix32(self.0 + rhs.0)
    }
}

impl ops::Sub for Fix32 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Fix32(self.0 - rhs.0)
    }
}

impl ops::Mul for Fix32 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        let a = self.0 as i64;
        let b = rhs.0 as i64;
        let prod = a * b;
        Fix32((prod >> FRAC_BITS) as i32)
    }
}

impl ops::Div for Fix32 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        if rhs.0 == 0 {
            if cfg!(debug_assertions) {
                panic!("Fix32 division by zero");
            }
            return if self.0 < 0 {
                Fix32(i32::MIN)
            } else {
                Fix32(i32::MAX)
            };
        }
        let a = self.0 as i64;
        let b = rhs.0 as i64;
        let scaled = a << FRAC_BITS;
        let quotient = scaled / b;
        Fix32(quotient.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32)
    }
}

impl ops::Neg for Fix32 {
    type Output = Self;
    fn neg(self) -> Self {
        Fix32(-self.0)
    }
}

impl ops::AddAssign for Fix32 {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl ops::SubAssign for Fix32 {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl ops::MulAssign for Fix32 {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl ops::DivAssign for Fix32 {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}

impl std::fmt::Display for Fix32 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let raw = i64::from(self.0);
        let magnitude = raw.abs();
        let whole = magnitude / i64::from(SCALE);
        let fraction = (magnitude % i64::from(SCALE)) * 10_000 / i64::from(SCALE);
        if raw < 0 {
            write!(f, "-{whole}.{fraction:04}")
        } else {
            write!(f, "{whole}.{fraction:04}")
        }
    }
}

impl From<i32> for Fix32 {
    fn from(val: i32) -> Self {
        Fix32::from_int(val)
    }
}

// Serialization support
impl serde::Serialize for Fix32 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for Fix32 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = i32::deserialize(deserializer)?;
        Ok(Fix32(raw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_int() {
        assert_eq!(Fix32::from_int(5).raw(), 5 * SCALE);
        assert_eq!(Fix32::from_int(-3).raw(), -3 * SCALE);
    }

    #[test]
    fn test_from_ratio() {
        assert_eq!(Fix32::from_ratio(3, 2).raw(), SCALE + SCALE / 2);
    }

    #[test]
    fn test_add() {
        let a = Fix32::from_int(3);
        let b = Fix32::from_int(4);
        assert_eq!(a + b, Fix32::from_int(7));
    }

    #[test]
    fn test_sub() {
        let a = Fix32::from_int(10);
        let b = Fix32::from_int(3);
        assert_eq!(a - b, Fix32::from_int(7));
    }

    #[test]
    fn test_mul() {
        assert_eq!(
            Fix32::from_ratio(3, 2) * Fix32::from_int(2),
            Fix32::from_int(3)
        );
    }

    #[test]
    fn test_div() {
        assert_eq!(
            Fix32::from_int(3) / Fix32::from_int(2),
            Fix32::from_ratio(3, 2)
        );
    }

    #[test]
    fn test_trunc() {
        let a = Fix32::from_ratio(15, 4);
        assert_eq!(a.trunc(), 3);
        let b = Fix32::from_ratio(-15, 4);
        assert_eq!(b.trunc(), -3);
    }

    #[test]
    fn test_round() {
        assert_eq!(Fix32::from_ratio(17, 5).round(), 3);
        assert_eq!(Fix32::from_ratio(18, 5).round(), 4);
        assert_eq!(Fix32::from_ratio(-17, 5).round(), -3);
    }

    #[test]
    fn test_abs() {
        assert_eq!(Fix32::from_int(-5).abs(), Fix32::from_int(5));
    }

    #[test]
    fn test_ordering() {
        let a = Fix32::from_int(3);
        let b = Fix32::from_int(5);
        assert!(a < b);
        assert!(b > a);
    }

    #[test]
    fn test_determinism() {
        let a = Fix32::from_ratio(3, 2);
        let b = Fix32::from_ratio(9, 4);
        let c = (a * b) / (a + b);
        assert_eq!(c.raw(), 921);
    }

    #[test]
    fn test_saturating_ops() {
        let max = Fix32(i32::MAX);
        let one = Fix32::from_int(1);
        // Saturating add should not overflow
        let result = max.saturating_add(one);
        assert_eq!(result, max);
    }

    #[test]
    fn test_lerp() {
        let start = Fix32::from_int(0);
        let end = Fix32::from_int(10);
        let half = Fix32::from_ratio(1, 2);
        let mid = start.lerp(end, half);
        assert_eq!(mid, Fix32::from_int(5));
    }

    #[test]
    fn test_clamp() {
        let low = Fix32::from_int(0);
        let high = Fix32::from_int(10);
        assert_eq!(Fix32::from_int(-5).clamp(low, high), low);
        assert_eq!(Fix32::from_int(5).clamp(low, high), Fix32::from_int(5));
        assert_eq!(Fix32::from_int(15).clamp(low, high), high);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let v = Fix32::from_ratio(22, 7);
        let raw = v.raw();
        let back = Fix32::from_raw(raw);
        assert_eq!(v, back);
    }

    #[test]
    fn multiplication_rounds_negative_infinity() {
        let tiny_negative = Fix32::from_raw(-1);
        assert_eq!((tiny_negative * Fix32::from_ratio(1, 2)).raw(), -1);
    }

    #[test]
    fn display_uses_integer_formatting() {
        assert_eq!(Fix32::from_ratio(-3, 2).to_string(), "-1.5000");
    }
}
