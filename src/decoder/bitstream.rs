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

    /// Extract data bits and per-module confidence bytes using the same traversal.
    ///
    /// `module_confidence` is row-major, one byte per module (0-255).
    pub fn extract_with_confidence(
        matrix: &BitMatrix,
        dimension: usize,
        func: &FunctionMask,
        start_upward: bool,
        swap_columns: bool,
        module_confidence: &[u8],
    ) -> (Vec<bool>, Vec<u8>) {
        if module_confidence.len() != dimension * dimension {
            return (
                Self::extract_with_options(matrix, dimension, func, start_upward, swap_columns),
                Vec::new(),
            );
        }

        let mut bits = Vec::new();
        let mut confidence = Vec::new();

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

            let mut push_cell = |row: usize, c: i32| {
                if c >= 0 && Self::is_data_module(func, row, c as usize, dimension) {
                    let cx = c as usize;
                    bits.push(matrix.get(cx, row));
                    confidence.push(module_confidence[row * dimension + cx]);
                }
            };

            if upward {
                for row in (0..dimension).rev() {
                    push_cell(row, first_col);
                    push_cell(row, second_col);
                }
            } else {
                for row in 0..dimension {
                    push_cell(row, first_col);
                    push_cell(row, second_col);
                }
            }

            upward = !upward;
            col -= 2;
        }

        (bits, confidence)
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
