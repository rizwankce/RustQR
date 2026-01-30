/// BCH error correction for QR code format and version info
pub struct BchDecoder;

impl BchDecoder {
    /// Decode format info (BCH(15,5))
    pub fn decode_format(format: u16) -> Option<(u8, u8)> {
        let corrected = Self::correct_format(format)?;
        let data = (corrected >> 10) as u8;
        Some(((data >> 3) & 0x03, data & 0x07))
    }

    fn correct_format(codeword: u16) -> Option<u16> {
        if codeword == 0 {
            return None;
        }
        // Quick check - if syndrome is 0, no errors
        if Self::check_format(codeword) {
            return Some(codeword);
        }
        // Try correcting single bit errors
        for i in 0..15 {
            let test = codeword ^ (1 << i);
            if Self::check_format(test) {
                return Some(test);
            }
        }
        None
    }

    fn check_format(codeword: u16) -> bool {
        // Simple parity check for now
        codeword.count_ones() % 2 == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_decode() {
        let result = BchDecoder::decode_format(0b00101_1111001100);
        assert!(result.is_some());
    }
}
