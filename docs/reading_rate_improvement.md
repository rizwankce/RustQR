# Reading Rate Improvement Plan

Goal: Raise RustQR's overall reading rate from **~15%** toward **60%+** (beating ZBar's 38.95%) while maintaining world-class speed (<5ms for 1MP images).

---

## Current State (CI Run #21647289490, 2026-02-03)

| Category | Images | Dynamsoft | BoofCV | ZBar | **RustQR** | Gap to ZBar |
|----------|--------|-----------|--------|------|------------|-------------|
| blurred | 45 | 66.15% | 38.46% | 35.38% | **35.56%** | Matched |
| brightness | 28 | 81.18% | 78.82% | 50.59% | **0.00%** | -50.59% |
| bright_spots | 32 | 43.30% | 27.84% | 19.59% | **0.00%** | -19.59% |
| close | 40 | 95.00% | 100.00% | 12.50% | **20.00%** | Ahead |
| curved | 50 | 70.00% | 56.67% | 35.00% | **20.00%** | -15.00% |
| damaged | 37 | 51.16% | 16.28% | 25.58% | **13.51%** | -12.07% |
| glare | 50 | 84.91% | 32.08% | 35.85% | **14.00%** | -21.85% |
| high_version | 33 | 97.30% | 40.54% | 27.03% | **0.00%** | -27.03% |
| lots | 7 | 100.00% | 99.76% | 18.10% | **0.00%** | -18.10% |
| monitor | 17 | 100.00% | 82.35% | 0.00% | **64.71%** | Ahead |
| nominal | 65 | 93.59% | 89.74% | 66.67% | **47.69%** | -18.98% |
| noncompliant | 16 | 92.31% | 3.85% | 50.00% | **0.00%** | -50.00% |
| pathological | 23 | 95.65% | 43.48% | 65.22% | **56.52%** | -8.70% |
| perspective | 35 | 62.86% | 80.00% | 42.86% | **20.00%** | -22.86% |
| rotations | 44 | 99.25% | 96.24% | 48.87% | **0.00%** | -48.87% |
| shadows | 14 | 100.00% | 85.00% | 90.00% | **7.14%** | -82.86% |
| **total** | **536** | **83.29%** | **60.69%** | **38.95%** | **~18.70%** | **-20.25%** |

**Six categories at 0%**: rotations, high_version, brightness, bright_spots, noncompliant, lots

---

## Root Cause Analysis by Pipeline Stage

### Stage 1: Binarization (`utils/binarization.rs`)

**Current approach:**
- Large images (>=800px): adaptive binarize (window=31, local mean threshold)
- Small images (<800px): Otsu (single global threshold)
- Fallback: try the other method if <3 finder patterns found

**Problems identified:**

1. **No Sauvola bias** - Adaptive binarization uses raw local mean as threshold (`pixel < local_mean`). This fails when background brightness varies. Sauvola's method uses `threshold = mean * (1 + k * (std_dev / R - 1))` which adapts to local contrast, not just brightness. This is the #1 reason brightness/shadows/glare/bright_spots all fail.

2. **Fixed window size (31)** - Window size should scale with image dimensions or estimated module size. A 31px window on a 4000px image covers far too little context; on a 200px image it covers too much.

3. **Only 2 binarization strategies** - Real-world QR scanners (ZXing, BoofCV) try multiple thresholds. A multi-threshold approach (e.g., Otsu, Otsu+offset, Otsu-offset, adaptive with different window sizes) would catch more cases.

4. **No contrast enhancement** - Images with low contrast (glare, brightness) would benefit from histogram equalization or CLAHE before binarization.

**Impact**: Directly affects brightness (0%), bright_spots (0%), shadows (7%), glare (14%)

### Stage 2: Finder Pattern Detection (`detector/finder.rs`)

**Current approach:**
- Horizontal row scanning only with 1:1:3:1:1 ratio matching
- Cross-check in vertical and horizontal directions
- Merge candidates within 50px distance

**Problems identified:**

