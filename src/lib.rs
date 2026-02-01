//! RustQR - World's fastest QR code scanning library
//!
//! A pure Rust QR code detection and decoding library with zero dependencies.
//! Designed for maximum speed and cross-platform compatibility.

#![warn(missing_docs)]
#![allow(clippy::missing_docs_in_private_items)]

/// QR code decoding modules (error correction, format extraction, data modes)
pub mod decoder;
/// QR code detection modules (finder patterns, alignment, timing)
pub mod detector;
/// Core data structures (QRCode, BitMatrix, Point, etc.)
pub mod models;
/// Utility functions (grayscale, binarization, geometry)
pub mod utils;

pub use models::{BitMatrix, ECLevel, MaskPattern, Point, QRCode, Version};

use decoder::qr_decoder::QrDecoder;
use detector::finder::{FinderDetector, FinderPattern};
use utils::binarization::otsu_binarize;
use utils::grayscale::{rgb_to_grayscale, rgb_to_grayscale_with_buffer};
use utils::memory_pool::BufferPool;

/// Detect QR codes in an RGB image
///
/// # Arguments
/// * `image` - Raw RGB bytes (3 bytes per pixel)
/// * `width` - Image width in pixels
/// * `height` - Image height in pixels
///
/// # Returns
/// Vector of detected QR codes
///
/// Uses pyramid detection for large images (800px+) for better performance
pub fn detect(image: &[u8], width: usize, height: usize) -> Vec<QRCode> {
    // Step 1: Convert to grayscale
    let gray = rgb_to_grayscale(image, width, height);

    // Step 2: Binarize
    let binary = otsu_binarize(&gray, width, height);

    // Step 3: Detect finder patterns
    // Use pyramid detection for very large images (1600px+) for better performance
    let finder_patterns = if width >= 1600 && height >= 1600 {
        FinderDetector::detect_with_pyramid(&binary)
    } else {
        FinderDetector::detect(&binary)
    };

    // Step 4: Group finder patterns into potential QR codes and decode
    let mut results = Vec::new();

    // Group finder patterns by proximity and similar module size
    let groups = group_finder_patterns(&finder_patterns);

    #[cfg(debug_assertions)]
    eprintln!(
        "DEBUG: Found {} finder patterns, formed {} groups",
        finder_patterns.len(),
        groups.len()
    );

    // Try to decode each group of 3 patterns
    for (group_idx, group) in groups.iter().enumerate() {
        if group.len() >= 3 {
            #[cfg(debug_assertions)]
            eprintln!(
                "DEBUG: Trying group {} with patterns {:?}",
                group_idx, group
            );

            // Try the first 3 patterns in this group
            match QrDecoder::decode(
                &binary,
                &finder_patterns[group[0]].center,
                &finder_patterns[group[1]].center,
                &finder_patterns[group[2]].center,
            ) {
                Some(qr) => {
                    #[cfg(debug_assertions)]
                    eprintln!("DEBUG: Group {} decoded successfully!", group_idx);
                    results.push(qr);
                }
                None => {
                    #[cfg(debug_assertions)]
                    eprintln!("DEBUG: Group {} failed to decode", group_idx);
                }
            }
        }
    }

    results
}

/// Group finder patterns that likely belong to the same QR code
/// Patterns in the same group should have similar module sizes and form a valid triangle
fn group_finder_patterns(patterns: &[FinderPattern]) -> Vec<Vec<usize>> {
    let mut groups: Vec<Vec<usize>> = Vec::new();

    // Try all combinations of 3 patterns
    for i in 0..patterns.len() {
        for j in (i + 1)..patterns.len() {
            for k in (j + 1)..patterns.len() {
                let pi = &patterns[i];
                let pj = &patterns[j];
                let pk = &patterns[k];

                // Check module sizes are similar
                let size_ratio_ij = pi.module_size / pj.module_size;
                let size_ratio_ik = pi.module_size / pk.module_size;
                let size_ratio_jk = pj.module_size / pk.module_size;

                let sizes_ok = size_ratio_ij >= 0.7
                    && size_ratio_ij <= 1.4
                    && size_ratio_ik >= 0.7
                    && size_ratio_ik <= 1.4
                    && size_ratio_jk >= 0.7
                    && size_ratio_jk <= 1.4;

                if !sizes_ok {
                    continue;
                }

                // Check distances form a reasonable triangle
                let d_ij = pi.center.distance(&pj.center);
                let d_ik = pi.center.distance(&pk.center);
                let d_jk = pj.center.distance(&pk.center);

                // All distances should be roughly similar (within 2:1 ratio)
                let max_d = d_ij.max(d_ik).max(d_jk);
                let min_d = d_ij.min(d_ik).min(d_jk);

                if max_d / min_d >= 2.0 {
                    continue;
                }

                // Minimum distance should be at least ~7 modules (finder pattern size)
                let avg_module = (pi.module_size + pj.module_size + pk.module_size) / 3.0;
                if min_d < avg_module * 7.0 {
                    continue;
                }

                // Maximum distance check: QR codes can't be arbitrarily large
                // Version 40 QR code is 177x177 modules, at ~3px min module = ~531px max
                // Add some margin: reject if max distance > 600 pixels
                if max_d > 600.0 {
                    continue;
                }

                // Valid group found
                groups.push(vec![i, j, k]);
            }
        }
    }

    groups
}

