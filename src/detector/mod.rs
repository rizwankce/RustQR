//! QR code detection modules
//!
//! This module contains all the logic for detecting QR codes in images:
//! - Finder pattern detection (the three square markers)
//! - Alignment pattern detection (for larger QR codes)
//! - Timing pattern reading (to establish the grid)
//! - Perspective transform (to correct for skew/rotation)
//! - Image pyramid for multi-scale detection (Phase 2 optimization)
//! - Connected components for O(k) pattern detection (Phase 2 optimization)

/// Alignment pattern detection for QR versions 2+
pub mod alignment;
/// Connected components labeling for efficient pattern detection
pub mod connected_components;
/// Contour/square-region proposals as a secondary detector family
pub mod contour;
/// Finder pattern detection using 1:1:3:1:1 ratio scanning
pub mod finder;
/// Image pyramid for multi-scale finder detection
pub mod pyramid;
/// Timing pattern reading between finder patterns
pub mod timing;
/// Sample grid extraction and perspective correction
pub mod transform;
