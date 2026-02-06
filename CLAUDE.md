# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

RustQR is a high-performance QR code detection and decoding library written in pure Rust. The goal is to be the world's fastest QR scanner while maintaining a clean, safe implementation with zero unsafe code and zero external dependencies.

**Performance Targets:**
- <5ms detection for 1MP images (✅ achieved: ~4.2ms parallel)
- Faster than BoofCV (~15-20ms) and ZBar (~10-15ms)
- Current reading rate: ~18.7% (target: 60%+)

## Build & Test Commands

### Building
```bash
cargo build                              # Debug build
cargo build --release                    # Release build
cargo build --features tools             # Include CLI tools
```

### Testing
```bash
cargo test                               # Run all tests
cargo test --lib --release               # Run library tests (matches CI)
cargo test test_decode_monitor_image001 -- --nocapture  # Run specific test

# Test tuning environment variables:
QR_MAX_DIM=0 QR_DEBUG=1 cargo test test_decode_monitor_image001 -- --nocapture
# - QR_MAX_DIM: Max image dimension (default: 1200, set to 0 to disable downscaling)
# - QR_DEBUG=1: Enable debug logging in detection/decoder paths
```

### Benchmarking
```bash
# Synthetic benchmarks
cargo bench -- qr_detect

# Real QR image benchmarks
cargo bench --features tools --bench real_qr_images

# Reading rate benchmark (full dataset)
cargo run --features tools --bin qrtool --release -- reading-rate

# Quick reading-rate runs (limit dataset)
cargo run --features tools --bin qrtool --release -- reading-rate --limit 3
# Also supports QR_BENCH_LIMIT env var
```

### Code Quality
```bash
cargo fmt -- --check                     # Check formatting (CI-gated)
cargo clippy --all-targets --all-features  # Lint checks
```

## Architecture Overview

### Module Structure

```
src/
├── lib.rs                    # Public API (detect, Detector) and pipeline orchestration
├── detector/                 # Detection pipeline (finder, alignment, timing, transform, pyramid)
│   ├── finder.rs            # Finder pattern detection (1:1:3:1:1 ratio scanning)
│   ├── alignment.rs         # Alignment pattern detection
│   ├── timing.rs            # Timing pattern validation (exists but currently unused)
│   ├── transform.rs         # Perspective transform computation
│   ├── pyramid.rs           # Multi-scale detection for large images
│   └── connected_components.rs  # O(k) algorithm (slower on uniform images, limited use)
├── decoder/                  # Decoding pipeline (format, version, Reed-Solomon, modes)
│   ├── qr_decoder.rs        # Main decode orchestration, grid sampling, orientation handling
│   ├── format.rs            # Format info extraction with BCH(15,5) error correction
│   ├── version.rs           # Version info extraction with BCH(18,6) correction
│   ├── reed_solomon.rs      # Reed-Solomon error correction
│   ├── bitstream.rs         # Bit extraction and deinterleaving
│   ├── unmask.rs            # Data unmasking (patterns 0-7)
│   ├── function_mask.rs     # Function pattern masks (finder, alignment, timing, format, version)
│   ├── modes/               # Data mode decoders (numeric, alphanumeric, byte)
│   └── tables.rs            # Lookup tables for EC blocks, polynomial math
├── models/                   # Core data structures
│   ├── qr_code.rs           # QRCode struct (content, version, EC level, position)
│   ├── matrix.rs            # BitMatrix for binary image representation
│   └── point.rs             # Point2D for geometry
├── utils/                    # Low-level image/math helpers
│   ├── grayscale.rs         # RGB→Gray conversion (SIMD-optimized, 146x speedup)
│   ├── binarization.rs      # Otsu, adaptive, threshold methods
│   ├── geometry.rs          # Distance, angle calculations
│   ├── fixed_point.rs       # Fixed-point math (foundation for future DLT optimization)
│   └── memory_pool.rs       # Buffer reuse for batch processing
├── bin/qrtool.rs            # CLI tool for reading-rate benchmarks (feature-gated)
└── tools/                    # Benchmark helpers (feature-gated)
```

### Detection Pipeline Flow

1. **Grayscale Conversion** (`utils/grayscale.rs`)
   - SIMD-optimized RGB→Gray (146x faster than naive)
   - Parallel version available: `rgb_to_grayscale_parallel()` (1.75x-3.28x speedup)

2. **Binarization** (`utils/binarization.rs`)
   - Large images (≥800px): adaptive_binarize (window=31)
   - Small images (<800px): otsu_binarize (global threshold)
   - Fallback: try alternate method if <3 finder patterns found
   - **Known issues**: No Sauvola bias, fixed window size, only 2 strategies

3. **Finder Pattern Detection** (`detector/finder.rs`)
   - Horizontal row scanning for 1:1:3:1:1 ratio
   - Cross-check in vertical/horizontal directions
   - Merge candidates within 50px distance
   - Pyramid detection for very large images (≥1600px)
   - **Known issues**: No vertical column scanning, no diagonal scanning (causes rotations=0%)

4. **Pattern Grouping** (`lib.rs`)
   - Bin patterns by module size (1.25x ratio bins)
   - Build groups of 3 with geometric constraints
   - Right angle check: cosine < 0.3
   - **Known issues**: Size ratio 1.5x limit too tight for perspective images

