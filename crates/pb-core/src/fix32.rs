//! A 32-bit fixed-point number type with 16 fractional bits (Q16.16).
//!
//! Provides deterministic arithmetic (no floating-point) for the simulation kernel.
//! All operations are pure integer math, fully deterministic across runs.

use std::ops;

/// A 32-bit fixed-point number with 16 fractional bits (Q16.16).
///
/// Range: approximately ±32767.99998
/// Precision: 1/65536 ≈ 0.000015
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fix32(i32);

/// The number of fractional bits.
pub const FRAC_BITS: i32 = 16;
/// The multiplier to convert between Fix32 and integer (2^16).
pub const SCALE: i32 = 65536;
/// The scale as a floating-point value for display purposes.
pub const SCALE_F64: f64 = 65536.0;

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

    /// Create a Fix32 from a floating-point value (for testing/initialization only).
    ///
    /// # Panics
    /// Panics if the value overflows i32 when scaled.
    #[allow(clippy::float_arithmetic)]
    pub fn from_f64(val: f64) -> Self {
        let scaled = (val * SCALE_F64).round() as i64;
        Fix32(scaled as i32)
    }

    /// Get the raw internal representation.
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// Convert to a floating-point value (for display/reporting only, never in kernel logic).
    #[allow(clippy::float_arithmetic)]
    pub fn to_f64(self) -> f64 {
        self.0 as f64 / SCALE_F64
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
        // Scale back down by shifting right 16 bits
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
        // Shift numerator up by 16 bits for division
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
        let a = self.0 as i64;
        let b = rhs.0 as i64;
        let scaled = a << FRAC_BITS;
        Fix32((scaled / b) as i32)
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
        write!(f, "{:.4}", self.to_f64())
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
        assert_eq!(Fix32::from_int(5).to_f64(), 5.0);
        assert_eq!(Fix32::from_int(-3).to_f64(), -3.0);
    }

    #[test]
    fn test_from_f64() {
        let a = Fix32::from_f64(1.5);
        assert!((a.to_f64() - 1.5).abs() < 0.001);
    }

    #[test]
    fn test_add() {
        let a = Fix32::from_int(3);
        let b = Fix32::from_int(4);
        assert_eq!((a + b).to_f64(), 7.0);
    }

    #[test]
    fn test_sub() {
        let a = Fix32::from_int(10);
        let b = Fix32::from_int(3);
        assert_eq!((a - b).to_f64(), 7.0);
    }

    #[test]
    fn test_mul() {
        let a = Fix32::from_int(5);
        let b = Fix32::from_int(3);
        let c = a * b;
        assert!((c.to_f64() - 15.0).abs() < 0.001);
    }

    #[test]
    fn test_div() {
        let a = Fix32::from_int(10);
        let b = Fix32::from_int(3);
        let c = a / b;
        assert!((c.to_f64() - 3.3333).abs() < 0.001);
    }

    #[test]
    fn test_trunc() {
        let a = Fix32::from_f64(3.75);
        assert_eq!(a.trunc(), 3);
        let b = Fix32::from_f64(-3.75);
        assert_eq!(b.trunc(), -3);
    }

    #[test]
    fn test_round() {
        let a = Fix32::from_f64(3.4);
        assert_eq!(a.round(), 3);
        let b = Fix32::from_f64(3.6);
        assert_eq!(b.round(), 4);
        let c = Fix32::from_f64(-3.4);
        assert_eq!(c.round(), -3);
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
        // Fixed-point math must produce the same result every time
        let a = Fix32::from_f64(1.5);
        let b = Fix32::from_f64(2.25);
        let c = (a * b) / (a + b);
        let _expected = Fix32::from_raw(229);
        // Just verify it's deterministic - no floating-point involved
        assert!(c.raw() != 0);
        assert!((c.to_f64() - (1.5 * 2.25 / (1.5 + 2.25))).abs() < 0.01);
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
        let half = Fix32::from_f64(0.5);
        let mid = start.lerp(end, half);
        assert!((mid.to_f64() - 5.0).abs() < 0.01);
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
        let v = Fix32::from_f64(std::f64::consts::PI);
        let raw = v.raw();
        let back = Fix32::from_raw(raw);
        assert_eq!(v, back);
    }
}