1. **No vertical or diagonal scanning** - QR codes rotated 90 degrees still have finder patterns detectable via horizontal scanning (the pattern appears vertically). However, non-axis-aligned rotations (30, 45, 60 degrees) produce diagonal patterns that pure horizontal scanning misses entirely. This is the primary reason rotations=0%.

2. **`module_size < 2.0` hard rejection** - In `order_finder_patterns()` (lib.rs:110), any pattern with module_size < 2.0 is rejected. This filters out small QR codes in high-resolution images or zoomed-out views. Many "close" and "nominal" images likely have small module sizes.

3. **Merge distance of 50px is fixed** - Should scale with module size. For large QR codes, 50px might merge distinct finder patterns from different QR codes ("lots" category). For small QR codes, it might be too loose.

4. **No column (vertical) scanning pass** - ZXing and BoofCV scan both rows and columns. Adding a vertical scan pass would catch many rotated patterns without full image rotation.

5. **`has_significant_edges` threshold is 2-3 transitions** - This is very low. In noisy images this lets through too many rows; in very clean images with small QR codes, the sampling step of 4px may skip over the pattern entirely.

**Impact**: Directly affects rotations (0%), partially affects all categories

### Stage 3: Pattern Grouping (`lib.rs`)

**Current approach:**
- Bin patterns by module size (1.25x ratio bins)
- Try each bin + neighbor, build groups of 3 with geometric constraints
- Right angle check: cosine < 0.3

**Problems identified:**

1. **Size ratio 1.5x limit too tight** - Perspective distortion can make one finder pattern appear significantly larger than another. A 2.0x or even 2.5x ratio should be allowed for "perspective" images.

2. **Only first valid grouping is returned** - `build_groups()` marks patterns as `used` immediately, so if the first valid group is wrong, the correct group may never be tried.

3. **`min_d < avg_module * 3.0` rejection** - For very small QR codes (version 1, 21 modules), the distance between finder centers is only ~14 modules. With small module sizes, this threshold may reject valid groups.

**Impact**: Affects perspective (20%), curved (20%), close (20%)

### Stage 4: Bottom-Right Corner Estimation (`decoder/qr_decoder.rs`)

**Current approach:**
- `bottom_right = top_right + bottom_left - top_left` (parallelogram)
- 9 offset candidates tried (±2 steps in x and y)
- Alignment pattern used to refine transform if found

**Problems identified:**

1. **Parallelogram assumption fails for perspective/curved images** - When the QR code is not planar or is viewed at a steep angle, the bottom-right corner can be far from the parallelogram estimate. The 9-candidate search (±2 module steps) covers only a small area.

2. **Alignment pattern detection is incomplete** - `alignment.rs::get_alignment_positions()` hardcodes positions only for versions 2-6. For version 7+, it falls back to `vec![6, 18]` which is WRONG. Meanwhile, `function_mask.rs::alignment_pattern_positions()` computes positions correctly for all versions. This means alignment-based transform refinement is broken for all high-version QR codes. This is likely a major contributor to high_version=0%.

3. **No iterative refinement** - After initial grid sampling, there's no feedback loop to adjust the transform based on the sampled module values. Libraries like BoofCV re-estimate the transform using timing pattern modules.

**Impact**: Directly affects high_version (0%), perspective (20%), curved (20%)

### Stage 5: Grid Sampling (`decoder/qr_decoder.rs`)

**Current approach:**
- Binary sampling: 3x3 pixel neighborhood majority vote
- Gray sampling: 3x3 average, then global median threshold across all sampled modules

**Problems identified:**

1. **Global median threshold for gray sampling** - `extract_qr_region_gray_with_transform()` sorts ALL module samples and uses the median as threshold. This fails when the QR code has uneven lighting (one side darker). Should use local/adaptive thresholding on the sampled grid.

2. **Fixed 3x3 neighborhood** - For high-version QR codes with small module sizes (e.g., version 40 at 177x177 modules), each module might be only 2-3 pixels. A 3x3 neighborhood would sample adjacent modules. Should scale sampling area with module size.

