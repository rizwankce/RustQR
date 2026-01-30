/// Byte mode decoder (Mode 0100) for 8-bit data
/// Decode byte mode data (8 bits per character)
pub struct ByteDecoder;

impl ByteDecoder {
    pub fn decode(bits: &[bool], character_count: usize) -> Option<(String, usize)> {
        let mut bytes = Vec::with_capacity(character_count);
        let mut bit_idx = 0;

        for _ in 0..character_count {
            if bit_idx + 8 > bits.len() {
                return None;
            }

            let mut byte: u8 = 0;
            for i in 0..8 {
                byte = (byte << 1) | (bits[bit_idx + i] as u8);
            }

            bytes.push(byte);
            bit_idx += 8;
        }

        // Convert bytes to UTF-8 string
        match String::from_utf8(bytes) {
            Ok(s) => Some((s, bit_idx)),
            Err(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_byte_decode() {
        // "HI" in ASCII = 72, 73
        // H = 0x48 = 01001000
        // I = 0x49 = 01001001
        let bits = vec![
            false, true, false, false, true, false, false, false, // H
            false, true, false, false, true, false, false, true, // I
        ];
        let result = ByteDecoder::decode(&bits, 2);
        assert!(result.is_some());
        let (decoded, _) = result.unwrap();
        assert_eq!(decoded, "HI");
    }
}
