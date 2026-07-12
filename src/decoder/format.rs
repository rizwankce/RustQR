use crate::decoder::bch::BchDecoder;
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

        let result_a = Self::decode_best_direction(bits_a);
        let result_b = Self::decode_best_direction(bits_b);

        match (result_a, result_b) {
            (Some((a, dist_a)), Some((b, dist_b))) => {
                if dist_a <= dist_b {
                    Some(a)
                } else {
                    Some(b)
                }
            }
            (Some((a, _)), None) => Some(a),
            (None, Some((b, _))) => Some(b),
            (None, None) => None,
        }
    }

    /// Extract soft format candidates with BCH distance up to `max_dist`.
    /// Returns candidates sorted by distance (best first), excluding any
    /// that would have been returned by `extract()` (distance ≤ 3).
    pub fn extract_soft(matrix: &BitMatrix, max_dist: u32) -> Vec<Self> {
        let bits_a = Self::read_format_bits_top_left(matrix);
        let bits_b = Self::read_format_bits_other(matrix);

        let mut candidates: Vec<(Self, u32)> = Vec::new();
        let mut seen = [false; 32]; // track ec_bits*8 + mask combos

        for &bits_opt in &[bits_a, bits_b] {
            let Some(bits) = bits_opt else { continue };
            for &b in &[bits, Self::reverse_15(bits)] {
                for ecl_bits in 0..4u8 {
                    for mask in 0..8u8 {
                        let combo = (ecl_bits * 8 + mask) as usize;
                        if seen[combo] {
                            continue;
                        }
                        let data = (ecl_bits << 3) | mask;
                        let dist = (BchDecoder::format_codeword(data) ^ b).count_ones();
                        if dist > 3 && dist <= max_dist {
                            seen[combo] = true;
                            let ec_level = match ecl_bits {
                                0 => ECLevel::M,
                                1 => ECLevel::L,
                                2 => ECLevel::H,
                                3 => ECLevel::Q,
                                _ => continue,
                            };
                            if let Some(mask_pattern) = MaskPattern::from_bits(mask) {
                                candidates.push((
                                    Self {
                                        ec_level,
                                        mask_pattern,
                                    },
                                    dist,
                                ));
                            }
                        }
                    }
                }
            }
        }

        candidates.sort_by_key(|(_, d)| *d);
        candidates.truncate(4);
        candidates.into_iter().map(|(info, _)| info).collect()
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
        let decoded = BchDecoder::decode_format_with_distance(format_bits)?;
        let ec_level = match decoded.value >> 3 {
            0 => ECLevel::M,
            1 => ECLevel::L,
            2 => ECLevel::H,
            3 => ECLevel::Q,
            _ => return None,
        };
        Some((
            Self {
                ec_level,
                mask_pattern: MaskPattern::from_bits(decoded.value & 0x07)?,
            },
            decoded.distance,
        ))
    }

    fn decode_best_direction(format_bits: u16) -> Option<(Self, u32)> {
        let forward = Self::decode_with_distance(format_bits);
        let reversed = Self::decode_with_distance(Self::reverse_15(format_bits));
        match (forward, reversed) {
            (Some(forward), Some(reversed)) => {
                if forward.1 <= reversed.1 {
                    Some(forward)
                } else {
                    Some(reversed)
                }
            }
            (Some(result), None) | (None, Some(result)) => Some(result),
            (None, None) => None,
        }
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

    #[test]
    fn reversed_exact_format_beats_forward_near_match() {
        // H / mask 0 has the masked format codeword 0x1689. Matrix placement
        // exposes its least-significant bit first to the extraction traversal.
        let placed_bits = FormatInfo::reverse_15(0x1689);
        let (info, distance) = FormatInfo::decode_best_direction(placed_bits).unwrap();
        assert_eq!(distance, 0);
        assert_eq!(info.ec_level, ECLevel::H);
        assert_eq!(info.mask_pattern, MaskPattern::Pattern0);
    }
}
