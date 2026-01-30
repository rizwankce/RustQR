/// Version information extraction for QR codes v7+
use crate::models::BitMatrix;

/// Version info is 18 bits (6 data + 12 ECC) for versions 7-40
pub struct VersionInfo;

impl VersionInfo {
    /// Extract version from QR code matrix (versions 7+ only)
    pub fn extract(matrix: &BitMatrix) -> Option<u8> {
        let size = matrix.width();
        if size < 45 {
            // Version 6 or below - no version info area
            return None;
        }

        // Try reading from top-right and bottom-left
        let bits_top_right = Self::read_version_bits_top_right(matrix)?;
        let bits_bottom_left = Self::read_version_bits_bottom_left(matrix)?;

        // They should match
        if bits_top_right == bits_bottom_left {
            Self::decode(bits_top_right)
        } else {
            // Try to correct errors
            Self::decode_with_correction(bits_top_right, bits_bottom_left)
        }
    }

    fn read_version_bits_top_right(matrix: &BitMatrix) -> Option<u32> {
        let size = matrix.width();
        let mut bits: u32 = 0;

        // Version info is in a 6x3 block below the top-right finder pattern
        // At the top-right corner, going down
        for row in 0..6 {
            for col in (size - 11)..(size - 8) {
                let is_black = matrix.get(col, row);
                bits = (bits << 1) | (is_black as u32);
            }
        }

        Some(bits)
    }

    fn read_version_bits_bottom_left(matrix: &BitMatrix) -> Option<u32> {
        let size = matrix.width();
        let mut bits: u32 = 0;

        // Version info is in a 3x6 block to the right of the bottom-left finder pattern
        for col in 0..6 {
            for row in (size - 11)..(size - 8) {
                let is_black = matrix.get(col, row);
                bits = (bits << 1) | (is_black as u32);
            }
        }

        Some(bits)
    }

    fn decode(version_bits: u32) -> Option<u8> {
        // BCH(18,6) decoding
        let corrected = Self::correct_errors(version_bits)?;

        // Top 6 bits are version number
        let version = (corrected >> 12) as u8;

        if version >= 7 && version <= 40 {
            Some(version)
        } else {
            None
        }
    }

    fn decode_with_correction(bits1: u32, bits2: u32) -> Option<u8> {
        // Try to use both copies to correct errors
        // If one is valid, use it
        if let Some(v) = Self::decode(bits1) {
            return Some(v);
        }
        if let Some(v) = Self::decode(bits2) {
            return Some(v);
        }
        None
    }

    fn correct_errors(mut codeword: u32) -> Option<u32> {
        // BCH(18,6) can correct up to 3 errors
        if Self::check_version(codeword) {
            return Some(codeword);
        }

        // Try single-bit corrections
        for i in 0..18 {
            let test = codeword ^ (1 << i);
            if Self::check_version(test) {
                return Some(test);
            }
        }

        None
    }

    fn check_version(codeword: u32) -> bool {
        // BCH(18,6) generator: x^12 + x^11 + x^10 + x^9 + x^8 + x^5 + x^2 + 1
        const GENERATOR: u32 = 0x1f25;
        let mut remainder = codeword;

        for _ in 0..6 {
            if remainder & 0x20000 != 0 {
                remainder ^= GENERATOR << 5;
            }
            remainder <<= 1;
        }

        let syndrome = (remainder >> 6) & 0xFFF;
        syndrome == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_check() {
        // Valid version info should pass check
        assert!(VersionInfo::check_version(0));
    }
}
