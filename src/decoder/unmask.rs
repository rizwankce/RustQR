/// Unmask QR code by applying the mask pattern
use crate::decoder::function_mask::FunctionMask;
use crate::models::{BitMatrix, MaskPattern};

/// Unmask QR code matrix by XORing with mask pattern
pub fn unmask(matrix: &mut BitMatrix, mask_pattern: &MaskPattern, func: &FunctionMask) {
    let width = matrix.width();
    let height = matrix.height();

    for y in 0..height {
        for x in 0..width {
            if !func.is_function(x, y) && mask_pattern.is_masked(y, x) {
                // XOR the bit (toggle)
                matrix.toggle(x, y);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unmask() {
        let mut matrix = BitMatrix::new(21, 21);

        // Set some bits (use a data module location)
        matrix.set(10, 10, true);
        matrix.set(11, 10, false);
        matrix.set(10, 11, true);

        // Apply mask pattern 0
        let func = FunctionMask::new(1);
        unmask(&mut matrix, &MaskPattern::Pattern0, &func);

        // Check that masked positions were toggled
        // Pattern0: (i + j) % 2 == 0
        // Position (10,10): (10+10)%2=0, should be toggled (true -> false)
        assert!(!matrix.get(10, 10));
    }
}
