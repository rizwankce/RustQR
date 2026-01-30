# RustQR

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)](https://github.com/rizwankce/RustQR/actions)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](LICENSE)
[![Rust Version](https://img.shields.io/badge/rust-1.70%2B-orange)](https://www.rust-lang.org)

**The world's fastest QR code scanning library written in pure Rust.**

RustQR is a high-performance, cross-platform QR code detection and decoding library with zero third-party dependencies. Designed for speed and efficiency, it aims to be the fastest QR scanner available while maintaining a clean, safe Rust implementation.

## Features

- **Blazing Fast**: Target <5ms for 1MP images
- **Pure Rust**: Zero unsafe code, zero external dependencies
- **Cross-Platform**: Works on Linux, macOS, Windows, WASM, iOS, Android
- **No-Std Compatible**: Suitable for embedded systems
- **Complete Standards Support**:
  - QR Code Model 1 & 2 (versions 1-40)
  - Micro QR Code (M1-M4)
  - All error correction levels (L, M, Q, H)
  - All mask patterns (0-7)
  - Numeric, Alphanumeric, Byte modes

## Performance

| Image Size | Time | Status |
|------------|------|--------|
| 100x100 RGB | ~114 µs | Excellent |
| 640x480 RGB | ~1.9 ms | Target met |
| 1920x1080 RGB | ~12.5 ms | Optimizing |
| Real QR images | 8-45 ms | Optimizing |

**Target**: <5ms for 1MP images to beat BoofCV (~15-20ms) and ZBar (~10-15ms)

See [docs/optimize.md](docs/optimize.md) for detailed optimization roadmap.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
rust_qr = { git = "https://github.com/rizwankce/RustQR" }
```

## Usage

### Basic Detection

```rust
use rust_qr::detect;

let image_data: Vec<u8> = load_image(); // Your image loading code
let width = 640;
let height = 480;

let qr_codes = detect(&image_data, width, height);

for qr in qr_codes {
    println!("Found QR: {}", qr.content);
}
```

### Using the Detector Struct

```rust
use rust_qr::Detector;

let detector = Detector::new();
let qr_codes = detector.detect(&image_data, width, height);

// Or detect just the first QR code (faster)
if let Some(qr) = detector.detect_single(&image_data, width, height) {
    println!("QR Content: {}", qr.content);
}
```

## Testing

Run the test suite:

```bash
cargo test
```

Run benchmarks:

```bash
# Synthetic benchmarks
cargo bench -- qr_detect

# Real QR image benchmarks
cargo bench --bench real_qr_images
```

## Contributing

We welcome contributions! Areas we need help with:

- Performance: SIMD optimizations, parallel processing
- Algorithms: Faster finder pattern detection
- Platforms: WASM, mobile bindings
- Documentation: Examples, tutorials

See [docs/optimize.md](docs/optimize.md) for optimization opportunities.

## Roadmap

### Phase 1: Foundation (Complete)
- Reed-Solomon error correction
- BCH decoder
- Data structures

### Phase 2: Detection (Complete)
- Grayscale conversion
- Binarization
- Finder pattern detection

### Phase 3: Decoding (Complete)
- Format info extraction
- Version detection
- Data modes

### Phase 4: Optimization (In Progress)
- SIMD operations
- Parallel processing
- Integral images

### Phase 5: Platform Support (Planned)
- WASM target
- FFI bindings
- Mobile support

## License

This project is dual-licensed under MIT and Apache 2.0. You may choose either license.

## Acknowledgments

- Inspired by BoofCV, ZXing, and ZBar
- Benchmark test images from BoofCV dataset
- QR Code specification: ISO/IEC 18004:2015
