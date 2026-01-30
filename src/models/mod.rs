//! Core data structures for QR code processing
//!
//! This module defines the main types used throughout the library:
//! - BitMatrix: Compact storage for binary QR data
//! - Point: 2D coordinates for geometry calculations
//! - QRCode: Result type containing decoded data
//! - Version, ECLevel, MaskPattern: QR code metadata

pub mod matrix;
pub mod point;
pub mod qr_code;

pub use matrix::BitMatrix;
pub use point::Point;
pub use qr_code::{ECLevel, MaskPattern, QRCode, Version};
