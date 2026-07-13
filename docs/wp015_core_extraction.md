# WP-015 core-extraction reassessment

This is a current source and build-graph assessment, not a `no_std` support
claim.  The published `rust_qr` crate remains a `std`-based image detector and
decoder.

## What is independently established today

The minimal feature configuration has no normal third-party dependencies:

```text
cargo tree --no-default-features --edges normal
rust_qr v0.1.0
```

`cargo test --lib --no-default-features` passed 119 library tests on the local
macOS AArch64 host on 2026-07-13.  This verifies the existing scalar,
`std`-based configuration only.  It does not compile a foreign target and does
not make a `no_std` claim.

The optional-dependency boundary is also real in the manifest:

| Feature | Normal dependency introduced |
| --- | --- |
| `parallel` | `rayon` |
| `image-loading` | `image` |
| `tools` | `clap` and `image-loading` |
| `simd` | none; private architecture-specific kernels |

`cargo tree --all-features --edges normal` contains `clap`, `image`, and
`rayon`; the minimal normal graph contains none of them.  The latter does not
remove the Rust standard library from the crate.

## Why an extracted matrix decoder is not a small safe change

There is a plausible `alloc`-compatible *primitive* seam:

- `models/matrix.rs` (`BitMatrix`), `models/point.rs`, and the QR value types;
- BCH, function-mask, bitstream, format/version extraction, unmasking, tables,
  Reed-Solomon, and mode parsing.

Those modules need explicit `alloc::{vec::Vec, string::String}` imports, but
their normal implementation does not itself need an OS, image loader, Rayon,
or CLI.  They are not yet an independently usable decoder API.  In particular:

- `QRCode` exposes owned `Vec<u8>` and `String` results, so it requires a
  deliberate alloc-facing result contract.
- `qr_decoder.rs` owns `DecodeRequestContext` with `std::time::Instant` and
  deadline checks.
- `matrix_decode.rs` imports `Instant` for uncertain-module recovery and is
  coupled to that request context.
- `payload.rs` and geometry/orientation recovery are private children of
  `QrDecoder`, not a standalone matrix API.
- The public `try_detect` pipeline depends on `std` synchronization,
  `Duration`/`Instant`, detector collections, grayscale conversion, and
  diagnostic strings; it cannot be conditionally compiled out while preserving
  the current public crate API.

Copying only the primitive modules to a new crate would produce a forked set
of QR helpers, not an extracted, regression-tested decoder.  Claiming that as
a supported `no_std` core would be misleading.  Conversely, moving the full
matrix path now would change the scheduling/recovery API and duplicate its
conformance responsibility while WP-006/WP-011 correctness gates remain
active.

## Required extraction design before implementation

The first real core release should be a new workspace crate with a narrow,
documented matrix-only API, for example a caller-supplied `BitMatrix` plus an
explicit recovery budget that has no wall-clock type.  The hosted crate should
depend on and re-export that crate; there must be no duplicate decoder source.
It must include:

1. `#![no_std]` and `extern crate alloc`, with a normal dependency graph that
   excludes `image`, `rayon`, and `clap`.
2. Matrix conformance fixtures shared with the hosted decoder, including all
   supported payload-mode and Reed-Solomon tests rather than copied smoke
   tests.
3. A target build lane such as
   `cargo +1.85 build -p rust_qr_matrix_core --target wasm32-unknown-unknown`
   after the target is installed in CI, plus the native core test lane.
4. Hosted parity tests demonstrating that the public `rust_qr` matrix decode
   calls the same core implementation.

Until those conditions exist, the correct status is: optional hosted
dependencies are separated; `no_std`, WASM, iOS, Android, and bindings are
unsupported.
