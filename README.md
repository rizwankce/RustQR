# RustQR

[![CI](https://github.com/rizwankce/RustQR/actions/workflows/ci.yml/badge.svg)](https://github.com/rizwankce/RustQR/actions/workflows/ci.yml)
[![Benchmark](https://github.com/rizwankce/RustQR/actions/workflows/benchmark.yml/badge.svg)](https://github.com/rizwankce/RustQR/actions/workflows/benchmark.yml)
[![License](https://img.shields.io/badge/license-MIT%20or%20Apache--2.0-blue)](Cargo.toml)
[![Rust](https://img.shields.io/badge/rust-stable-orange)](https://www.rust-lang.org)

Fast QR detection/decoding rebuild in Rust, with benchmark-driven development against BoofCV image sets.

## Status

- Active rebuild branch: `scratch_from_scratch_rebuild`
- Current target: `>=90%` reading rate with median runtime `<=1000 ms/image` on BoofCV benchmarks
- Source-of-truth plan: `docs/new_from_scratch.md`

The pipeline is functional end-to-end and benchmark automation is live. The project is not API-stable yet.

## Features

- Pure Rust codebase with `#![forbid(unsafe_code)]`
- Multi-stage QR pipeline:
  - `proposal_ensemble`
  - `hypothesis_search`
  - `geometry_refinement`
  - `decode_engine`
  - `multi_qr_iteration`
- Real image decode path with telemetry and stage timing
- Reading-rate harness with profile-based dataset selection
- GitHub Actions support for CI and on-demand benchmark runs

## Installation

This rebuild is currently branch-first. Add via git dependency:

```toml
[dependencies]
rust_qr = { git = "https://github.com/rizwankce/RustQR", branch = "scratch_from_scratch_rebuild" }
```

## Usage

Library API:

```rust
use rust_qr::detect;

let image_rgb: Vec<u8> = vec![0; 640 * 480 * 3];
let codes = detect(&image_rgb, 640, 480);
for code in codes {
    println!("{}", code.payload);
}
```

CLI:

```bash
# smoke
cargo run --bin qrtool -- smoke

# reading-rate
cargo run --bin qrtool -- reading-rate --profile boofcv-all --artifact target/reading_rate_boofcv_all.json

# reading-rate with KPI gate thresholds
cargo run --bin qrtool -- reading-rate --profile boofcv-all \
  --gate-global-rate-min 0.40 \
  --gate-rotations-rate-min 0.45 \
  --gate-high-version-rate-min 0.09 \
  --gate-median-runtime-ms-max 1000 \
  --artifact target/reading_rate_boofcv_all_gated.json
```

## Benchmarking

Run one profile:

```bash
cargo run --bin qrtool -- reading-rate --profile boofcv-rotations --artifact target/rr_rotations.json
```

Run full sweep (all profiles) in GitHub Actions:

- Workflow: `Benchmark`
- Dispatch input: `profile=all-profiles`
- KPI gate inputs (`gate_global_rate_min`, `gate_rotations_rate_min`, `gate_high_version_rate_min`, `gate_median_runtime_ms_max`) are evaluated on the `boofcv-all` lane and fail the job when violated.

Profile families:
- `boofcv-all`
- `boofcv-<category>` where category is one of:
  - `blurred`, `brightness`, `bright-spots`, `close`, `curved`, `damaged`, `glare`, `high-version`, `lots`, `monitor`, `nominal`, `noncompliant`, `pathological`, `perspective`, `rotations`, `shadows`
- `payload-validated` (strict payload match set)

Legacy aliases:
- `monitor-smoke` -> `boofcv-monitor`
- `nominal-smoke` -> `boofcv-nominal`

Semantics note:
- `boofcv-*` profiles are annotation-label based ("any decode" counts as matched).
- `payload-validated` is strict payload equality.

## Reading Rate Snapshot

Latest RustQR values below are from GitHub Actions run `21927167412` on `scratch_from_scratch_rebuild` (completed on `2026-02-12`).

| Category | Images | Dynamsoft | BoofCV | ZBar | RustQR |
|----------|--------|-----------|--------|------|--------|
| blurred | 45 | 66.15% | 38.46% | 35.38% | **31.11%** |
| brightness | 28 | 81.18% | 78.82% | 50.59% | **25.00%** |
| bright_spots | 32 | 43.30% | 27.84% | 19.59% | **0.00%** |
| close | 40 | 95.00% | 100.00% | 12.50% | **97.50%** |
| curved | 50 | 70.00% | 56.67% | 35.00% | **26.00%** |
| damaged | 37 | 51.16% | 16.28% | 25.58% | **21.62%** |
| glare | 50 | 84.91% | 32.08% | 35.85% | **20.00%** |
| high_version | 33 | 97.30% | 40.54% | 27.03% | **9.09%** |
| lots | 7 | 100.00% | 99.76% | 18.10% | **0.00%** |
| monitor | 17 | 100.00% | 82.35% | 0.00% | **94.12%** |
| nominal | 65 | 93.59% | 89.74% | 66.67% | **73.85%** |
| noncompliant | 16 | 92.31% | 3.85% | 50.00% | **6.25%** |
| pathological | 23 | 95.65% | 43.48% | 65.22% | **56.52%** |
| perspective | 35 | 62.86% | 80.00% | 42.86% | **74.29%** |
| rotations | 44 | 99.25% | 96.24% | 48.87% | **45.45%** |
| shadows | 14 | 100.00% | 85.00% | 90.00% | **35.71%** |
| total | 536 | 83.29% | 60.69% | 38.95% | **41.60%** |

Run-level summary:
- `boofcv-all`: `223 / 536` matched, median `1908.431 ms/image`
- `payload-validated`: `26 / 26` matched, median `156.693 ms/image`

## Documentation

- Rebuild strategy: `docs/new_from_scratch.md`
- Lessons learned: `docs/learnings_from_old_code_base_try.md`
- TODO workpackets: `docs/todo/01_feature_todo.txt`
- Completed workpackets: `docs/completed/01_feature.todo.txt`

## Contributing

Issues and pull requests are welcome.

Before opening a PR:

```bash
cargo fmt -- --check
cargo test
```

For benchmark-impacting changes, include:
- commands run
- artifact path(s)
- before/after rate + runtime deltas

## License

Dual-licensed under MIT or Apache-2.0 (`MIT OR Apache-2.0` in `Cargo.toml`).
