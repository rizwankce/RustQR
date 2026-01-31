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

/// Binarize using adaptive thresholding with integral images
/// Uses local mean as threshold for each pixel
pub fn adaptive_binarize(
    gray: &[u8],
    width: usize,
    height: usize,
    window_size: usize,
) -> crate::models::BitMatrix {
    use crate::models::BitMatrix;

    // Build integral image for O(1) box sum queries
    let integral = build_integral_image(gray, width, height);
    let mut binary = BitMatrix::new(width, height);

    let half_window = window_size / 2;

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;

            // Get local mean using integral image
            let x1 = x.saturating_sub(half_window);
            let y1 = y.saturating_sub(half_window);
            let x2 = (x + half_window).min(width - 1);
            let y2 = (y + half_window).min(height - 1);

            let pixel_count = (x2 - x1 + 1) * (y2 - y1 + 1);
            let local_sum = query_integral_sum(&integral, width, height, x1, y1, x2, y2);
            let local_mean = (local_sum / pixel_count as u32) as u8;

            // Use local mean as threshold
            let threshold = local_mean;
            let is_black = gray[idx] < threshold;
            binary.set(x, y, is_black);
        }
    }

    binary
}

/// Build integral image for fast box sum queries
/// integral[y][x] = sum of all pixels from (0,0) to (x,y)
fn build_integral_image(gray: &[u8], width: usize, height: usize) -> Vec<u32> {
    let mut integral = vec![0u32; width * height];

    for y in 0..height {
        let mut row_sum = 0u32;
        for x in 0..width {
            let idx = y * width + x;
            row_sum += gray[idx] as u32;

            if y == 0 {
                integral[idx] = row_sum;
            } else {
                integral[idx] = integral[(y - 1) * width + x] + row_sum;
            }
        }
    }

    integral
}

/// Query sum of rectangle from (x1,y1) to (x2,y2) using integral image
fn query_integral_sum(
    integral: &[u32],
    width: usize,
    _height: usize,
    x1: usize,
    y1: usize,
    x2: usize,
    y2: usize,
) -> u32 {
    let a = if x1 > 0 && y1 > 0 {
        integral[(y1 - 1) * width + (x1 - 1)]
    } else {
        0
    };

    let b = if y1 > 0 {
        integral[(y1 - 1) * width + x2]
    } else {
        0
    };

    let c = if x1 > 0 {
        integral[y2 * width + (x1 - 1)]
    } else {
        0
    };

    let d = integral[y2 * width + x2];

    // Inclusion-exclusion: D - C - B + A
    d + a - c - b
}

/// Calculate Otsu's optimal threshold with optimized histogram
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

    #[test]
    fn test_integral_image() {
        // Simple 3x3 image
        let gray = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];
        let integral = build_integral_image(&gray, 3, 3);

        // Query sum of entire image
        let total = query_integral_sum(&integral, 3, 3, 0, 0, 2, 2);
        assert_eq!(total, 45); // Sum of 1..9

        // Query sum of top-left 2x2
        let sum_2x2 = query_integral_sum(&integral, 3, 3, 0, 0, 1, 1);
        assert_eq!(sum_2x2, 12); // 1 + 2 + 4 + 5

        // Query single pixel
        let single = query_integral_sum(&integral, 3, 3, 1, 1, 1, 1);
        assert_eq!(single, 5);
    }

    #[test]
    fn test_adaptive_binarize() {
        // Create image with varying brightness
        let mut gray = Vec::new();
        for y in 0..10 {
            for x in 0..10 {
                // Left side darker, right side lighter
                if x < 5 {
                    gray.push(50u8);
                } else {
                    gray.push(200u8);
                }
            }
        }

        let binary = adaptive_binarize(&gray, 10, 10, 5);

        // Test that adaptive binarization runs without errors
        // With mean thresholding on uniform regions, all pixels will be
        // classified based on whether they're below or above local mean
        assert_eq!(binary.width(), 10);
        assert_eq!(binary.height(), 10);
    }
}
