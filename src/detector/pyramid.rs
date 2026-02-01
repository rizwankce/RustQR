/// Image pyramid for multi-scale finder pattern detection
///
/// Creates downscaled versions of the image to enable fast coarse detection
/// followed by refinement at full resolution near candidate locations.
use crate::models::BitMatrix;

/// An image pyramid with multiple scales
pub struct ImagePyramid {
    /// Original (full resolution) image
    pub level0: BitMatrix,
    /// Half resolution (0.5x)
    pub level1: Option<BitMatrix>,
    /// Quarter resolution (0.25x)
    pub level2: Option<BitMatrix>,
    /// Original dimensions
    pub original_width: usize,
    pub original_height: usize,
}

impl ImagePyramid {
    /// Create a pyramid from a binary image
    /// For small images (< 400px), only creates level0
    /// For medium images (400-800px), creates level0 and level1
    /// For large images (> 800px), creates all 3 levels
    pub fn new(matrix: BitMatrix) -> Self {
        let width = matrix.width();
        let height = matrix.height();

        // Only create downscaled versions for larger images
        let level1 = if width >= 400 && height >= 400 {
            Some(Self::downscale_by_2(&matrix))
        } else {
            None
        };

        let level2 = if width >= 800 && height >= 800 {
            level1.as_ref().map(|l1| Self::downscale_by_2(l1))
        } else {
            None
        };

        Self {
            original_width: width,
            original_height: height,
            level0: matrix,
            level1,
            level2,
        }
    }

    /// Downscale a binary image by 2x using majority voting
    /// Each 2x2 block becomes 1 pixel (black if 2+ pixels are black)
    fn downscale_by_2(matrix: &BitMatrix) -> BitMatrix {
        let src_width = matrix.width();
        let src_height = matrix.height();
        let dst_width = src_width / 2;
        let dst_height = src_height / 2;

        let mut result = BitMatrix::new(dst_width, dst_height);

        for y in 0..dst_height {
            for x in 0..dst_width {
                // Sample 2x2 block
                let src_y = y * 2;
                let src_x = x * 2;

                let mut black_count = 0;
                if matrix.get(src_x, src_y) {
                    black_count += 1;
                }
                if matrix.get(src_x + 1, src_y) {
                    black_count += 1;
                }
                if matrix.get(src_x, src_y + 1) {
                    black_count += 1;
                }
                if matrix.get(src_x + 1, src_y + 1) {
                    black_count += 1;
                }

                // Majority vote: black if 2+ pixels are black
                if black_count >= 2 {
                    result.set(x, y, true);
                }
            }
        }

        result
    }

    /// Get the coarsest level that should be used for detection
    /// Returns the level and scale factor
    pub fn coarsest_detection_level(&self) -> (&BitMatrix, f32) {
        // Start with the most downscaled version for large images
        if let Some(ref level2) = self.level2 {
            (level2, 4.0)
        } else if let Some(ref level1) = self.level1 {
            (level1, 2.0)
        } else {
            (&self.level0, 1.0)
        }
    }

    /// Get the full resolution image
    pub fn full_resolution(&self) -> &BitMatrix {
        &self.level0
    }

    /// Map coordinates from a downscaled level back to original resolution
    pub fn map_to_original(&self, x: f32, y: f32, scale: f32) -> (f32, f32) {
        (x * scale, y * scale)
    }

    /// Get bounding box in original coordinates for a point in downscaled image
    /// Returns (min_x, min_y, max_x, max_y) in original pixels
    pub fn get_search_window(
        &self,
        x: usize,
        y: usize,
        scale: f32,
        window_size: usize,
    ) -> (usize, usize, usize, usize) {
        let orig_x = (x as f32 * scale) as usize;
        let orig_y = (y as f32 * scale) as usize;
        let window = (window_size as f32 * scale) as usize;

        let min_x = orig_x.saturating_sub(window);
        let min_y = orig_y.saturating_sub(window);
        let max_x = (orig_x + window).min(self.original_width - 1);
        let max_y = (orig_y + window).min(self.original_height - 1);

        (min_x, min_y, max_x, max_y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downscale_by_2() {
        // Create a 400x400 image with a black square in the center
        let mut original = BitMatrix::new(400, 400);
        // Set a 100x100 black square in center
        for y in 150..250 {
            for x in 150..250 {
                original.set(x, y, true);
            }
        }

        let pyramid = ImagePyramid::new(original);

        // Should have level1 (200x200)
        assert!(pyramid.level1.is_some());
        let level1 = pyramid.level1.as_ref().unwrap();
        assert_eq!(level1.width(), 200);
        assert_eq!(level1.height(), 200);

        // Center pixels should be black (the 100x100 block downscaled to 50x50)
        assert!(level1.get(100, 100));
    }

    #[test]
    fn test_pyramid_levels() {
        // Small image - only level0
        let small = BitMatrix::new(100, 100);
        let pyramid_small = ImagePyramid::new(small);
        assert!(pyramid_small.level1.is_none());
        assert!(pyramid_small.level2.is_none());

        // Medium image - level0 and level1
        let medium = BitMatrix::new(500, 500);
        let pyramid_medium = ImagePyramid::new(medium);
        assert!(pyramid_medium.level1.is_some());
        assert!(pyramid_medium.level2.is_none());

        // Large image - all levels
        let large = BitMatrix::new(1000, 1000);
        let pyramid_large = ImagePyramid::new(large);
        assert!(pyramid_large.level1.is_some());
        assert!(pyramid_large.level2.is_some());
    }

    #[test]
    fn test_coordinate_mapping() {
        let matrix = BitMatrix::new(800, 800);
        let pyramid = ImagePyramid::new(matrix);

        // Test mapping from level2 (4x downscale) back to original
        let (x, y) = pyramid.map_to_original(10.0, 20.0, 4.0);
        assert_eq!(x, 40.0);
        assert_eq!(y, 80.0);
    }
}
