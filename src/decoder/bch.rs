/// BCH error correction for QR Code format information.
///
/// Format information is a masked BCH(15,5) code. Rather than treating its
/// parity as a generic even-parity bit, decode against all 32 valid QR format
/// codewords and accept only the ISO-guaranteed Hamming radius of three.
pub struct BchDecoder;

impl BchDecoder {
    /// Decode a masked 15-bit QR format codeword into `(ec_level_bits, mask)`.
    pub fn decode_format(format: u16) -> Option<(u8, u8)> {
        (0..32u8)
            .map(|data| (data, (Self::format_codeword(data) ^ format).count_ones()))
            .filter(|(_, distance)| *distance <= 3)
            .min_by_key(|(_, distance)| *distance)
            .map(|(data, _)| ((data >> 3) & 0x03, data & 0x07))
    }

    fn format_codeword(data: u8) -> u16 {
        const GENERATOR: u16 = 0x537;
        const MASK: u16 = 0x5412;

        let mut remainder = data as u16;
        for _ in 0..10 {
            remainder = (remainder << 1) ^ (((remainder >> 9) & 1) * GENERATOR);
        }
        ((data as u16) << 10) | remainder ^ MASK
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bch_corrects_up_to_three_bits_only() {
        // H / mask 0 is the masked format codeword 0x1689.
        let codeword = 0x1689;
        assert_eq!(BchDecoder::decode_format(codeword), Some((2, 0)));
        assert_eq!(
            BchDecoder::decode_format(codeword ^ 0b1_0010_0000),
            Some((2, 0))
        );
        assert_eq!(BchDecoder::decode_format(codeword ^ 0b1_1011_0000), None);
    }
}