/// Detect QR codes from a pre-computed grayscale image
///
/// # Arguments
/// * `image` - Grayscale bytes (1 byte per pixel)
/// * `width` - Image width in pixels
/// * `height` - Image height in pixels
///
/// # Returns
/// Vector of detected QR codes
pub fn detect_from_grayscale(image: &[u8], width: usize, height: usize) -> Vec<QRCode> {
    // Step 1: Binarize
    let binary = otsu_binarize(image, width, height);

    // Step 2: Detect finder patterns
    let finder_patterns = FinderDetector::detect(&binary);

    // Step 3: Group finder patterns and decode QR codes
    let mut results = Vec::new();

    let groups = group_finder_patterns(&finder_patterns);

    for group in groups {
        if group.len() >= 3 {
            if let Some(qr) = QrDecoder::decode(
                &binary,
                &finder_patterns[group[0]].center,
                &finder_patterns[group[1]].center,
                &finder_patterns[group[2]].center,
            ) {
                results.push(qr);
            }
        }
    }

    results
}

/// Detect QR codes using a reusable buffer pool (faster for batch processing)
///
/// This version uses pre-allocated buffers to avoid repeated memory allocations.
/// Use this when processing multiple images of similar size.
///
/// # Example
/// ```
/// use rust_qr::utils::memory_pool::BufferPool;
///
/// let mut pool = BufferPool::new();
/// let image = vec![0u8; 640 * 480 * 3]; // RGB image buffer
/// let codes = rust_qr::detect_with_pool(&image, 640, 480, &mut pool);
/// ```
pub fn detect_with_pool(
    image: &[u8],
    width: usize,
    height: usize,
    pool: &mut BufferPool,
) -> Vec<QRCode> {
    let pixel_count = width * height;

    // Step 1: Convert to grayscale using pre-allocated buffer
    let gray_buffer = pool.get_grayscale_buffer(pixel_count);
    rgb_to_grayscale_with_buffer(image, width, height, gray_buffer);

    // Step 2: Binarize (creates new BitMatrix - could also be pooled)
    let binary = otsu_binarize(gray_buffer, width, height);

    // Step 3: Detect finder patterns
    let finder_patterns = FinderDetector::detect(&binary);

    // Step 4: Group and decode
    let mut results = Vec::new();

    let groups = group_finder_patterns(&finder_patterns);

    for group in groups {
        if group.len() >= 3 {
            if let Some(qr) = QrDecoder::decode(
                &binary,
                &finder_patterns[group[0]].center,
                &finder_patterns[group[1]].center,
                &finder_patterns[group[2]].center,
            ) {
                results.push(qr);
            }
        }
    }

    results
}

/// Detector with configuration options and optional buffer pool
pub struct Detector {
    /// Optional buffer pool for memory reuse
    pool: Option<BufferPool>,
}

impl Detector {
    /// Create a new detector with default settings
    pub fn new() -> Self {
        Self { pool: None }
    }

    /// Create a detector with buffer pooling enabled
    pub fn with_pool() -> Self {
        Self {
            pool: Some(BufferPool::new()),
        }
    }

    /// Create a detector with a specific pool capacity
    pub fn with_pool_capacity(capacity: usize) -> Self {
        Self {
            pool: Some(BufferPool::with_capacity(capacity)),
        }
    }

    /// Detect QR codes in an image
    pub fn detect(&mut self, image: &[u8], width: usize, height: usize) -> Vec<QRCode> {
        match &mut self.pool {
            Some(pool) => detect_with_pool(image, width, height, pool),
            None => detect(image, width, height),
        }
    }

    /// Detect a single QR code (faster if you know there's only one)
    pub fn detect_single(&mut self, image: &[u8], width: usize, height: usize) -> Option<QRCode> {
        let codes = self.detect(image, width, height);
        codes.into_iter().next()
    }

    /// Clear the internal buffer pool (keeps capacity)
    pub fn clear_pool(&mut self) {
        if let Some(pool) = &mut self.pool {
            pool.clear();
        }
    }
}

impl Default for Detector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_empty() {
        // Test with empty image
        let image = vec![0u8; 300]; // 10x10 RGB
        let codes = detect(&image, 10, 10);
        assert!(codes.is_empty());
    }

    #[test]
    fn test_real_qr() {
        // Load a real QR code image and see how many finder patterns we detect
        let img_path = "/Users/rizwan/Downloads/qrcodes 2/detection/monitor/image001.jpg";
        let img = image::open(img_path).expect("Failed to load image");
        let rgb_img = img.to_rgb8();
        let (width, height) = (rgb_img.width() as usize, rgb_img.height() as usize);

        println!("Loaded image: {}x{} pixels", width, height);

        // Convert to flat RGB buffer
        let rgb_bytes: Vec<u8> = rgb_img.into_raw();

        // Convert to grayscale
        let gray = rgb_to_grayscale(&rgb_bytes, width, height);
        println!("Converted to grayscale: {} bytes", gray.len());

        // Binarize
        let binary = otsu_binarize(&gray, width, height);
        println!("Binarized: {}x{} matrix", binary.width(), binary.height());

        // Detect finder patterns
        let patterns = FinderDetector::detect(&binary);
        println!("Found {} finder patterns:", patterns.len());

        for (i, p) in patterns.iter().enumerate() {
            println!(
                "  Pattern {}: center=({:.1}, {:.1}), module_size={:.2}",
                i, p.center.x, p.center.y, p.module_size
            );
        }

        // Also try grouping to see how many valid groups we get
        let groups = group_finder_patterns(&patterns);
        println!("Formed {} valid groups of 3 patterns", groups.len());

        // Assert at least something to make the test fail visibly if we find nothing
        assert!(
            !patterns.is_empty(),
            "Expected to find at least 3 finder patterns, found {}",
            patterns.len()
        );
    }
}
