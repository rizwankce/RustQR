//! Utility functions for image processing
//!
//! This module provides helper functions for QR code detection:
//! - Grayscale conversion (RGB/RGBA to luminance)
//! - Binarization (Otsu's method and threshold-based)
//! - Geometry (perspective transforms, distance calculations)
//! - Memory pools (buffer reuse for performance)
//! - Fixed-point arithmetic (16.16 format for fast transforms)

pub mod binarization;
pub mod fixed_point;
pub mod geometry;
pub mod grayscale;
pub mod memory_pool;
