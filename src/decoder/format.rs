/// Format information extraction from QR code
use crate::models::{BitMatrix, ECLevel, MaskPattern};

/// Format info is 15 bits (5 data + 10 ECC)
/// Located at fixed positions in QR code corners
pub struct FormatInfo {
    pub ec_level: ECLevel,
    pub mask_pattern: MaskPattern,
}

impl FormatInfo {
    /// Extract format info from QR code matrix
    pub fn extract(matrix: &BitMatrix) -> Option<Self> {
        let bits_a = Self::read_format_bits_top_left(matrix)?;
        let bits_b = Self::read_format_bits_other(matrix)?;

        let bits_a_rev = Self::reverse_15(bits_a);
        let bits_b_rev = Self::reverse_15(bits_b);

        // Try both copies (and reversed); take the one with the smallest Hamming distance.
        let (best_a, dist_a) = Self::decode_with_distance(bits_a)
            .or_else(|| Self::decode_with_distance(bits_a_rev))?;
        let (best_b, dist_b) = Self::decode_with_distance(bits_b)
            .or_else(|| Self::decode_with_distance(bits_b_rev))?;

        if dist_a <= dist_b {
            Some(best_a)
        } else {
            Some(best_b)
        }
    }

    fn read_format_bits_top_left(matrix: &BitMatrix) -> Option<u16> {
        let size = matrix.width();
        if size < 21 {
            return None;
        }
        let mut bits: u16 = 0;

        // Order matches QR spec (and Nayuki) bit placement.
        for row in 0..6 {
            bits = (bits << 1) | (matrix.get(8, row) as u16);
        }
        bits = (bits << 1) | (matrix.get(8, 7) as u16);
        bits = (bits << 1) | (matrix.get(8, 8) as u16);
        bits = (bits << 1) | (matrix.get(7, 8) as u16);
        for col in (0..6).rev() {
            bits = (bits << 1) | (matrix.get(col, 8) as u16);
        }

        Some(bits)
    }

    fn read_format_bits_other(matrix: &BitMatrix) -> Option<u16> {
        let size = matrix.width();
        if size < 21 {
            return None;
        }
        let mut bits: u16 = 0;

        for j in 0..8 {
            bits = (bits << 1) | (matrix.get(size - 1 - j, 8) as u16);
        }
        for row in (size - 7)..=size - 1 {
            bits = (bits << 1) | (matrix.get(8, row) as u16);
        }

        Some(bits)
    }

    fn decode_with_distance(format_bits: u16) -> Option<(Self, u32)> {
        let mut best: Option<(Self, u32)> = None;
        for ecl_bits in 0..4u16 {
            for mask in 0..8u16 {
                let data = (ecl_bits << 3) | mask;
                let mut rem = data;
                for _ in 0..10 {
                    rem = (rem << 1) ^ (((rem >> 9) & 1) * 0x537);
                }
                let candidate = ((data << 10) | rem) ^ 0x5412;
                let dist = (candidate ^ format_bits).count_ones();
                if dist <= 3 {
                    let ec_level = match ecl_bits {
                        0 => ECLevel::M,
                        1 => ECLevel::L,
                        2 => ECLevel::H,
                        3 => ECLevel::Q,
                        _ => continue,
                    };
                    let mask_pattern = MaskPattern::from_bits(mask as u8)?;
                    let info = Self {
                        ec_level,
                        mask_pattern,
                    };
                    match best {
                        Some((_, best_dist)) if dist >= best_dist => {}
                        _ => best = Some((info, dist)),
                    }
                }
            }
        }
        best
    }

    fn reverse_15(bits: u16) -> u16 {
        let mut out = 0u16;
        for i in 0..15 {
            out = (out << 1) | ((bits >> i) & 1);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_extraction() {
        // Create a simple 21x21 matrix with some format bits
        let matrix = BitMatrix::new(21, 21);

        // Set format info for EC level M (01), mask pattern 0 (000)
        // Format bits: 00101 (5 bits) + ECC (10 bits)
        // Near top-left finder pattern

        // This is a simplified test - actual format bits would need proper ECC
        // Just verify the extraction function doesn't panic
        let _ = FormatInfo::extract(&matrix);
    }

}
