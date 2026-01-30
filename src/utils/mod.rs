//! Utility functions for image processing
//!
//! This module provides helper functions for QR code detection:
//! - Grayscale conversion (RGB/RGBA to luminance)
//! - Binarization (Otsu's method and threshold-based)
//! - Geometry (perspective transforms, distance calculations)

pub mod binarization;
pub mod geometry;
pub mod grayscale;
