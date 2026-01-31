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
use detector::finder::FinderDetector;
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
pub fn detect(image: &[u8], width: usize, height: usize) -> Vec<QRCode> {
    // Step 1: Convert to grayscale
    let gray = rgb_to_grayscale(image, width, height);

    // Step 2: Binarize
    let binary = otsu_binarize(&gray, width, height);

    // Step 3: Detect finder patterns
    let finder_patterns = FinderDetector::detect(&binary);

    // Step 4: Group finder patterns into potential QR codes and decode
    let mut results = Vec::new();

    // Need at least 3 finder patterns for a valid QR code
    if finder_patterns.len() >= 3 {
        // Try to find valid QR code combinations
        // For simplicity, try first 3 patterns
        // TODO: Better grouping logic
        if let Some(qr) = QrDecoder::decode(
            &binary,
            &finder_patterns[0].center,
            &finder_patterns[1].center,
            &finder_patterns[2].center,
        ) {
            results.push(qr);
        }
    }

    results
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

    // Step 3: Decode QR codes
    let mut results = Vec::new();

    if finder_patterns.len() >= 3 {
        if let Some(qr) = QrDecoder::decode(
            &binary,
            &finder_patterns[0].center,
            &finder_patterns[1].center,
            &finder_patterns[2].center,
        ) {
            results.push(qr);
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

    // Step 4: Decode
    let mut results = Vec::new();

    if finder_patterns.len() >= 3 {
        if let Some(qr) = QrDecoder::decode(
            &binary,
            &finder_patterns[0].center,
            &finder_patterns[1].center,
            &finder_patterns[2].center,
        ) {
            results.push(qr);
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
}
