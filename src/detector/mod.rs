//! QR code detection modules
//!
//! This module contains all the logic for detecting QR codes in images:
//! - Finder pattern detection (the three square markers)
//! - Alignment pattern detection (for larger QR codes)
//! - Timing pattern reading (to establish the grid)
//! - Perspective transform (to correct for skew/rotation)

/// Alignment pattern detection for QR versions 2+
pub mod alignment;
/// Finder pattern detection using 1:1:3:1:1 ratio scanning
pub mod finder;
/// Timing pattern reading between finder patterns
pub mod timing;
/// Sample grid extraction and perspective correction
pub mod transform;
