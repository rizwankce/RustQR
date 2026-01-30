use super::{BitMatrix, Point};

/// QR Code version (1-40 for Model 2)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    /// Model 1 QR code (versions 1-14)
    Model1(u8),
    /// Model 2 QR code (versions 1-40)
    Model2(u8),
    /// Micro QR code (versions M1-M4)
    Micro(u8),
}

impl Version {
    /// Get the version number (1-40 for Model 2)
    pub fn number(&self) -> u8 {
        match self {
            Version::Model1(v) => *v,
            Version::Model2(v) => *v,
            Version::Micro(v) => *v,
        }
    }

    /// Get the size in modules (width = height)
    pub fn size(&self) -> usize {
        match self {
            Version::Model1(v) => 4 * (*v as usize) + 17,
            Version::Model2(v) => 4 * (*v as usize) + 17,
            Version::Micro(v) => match v {
                1 => 11,
                2 => 13,
                3 => 15,
                4 => 17,
                _ => 0,
            },
        }
    }

    /// Check if this is a Micro QR code
    pub fn is_micro(&self) -> bool {
        matches!(self, Version::Micro(_))
    }
}

/// Error correction level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ECLevel {
    /// Low (~7% recovery capacity)
    L = 0,
    /// Medium (~15% recovery capacity)
    M = 1,
    /// Quartile (~25% recovery capacity)
    Q = 2,
    /// High (~30% recovery capacity)
    H = 3,
}

impl ECLevel {
    /// Get error correction level from bits (00=L, 01=M, 10=Q, 11=H)
    pub fn from_bits(bits: u8) -> Option<Self> {
        match bits & 0x03 {
            0 => Some(ECLevel::L),
            1 => Some(ECLevel::M),
            2 => Some(ECLevel::Q),
            3 => Some(ECLevel::H),
            _ => None,
        }
    }

    /// Get total error correction codewords for version and level
    pub fn ec_codewords(&self, version: &Version) -> usize {
        // Simplified - actual values are from ISO spec tables
        let base = match version {
            Version::Model2(v) => match *v {
                1 => 7,
                2 => 10,
                3 => 15,
                _ => 20,
            },
            _ => 7,
        };

        match self {
            ECLevel::L => base,
            ECLevel::M => base * 2,
            ECLevel::Q => base * 3,
            ECLevel::H => base * 4,
        }
    }
}

/// Mask pattern (0-7)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskPattern {
    /// (i + j) % 2 == 0
    Pattern0 = 0,
    /// i % 2 == 0
    Pattern1 = 1,
    /// j % 3 == 0
    Pattern2 = 2,
    /// (i + j) % 3 == 0
    Pattern3 = 3,
    /// (i/2 + j/3) % 2 == 0
    Pattern4 = 4,
    /// (i*j)%2 + (i*j)%3 == 0
    Pattern5 = 5,
    /// ((i*j)%2 + (i*j)%3) % 2 == 0
    Pattern6 = 6,
    /// ((i+j)%2 + (i*j)%3) % 2 == 0
    Pattern7 = 7,
}

impl MaskPattern {
    /// Get mask pattern from bits
    pub fn from_bits(bits: u8) -> Option<Self> {
        match bits & 0x07 {
            0 => Some(MaskPattern::Pattern0),
            1 => Some(MaskPattern::Pattern1),
            2 => Some(MaskPattern::Pattern2),
            3 => Some(MaskPattern::Pattern3),
            4 => Some(MaskPattern::Pattern4),
            5 => Some(MaskPattern::Pattern5),
            6 => Some(MaskPattern::Pattern6),
            7 => Some(MaskPattern::Pattern7),
            _ => None,
        }
    }

    /// Check if module at (i, j) should be masked
    pub fn is_masked(&self, i: usize, j: usize) -> bool {
        match self {
            MaskPattern::Pattern0 => (i + j) % 2 == 0,
            MaskPattern::Pattern1 => i % 2 == 0,
            MaskPattern::Pattern2 => j % 3 == 0,
            MaskPattern::Pattern3 => (i + j) % 3 == 0,
            MaskPattern::Pattern4 => (i / 2 + j / 3) % 2 == 0,
            MaskPattern::Pattern5 => ((i * j) % 2 + (i * j) % 3) == 0,
            MaskPattern::Pattern6 => (((i * j) % 2) + ((i * j) % 3)) % 2 == 0,
            MaskPattern::Pattern7 => (((i + j) % 2) + ((i * j) % 3)) % 2 == 0,
        }
    }
}

/// Detected QR code
#[derive(Debug, Clone)]
pub struct QRCode {
    /// Raw decoded bytes
    pub data: Vec<u8>,
    /// Decoded content as UTF-8 string
    pub content: String,
    /// QR code version
    pub version: Version,
    /// Error correction level
    pub error_correction: ECLevel,
    /// Mask pattern used
    pub mask_pattern: MaskPattern,
    /// Corner points in image coordinates
    pub position: [Point; 4],
    /// Module matrix (true = black, false = white)
    pub modules: BitMatrix,
    /// Detection confidence (0.0 - 1.0)
    pub confidence: f32,
}

impl QRCode {
    /// Create a new QR code with decoded data
    pub fn new(
        data: Vec<u8>,
        content: String,
        version: Version,
        error_correction: ECLevel,
        mask_pattern: MaskPattern,
    ) -> Self {
        Self {
            data,
            content,
            version,
            error_correction,
            mask_pattern,
            position: [Point::default(); 4],
            modules: BitMatrix::new(0, 0),
            confidence: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_size() {
        assert_eq!(Version::Model2(1).size(), 21);
        assert_eq!(Version::Model2(2).size(), 25);
        assert_eq!(Version::Model2(40).size(), 177);
    }

    #[test]
    fn test_ec_level() {
        assert_eq!(ECLevel::from_bits(0b00), Some(ECLevel::L));
        assert_eq!(ECLevel::from_bits(0b01), Some(ECLevel::M));
        assert_eq!(ECLevel::from_bits(0b10), Some(ECLevel::Q));
        assert_eq!(ECLevel::from_bits(0b11), Some(ECLevel::H));
    }

    #[test]
    fn test_mask_pattern() {
        let mask = MaskPattern::Pattern0;
        assert!(mask.is_masked(0, 0));
        assert!(!mask.is_masked(0, 1));
        assert!(mask.is_masked(1, 1));
    }
}
