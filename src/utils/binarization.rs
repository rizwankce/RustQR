/// Convert grayscale image to binary using Otsu's thresholding method
/// Returns a BitMatrix where true = black, false = white
pub fn otsu_binarize(gray: &[u8], width: usize, height: usize) -> crate::models::BitMatrix {
    use crate::models::BitMatrix;

    let mut binary = BitMatrix::new(width, height);
    otsu_binarize_into(gray, width, height, &mut binary);
    binary
}

/// Otsu binarization writing into an existing BitMatrix (avoids allocation)
pub fn otsu_binarize_into(
    gray: &[u8],
    width: usize,
    height: usize,
    output: &mut crate::models::BitMatrix,
) {
    output.reset(width, height);
    let threshold = calculate_otsu_threshold(gray);

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            let is_black = gray[idx] < threshold;
            output.set(x, y, is_black);
        }
    }
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

    let mut binary = BitMatrix::new(width, height);
    let integral = build_integral_image(gray, width, height);
    adaptive_binarize_core(gray, width, height, window_size, &mut binary, &integral);
    binary
}

/// Adaptive binarization writing into existing buffers (avoids allocation)
pub fn adaptive_binarize_into(
    gray: &[u8],
    width: usize,
    height: usize,
    window_size: usize,
    output: &mut crate::models::BitMatrix,
    integral: &mut Vec<u32>,
) {
    output.reset(width, height);
    build_integral_image_into(gray, width, height, integral);
    adaptive_binarize_core(gray, width, height, window_size, output, integral);
}

/// Core adaptive binarization logic shared by allocating and _into variants
fn adaptive_binarize_core(
    gray: &[u8],
    width: usize,
    height: usize,
    window_size: usize,
    binary: &mut crate::models::BitMatrix,
    integral: &[u32],
) {
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
            let local_sum = query_integral_sum(integral, width, height, x1, y1, x2, y2);
            let local_mean = (local_sum / pixel_count as u32) as u8;

            // Use local mean as threshold
            let threshold = local_mean;
            let is_black = gray[idx] < threshold;
            binary.set(x, y, is_black);
        }
    }
}

/// Build integral image for fast box sum queries
/// integral[y][x] = sum of all pixels from (0,0) to (x,y)
fn build_integral_image(gray: &[u8], width: usize, height: usize) -> Vec<u32> {
    let mut integral = vec![0u32; width * height];
    build_integral_image_into(gray, width, height, &mut integral);
    integral
}

/// Build integral image into an existing buffer (avoids allocation)
fn build_integral_image_into(gray: &[u8], width: usize, height: usize, integral: &mut Vec<u32>) {
    let len = width * height;
    integral.resize(len, 0);
    integral.fill(0);

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

/// Binarize using Sauvola's method which adapts to local contrast.
/// threshold = mean * (1 + k * (std_dev / R - 1))
/// This handles uneven illumination better than simple adaptive thresholding.
pub fn sauvola_binarize(
    gray: &[u8],
    width: usize,
    height: usize,
    window_size: usize,
    k: f32,
) -> crate::models::BitMatrix {
    use crate::models::BitMatrix;

    let mut binary = BitMatrix::new(width, height);
    let integral = build_integral_image(gray, width, height);
    let integral_sq = build_integral_sq_image(gray, width, height);
    sauvola_binarize_core(
        gray,
        width,
        height,
        window_size,
        k,
        &mut binary,
        &integral,
        &integral_sq,
    );
    binary
}

/// Sauvola binarization writing into existing buffers (avoids allocation)
#[allow(clippy::too_many_arguments)]
pub fn sauvola_binarize_into(
    gray: &[u8],
    width: usize,
    height: usize,
    window_size: usize,
    k: f32,
    output: &mut crate::models::BitMatrix,
    integral: &mut Vec<u32>,
    integral_sq: &mut Vec<u64>,
) {
    output.reset(width, height);
    build_integral_image_into(gray, width, height, integral);
    build_integral_sq_image_into(gray, width, height, integral_sq);
    sauvola_binarize_core(
        gray,
        width,
        height,
        window_size,
        k,
        output,
        integral,
        integral_sq,
    );
}

/// Core Sauvola binarization logic
#[allow(clippy::too_many_arguments)]
fn sauvola_binarize_core(
    gray: &[u8],
    width: usize,
    height: usize,
    window_size: usize,
    k: f32,
    binary: &mut crate::models::BitMatrix,
    integral: &[u32],
    integral_sq: &[u64],
) {
    let half_window = window_size / 2;
    const R: f64 = 128.0;

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;

            let x1 = x.saturating_sub(half_window);
            let y1 = y.saturating_sub(half_window);
            let x2 = (x + half_window).min(width - 1);
            let y2 = (y + half_window).min(height - 1);

            let pixel_count = (x2 - x1 + 1) * (y2 - y1 + 1);
            let local_sum = query_integral_sum(integral, width, height, x1, y1, x2, y2);
            let local_sq_sum = query_integral_sq_sum(integral_sq, width, x1, y1, x2, y2);

            let mean = local_sum as f64 / pixel_count as f64;
            let mean_sq = local_sq_sum as f64 / pixel_count as f64;
            let variance = (mean_sq - mean * mean).max(0.0);
            let std_dev = variance.sqrt();

            let threshold = mean * (1.0 + k as f64 * (std_dev / R - 1.0));
            let is_black = (gray[idx] as f64) < threshold;
            binary.set(x, y, is_black);
        }
    }
}