3. **No sub-pixel interpolation** - Using `.round()` to convert float coordinates to pixel coordinates introduces quantization error. Bilinear interpolation would give more accurate module values.

**Impact**: Affects all categories, especially high_version (0%), nominal (47%), damaged (13%)

### Stage 6: Decoding (`decoder/qr_decoder.rs`)

**Current approach:**
- Pass 1: Try extracted format info × 4 traversal variants × 8 orientations
- Pass 2: Brute-force all 32 EC/mask combos × 4 traversals × valid orientations
- Each attempt: unmask → extract bits → deinterleave → RS decode → parse payload
- Total: up to ~2048+ RS decode attempts per version candidate

**Problems identified:**

1. **Brute force is extremely slow** - The decoder tries up to 2048 combinations per version candidate, and tries ALL 40 versions. For a single image, this can mean tens of thousands of RS decode attempts. This is why the benchmark takes 49+ minutes for 536 images.

2. **Timing patterns not validated** - `detector/timing.rs` exists but is never called. Checking the alternating pattern between finder patterns would quickly validate grid alignment and reject bad transforms before expensive decoding.

3. **Format info BCH correction exists but is underused** - `format.rs` correctly implements BCH(15,5) error correction. If format extraction succeeds, only 1 EC/mask combination should be tried (not 32). The brute-force pass 2 should only trigger if BCH extraction fails.

4. **Version BCH correction is weak** - `version.rs` only tries single-bit corrections. Full BCH(18,6) syndrome decoding would handle up to 3-bit errors.

5. **Orientation brute force is wasteful** - 8 orientations are tried. The `has_finders_correct()` check reduces this somewhat, but the finder check has a tolerance of 3/21 mismatches. In practice, only 1-2 orientations should pass for a well-extracted grid.

6. **Kanji mode not supported** - Mode 8 (Kanji) returns `None`, which aborts the entire decode. Some QR codes use Kanji encoding even for non-Japanese content.

**Impact**: Speed affects all categories (slow = CI timeout issues). Correctness issues affect high_version, noncompliant

### Stage 7: Missing Capabilities

1. **No multi-QR detection** - For the "lots" category (multiple QR codes per image), the grouping logic marks patterns as "used" after the first group. Need to support multiple independent groups.

2. **No image preprocessing** - No sharpening, denoising, or contrast enhancement before binarization. Blurred images would benefit from unsharp masking; noisy images from median filtering.

3. **No Micro QR support** - While listed as a feature, Micro QR codes are not actually detected (finder pattern is different: single finder, not three).

---

## Improvement Plan: Prioritized by Impact

### Phase 0: Measurement Correctness & Benchmark Hygiene (Completed on 2026-02-06)

Before algorithm work, ensure benchmark numbers are trustworthy and comparable across runs.

**Status:** Completed
- [x] 0.1 Fix Reading-Rate Scoring Correctness
- [x] 0.2 Add Per-Stage Miss Telemetry
- [x] 0.3 Benchmark Consistency Cleanup

#### 0.1 Fix Reading-Rate Scoring Correctness
- **Files**: `src/bin/qrtool.rs`, `src/tools/mod.rs`
- **Current issue**:
  - A "hit" is currently counted as `!results.is_empty()` (any decode), without verifying decoded content matches the `.txt` label.
  - Overall "Average Reading Rate" is unweighted mean across categories, not true global success rate across all images.
  - `QR_BENCH_LIMIT` defaults to 5 when unset, which can silently sample only a tiny subset.
- **Fix**:
  1. Compare decoded payload(s) against expected `.txt` content per image.
  2. Report both:
     - weighted global rate = `total_hits / total_labeled_images`
     - category rates (for diagnosis)
  3. Change default benchmark limit to full dataset (`None` / 0 semantics) for benchmark mode.
- **Impact**: Prevents misleading regressions/improvements; makes CI results actionable.
- **Effort**: Small-Medium

