/// Fixed-point arithmetic module (16.16 format)
///
/// 16.16 fixed-point representation:
/// - 16 bits integer part
/// - 16 bits fractional part
/// - Range: approximately ±32767.9999
/// - Precision: 1/65536 ≈ 0.000015
///
/// Fixed-point type (16.16 format)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fixed(i32);

impl Fixed {
    /// Number of fractional bits
    const FRACTIONAL_BITS: i32 = 16;
    /// Scaling factor: 2^16 = 65536
    const SCALE: i32 = 65536;

    /// Create from integer
    pub const fn from_i32(n: i32) -> Self {
        Fixed(n.wrapping_shl(Self::FRACTIONAL_BITS as u32))
    }

    /// Create from float
    pub fn from_f32(f: f32) -> Self {
        Fixed((f * (Self::SCALE as f32)) as i32)
    }

    /// Convert to float
    pub fn to_f32(&self) -> f32 {
        (self.0 as f32) / (Self::SCALE as f32)
    }

    /// Convert to integer (truncates)
    pub fn to_i32(&self) -> i32 {
        self.0 >> Self::FRACTIONAL_BITS
    }

    /// Add
    pub fn add(&self, other: &Fixed) -> Fixed {
        Fixed(self.0.wrapping_add(other.0))
    }

    /// Multiply
    pub fn mul(&self, other: &Fixed) -> Fixed {
        let product = (self.0 as i64).wrapping_mul(other.0 as i64);
        Fixed((product >> Self::FRACTIONAL_BITS) as i32)
    }

    /// Divide
    pub fn div(&self, other: &Fixed) -> Option<Fixed> {
        if other.0 == 0 {
            return None;
        }
        let dividend = (self.0 as i64) << Self::FRACTIONAL_BITS;
        Some(Fixed((dividend / other.0 as i64) as i32))
    }
}

/// 3x3 fixed-point matrix
#[derive(Debug, Clone, Copy)]
pub struct FixedMatrix3x3 {
    pub m: [[Fixed; 3]; 3],
}

impl FixedMatrix3x3 {
    /// Identity matrix
    pub fn identity() -> Self {
        let one = Fixed::from_i32(1);
        let zero = Fixed::from_i32(0);

        Self {
            m: [[one, zero, zero], [zero, one, zero], [zero, zero, one]],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_basic() {
        let one = Fixed::from_i32(1);
        let two = Fixed::from_i32(2);
        let three = one.add(&two);
        assert_eq!(three.to_i32(), 3);
    }
}
