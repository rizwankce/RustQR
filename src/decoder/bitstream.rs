/// Bitstream extraction from QR code matrix
use crate::decoder::function_mask::FunctionMask;
use crate::models::BitMatrix;

/// Extract raw bitstream from QR code matrix following zigzag pattern
pub struct BitstreamExtractor;

impl BitstreamExtractor {
    /// Extract data bits from matrix (excluding function patterns)
    pub fn extract(matrix: &BitMatrix, dimension: usize, func: &FunctionMask) -> Vec<bool> {
        Self::extract_with_options(matrix, dimension, func, true, false)
    }

    /// Extract data bits with traversal options (for robustness).
    pub fn extract_with_options(
        matrix: &BitMatrix,
        dimension: usize,
        func: &FunctionMask,
        start_upward: bool,
        swap_columns: bool,
    ) -> Vec<bool> {
        let mut bits = Vec::new();

        let mut upward = start_upward;
        let mut col = dimension as i32 - 1;

        while col > 0 {
            if col == 6 {
                col -= 1;
                continue;
            }

            let (first_col, second_col) = if swap_columns {
                (col - 1, col)
            } else {
                (col, col - 1)
            };

            if upward {
                for row in (0..dimension).rev() {
                    if first_col >= 0
                        && Self::is_data_module(func, row, first_col as usize, dimension)
                    {
                        bits.push(matrix.get(first_col as usize, row));
                    }
                    if second_col >= 0
                        && Self::is_data_module(func, row, second_col as usize, dimension)
                    {
                        bits.push(matrix.get(second_col as usize, row));
                    }
                }
            } else {
                for row in 0..dimension {
                    if first_col >= 0
                        && Self::is_data_module(func, row, first_col as usize, dimension)
                    {
                        bits.push(matrix.get(first_col as usize, row));
                    }
                    if second_col >= 0
                        && Self::is_data_module(func, row, second_col as usize, dimension)
                    {
                        bits.push(matrix.get(second_col as usize, row));
                    }
                }
            }

            upward = !upward;
            col -= 2;
        }

        bits
    }

    /// Check if a module is a data module (not a function pattern)
    fn is_data_module(func: &FunctionMask, row: usize, col: usize, _dimension: usize) -> bool {
        !func.is_function(col, row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitstream_extraction() {
        let matrix = BitMatrix::new(21, 21);
        let func = FunctionMask::new(1);
        let bits = BitstreamExtractor::extract(&matrix, 21, &func);
        // Should extract some bits (exact count depends on version and function patterns)
        assert!(!bits.is_empty());
    }
}
