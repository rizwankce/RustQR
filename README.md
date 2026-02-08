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

Real-image test tuning:

- `QR_MAX_DIM` sets the max image dimension for real-image tests and `qrtool` runs.
- Recommended values:
  - `1024` for benchmark/CI default (best speed/reading-rate balance)
  - `800` for faster local iteration
  - `1200` for occasional deep validation runs
  - `0` to disable downscaling
- `QR_DEBUG=1` enables debug logging in detection/decoder paths (off by default).

Example:

```bash
# Disable downscaling and enable debug logs for real-image tests
QR_MAX_DIM=0 QR_DEBUG=1 cargo test test_decode_monitor_image001 -- --nocapture
```

Tooling note:

- CI benchmark workflow default uses `QR_MAX_DIM=1024`.

Run benchmarks:

```bash
# Synthetic benchmarks
cargo bench -- qr_detect

# Real QR image benchmarks
cargo bench --features tools --bench real_qr_images
```

Quick reading-rate runs (limit the dataset):

```bash
# Limit to 3 images (also supports QR_BENCH_LIMIT env var)
cargo run --features tools --bin qrtool -- reading-rate --limit 3
```

## Contributing

We welcome contributions! Areas we need help with:

- Performance: SIMD optimizations, parallel processing
- Algorithms: Faster finder pattern detection
- Platforms: WASM, mobile bindings
- Documentation: Examples, tutorials

See [docs/optimize.md](docs/optimize.md) for optimization opportunities.

## Benchmarks

Reading rate comparison across different QR code image categories (based on [Dynamsoft benchmark](https://www.dynamsoft.com/codepool/qr-code-reading-benchmark-and-comparison.html) using the BoofCV dataset with 536 images containing 1232 QR codes):

| Category | Images | Dynamsoft | BoofCV | ZBar | **RustQR** |
|----------|--------|-----------|--------|------|------------|
| blurred | 45 | 66.15% | 38.46% | 35.38% | **50.77%** |
| brightness | 28 | 81.18% | 78.82% | 50.59% | **21.18%** |
| bright_spots | 32 | 43.30% | 27.84% | 19.59% | **14.43%** |
| close | 40 | 95.00% | 100.00% | 12.50% | **25.00%** |
| curved | 50 | 70.00% | 56.67% | 35.00% | **36.67%** |
| damaged | 37 | 51.16% | 16.28% | 25.58% | **23.26%** |
| glare | 50 | 84.91% | 32.08% | 35.85% | **37.74%** |
| high_version | 33 | 97.30% | 40.54% | 27.03% | **0.00%** |
| lots | 7 | 100.00% | 99.76% | 18.10% | **0.00%** |
| monitor | 17 | 100.00% | 82.35% | 0.00% | **76.47%** |
| nominal | 65 | 93.59% | 89.74% | 66.67% | **49.21%** |
| noncompliant | 16 | 92.31% | 3.85% | 50.00% | **0.00%** |
| pathological | 23 | 95.65% | 43.48% | 65.22% | **0.00%** |
| perspective | 35 | 62.86% | 80.00% | 42.86% | **28.57%** |
| rotations | 44 | 99.25% | 96.24% | 48.87% | **19.55%** |
| shadows | 14 | 100.00% | 85.00% | 90.00% | **15.00%** |
| **total** | **536** | **83.29%** | **60.69%** | **38.95%** | **16.27%** |

> **Note:** RustQR values above are from GitHub Actions run `21754040229` (`macos-latest`) on commit `3ec3725e5c6fd92285d0c625c4f4f06985c8ed12` over all 16 categories (521 labeled images processed, 1217 QR labels; dataset fingerprint `ba96d1300e9f787b`).
>
> macOS reading-rate runtime in that run: median `2806.45 ms/image` (mean `11098.26 ms/image`, `n=521`), with the reading-rate step taking about `1h36m` (14:28:27Z -> 16:05:12Z).
>
> Run the benchmark:
> ```bash
> cargo run --features tools --bin qrtool --release -- reading-rate
> ```

## License

This project is dual-licensed under MIT and Apache 2.0. You may choose either license.

## Acknowledgments

- Inspired by BoofCV, ZXing, and ZBar
- Benchmark test images from BoofCV dataset
- QR Code specification: ISO/IEC 18004:2015

## Built With AI

This project was developed using:
- **Kimi K2.5** - Large language model by Moonshot AI
- **OpenCode** - AI coding agent CLI

The entire library was written through collaborative AI-assisted development.