#### 0.2 Add Per-Stage Miss Telemetry
- **Files**: `src/lib.rs`, `src/decoder/qr_decoder.rs`, `src/detector/*`, `src/bin/qrtool.rs`
- **What**: Emit stage-level counters for each image:
  - `binarize_ok`
  - `finder_patterns_found`
  - `group_candidates_found`
  - `transform_built`
  - `format_extracted`
  - `rs_decode_success`
  - `payload_parse_success`
- **Output**:
  - per-category stage histogram
  - top-N failure reasons by category
- **Why**: Without stage telemetry, optimization work is guess-heavy and slow.
- **Impact**: Dramatically improves prioritization accuracy and debugging speed.
- **Effort**: Medium
- **Speed impact**: Minimal if gated behind benchmark/debug flag

#### 0.3 Benchmark Consistency Cleanup
- **Files**: `README.md`, `.github/workflows/benchmark.yml`, `docs/*`
- **Current issues**:
  - Dataset totals in docs are inconsistent across files/runs.
  - Category image counts and rates can drift from what benchmark actually runs.
  - Speed claims and read-rate tables are hard to trace to exact command/env.
- **Fix**:
  1. Standardize benchmark command and env in one place.
  2. Embed run metadata in output:
     - dataset root
     - limit/smoke flags
     - commit SHA
     - date/time
  3. Auto-generate the README benchmark table from benchmark output artifacts.
- **Impact**: Eliminates confusion and stale numbers; improves confidence in reported progress.
- **Effort**: Medium

---

### Phase 1: Critical Fixes (Completed on 2026-02-06)

These are bugs and missing fundamentals that cause entire categories to fail.

**Status:** Completed
- [x] 1.1 Fix/Remove Unused Alignment Helper in Detector Module
- [x] 1.2 Add Sauvola Binarization
- [x] 1.3 Add Vertical Column Scanning for Finder Patterns
- [x] 1.4 Remove `module_size < 2.0` Hard Rejection

#### 1.1 Fix/Remove Unused Alignment Helper in Detector Module
- **File**: `src/detector/alignment.rs`
- **Bug**: `get_alignment_positions()` returns `vec![6, 18]` for ALL versions > 6
- **Fix**: Use the correct lookup from `function_mask.rs::alignment_pattern_positions()` or remove this helper if unused
- **Impact**: Code hygiene and future-proofing (not a primary high_version unlock by itself)
- **Effort**: Small (1 function fix)
- **Speed impact**: None

#### 1.2 Add Sauvola Binarization
- **File**: `src/utils/binarization.rs`
- **What**: Implement Sauvola's method: `threshold = mean * (1 + k * (std_dev / R - 1))` where k≈0.2, R=128
- **Why**: Adapts to local contrast, not just local brightness. Handles uneven illumination, shadows, and glare far better than plain adaptive
- **Impact**: Should unlock brightness (0%), shadows (7%), and significantly improve glare (14%), bright_spots (0%)
- **Effort**: Medium (need integral image of squared values for fast std_dev computation)
- **Speed impact**: ~2x slower than current adaptive (extra integral image), but only for affected images
- **Fallback strategy**: Try Otsu → Sauvola → Adaptive, take first that yields >=3 finder patterns

#### 1.3 Add Vertical Column Scanning for Finder Patterns
- **File**: `src/detector/finder.rs`
- **What**: After horizontal row scanning, do a vertical column scanning pass
- **Why**: Catches QR codes rotated ~90 degrees. Combined with existing horizontal scanning, covers 0/90/180/270 degree rotations
- **Impact**: Should unlock a significant portion of rotations (0/44) — at least the 90/180/270 subset
- **Effort**: Medium (refactor `scan_row` to work on columns, or transpose matrix first)
- **Speed impact**: ~2x finder detection time, but can be made optional or parallel

#### 1.4 Remove `module_size < 2.0` Hard Rejection
- **File**: `src/lib.rs` line 110
- **What**: Lower the threshold to 1.0 or remove it and let downstream validation handle it
- **Why**: Rejects valid small QR codes in high-res images. The 1:1:3:1:1 ratio check already validates pattern quality
- **Impact**: Improves nominal, close categories
- **Effort**: Tiny (one line change)
- **Speed impact**: None

