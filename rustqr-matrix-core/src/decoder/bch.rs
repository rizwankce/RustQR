//! BCH decoding for QR format and version information.
//!
//! QR uses two short, fixed BCH codes.  Keeping their codeword construction
//! and nearest-codeword search here prevents format and version handling from
//! silently drifting apart.

/// QR's guaranteed BCH correction radius for both fixed information fields.
pub const CORRECTION_RADIUS: u32 = 3;

/// A BCH candidate together with the observed Hamming distance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodedBch<T> {
    pub value: T,
    pub distance: u32,
}

pub struct BchDecoder;

impl BchDecoder {
    /// Decode a masked 15-bit format codeword into `(ec_level_bits, mask)`.
    ///
    /// This compatibility helper preserves the original public API. Call
    /// [`Self::decode_format_with_distance`] when validation needs the BCH
    /// correction evidence.
    pub fn decode_format(format: u16) -> Option<(u8, u8)> {
        Self::decode_format_with_distance(format)
            .map(|decoded| ((decoded.value >> 3) & 0x03, decoded.value & 0x07))
    }

    /// Decode a masked 15-bit format codeword and report its Hamming distance.
    pub fn decode_format_with_distance(format: u16) -> Option<DecodedBch<u8>> {
        nearest_codeword(format as u32, 0..32u8, |data| {
            Self::format_codeword(data) as u32
        })
    }

    /// Decode an unmasked 18-bit version codeword into versions 7 through 40.
    pub fn decode_version(version: u32) -> Option<DecodedBch<u8>> {
        nearest_codeword(version, 7..=40u8, Self::version_codeword)
    }

    /// Construct the masked QR format BCH(15,5) codeword for its five data bits.
    pub fn format_codeword(data: u8) -> u16 {
        const GENERATOR: u16 = 0x537;
        const MASK: u16 = 0x5412;
        let remainder = bch_remainder(data as u32, 5, 10, GENERATOR as u32);
        (((data as u16) << 10) | remainder as u16) ^ MASK
    }

    /// Construct the QR version BCH(18,6) codeword for versions 7 through 40.
    pub fn version_codeword(version: u8) -> u32 {
        const GENERATOR: u32 = 0x1f25;
        let remainder = bch_remainder(version as u32, 6, 12, GENERATOR);
        ((version as u32) << 12) | remainder
    }
}

fn bch_remainder(data: u32, data_bits: u32, ecc_bits: u32, generator: u32) -> u32 {
    debug_assert!(data < (1 << data_bits));
    let mut remainder = data << ecc_bits;
    for bit in (ecc_bits..=(data_bits + ecc_bits - 1)).rev() {
        if (remainder >> bit) & 1 != 0 {
            remainder ^= generator << (bit - ecc_bits);
        }
    }
    remainder & ((1 << ecc_bits) - 1)
}

fn nearest_codeword<T: Copy>(
    received: u32,
    candidates: impl IntoIterator<Item = T>,
    codeword: impl Fn(T) -> u32,
) -> Option<DecodedBch<T>> {
    candidates
        .into_iter()
        .map(|value| DecodedBch {
            value,
            distance: (codeword(value) ^ received).count_ones(),
        })
        .filter(|candidate| candidate.distance <= CORRECTION_RADIUS)
        .min_by_key(|candidate| candidate.distance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bch_corrects_up_to_three_bits_only() {
        // H / mask 0 is the masked format codeword 0x1689.
        let codeword = 0x1689;
        assert_eq!(
            BchDecoder::decode_format_with_distance(codeword),
            Some(DecodedBch {
                value: 16,
                distance: 0
            })
        );
        assert_eq!(BchDecoder::decode_format(codeword), Some((2, 0)));
        assert_eq!(
            BchDecoder::decode_format_with_distance(codeword ^ 0b1_0010_0000),
            Some(DecodedBch {
                value: 16,
                distance: 2
            })
        );
        assert_ne!(
            BchDecoder::decode_format_with_distance(codeword ^ 0b1_1011_0000)
                .map(|decoded| decoded.value),
            Some(16),
            "four corrupted bits must not be accepted as the original data"
        );
    }

    #[test]
    fn version_bch_corrects_up_to_three_bits_only() {
        let codeword = BchDecoder::version_codeword(7);
        assert_eq!(
            BchDecoder::decode_version(codeword),
            Some(DecodedBch {
                value: 7,
                distance: 0
            })
        );
        assert_eq!(
            BchDecoder::decode_version(codeword ^ 0b1_0000_0101),
            Some(DecodedBch {
                value: 7,
                distance: 3
            })
        );
        assert_ne!(
            BchDecoder::decode_version(codeword ^ 0b11_0000_0101).map(|decoded| decoded.value),
            Some(7),
            "four corrupted bits must not be accepted as the original version"
        );
    }
}
