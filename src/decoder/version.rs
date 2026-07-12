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
        Self::decode_with_distance(version_bits)
            .or_else(|| Self::decode_with_distance(Self::reverse_18(version_bits)))
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

    fn decode_with_distance(codeword: u32) -> Option<u8> {
        (7..=40)
            .map(|version| (version, Self::version_codeword(version) ^ codeword))
            .map(|(version, difference)| (version, difference.count_ones()))
            .filter(|(_, distance)| *distance <= 3)
            .min_by_key(|(_, distance)| *distance)
            .map(|(version, _)| version)
    }

    fn version_codeword(version: u8) -> u32 {
        const GENERATOR: u32 = 0x1f25;
        let mut remainder = version as u32;
        for _ in 0..12 {
            remainder = (remainder << 1) ^ (((remainder >> 11) & 1) * GENERATOR);
        }
        ((version as u32) << 12) | remainder
    }

    fn reverse_18(bits: u32) -> u32 {
        let mut reversed = 0;
        for index in 0..18 {
            reversed = (reversed << 1) | ((bits >> index) & 1);
        }
        reversed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_check() {
        for version in 7..=40 {
            let codeword = VersionInfo::version_codeword(version);
            assert_eq!(VersionInfo::decode(codeword), Some(version));
            assert_eq!(VersionInfo::decode(codeword ^ 0b111), Some(version));
        }
        assert_eq!(VersionInfo::decode(0), None);
    }
}