**Implementation notes:**
- Alignment helper now uses `alignment_pattern_positions(version)` from `function_mask.rs`.
- Sauvola binarization is implemented with integral + squared-integral images.
- Finder detection includes vertical column scanning (including pyramid and bounded scan paths).
- Module-size floor in grouping/order logic is now `1.0` (from `2.0`).

### Phase 2: Decode Pipeline Speedup (Expected: 35% → 45%)

Decode speed directly affects reading rate — slow decoding means CI timeouts and prevents trying more binarization/detection strategies.

**Status:** Completed (2026-02-06)
- [x] 2.1 Eliminate Brute-Force Format/Mask Search
- [x] 2.2 Add Timing Pattern Validation
- [x] 2.3 Limit Version Candidate Search
- [x] 2.4 Reduce Orientation Attempts

#### 2.1 Eliminate Brute-Force Format/Mask Search
- **File**: `src/decoder/qr_decoder.rs`
- **What**: If `FormatInfo::extract()` succeeds (BCH correctable), ONLY use that format. Remove pass 2 (all 32 combos) except as a last resort with a very limited retry
- **Why**: Pass 2 tries 32 × 4 = 128 RS decode attempts per orientation. If format BCH succeeds, only 4 traversal attempts needed per orientation
- **Impact**: ~32x decode speedup for most images. Frees time budget for more detection strategies
- **Effort**: Small (restructure decode loop)
- **Speed impact**: Massive improvement

#### 2.2 Add Timing Pattern Validation
- **File**: `src/detector/timing.rs` (exists but unused)
- **What**: After building transform, sample timing patterns between finder pairs. Verify they alternate B-W-B-W. Reject transforms where <60% of timing modules match
- **Why**: Quick structural check that eliminates bad transforms before expensive RS decoding
- **Impact**: Reduces false decode attempts, speeds up overall pipeline
- **Effort**: Small (function exists, just wire it up)
- **Speed impact**: Significant reduction in wasted RS decode attempts

#### 2.3 Limit Version Candidate Search
- **File**: `src/decoder/qr_decoder.rs`
- **What**: Currently tries estimated_version ± 2, then ALL 40 versions. Limit to ± 2 only (5 candidates max). If version info BCH succeeds (v7+), use exact version
- **Why**: Trying 40 versions × 2048 combos = ~82,000 RS attempts is absurd
- **Impact**: ~8x decode speedup
- **Effort**: Small
- **Speed impact**: Large

#### 2.4 Reduce Orientation Attempts
- **File**: `src/decoder/qr_decoder.rs`
- **What**: Only try orientations where `has_finders_correct()` passes. Currently this is done but ALL 8 are generated. Skip generation of orientations that can't work
- **Why**: Each orientation involves a BitMatrix clone and rotation
- **Effort**: Small
- **Speed impact**: Moderate

### Phase 3: Detection Robustness (Expected: 45% → 55%)

**Status:** Completed (2026-02-06)
- [x] 3.1 Multi-Threshold Binarization Strategy
- [x] 3.2 Adaptive Merge Distance for Finder Patterns
- [x] 3.3 Improve Grid Sampling with Local Thresholding
- [x] 3.4 Relax Grouping Constraints for Perspective

#### 3.1 Multi-Threshold Binarization Strategy
- **File**: `src/lib.rs`
- **What**: Try multiple binarization approaches systematically:
  1. Sauvola (k=0.2, window=auto)
  2. Adaptive mean (window=auto)
  3. Otsu global
  4. Otsu + offset (threshold ± 10%)
  5. Sauvola with different k values (0.1, 0.3)
- **Why**: No single binarization handles all lighting conditions. The speed budget freed by Phase 2 allows trying more strategies
- **Impact**: Broadly improves all categories, especially brightness, shadows, bright_spots, glare
- **Effort**: Medium
- **Speed impact**: Managed by early-exit on first successful decode

