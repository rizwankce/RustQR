//! Kanji mode decoder (Mode 1000).
//!
//! QR Kanji values encode pairs of Shift-JIS bytes in 13 bits.  The returned
//! bytes deliberately remain Shift-JIS; callers can choose an appropriate
//! text conversion without losing the original payload.

pub struct KanjiDecoder;

impl KanjiDecoder {
    /// Decode `character_count` QR Kanji values into their original Shift-JIS
    /// byte pairs, returning the bytes and number of consumed bits.
    pub fn decode(bits: &[bool], character_count: usize) -> Option<(Vec<u8>, usize)> {
        let required_bits = character_count.checked_mul(13)?;
        if bits.len() < required_bits {
            return None;
        }

        let mut bytes = Vec::with_capacity(character_count * 2);
        for chunk in bits[..required_bits].chunks_exact(13) {
            let value = chunk
                .iter()
                .fold(0u16, |value, bit| (value << 1) | u16::from(*bit));
            let subtracted = ((value / 0xC0) << 8) | (value % 0xC0);
            let shift_jis = if subtracted < 0x1F00 {
                subtracted + 0x8140
            } else {
                subtracted + 0xC140
            };
            bytes.extend_from_slice(&shift_jis.to_be_bytes());
        }
        Some((bytes, required_bits))
    }
}

#[cfg(test)]
mod tests {
    use super::KanjiDecoder;

    #[test]
    fn decodes_qr_kanji_values_to_original_shift_jis_bytes() {
        // "漢字" in Shift-JIS is 8a bf 8e 9a. Its QR Kanji values are
        // 0x073f and 0x0a1a, respectively.
        let bits = "00111001111110101000011010"
            .bytes()
            .map(|bit| bit == b'1')
            .collect::<Vec<_>>();

        let (bytes, used) = KanjiDecoder::decode(&bits, 2).expect("valid Kanji data");
        assert_eq!(used, 26);
        assert_eq!(bytes, [0x8a, 0xbf, 0x8e, 0x9a]);
    }

    #[test]
    fn rejects_truncated_kanji_data() {
        assert!(KanjiDecoder::decode(&[false; 12], 1).is_none());
    }
}
