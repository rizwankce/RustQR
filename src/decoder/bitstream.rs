/// Bitstream extraction from QR code matrix
use crate::models::BitMatrix;

/// Extract raw bitstream from QR code matrix following zigzag pattern
pub struct BitstreamExtractor;

impl BitstreamExtractor {
    /// Extract data bits from matrix (excluding function patterns)
    pub fn extract(matrix: &BitMatrix, dimension: usize) -> Vec<bool> {
        let mut bits = Vec::new();

        // QR codes are read in a zigzag pattern starting from bottom-right
        // Going up in 2-column strips, alternating direction

        let mut upward = true;
        let mut col = dimension as i32 - 1;

        while col > 0 {
            // Skip timing column (column 6)
            if col == 6 {
                col -= 1;
                continue;
            }

            if upward {
                // Read bottom to top
                for row in (0..dimension).rev() {
                    if Self::is_data_module(matrix, row, col as usize, dimension) {
                        bits.push(matrix.get(col as usize, row));
                    }
                    // Check adjacent column
                    if col > 0 {
                        if Self::is_data_module(matrix, row, (col - 1) as usize, dimension) {
                            bits.push(matrix.get((col - 1) as usize, row));
                        }
                    }
                }
            } else {
                // Read top to bottom
                for row in 0..dimension {
                    if Self::is_data_module(matrix, row, col as usize, dimension) {
                        bits.push(matrix.get(col as usize, row));
                    }
                    // Check adjacent column
                    if col > 0 {
                        if Self::is_data_module(matrix, row, (col - 1) as usize, dimension) {
                            bits.push(matrix.get((col - 1) as usize, row));
                        }
                    }
                }
            }

            upward = !upward;
            col -= 2;
        }

        bits
    }

    /// Check if a module is a data module (not a function pattern)
    fn is_data_module(_matrix: &BitMatrix, _row: usize, _col: usize, _dimension: usize) -> bool {
        // TODO: Check against function patterns (finder, timing, alignment, etc.)
        // For now, assume all modules are data
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitstream_extraction() {
        let matrix = BitMatrix::new(21, 21);
        let bits = BitstreamExtractor::extract(&matrix, 21);
        // Should extract some bits (exact count depends on version and function patterns)
        assert!(!bits.is_empty());
    }
}
