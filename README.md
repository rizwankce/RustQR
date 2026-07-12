# RustQR

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)](https://github.com/rizwankce/RustQR/actions)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](LICENSE)

RustQR is an experimental QR Code Model 2 detector and decoder written in
Rust. It prioritizes measurable reading-rate and latency improvements, but it
is not yet a complete ISO/IEC 18004 implementation or a production-ready
replacement for established scanners.

## Capabilities

Status is based on the current implementation and automated tests. “Partial”
means that a code path exists but coverage or end-to-end validation is
incomplete.

| Capability | Status | Evidence / limitation |
|---|---|---|
| Model 2 | Partial | Unit tests cover known v1 matrices and modes; ignored real-image regressions exercise selected images in `tests/decode_regression_tests.rs` |
| Versions 1-40 | Partial | Tables and parsing cover 1-40; high-version decoding has an ignored, non-strict regression test and remains weak in the published dataset |
| EC levels L/M/Q/H | Partial | Format parsing and block tables support all four; there is no end-to-end fixture for every level/version pair |
| Masks 0-7 | Partial | `MaskPattern` implements all masks, but unit and matrix tests do not exhaust all eight end to end |
| Numeric, alphanumeric, byte | Tested at unit level | Payload tests cover each mode and mixed-mode decoding |
| ECI | Partial | Assignment numbers are parsed but ignored; byte payloads are rendered as UTF-8 lossily |
| Kanji | Partial | Shift-JIS code units are reconstructed, but text is rendered lossily and lacks a dedicated test |
| Inverted symbols | Partial | The decoder retries an inverted sampled matrix; no dedicated end-to-end regression fixture |
| Rotated / mirrored symbols | Partial | Orientation retries include rotations and reflections; a rotated real-image test is ignored by default |
| Multiple symbols per image | Partial | The pipeline can return multiple results, but the ignored regression only requires at least one result |
| Model 1 | Unsupported | `Version::Model1` is a data-model placeholder; the detector/decoder implements Model 2 geometry |
| Micro QR | Unsupported | `Version::Micro` is a data-model placeholder; single-finder Micro QR detection is not implemented |
| GS1 / FNC1 | Unsupported | FNC1 mode indicators are not parsed |
| Structured Append | Unsupported | Structured Append mode is not parsed |
| Linux, macOS, Windows | Tested in CI | `.github/workflows/ci.yml` runs library tests on native hosted runners; this is not mobile support |
| WASM, iOS, Android | Planned | No build or test lane currently verifies these targets |
| `no_std` | Unsupported | The crate still exposes a `std`-based public API; feature separation is not an alloc-only core |

The implementation uses small, private `unsafe` SIMD kernels on x86_64 and
AArch64. Safe public image entry points validate dimensions, format, stride,
and buffer length before those kernels are reached.

## Installation

```toml
[dependencies]
rust_qr = { git = "https://github.com/rizwankce/RustQR" }
```

## Features and compatibility

RustQR's default features preserve the normal desktop implementation:

- `parallel` enables Rayon-backed scan and grayscale helpers. Disabling it
  keeps the same public helpers and uses scalar, deterministic fallbacks.
- `simd` enables private x86_64/AArch64 grayscale kernels. It can be disabled
  for a scalar build; it is not a claim of support for every CPU target.
- `image-loading` adds the optional `image` dependency used by the CLI loader.
- `tools` enables `qrtool` and includes `image-loading`.

The minimal supported library configuration is checked in CI with
`cargo test --lib --no-default-features`. It is still a `std` crate: a separate
`no_std + alloc` matrix-decoding crate has not yet been extracted. The declared
minimum supported Rust version is 1.85 and has its own CI lane.

## Usage

New code should use the checked API:

```rust
use rust_qr::{try_detect, ImageInput, PixelFormat};

let pixels: Vec<u8> = load_image();
let input = ImageInput::new(&pixels, 640, 480, PixelFormat::Rgb);
let qr_codes = try_detect(input)?;
```

`detect(&pixels, width, height)` remains as a compatibility wrapper for packed
RGB input. Invalid input returns an empty result through that legacy API;
`try_detect` returns a structured `InputError`.

## Testing

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Slow real-image regressions are ignored by default. Run a relevant test
explicitly when changing detection behavior, for example:

```bash
QR_MAX_DIM=800 cargo test --test decode_regression_tests test_decode_rotated \
  --release -- --ignored --nocapture
```

## Benchmarks

Criterion microbenchmarks measure individual components or a fixed benchmark
input. They do not establish successful end-to-end decode latency: a fast
attempt that returns no correct payload is not a successful scan.

The reading-rate tool exercises the image pipeline and reports correctness and
runtime over a dataset. Run a small local sample with:

```bash
QR_MAX_DIM=800 cargo run --features tools --bin qrtool --release -- \
  reading-rate --category rotations --limit 3
```

`--limit` takes precedence over `QR_BENCH_LIMIT`. With neither set, the full
dataset is used. GitHub's Fast Benchmark defaults to 25 images per category;
Full Benchmark defaults to all 536 images with `QR_MAX_DIM=1024` on Linux,
macOS, and Windows.

The table formerly published here is a historical result from run
`21837108650`, commit `f26d7e8`, on `macos-latest` with
`QR_MAX_DIM=1024` and dataset fingerprint `ba96d1300e9f787b`. Its BoofCV score
used expected symbol counts rather than one-to-one localization or payload
validation, and its timing included the then-current pipeline boundaries.
Consequently it must not be treated as a current accuracy baseline or as a
competitor-quality comparison. See `TODO.md` WP-002 for the evaluator work
required before publishing new comparative claims.

Performance results are publishable only when they identify the dataset,
commit, platform, preprocessing, evaluator schema, and timing boundary.
Component microbenchmarks must be labeled separately from successful
end-to-end decode latency.

## Documentation

- `docs/spec.md` — current capability matrix and roadmap
- `docs/optimize.md` — dated historical optimization snapshot
- `docs/decoder_status.md` — dated decoder repair snapshot
- `docs/reading_rate_improvement.md` — historical improvement worklog
- `TODO.md` — current execution queue and benchmark policy

## License

Dual-licensed under MIT or Apache-2.0.