/// Build integral image of squared pixel values for variance computation
fn build_integral_sq_image(gray: &[u8], width: usize, height: usize) -> Vec<u64> {
    let mut integral_sq = vec![0u64; width * height];
    build_integral_sq_image_into(gray, width, height, &mut integral_sq);
    integral_sq
}

/// Build integral image of squared pixel values into an existing buffer
fn build_integral_sq_image_into(
    gray: &[u8],
    width: usize,
    height: usize,
    integral_sq: &mut Vec<u64>,
) {
    let len = width * height;
    integral_sq.resize(len, 0);
    integral_sq.fill(0);

    for y in 0..height {
        let mut row_sum = 0u64;
        for x in 0..width {
            let idx = y * width + x;
            let val = gray[idx] as u64;
            row_sum += val * val;

            if y == 0 {
                integral_sq[idx] = row_sum;
            } else {
                integral_sq[idx] = integral_sq[(y - 1) * width + x] + row_sum;
            }
        }
    }
}

/// Query sum of squared values in rectangle from (x1,y1) to (x2,y2)
fn query_integral_sq_sum(
    integral_sq: &[u64],
    width: usize,
    x1: usize,
    y1: usize,
    x2: usize,
    y2: usize,
) -> u64 {
    let a = if x1 > 0 && y1 > 0 {
        integral_sq[(y1 - 1) * width + (x1 - 1)]
    } else {
        0
    };

    let b = if y1 > 0 {
        integral_sq[(y1 - 1) * width + x2]
    } else {
        0
    };

    let c = if x1 > 0 {
        integral_sq[y2 * width + (x1 - 1)]
    } else {
        0
    };

    let d = integral_sq[y2 * width + x2];

    d + a - c - b
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
        for _y in 0..10 {
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

    #[test]
    fn test_integral_sq_image() {
        let gray = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];
        let integral_sq = build_integral_sq_image(&gray, 3, 3);

        // Query sum of squared values for entire image
        // 1+4+9+16+25+36+49+64+81 = 285
        let total = query_integral_sq_sum(&integral_sq, 3, 0, 0, 2, 2);
        assert_eq!(total, 285);

        // Query single pixel (5^2 = 25)
        let single = query_integral_sq_sum(&integral_sq, 3, 1, 1, 1, 1);
        assert_eq!(single, 25);
    }

    #[test]
    fn test_sauvola_binarize() {
        // Create image with varying brightness
        let mut gray = Vec::new();
        for _y in 0..20 {
            for x in 0..20 {
                if x < 10 {
                    gray.push(50u8);
                } else {
                    gray.push(200u8);
                }
            }
        }

        let binary = sauvola_binarize(&gray, 20, 20, 5, 0.2);

        assert_eq!(binary.width(), 20);
        assert_eq!(binary.height(), 20);
    }
}
