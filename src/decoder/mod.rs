//! QR code decoding modules
//!
//! This module contains all the logic for decoding QR codes after detection:
//! - Error correction (Reed-Solomon, BCH)
//! - Format and version information extraction
//! - Data mode decoding (numeric, alphanumeric, byte, kanji)
//! - Bitstream extraction and unmasking

/// BCH error correction for format and version info
pub mod bch;
/// Bitstream extraction from QR matrix
pub mod bitstream;
pub mod config;
/// Format information extraction (mask pattern, EC level)
pub mod format;
/// Function module mask builder (finder/timing/format/alignment/version)
pub mod function_mask;
/// Data mode decoders (numeric, alphanumeric, byte)
pub mod modes;
/// Main QR decoder that orchestrates the decoding pipeline
pub mod qr_decoder;
/// Reed-Solomon error correction
pub mod reed_solomon;
/// QR specification tables (ECC codewords/blocks)
pub mod tables;
/// QR code unmasking (removes mask patterns)
pub mod unmask;
/// Version information extraction (versions 7-40)
pub mod version;
