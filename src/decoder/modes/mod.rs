//! QR code data mode decoders
//!
//! This module contains decoders for different QR data modes:
//! - Numeric: Efficient encoding for digits (0-9)
//! - Alphanumeric: Letters, numbers, and symbols
//! - Byte: 8-bit data (UTF-8, binary, etc.)

pub mod alphanumeric;
pub mod byte;
pub mod numeric;
