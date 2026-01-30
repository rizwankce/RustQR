/// Alphanumeric mode decoder (Mode 0010)
/// Alphanumeric character set: 0-9, A-Z, space, $%*+-./:
const ALPHANUMERIC_TABLE: [char; 45] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I',
    'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', ' ', '$',
    '%', '*', '+', '-', '.', '/', ':',
];

/// Decode alphanumeric data
/// Pairs = 11 bits, single = 6 bits
pub struct AlphanumericDecoder;

impl AlphanumericDecoder {
    pub fn decode(bits: &[bool], character_count: usize) -> Option<(String, usize)> {
        let mut result = String::new();
        let mut bit_idx = 0;
        let mut chars_remaining = character_count;

        while chars_remaining > 0 {
            if chars_remaining >= 2 {
                // Decode pair (11 bits)
                if bit_idx + 11 > bits.len() {
                    return None;
                }

                let mut value: u16 = 0;
                for i in 0..11 {
                    value = (value << 1) | (bits[bit_idx + i] as u16);
                }

                let first_char = (value / 45) as usize;
                let second_char = (value % 45) as usize;

                if first_char < 45 && second_char < 45 {
                    result.push(ALPHANUMERIC_TABLE[first_char]);
                    result.push(ALPHANUMERIC_TABLE[second_char]);
                }

                bit_idx += 11;
                chars_remaining -= 2;
            } else {
                // Decode single character (6 bits)
                if bit_idx + 6 > bits.len() {
                    return None;
                }

                let mut value: u8 = 0;
                for i in 0..6 {
                    value = (value << 1) | (bits[bit_idx + i] as u8);
                }

                if (value as usize) < 45 {
                    result.push(ALPHANUMERIC_TABLE[value as usize]);
                }

                bit_idx += 6;
                chars_remaining -= 1;
            }
        }

        Some((result, bit_idx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alphanumeric_decode() {
        // Encode "A1" = (10 * 45 + 1) = 451 = 0b00111000011 (11 bits)
        let bits = vec![
            false, false, true, true, true, false, false, false, false, true, true,
        ];
        let result = AlphanumericDecoder::decode(&bits, 2);
        assert!(result.is_some());
        let (decoded, _) = result.unwrap();
        assert_eq!(decoded, "A1");
    }
}
