#![no_std]
//! Alloc-only, deterministic QR Model 2 matrix decoding.
//!
//! This crate deliberately exposes only a strict canonical matrix path and a
//! known-erasure variant. Image geometry, deadline/cancellation, confidence
//! beam repair, soft BCH guesses, and non-canonical traversal recovery remain
//! in the hosted `rust_qr` crate.

#[allow(unused_imports)]
#[macro_use]
extern crate alloc;

mod matrix;
pub use matrix::BitMatrix;

pub mod decoder {
    pub mod bch;
    pub mod bitstream;
    pub mod format;
    pub mod function_mask;
    pub mod modes;
    pub mod orientation;
    pub mod payload;
    pub mod reed_solomon;
    pub mod tables;
    pub mod unmask;
    pub mod version;
}

use alloc::{string::String, vec, vec::Vec};
use decoder::{format::FormatInfo, payload};

/// Error correction level encoded by QR format information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ECLevel {
    L = 0,
    M = 1,
    Q = 2,
    H = 3,
}
impl ECLevel {
    pub const fn from_bits(bits: u8) -> Option<Self> {
        match bits & 3 {
            0 => Some(Self::L),
            1 => Some(Self::M),
            2 => Some(Self::Q),
            3 => Some(Self::H),
            _ => None,
        }
    }
}