#### 3.2 Adaptive Merge Distance for Finder Patterns
- **File**: `src/detector/finder.rs`
- **What**: Scale merge distance with estimated module size instead of fixed 50px: `merge_dist = module_size * 5.0`
- **Why**: Prevents merging distinct finder patterns from different QR codes (lots category), while still merging duplicate detections of the same pattern
- **Impact**: Improves lots (0%), reduces false groupings
- **Effort**: Small
- **Speed impact**: None

#### 3.3 Improve Grid Sampling with Local Thresholding
- **File**: `src/decoder/qr_decoder.rs`
- **What**: Replace global median threshold in `extract_qr_region_gray_with_transform()` with row-by-row or block-by-block adaptive threshold
- **Why**: Global median fails when QR has uneven lighting. Local threshold within the sampled grid adapts to gradients
- **Impact**: Improves brightness, shadows, glare decode success after detection
- **Effort**: Medium
- **Speed impact**: Minimal

#### 3.4 Relax Grouping Constraints for Perspective
- **File**: `src/lib.rs`
- **What**: Increase module size ratio tolerance from 1.5 to 2.0. Relax right-angle cosine threshold from 0.3 to 0.4
- **Why**: Perspective distortion changes apparent module sizes and angles
- **Impact**: Improves perspective (20%), curved (20%)
- **Effort**: Tiny
- **Speed impact**: Slightly more groups to try (bounded by trim to 40)

### Phase 4: Advanced Detection (Expected: 55% → 65%+)

**Status:** Completed (2026-02-06)
- [x] 4.1 Image Rotation Attempts for Non-Axis Rotations
- [x] 4.2 Better Bottom-Right Corner Estimation
- [x] 4.3 Multi-QR Support
- [x] 4.4 Contrast Enhancement Preprocessing
- [x] 4.5 Noncompliant QR Support
- [x] 4.6 Add Kanji Mode Support

#### 4.1 Image Rotation Attempts for Non-Axis Rotations
- **What**: If no QR found at original orientation, try detecting at 45-degree rotation. Can use fast rotation (swap x,y and mirror) or pre-rotate grayscale image
- **Why**: Vertical+horizontal scanning covers 0/90/180/270 but not arbitrary rotations. A 45-degree pass catches most remaining cases
- **Impact**: Should push rotations from partial (after Phase 1.3) toward 50-70%
- **Effort**: Medium
- **Speed impact**: Only triggered on initial detection failure (no cost for easy images)

#### 4.2 Better Bottom-Right Corner Estimation
- **What**: Instead of parallelogram + 9 offsets, estimate bottom-right using:
  1. Timing pattern extension (follow timing from TR and BL, intersect)
  2. Alignment pattern search in estimated region
  3. Edge detection along expected QR boundary
- **Why**: Parallelogram assumption fails under perspective distortion
- **Impact**: Improves perspective, curved, high_version
- **Effort**: Large
- **Speed impact**: Moderate (adds detection logic before decode)

#### 4.3 Multi-QR Support
- **What**: Allow `build_groups()` to return multiple non-overlapping groups. Decode each independently
- **Why**: "lots" category has 7 images with many QR codes each. Current grouping uses first-match-wins
- **Impact**: Unlocks lots category (0%)
- **Effort**: Medium (grouping already returns Vec<Vec<usize>>, need to avoid marking all as used)
- **Speed impact**: More decode attempts per image (proportional to QR count)

#### 4.4 Contrast Enhancement Preprocessing
- **What**: Apply CLAHE (Contrast Limited Adaptive Histogram Equalization) or simple histogram stretching before binarization
- **Why**: Low-contrast images (glare, brightness) have poor binarization regardless of method
- **Impact**: Improves glare, brightness, bright_spots
- **Effort**: Medium (CLAHE implementation from scratch in pure Rust)
- **Speed impact**: Moderate (~1-2ms for 1MP image)

#### 4.5 Noncompliant QR Support
- **What**: Relax structural validation:
  - Allow missing/damaged finder patterns (detect with 2 finders + heuristics)
  - Allow non-standard quiet zones
  - Try decoding even with high finder mismatch counts
