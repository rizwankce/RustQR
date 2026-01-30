/// Convert grayscale image to binary using Otsu's thresholding method
/// Returns a BitMatrix where true = black, false = white
pub fn otsu_binarize(gray: &[u8], width: usize, height: usize) -> crate::models::BitMatrix {
    use crate::models::BitMatrix;

    let threshold = calculate_otsu_threshold(gray);
    let mut binary = BitMatrix::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            let is_black = gray[idx] < threshold;
            binary.set(x, y, is_black);
        }
    }

    binary
}

/// Calculate Otsu's optimal threshold
fn calculate_otsu_threshold(gray: &[u8]) -> u8 {
    // Build histogram
    let mut histogram = [0u32; 256];
    for &pixel in gray {
        histogram[pixel as usize] += 1;
    }

    let total_pixels = gray.len() as f64;
    let mut max_variance = 0.0;
    let mut optimal_threshold = 128u8;

    for threshold in 0..=255 {
        let mut class1_pixels = 0u32;
        let mut class1_sum = 0u32;
        let mut class2_pixels = 0u32;
        let mut class2_sum = 0u32;

        for intensity in 0..=255 {
            let count = histogram[intensity as usize];
            if intensity < threshold {
                class1_pixels += count;
                class1_sum += count * intensity as u32;
            } else {
                class2_pixels += count;
                class2_sum += count * intensity as u32;
            }
        }

        if class1_pixels == 0 || class2_pixels == 0 {
            continue;
        }

        let class1_mean = class1_sum as f64 / class1_pixels as f64;
        let class2_mean = class2_sum as f64 / class2_pixels as f64;

        let weight1 = class1_pixels as f64 / total_pixels;
        let weight2 = class2_pixels as f64 / total_pixels;

        let variance = weight1 * weight2 * (class1_mean - class2_mean).powi(2);

        if variance > max_variance {
            max_variance = variance;
            optimal_threshold = threshold;
        }
    }

    optimal_threshold
}

/// Simple global threshold binarization
pub fn threshold_binarize(
    gray: &[u8],
    width: usize,
    height: usize,
    threshold: u8,
) -> crate::models::BitMatrix {
    use crate::models::BitMatrix;

    let mut binary = BitMatrix::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            let is_black = gray[idx] < threshold;
            binary.set(x, y, is_black);
        }
    }

    binary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threshold_binarize() {
        let gray = vec![100, 150, 200, 50]; // 2x2 image
        let binary = threshold_binarize(&gray, 2, 2, 128);

        // Pixels < 128 should be black (true)
        assert!(binary.get(0, 0)); // 100 < 128
        assert!(!binary.get(1, 0)); // 150 >= 128
        assert!(!binary.get(0, 1)); // 200 >= 128
        assert!(binary.get(1, 1)); // 50 < 128
    }

    #[test]
    fn test_otsu_binarize() {
        // Create a simple two-class image
        let mut gray = vec![50u8; 50]; // Dark class
        gray.extend(vec![200u8; 50]); // Light class

        let binary = otsu_binarize(&gray, 10, 10);

        // Otsu should separate around 125
        // Top half should be black (true), bottom half white (false)
        assert!(binary.get(0, 0)); // Dark
        assert!(!binary.get(0, 7)); // Light
    }
}