/// QR data mask pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskPattern {
    Pattern0 = 0,
    Pattern1 = 1,
    Pattern2 = 2,
    Pattern3 = 3,
    Pattern4 = 4,
    Pattern5 = 5,
    Pattern6 = 6,
    Pattern7 = 7,
}
impl MaskPattern {
    pub const fn from_bits(bits: u8) -> Option<Self> {
        match bits & 7 {
            0 => Some(Self::Pattern0),
            1 => Some(Self::Pattern1),
            2 => Some(Self::Pattern2),
            3 => Some(Self::Pattern3),
            4 => Some(Self::Pattern4),
            5 => Some(Self::Pattern5),
            6 => Some(Self::Pattern6),
            7 => Some(Self::Pattern7),
            _ => None,
        }
    }
    pub const fn is_masked(self, i: usize, j: usize) -> bool {
        match self {
            Self::Pattern0 => (i + j) % 2 == 0,
            Self::Pattern1 => i % 2 == 0,
            Self::Pattern2 => j % 3 == 0,
            Self::Pattern3 => (i + j) % 3 == 0,
            Self::Pattern4 => (i / 2 + j / 3) % 2 == 0,
            Self::Pattern5 => ((i * j) % 2 + (i * j) % 3) == 0,
            Self::Pattern6 => (((i * j) % 2) + ((i * j) % 3)) % 2 == 0,
            Self::Pattern7 => ((i + j) % 2 + (i * j) % 3) % 2 == 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fnc1Position {
    First,
    Second { application_indicator: u8 },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuredAppendInfo {
    pub index: u8,
    pub total_symbols: u8,
    pub parity: u8,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QRCodeMetadata {
    pub eci_assignment: Option<u32>,
    pub fnc1: Option<Fnc1Position>,
    pub structured_append: Option<StructuredAppendInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatrixDecodeResult {
    pub data: Vec<u8>,
    pub content: String,
    pub version: u8,
    pub error_correction: ECLevel,
    pub mask_pattern: MaskPattern,
    pub metadata: QRCodeMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatrixDecodeError {
    InvalidDimensions,
    VersionDimensionMismatch,
    InvalidConfidenceLength,
    ErasureModuleOutOfBounds,
    DecodeFailed,
}

#[derive(Debug, Clone, Copy)]
pub enum MatrixErasureEvidence<'a> {
    ModuleConfidence(&'a [u8]),
    ErasedModules(&'a [(usize, usize)]),
}

/// Allocation-free cap for known-erasure Reed-Solomon repair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatrixRecoveryBudget {
    erasure_attempts_remaining: usize,
}
impl MatrixRecoveryBudget {
    pub const fn new(limit: usize) -> Self {
        Self {
            erasure_attempts_remaining: limit,
        }
    }
    pub const fn erasure_attempts_remaining(&self) -> usize {
        self.erasure_attempts_remaining
    }
    pub fn try_consume_erasure_attempt(&mut self) -> bool {
        if self.erasure_attempts_remaining == 0 {
            false
        } else {
            self.erasure_attempts_remaining -= 1;
            true
        }
    }
}

#[derive(Default)]
pub(crate) struct DecodeCounters {
    pub nonzero_remainder_bit_rejections: usize,
    pub rs_candidate_attempts: usize,
    pub rs_block_attempts: usize,
    pub rs_block_successes: usize,
    pub rs_block_failures: usize,
    pub rs_erasure_attempts: usize,
    pub rs_erasure_successes: usize,
    pub rs_erasure_count_hist: [usize; 4],
    pub unsupported_payloads: usize,
}
pub(crate) struct DecodeRequestContext {
    budget: MatrixRecoveryBudget,
    counters: DecodeCounters,
}
impl DecodeRequestContext {
    pub(crate) const fn new(limit: usize) -> Self {
        Self {
            budget: MatrixRecoveryBudget::new(limit),
            counters: DecodeCounters {
                nonzero_remainder_bit_rejections: 0,
                rs_candidate_attempts: 0,
                rs_block_attempts: 0,
                rs_block_successes: 0,
                rs_block_failures: 0,
                rs_erasure_attempts: 0,
                rs_erasure_successes: 0,
                rs_erasure_count_hist: [0; 4],
                unsupported_payloads: 0,
            },
        }
    }
    pub(crate) fn try_consume_erasure_attempt(&mut self) -> bool {
        self.budget.try_consume_erasure_attempt()
    }
    #[cfg(test)]
    pub(crate) fn counters(&self) -> &DecodeCounters {
        &self.counters
    }
    pub(crate) fn counters_mut(&mut self) -> &mut DecodeCounters {
        &mut self.counters
    }
}
impl Default for DecodeRequestContext {
    fn default() -> Self {
        Self::new(8)
    }
}

fn validate_input(matrix: &BitMatrix, version: u8) -> Result<(), MatrixDecodeError> {
    if matrix.width() != matrix.height() || version == 0 || version > 40 || matrix.width() < 21 {
        return Err(MatrixDecodeError::InvalidDimensions);
    }
    if matrix.width() != 17 + 4 * version as usize {
        return Err(MatrixDecodeError::VersionDimensionMismatch);
    }
    Ok(())
}

/// Strict deterministic decode: one canonical orientation, format, and data traversal.
pub fn decode_strict(
    matrix: &BitMatrix,
    version: u8,
) -> Result<MatrixDecodeResult, MatrixDecodeError> {
    validate_input(matrix, version)?;
    let format = FormatInfo::extract(matrix).ok_or(MatrixDecodeError::DecodeFailed)?;
    if version >= 7 && decoder::version::VersionInfo::extract(matrix) != Some(version) {
        return Err(MatrixDecodeError::DecodeFailed);
    }
    if !decoder::orientation::validate_structural_patterns(matrix, 0) {
        return Err(MatrixDecodeError::DecodeFailed);
    }
    payload::try_decode_single(
        matrix,
        version,
        &format,
        true,
        false,
        true,
        false,
        None,
        &mut DecodeRequestContext::default(),
    )
    .ok_or(MatrixDecodeError::DecodeFailed)
}

/// Strict deterministic decode with caller-known erased modules.
pub fn decode_with_erasures(
    matrix: &BitMatrix,
    version: u8,
    evidence: MatrixErasureEvidence<'_>,
) -> Result<MatrixDecodeResult, MatrixDecodeError> {
    validate_input(matrix, version)?;
    let dimension = matrix.width();
    let confidence = match evidence {
        MatrixErasureEvidence::ModuleConfidence(values) => {
            if values.len() != dimension * dimension {
                return Err(MatrixDecodeError::InvalidConfidenceLength);
            }
            values.to_vec()
        }
        MatrixErasureEvidence::ErasedModules(modules) => {
            let mut values = vec![u8::MAX; dimension * dimension];
            for &(x, y) in modules {
                if x >= dimension || y >= dimension {
                    return Err(MatrixDecodeError::ErasureModuleOutOfBounds);
                }
                values[y * dimension + x] = 0;
            }
            values
        }
    };
    let format = FormatInfo::extract(matrix).ok_or(MatrixDecodeError::DecodeFailed)?;
    if version >= 7 && decoder::version::VersionInfo::extract(matrix) != Some(version) {
        return Err(MatrixDecodeError::DecodeFailed);
    }
    payload::try_decode_single_deterministic_erasures(matrix, version, &format, &confidence)
        .ok_or(MatrixDecodeError::DecodeFailed)
}
