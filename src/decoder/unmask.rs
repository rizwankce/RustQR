/// Unmask QR code by applying the mask pattern
use crate::models::{BitMatrix, MaskPattern};

/// Unmask QR code matrix by XORing with mask pattern
pub fn unmask(matrix: &mut BitMatrix, mask_pattern: &MaskPattern) {
    let width = matrix.width();
    let height = matrix.height();

    for y in 0..height {
        for x in 0..width {
            if mask_pattern.is_masked(x, y) {
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

        // Set some bits
        matrix.set(0, 0, true);
        matrix.set(1, 0, false);
        matrix.set(0, 1, true);

        // Apply mask pattern 0
        unmask(&mut matrix, &MaskPattern::Pattern0);

        // Check that masked positions were toggled
        // Pattern0: (i + j) % 2 == 0
        // Position (0,0): (0+0)%2=0, should be toggled (true -> false)
        assert!(!matrix.get(0, 0));
    }
}
