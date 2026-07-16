use crate::BitMatrix;
/// Version information extraction for QR codes v7+.
use crate::decoder::bch::BchDecoder;
use alloc::vec::Vec;

/// Which redundant version-information field supplied the accepted codeword.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionInfoCopy {
    TopRight,
    BottomLeft,
}

/// BCH evidence for an extracted QR version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionInfoEvidence {
    pub version: u8,
    pub distance: u32,
    pub copy: VersionInfoCopy,
}

/// Version info is 18 bits (6 data + 12 BCH parity) for versions 7-40.
pub struct VersionInfo;

impl VersionInfo {
    /// Extract a version from either redundant version-information field.
    pub fn extract(matrix: &BitMatrix) -> Option<u8> {
        Self::extract_with_evidence(matrix).map(|evidence| evidence.version)
    }

    /// Extract version information and retain the BCH correction evidence.
    ///
    /// Both copies are decoded independently. If they disagree, the lower
    /// Hamming-distance result wins; equal evidence deliberately prefers the
    /// top-right copy so the choice is deterministic.
    pub fn extract_with_evidence(matrix: &BitMatrix) -> Option<VersionInfoEvidence> {
        if matrix.width() != matrix.height() || matrix.width() < 45 {
            return None;
        }

        let mut candidates = Vec::with_capacity(4);
        for (copy, bits) in [
            (
                VersionInfoCopy::TopRight,
                Self::read_version_bits_top_right(matrix)?,
            ),
            (
                VersionInfoCopy::BottomLeft,
                Self::read_version_bits_bottom_left(matrix)?,
            ),
        ] {
            // The module traversal has a defined direction, but sampled
            // matrices from image orientations may expose the reverse order.
            for codeword in [bits, Self::reverse_18(bits)] {
                if let Some(decoded) = BchDecoder::decode_version(codeword) {
                    candidates.push(VersionInfoEvidence {
                        version: decoded.value,
                        distance: decoded.distance,
                        copy,
                    });
                }
            }
        }
        candidates
            .into_iter()
            .min_by_key(|candidate| candidate.distance)
    }

    fn read_version_bits_top_right(matrix: &BitMatrix) -> Option<u32> {
        let size = matrix.width();
        if size < 45 {
            return None;
        }
        let mut bits = 0u32;
        // ISO placement: three columns by six rows next to the top-right
        // finder. The read order agrees with the matrix placement order.
        for row in 0..6 {
            for col in (size - 11)..(size - 8) {
                bits = (bits << 1) | matrix.get(col, row) as u32;
            }
        }
        Some(bits)
    }

    fn read_version_bits_bottom_left(matrix: &BitMatrix) -> Option<u32> {
        let size = matrix.width();
        if size < 45 {
            return None;
        }
        let mut bits = 0u32;
        // ISO placement: six columns by three rows next to the bottom-left
        // finder. This is the second, independently protected copy.
        for col in 0..6 {
            for row in (size - 11)..(size - 8) {
                bits = (bits << 1) | matrix.get(col, row) as u32;
            }
        }
        Some(bits)
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
    use crate::decoder::bch::CORRECTION_RADIUS;

    #[test]
    fn version_codewords_correct_up_to_three_bits_only() {
        for version in 7..=40 {
            let codeword = BchDecoder::version_codeword(version);
            assert_eq!(
                BchDecoder::decode_version(codeword).map(|decoded| decoded.value),
                Some(version)
            );
            assert_eq!(
                BchDecoder::decode_version(codeword ^ 0b111).map(|decoded| decoded.value),
                Some(version)
            );
        }
        assert_eq!(BchDecoder::decode_version(0), None);
        assert_eq!(CORRECTION_RADIUS, 3);
    }
}
