/// Compact bit matrix for storing binary data
#[derive(Debug, Clone)]
pub struct BitMatrix {
    width: usize,
    height: usize,
    data: Vec<u8>,
}

impl BitMatrix {
    /// Create a new bit matrix with given dimensions
    pub fn new(width: usize, height: usize) -> Self {
        let bytes_needed = (width * height + 7) / 8;
        Self {
            width,
            height,
            data: vec![0; bytes_needed],
        }
    }

    /// Get matrix width
    pub fn width(&self) -> usize {
        self.width
    }

    /// Get matrix height
    pub fn height(&self) -> usize {
        self.height
    }

    /// Get bit at (x, y)
    pub fn get(&self, x: usize, y: usize) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let index = y * self.width + x;
        let byte_index = index / 8;
        let bit_index = index % 8;
        (self.data[byte_index] >> bit_index) & 1 == 1
    }

    /// Set bit at (x, y)
    pub fn set(&mut self, x: usize, y: usize, value: bool) {
        if x >= self.width || y >= self.height {
            return;
        }
        let index = y * self.width + x;
        let byte_index = index / 8;
        let bit_index = index % 8;
        if value {
            self.data[byte_index] |= 1 << bit_index;
        } else {
            self.data[byte_index] &= !(1 << bit_index);
        }
    }

    /// Toggle bit at (x, y)
    pub fn toggle(&mut self, x: usize, y: usize) {
        if x >= self.width || y >= self.height {
            return;
        }
        let index = y * self.width + x;
        let byte_index = index / 8;
        let bit_index = index % 8;
        self.data[byte_index] ^= 1 << bit_index;
    }

    /// Clear all bits to 0
    pub fn clear(&mut self) {
        self.data.fill(0);
    }

    /// Get raw data as bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}

impl Default for BitMatrix {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bit_matrix() {
        let mut matrix = BitMatrix::new(8, 8);
        assert_eq!(matrix.width(), 8);
        assert_eq!(matrix.height(), 8);

        matrix.set(3, 4, true);
        assert!(matrix.get(3, 4));
        assert!(!matrix.get(3, 3));

        matrix.toggle(3, 4);
        assert!(!matrix.get(3, 4));

        matrix.clear();
        assert!(!matrix.get(3, 4));
    }

    #[test]
    fn test_out_of_bounds() {
        let mut matrix = BitMatrix::new(8, 8);
        matrix.set(10, 10, true); // Should not panic
        assert!(!matrix.get(10, 10));
    }
}
