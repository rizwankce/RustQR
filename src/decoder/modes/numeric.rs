/// Numeric mode decoder (Mode 0001)
/// Decode numeric mode data
/// Groups of 3 digits = 10 bits, 2 digits = 7 bits, 1 digit = 4 bits
pub struct NumericDecoder;

impl NumericDecoder {
    /// Decode numeric data from bit stream
    /// Returns (decoded_string, bits_consumed)
    pub fn decode(bits: &[bool], character_count: usize) -> Option<(String, usize)> {
        let mut result = String::new();
        let mut bit_idx = 0;
        let mut chars_remaining = character_count;

        while chars_remaining > 0 {
            let group_size = chars_remaining.min(3);
            let bits_needed = match group_size {
                3 => 10,
                2 => 7,
                1 => 4,
                _ => return None,
            };

            if bit_idx + bits_needed > bits.len() {
                return None;
            }

            // Read value from bits
            let mut value: u16 = 0;
            for i in 0..bits_needed {
                value = (value << 1) | (bits[bit_idx + i] as u16);
            }

            // Convert to digits
            let digits = match group_size {
                3 => format!("{:03}", value),
                2 => format!("{:02}", value),
                1 => format!("{}", value),
                _ => return None,
            };

            result.push_str(&digits);
            bit_idx += bits_needed;
            chars_remaining -= group_size;
        }

        Some((result, bit_idx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_numeric_decode() {
        // Test 3 digits (10 bits) - binary 11111111001 = 1017
        let bits = vec![true, true, true, true, true, true, true, false, false, true];
        let result = NumericDecoder::decode(&bits, 3);
        assert!(result.is_some());
        let (decoded, bits_used) = result.unwrap();
        assert_eq!(decoded, "1017");
        assert_eq!(bits_used, 10);
    }
}
