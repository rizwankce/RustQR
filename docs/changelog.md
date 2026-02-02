# Changelog & Next Steps

## 2026-02-02 — Benchmark Analysis & Improvement Plan

After running all 6 Criterion benchmark suites (`binarization`, `grayscale`, `memory_pool`, `qr_detect`, `real_qr_cc`, `real_qr_images`), the following next steps were identified:

### 1. Remove unconditional debug prints
There are unconditional `eprintln!` statements in `group_finder_patterns()` (`src/lib.rs`) that run in release builds. The `GROUP:` prints execute on every detection call and cause massive I/O overhead — the `real_qr_images` benchmark measured ~2.1s per iteration largely due to this. Wrap them in `#[cfg(debug_assertions)]` or remove entirely.

### 2. Fix grayscale NEON/unsafe warnings
`src/utils/grayscale.rs` has unused NEON loads (`rgba1`, `rgba2`, `rgba3`), missing `unsafe {}` blocks inside `unsafe fn` (required since Rust 2024 edition), and double-nested `unsafe` in scalar fallbacks. The RGBA NEON path only processes 4 pixels from `rgba0` instead of all 16. Fix all compiler warnings and complete the NEON vectorization.

### 3. Extend memory pool to cover BitMatrix
The pool currently only reuses the grayscale buffer (~2.4MB at 1080p), but the dominant allocations are the binarized `BitMatrix` objects from `adaptive_binarize()` and `otsu_binarize()`. Benchmarks show pool vs no-pool is identical at 640x480. Pool BitMatrix to get actual benefit.

### 4. Investigate detection regressions
Detection benchmarks show +68–93% regressions vs previous baselines. Some is debug print I/O, but after removing prints, profile to check whether the two-pass decode refactoring or finder validation introduced algorithmic cost. Connected-components path is ~10–13x slower than regular detection.

### 5. Add decode regression tests
Only 1 test (`test_real_qr`) checks pattern detection. No tests verify decoded output (data content, EC level, version). Add tests with known QR codes to protect the two-pass decoder and Reed-Solomon implementation.

### 6. Benchmark `real_qr_images` suite properly
Once debug prints are removed, this suite should be runnable. Current synthetic benchmarks use uniform gray data (`128u8`) which doesn't exercise realistic code paths. Validate with real images after fixes.
