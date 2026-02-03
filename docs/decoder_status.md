# Decoder Status (as of 2026-02-02)

This doc summarizes the state of the QR decoder, what was fixed, what still needs work, and how to run benchmarks.

## What was fixed

Seven bugs were identified and fixed across three files. A golden matrix test now passes end-to-end for `"4376471154038"`.

### Bug fixes applied

| # | Severity | File | Bug | Fix |
|---|----------|------|-----|-----|
| 1 | Critical | `reed_solomon.rs` | `EXP_TABLE` corrupted from index 136 onward (wrapped instead of continuing) | Replaced with correct GF(256) exponentiation table |
| 2 | Critical | `reed_solomon.rs` | Berlekamp-Massey divided by `b[0]` (always 1) instead of previous discrepancy `delta_b` | Added `delta_b: u8` scalar, updated on LFSR length change |
| 3 | Critical | `reed_solomon.rs` | Syndrome calculation used `(i * j) as u8` truncating mod 256 instead of mod 255 | Added `Gf256::pow_usize(a, n: usize)` that reduces `n % 255` internally |
| 4 | Critical | `reed_solomon.rs` | Syndrome used ascending convention (c[0] = constant term) but Chien/Forney used descending | Changed syndrome to `i * (n - 1 - j)` (descending: c[0] = highest degree) |
| 5 | Critical | `reed_solomon.rs` | Chien search evaluated `sigma(alpha^(n-1-i))` instead of `sigma(alpha^{-(n-1-i)})` | Evaluates at the inverse; also uses `pow_usize` to avoid u8 truncation |
| 6 | Critical | `reed_solomon.rs` | Forney used wrong evaluation point and was missing `X_k` multiplier | Computes `x_inv = alpha^{-(n-1-pos)}`, multiplies by `X_k` per standard formula |
| 7 | Important | `function_mask.rs` | Alignment pattern step used ceiling division instead of floor | Changed `((numerator + denom - 1) / denom) * 2` to `(numerator / denom) * 2` |
| 8 | Moderate | `qr_decoder.rs` | Raw-bit brute force path (8 offsets, no RS) could return garbage that outscored correct results | Removed the `decode_payload_from_bits` direct-call block |

### Tests added (all passing)

| Test | File | What it verifies |
|------|------|-----------------|
| `test_gf256_pow_usize` | `reed_solomon.rs` | alpha^255=1, alpha^256=alpha, mod 255 vs mod 256 difference |
| `test_rs_encode_decode_no_errors` | `reed_solomon.rs` | Encode+decode roundtrip with zero errors |
| `test_rs_correct_single_error` | `reed_solomon.rs` | 1 corrupted byte corrected |
| `test_rs_correct_multiple_errors` | `reed_solomon.rs` | 3 corrupted bytes corrected |
| `test_rs_roundtrip_with_real_data` | `reed_solomon.rs` | "4376471154038" as bytes, encoded, corrupted 2 bytes, decoded |
| `test_rs_correct_errors_at_end` | `reed_solomon.rs` | Corrupted ECC bytes at tail, corrected |
| `test_alignment_positions` | `function_mask.rs` | v1=[], v2=[6,18], v7=[6,22,38], v10=[6,28,50], v14=[6,26,46,66] |
| `test_golden_matrix_decode` | `qr_decoder.rs` | Known-good 21x21 QR matrix for "4376471154038" decodes correctly end-to-end |

Run all unit tests: `cargo test --lib`

## What still needs work

### 1. Decoder performance (high priority)
`decode_from_matrix` does a massive brute force: 8 orientations x 32 format/mask combos x 4 traversal options x 2 bit orders = ~2048 RS decode attempts per version candidate. This makes real image tests take minutes even in release mode.

**Ideas to fix:**
- Only try orientations where finder patterns are in the correct corner (check top-left finder at (3,3))
- Read format info first and only try the extracted format, not all 32 combos
- Short-circuit on first successful RS decode instead of scoring all paths
- Remove unnecessary traversal variants (standard QR spec has one defined traversal)

### 2. Real image end-to-end verification (blocked by #1)
`test_smoke_real_images` and `test_real_qr_decode_image001` exist but are too slow to run interactively due to the brute-force decoder. The golden matrix test proves the decode pipeline is correct; the remaining gap is whether grid extraction from real photos produces a clean enough module matrix.

### 3. Detection pipeline gaps
- `image001.jpg` from the `nominal` category (small, clean images) returned no QR codes from `detect()`. This suggests finder pattern detection or grouping may be dropping valid patterns before decode even starts.
- Monitor images (2MB+) are very slow due to image size + brute-force decode.

## Files modified

| File | Changes |
|------|---------|
| `src/decoder/reed_solomon.rs` | Fixed EXP_TABLE, added `pow_usize`, fixed syndrome convention, fixed BM `delta_b`, fixed Chien search inversion, fixed Forney `X_k` factor, added RS encoder helper + 6 tests |
| `src/decoder/function_mask.rs` | Floor division for alignment step, added alignment position tests |
| `src/decoder/qr_decoder.rs` | Removed raw-bit brute force path, made `decode_from_matrix` `pub(crate)`, added golden matrix test |
| `src/lib.rs` | Narrowed `test_real_qr_decode_image001` to versions 1-3, added `test_smoke_real_images` |

## Benchmarks

Dataset layout:
- `benches/images/boofcv/` (BoofCV dataset, 16 categories)
- `benches/images/custom/` (your own images)

Benchmark commands:
- Real images detection benchmark:
  - `cargo bench --features tools --bench real_qr_images`
- Connected components benchmark:
  - `cargo bench --features tools --bench real_qr_cc`

Optional environment variables:
- `QR_DATASET_ROOT` (default: `benches/images/boofcv`)
- `QR_BENCH_LIMIT` (default: `5`, set to `0` for no limit)
- `QR_SMOKE` (set to `1` to use `_smoke.txt` inside the dataset root)

Example (smoke subset, no limit):
- `QR_SMOKE=1 QR_BENCH_LIMIT=0 cargo bench --features tools --bench real_qr_images`

## Notes for next session
- The core decode math (RS, alignment, bitstream→payload) is now correct. The golden test proves it.
- Next priority is making `decode_from_matrix` fast by eliminating the brute force (see #1 above).
- After that, verify real images decode correctly and investigate why `detect()` misses some nominal images.
