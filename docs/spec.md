# RustQR Specification

## Project Overview

**Name:** RustQR  
**Language:** Rust  
**Purpose:** World's fastest QR code scanning library, cross-platform, zero third-party dependencies

## Core Philosophy

- **Speed First**: Every millisecond matters
- **Zero Dependencies**: Pure Rust implementation (except where absolutely critical)
- **Complete Standards Compliance**: Support ALL QR code variants
- **Cross-Platform**: Native performance on every platform
- **Library-First**: Designed for integration, not a standalone tool

## QR Code Standards to Support

### 1. ISO/IEC 18004:2015 (Core Standard)
All versions (1-40) with all models:

#### QR Code Model 1 (Original)
- Legacy support for older systems
- Versions 1-14 only
- Error correction levels: L, M, Q, H

#### QR Code Model 2 (Current Standard)
- All versions 1-40
- Error correction levels: L (~7%), M (~15%), Q (~25%), H (~30%)
- Alignment patterns for larger codes
- Format and version information areas

### 2. Micro QR Code
- Versions M1, M2, M3, M4
- Minimal size for small data
- Single position detection pattern
- Limited error correction

### 3. iQR Code (ISO/IEC 18004 Extension)
- Rectangular format options
- Versions 1-61 (both square and rectangular)
- Higher density than standard QR
- All error correction levels

### 4. Frame QR (SQRC-compatible)
- Frame area for visual elements
- Inside-Out pattern support
- Custom frame configurations

### 5. FCR (Fast Code Reading) Mode
- Optimized detection patterns
- High-speed scanning optimizations
- Reduced complexity for speed

## Technical Architecture

### Core Modules

```
RustQR/
├── src/
│   ├── lib.rs              # Public API
│   ├── detector/           # QR code detection
│   │   ├── finder.rs       # Finder pattern detection
│   │   ├── alignment.rs    # Alignment pattern detection
│   │   ├── timing.rs       # Timing pattern analysis
│   │   └── transform.rs    # Perspective correction
│   ├── decoder/            # Data decoding
│   │   ├── bitstream.rs    # Bit extraction from grid
│   │   ├── format.rs       # Format info decoding
│   │   ├── version.rs      # Version info decoding
│   │   └── reed_solomon.rs # Error correction
│   ├── encoder/            # QR code generation (future)
│   ├── models/             # Data structures
│   │   ├── qr_code.rs      # QR code representation
│   │   ├── point.rs        # 2D point types
│   │   └── matrix.rs       # Bit matrix
│   ├── modes/              # Data modes
│   │   ├── numeric.rs      # Numeric mode (0-9)
│   │   ├── alphanumeric.rs # Alphanumeric mode (0-9, A-Z, space, $%*+-./:)
│   │   ├── byte.rs         # Byte mode (ISO 8859-1, UTF-8)
│   │   ├── kanji.rs        # Kanji mode (Shift JIS)
│   │   └── eci.rs          # ECI mode (extended charsets)
│   └── utils/
│       ├── binarization.rs # Adaptive thresholding
│       ├── geometry.rs     # Geometric calculations
│       └── simd.rs         # SIMD optimizations (optional)
├── benches/                # Criterion benchmarks
├── tests/                  # Test vectors from ISO spec
└── docs/                   # Documentation
```

### Detection Pipeline

1. **Preprocessing**
   - Grayscale conversion
   - Adaptive binarization (Otsu + local thresholding)
   - Noise reduction

2. **Finder Pattern Detection**
   - Ratio-based scanning (1:1:3:1:1)
   - Cross-check in both directions
   - Clustering and filtering
   - Perspective estimation

3. **Alignment Pattern Detection** (for v2+ or large codes)
   - Predicted positions from version
   - Fine-tuning for distortion

4. **Timing Pattern Reading**
   - Establish sampling grid
   - Handle damaged patterns

5. **Sample Grid Extraction**
   - Perspective transform
   - Sub-pixel sampling
   - Bit matrix generation

### Decoding Pipeline

1. **Format Information Extraction**
   - Mask pattern identification
   - Error correction level
   - BCH error checking

2. **Version Information Extraction** (v7+)
   - BCH(18,6) decoding
   - Error detection

3. **Unmasking**
   - Apply mask pattern
   - Handle all 8 mask patterns

4. **Bitstream Extraction**
   - Zigzag reading pattern
   - Handle all function patterns

5. **Error Correction**
   - Reed-Solomon decoding
   - Up to error correction capacity
   - Erasure correction support

6. **Data Decoding**
   - Mode detection and switching
   - Character count indicator reading
   - Data deinterleaving
   - Charset conversion

## Benchmarking Strategy

### Goal: Become the Fastest QR Scanner

We will benchmark RustQR against the best libraries in the industry, following methodologies from:
- [Dynamsoft QR Code Benchmark](https://www.dynamsoft.com/codepool/qr-code-reading-benchmark-and-comparison.html)
- [BoofCV Performance Tests](https://boofcv.org/index.php?title=Performance:QrCode)
- [barcode-reading-benchmark repo](https://github.com/tony-xlh/barcode-reading-benchmark)

### Competitors to Beat

| Library | Language | Target Time to Beat |
|---------|----------|---------------------|
| BoofCV | Java | ~15-20ms per image |
| Dynamsoft | C++ | Commercial baseline |
| ZXing | Java | ~30-50ms per image |
| ZBar | C | ~10-15ms per image |

### Benchmark Scenarios

1. **Perfect conditions**: Clean QR codes, good lighting
2. **Damaged codes**: Missing modules, blur, rotation
3. **Multiple codes**: Detect 10+ codes in single image
4. **Various sizes**: From v1 to v40
5. **Real-world images**: Photos with perspective distortion

### Target Performance

- Single QR detection: < 5ms
- Batch processing: > 200 images/second
- Memory usage: < 10MB peak
- Zero-copy pipeline where possible

## API Design

### Simple API
```rust
pub fn detect(image: &[u8], width: usize, height: usize) -> Vec<QRCode>;
pub fn detect_from_grayscale(image: &[u8], width: usize, height: usize) -> Vec<QRCode>;
```

### Advanced API
```rust
pub struct Detector;
impl Detector {
    pub fn new() -> Self;
    pub fn detect(&self, image: Image) -> Vec<QRCode>;
}

pub struct QRCode {
    pub data: Vec<u8>,
    pub content: String,
    pub version: Version,
    pub error_correction: ECLevel,
    pub mask_pattern: MaskPattern,
}
```

## Development Phases

### Phase 1: Foundation (Weeks 1-4)
- Project structure and CI/CD
- Data structures (Point, Matrix, QRCode)
- Reed-Solomon implementation
- BCH decoder

### Phase 2: Detection (Weeks 5-8)
- Grayscale conversion
- Binarization
- Finder pattern detection
- Alignment pattern detection

### Phase 3: Decoding (Weeks 9-12)
- Format/version info
- Unmasking
- Bitstream extraction
- Error correction
- Data modes (numeric, alphanumeric, byte, kanji)

### Phase 4: Optimization (Weeks 13-16)
- SIMD operations
- Memory optimizations
- Benchmark comparison
- FFI bindings for other languages

## Testing Requirements

- ISO/IEC 18004:2015 test vectors
- [zxing-test-images](https://github.com/zxing/zxing/tree/master/core/src/test/resources) compatibility
- Custom test suite for edge cases
- Fuzzing tests for robustness
- Benchmark regression tests in CI