5. **Transform & Grid Sampling** (`decoder/qr_decoder.rs`)
   - Compute perspective transform from 3 finder patterns
   - Estimate bottom-right corner (parallelogram + 9 offset candidates)
   - Sample grid: 3x3 neighborhood majority vote or median threshold
   - **Known issues**: Parallelogram assumption fails on perspective/curved images

6. **Decoding** (`decoder/qr_decoder.rs`)
   - Pass 1: Try extracted format info × 4 traversal variants × 8 orientations
   - Pass 2: Brute-force all 32 EC/mask combos (if Pass 1 fails)
   - Each attempt: unmask → extract bits → deinterleave → RS decode → parse payload
   - **Known issues**: Brute force extremely slow (~2048+ RS attempts), timing patterns not validated

## Key Design Patterns

### Performance Optimizations
- **Early termination**: Skip rows without significant edges (2-3 transitions)
- **Pyramid detection**: Multi-scale detection for large images (12-16% faster)
- **Parallel processing**: Parallel grayscale conversion and finder detection (1.75x-3.28x speedup)
- **Memory pools**: Buffer reuse via `BufferPool` (ready for batch processing)
- **SIMD**: Grayscale conversion uses SIMD where available

### Telemetry System
The `DetectionTelemetry` struct tracks pipeline stage success/failure:
- `binarize_ok`, `finder_patterns_found`, `groups_found`, `transforms_built`
- `format_extracted`, `rs_decode_ok`, `payload_decoded`, `qr_codes_found`
- Used by reading-rate benchmarks to diagnose failure modes

### Environment Variables
- `QR_MAX_DIM`: Max image dimension for downscaling (default: 1200, 0=disabled)
- `QR_DEBUG=1`: Enable debug logging in detection/decoder
- `QR_BENCH_LIMIT`: Limit reading-rate dataset size (default: full dataset)

## Current Reading Rate Issues

See `docs/reading_rate_improvement.md` for detailed analysis. Key problems:

### Critical Issues (0% reading rate categories)
1. **rotations (0/44)**: No vertical/diagonal scanning for non-axis-aligned rotations
2. **brightness (0%)**: No Sauvola binarization (adapts to local contrast vs just brightness)
3. **high_version (0%)**: Alignment pattern detection broken for v7+ (uses wrong positions)
4. **bright_spots, noncompliant, lots (0%)**: Various binarization and multi-QR issues

### Performance Bottlenecks
1. **Brute-force decoding**: Up to 2048+ RS decode attempts per version (tries all 40 versions)
2. **Unused timing pattern validation**: `detector/timing.rs` exists but never called
3. **Global median threshold**: Grid sampling uses global median instead of local adaptive

### Near-term Improvement Priorities (from reading_rate_improvement.md Phase 1)
1. Add Sauvola binarization (unlocks brightness, shadows, glare)
2. Add vertical column scanning (partially unlocks rotations)
3. Remove `module_size < 2.0` hard rejection (improves nominal, close)
4. Eliminate brute-force format/mask search (32x decode speedup)
5. Limit version candidate search (8x decode speedup)

## Benchmark Datasets

**BoofCV dataset**: 536 images across 16 categories, 1232 total QR codes
- Located in `benches/images/` (gitignored, downloaded separately)
- Categories: blurred, brightness, bright_spots, close, curved, damaged, glare, high_version, lots, monitor, nominal, noncompliant, pathological, perspective, rotations, shadows
- Each image has a `.txt` label file with expected QR content
- Benchmark compares decoded content against labels (as of Phase 0 completion)

## Testing Guidelines

- Integration tests in `tests/decode_regression_tests.rs`
- Name tests by scenario: `test_decode_<category>_<image>`
- Deterministic assertions on decoded content/metadata when possible
- Real-image tests tunable with `QR_MAX_DIM` and `QR_DEBUG` env vars
- Run targeted tests during iteration, then `cargo test` before commit

## Commit Style

Recent history uses concise, imperative subjects with optional prefixes:
- `feat:` - new features
- `fix:` - bug fixes
- `refactor:` - code restructuring
- `test:` - test updates
- `docs:` - documentation
- `chore:` - tooling/config

Keep commits scoped (one logical change each). Include benchmark/test updates when behavior changes.

## Current Active Work

**Phase 0 (Completed 2026-02-06)**: Measurement correctness & benchmark hygiene
- Fixed reading-rate scoring to compare decoded content against labels
- Added per-stage telemetry (DetectionTelemetry)
- Standardized benchmark commands and reporting

**Next Phase (Phase 1)**: Critical fixes to raise reading rate from ~18.7% to ~35%
- Priority: Sauvola binarization, vertical scanning, decode speedup

## Important Context

- **Rust Edition**: 2024
- **Formatting**: `rustfmt` defaults (4-space indentation)
- **Naming**: `snake_case` for functions/modules, `PascalCase` for structs/enums, `SCREAMING_SNAKE_CASE` for constants
- **No unsafe code**: Pure safe Rust only
- **Zero external dependencies**: Only uses `rayon` (parallelism), `image` (I/O), `clap` (CLI, feature-gated)
- **Cross-platform**: Targets Linux, macOS, Windows, WASM, iOS, Android (no-std compatible goal)
