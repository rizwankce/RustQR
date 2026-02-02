/// Format information extraction from QR code
use crate::models::{BitMatrix, ECLevel, MaskPattern};

/// Format info is 15 bits (5 data + 10 ECC)
/// Located at fixed positions in QR code corners
pub struct FormatInfo {
    pub ec_level: ECLevel,
    pub mask_pattern: MaskPattern,
}

impl FormatInfo {
    /// Extract format info from QR code matrix
    pub fn extract(matrix: &BitMatrix) -> Option<Self> {
        eprintln!(
            "[DEBUG] FormatInfo::extract called, matrix size: {}x{}",
            matrix.width(),
            matrix.height()
        );

        // Try reading from top-left area (around finder pattern)
        let format_bits = match Self::read_format_bits_top_left(matrix) {
            Some(bits) => {
                eprintln!(
                    "[DEBUG] read_format_bits_top_left returned: 0b{:015b} (0x{:04X})",
                    bits, bits
                );
                bits
            }
            None => {
                eprintln!("[DEBUG] read_format_bits_top_left returned None");
                return None;
            }
        };

        Self::decode(format_bits)
    }

    fn read_format_bits_top_left(matrix: &BitMatrix) -> Option<u16> {
        let version_size = matrix.width();
        eprintln!(
            "[DEBUG] read_format_bits_top_left: matrix size = {}",
            version_size
        );
        if version_size < 21 {
            eprintln!("[DEBUG] Matrix too small, need at least 21x21");
            return None;
        }

        // Format info bits are located at:
        // - Row 8, columns 0-7 (excluding timing pattern at column 6)
        // - Column 8, rows 0-7 (excluding timing pattern at row 6)
        let mut bits: u16 = 0;
        let mut bit_count = 0;

        // Read row 8, columns 0-7 (skip column 6 which is timing)
        eprintln!("[DEBUG] Reading row 8, columns 0-7 (skip col 6):");
        for col in 0..8 {
            if col == 6 {
                continue; // Skip timing pattern
            }
            let is_black = matrix.get(col, 8);
            bits = (bits << 1) | (is_black as u16);
            bit_count += 1;
            eprintln!(
                "  col {}: {} (bit {})",
                col,
                if is_black { 1 } else { 0 },
                bit_count
            );
        }

        // Read column 8, rows 0-7 (skip row 6 which is timing, read bottom-up)
        eprintln!("[DEBUG] Reading column 8, rows 7-0 (skip row 6, bottom-up):");
        for row in (0..8).rev() {
            if row == 6 {
                continue; // Skip timing pattern
            }
            let is_black = matrix.get(8, row);
            bits = (bits << 1) | (is_black as u16);
            bit_count += 1;
            eprintln!(
                "  row {}: {} (bit {})",
                row,
                if is_black { 1 } else { 0 },
                bit_count
            );
        }

        eprintln!("[DEBUG] Total bits read: {}, expected: 15", bit_count);
        eprintln!("[DEBUG] Final bits value: 0b{:015b} (0x{:04X})", bits, bits);

        if bit_count == 15 {
            Some(bits)
        } else {
            None
        }
    }

    /// Decode 15-bit format info
    fn decode(format_bits: u16) -> Option<Self> {
        // Try to correct errors using BCH
        let corrected = Self::correct_errors(format_bits)?;

        // Extract data bits (top 5 bits)
        let data_bits = (corrected >> 10) & 0x1F;

        // EC level is bits 4-3
        let ec_bits = ((data_bits >> 3) & 0x03) as u8;
        let ec_level = ECLevel::from_bits(ec_bits)?;

        // Mask pattern is bits 2-0
        let mask_bits = (data_bits & 0x07) as u8;
        let mask_pattern = MaskPattern::from_bits(mask_bits)?;

        Some(Self {
            ec_level,
            mask_pattern,
        })
    }

    fn correct_errors(codeword: u16) -> Option<u16> {
        // BCH(15,5) can correct up to 3 errors
        // For simplicity, try all single-bit corrections first

        if Self::check_format(codeword) {
            return Some(codeword);
        }

        // Try flipping each bit
        for i in 0..15 {
            let test = codeword ^ (1 << i);
            if Self::check_format(test) {
                return Some(test);
            }
        }

        // Try double-bit errors
        for i in 0..15 {
            for j in (i + 1)..15 {
                let test = codeword ^ (1 << i) ^ (1 << j);
                if Self::check_format(test) {
                    return Some(test);
                }
            }
        }

        None
    }

    fn check_format(codeword: u16) -> bool {
        // BCH(15,5) generator polynomial: x^10 + x^8 + x^5 + x^4 + x^2 + x + 1
        // Simplified check - XOR with generator polynomial
        const GENERATOR: u16 = 0x537;
        let mut remainder = codeword;

        for _ in 0..5 {
            if remainder & 0x4000 != 0 {
                remainder ^= GENERATOR << 4;
            }
            remainder <<= 1;
        }

        let syndrome = (remainder >> 5) & 0x3FF;
        syndrome == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_extraction() {
        // Create a simple 21x21 matrix with some format bits
        let matrix = BitMatrix::new(21, 21);

        // Set format info for EC level M (01), mask pattern 0 (000)
        // Format bits: 00101 (5 bits) + ECC (10 bits)
        // Near top-left finder pattern

        // This is a simplified test - actual format bits would need proper ECC
        // Just verify the extraction function doesn't panic
        let _ = FormatInfo::extract(&matrix);
    }

    #[test]
    fn test_format_check() {
        // Test that a valid format passes the check
        // Format with all zeros should have syndrome 0
        assert!(FormatInfo::check_format(0));
    }
}