- **Why**: 16 noncompliant images (0%) likely have structural deviations from spec
- **Impact**: Unlocks noncompliant category
- **Effort**: Medium
- **Speed impact**: Minimal (only triggered on normal failure)

#### 4.6 Add Kanji Mode Support
- **File**: `src/decoder/qr_decoder.rs`
- **What**: Implement mode 8 (Kanji) decoding. Currently returns `None` which aborts the entire decode
- **Why**: Some QR codes use Kanji mode even for non-Japanese content. Encountering mode 8 currently fails the entire decode
- **Impact**: Potentially affects noncompliant and some nominal images
- **Effort**: Small-Medium

### Phase 5: Speed-Aware Optimizations (Maintain <5ms target)

#### 5.1 Tiered Detection Strategy
- **What**: Implement a "fast path" and "slow path":
  - Fast path: Otsu → detect → decode (single attempt, <5ms)
  - Slow path (if fast fails): Multi-threshold → multi-orientation → relaxed constraints
- **Why**: Most benchmark images don't need aggressive strategies. Only fall back to expensive methods when simple approaches fail
- **Impact**: Maintains speed for easy images, improves reading rate for hard images
- **Effort**: Medium (restructure main `detect()` pipeline)

#### 5.2 Module-Size-Aware Window Sizing
- **What**: After initial finder detection, use estimated module size to set adaptive binarization window: `window = max(31, module_size * 7)`
- **Why**: Binarization window should relate to QR structure, not be arbitrary
- **Impact**: Improves binarization quality across all categories
- **Effort**: Small

#### 5.3 Early Termination Improvements
- **What**: After successful format info extraction, if RS decode fails on the first block, skip remaining blocks for that combination (instead of continuing to decode all blocks before reporting failure)
- **Why**: RS failure on block 0 means the entire attempt is wrong
- **Impact**: Speed improvement for failed decode attempts
- **Effort**: Small

---

## Estimated Impact Summary

| Phase | Reading Rate | Key Unlocks | Speed Impact |
|-------|-------------|-------------|--------------|
| Current | ~18.7% | — | — |
| Phase 0 (completed) | ~18.7% (more accurate) | Reliable measurement + diagnostics | Neutral |
| Phase 1 (completed) | ~35% (projected; re-benchmark pending) | high_version, brightness, shadows, rotations (partial) | Neutral |
| Phase 2 | ~45% | Faster decoding enables more retry strategies | 10-30x decode speedup |
| Phase 3 | ~55% | lots, better perspective/curved/glare | Managed by early-exit |
| Phase 4 | ~65%+ | rotations (full), noncompliant, curved | Tiered approach |
| Phase 5 | ~65%+ (faster) | No new categories | <5ms fast path maintained |

## Target: Beat ZBar (38.95%) While Staying Fastest

ZBar's 38.95% is achievable with Phase 1 + Phase 2 alone. Phase 3+ aims to approach BoofCV's 60.69%.

**Speed constraint**: Every improvement must be evaluated against the <5ms target for 1MP images. Use tiered detection (fast path first, fallback to expensive methods) to maintain speed for common cases while handling edge cases.

---

## Recommended Implementation Order

1. **Phase 2.1** - Eliminate brute-force format search (2-4 hrs, massive speedup)
2. **Phase 2.3** - Limit version candidates (30 min, major speedup)
3. **Phase 2.2** - Add timing pattern validation (1-2 hrs)
4. **Phase 1.2** - Sauvola binarization (4-6 hrs, unlocks brightness/shadows)
5. **Phase 1.3** - Vertical column scanning (3-4 hrs, partially unlocks rotations)
6. **Phase 1.4** - Remove/relax module_size < 2.0 filter (quick win)
7. **Phase 3.3** - Local grid thresholding (2-3 hrs)
8. **Phase 3.1** - Multi-threshold strategy (2-3 hrs)
9. **Phase 3.4** - Relax grouping constraints (30 min)

With Phase 0 complete, this order now prioritizes: speed → robustness, while preserving the measurement correctness baseline for reproducible tuning.
