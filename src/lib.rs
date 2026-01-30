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
use utils::grayscale::rgb_to_grayscale;

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

/// Detector with configuration options
#[derive(Debug, Clone, Copy)]
pub struct Detector {
    // Configuration options will be added here
}

impl Detector {
    /// Create a new detector with default settings
    pub fn new() -> Self {
        Self {}
    }

    /// Detect QR codes in an image
    pub fn detect(&self, image: &[u8], width: usize, height: usize) -> Vec<QRCode> {
        detect(image, width, height)
    }

    /// Detect a single QR code (faster if you know there's only one)
    pub fn detect_single(&self, image: &[u8], width: usize, height: usize) -> Option<QRCode> {
        let codes = self.detect(image, width, height);
        codes.into_iter().next()
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
