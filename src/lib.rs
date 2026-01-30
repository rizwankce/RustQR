//! ScanRust - World's fastest QR code scanning library
//!
//! A pure Rust QR code detection and decoding library with zero dependencies.
//! Designed for maximum speed and cross-platform compatibility.

#![warn(missing_docs)]

pub mod decoder;
pub mod detector;
pub mod models;

pub use models::{BitMatrix, ECLevel, MaskPattern, Point, QRCode, Version};

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
    // TODO: Implement detection pipeline
    let _ = (image, width, height);
    Vec::new()
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
    // TODO: Implement detection pipeline
    let _ = (image, width, height);
    Vec::new()
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
