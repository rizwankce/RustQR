# RustQR Capability Status and Roadmap

This document describes the current implementation. It is not a claim of full
ISO/IEC 18004 compliance. Status is derived from source paths and tests in this
repository as of 2026-07-11.

## Status definitions

- **Tested**: an automated test makes a meaningful assertion for the feature.
- **Partial**: implementation exists, but coverage or end-to-end validation is
  incomplete.
- **Planned**: in scope, but no supported implementation exists.
- **Unsupported**: the public scanner does not implement the feature.

## Capability matrix

| Area | Status | Current evidence and boundary |
|---|---|---|
| QR Code Model 2 | Partial | `src/decoder/` implements Model 2 geometry; golden v1 matrix and selected ignored real-image tests exist |
| Model 2 versions 1-40 | Partial | Version/block tables cover 1-40, but real-image coverage is sparse and the high-version test permits no result |
| QR Code Model 1 | Unsupported | `Version::Model1` stores metadata only; Model 1 detection/decoding is not implemented |
| Micro QR M1-M4 | Unsupported | `Version::Micro` stores metadata only; the detector requires the three Model 2 finder patterns |
| EC L/M/Q/H | Partial | All levels are parsed and block tables exist; exhaustive level/version fixtures do not |
| Masks 0-7 | Partial | All formulas are implemented in `MaskPattern`; tests do not exercise every pattern end to end |
| Numeric | Tested | `test_decode_numeric_mode` and mode-unit tests |
| Alphanumeric | Tested | `test_decode_alphanumeric_mode` and mode-unit tests |
| Byte | Tested | `test_decode_payload_byte_mode` and golden matrix decoding |
| Mixed modes | Tested | `test_decode_mixed_modes` |
| Kanji | Partial | Shift-JIS code units are reconstructed, but text conversion is lossy and untested |
| ECI | Partial | Variable-length assignment numbers are consumed but ignored; no character-set conversion or dedicated test |
| GS1 / FNC1 | Unsupported | Mode indicators 0101 and 1001 are not parsed |
| Structured Append | Unsupported | Mode indicator 0011 is not parsed |
| Inverted symbols | Partial | Sampled matrices are retried inverted; no dedicated end-to-end fixture |
| Rotated symbols | Partial | Eight orientation transforms are attempted; real-image rotation coverage is ignored by default |
| Mirrored symbols | Partial | Reflection transforms exist in orientation retries; no dedicated fixture |
| Multiple symbols | Partial | Multiple groups/results are supported, but the existing ignored regression asserts only one or more |
| Linux x86_64 | Tested | CI build and release-library test lane |
| macOS x86_64 | Tested | CI build and release-library test lane |
| Windows x86_64 | Tested | CI build and release-library test lane |
| AArch64 | Partial | NEON kernels compile conditionally; no CI target verifies them |
| WASM, iOS, Android | Planned | No target-specific build, test, binding, or packaging lane |
| `no_std` | Unsupported | The crate and dependencies require `std`; no feature or CI lane exists |

## Public input API

`try_detect(ImageInput)` is the checked API. `ImageInput` describes grayscale,
RGB, or RGBA data and an optional row stride. It rejects zero dimensions,
invalid strides, arithmetic overflow, and short buffers with `InputError`.
Compatibility functions `detect` and `detect_from_grayscale` return an empty
vector for invalid input.

Private SIMD grayscale kernels use `unsafe` on x86_64 and AArch64. Their safe
callers validate lengths and checked arithmetic first; the crate therefore does
not claim to contain zero unsafe code.

## Implemented pipeline

The current Model 2 pipeline includes grayscale conversion, multiple
binarization strategies, finder grouping, perspective sampling, format and
version extraction, all eight mask formulas, block deinterleaving,
Reed-Solomon correction, and payload parsing for the modes described above.
It also contains bounded recovery heuristics. A code path existing is not the
same as comprehensive standards conformance; the matrix records that
distinction.

## Roadmap

Near-term work is tracked in `TODO.md`. Measurement correctness (WP-002) takes
precedence over new performance claims. Model 1, Micro QR, GS1/FNC1,
Structured Append, full ECI conversion, and verified non-desktop platforms
require dedicated implementations and conformance fixtures before their status
can change.
