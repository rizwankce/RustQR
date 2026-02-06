# Reading Rate Improvement Plan

Goal: Raise RustQR's overall reading rate from the public README baseline (**8.04%**) toward **38.95%+** (beat ZBar), then toward **60%+** (BoofCV range), while maintaining world-class speed (<5ms for 1MP images).

---

## Historical Snapshot for RCA (CI Run #21647289490, 2026-02-03)

This table is kept as root-cause context from the pre-Phase-1/2 optimization period. For Phase 6 gating and comparison, use the README baseline run metadata and full-dataset A/B harness.

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

**Status:** Completed (2026-02-06)
- [x] 5.1 Tiered Detection Strategy
- [x] 5.2 Module-Size-Aware Window Sizing
- [x] 5.3 Early Termination Improvements

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

### Phase 6: Accuracy Stabilization & Detector Diversification (README-aligned)

Goal of this phase: move weighted global reading rate from the public README baseline (**8.04%**) to **>=25% reproducible** on the full 536-image run, then continue toward ZBar (**38.95%**).

Public comparison baseline:
- README benchmark note: GitHub Actions run `21745898128` on commit `ba3cedd` (`macos-latest`).
- Full dataset only (`--limit 0` / no sampling), no smoke flags.
- Runtime guardrail: do not regress median per-image time by >15%.

#### Priority Gaps vs ZBar (from README)

| Category | RustQR | ZBar | Gap |
|----------|--------|------|-----|
| shadows | 5.00% | 90.00% | -85.00% |
| pathological | 0.00% | 65.22% | -65.22% |
| noncompliant | 0.00% | 50.00% | -50.00% |
| brightness | 2.35% | 50.59% | -48.24% |
| nominal | 32.05% | 66.67% | -34.62% |
| rotations | 14.29% | 48.87% | -34.58% |
| high_version | 0.00% | 27.03% | -27.03% |

**Execution order for Phase 6:** `6.1 -> 6.2 -> 6.7 -> 6.5 -> 6.4 -> 6.3 -> 6.6`

**Status:** Completed (2026-02-06)
- [x] 6.1 Add A/B Read-Rate Regression Harness (baseline vs candidate)
- [x] 6.2 Add Candidate Scoring + Top-K Decode Gating
- [x] 6.7 Single-QR First, Multi-QR Expansion Second
- [x] 6.5 Subpixel Sampling (Bilinear) + Adaptive Sampling Kernel
- [x] 6.4 Robust Homography Fit with Timing/Alignment Constraints
- [x] 6.3 Add Contour-Based Detection as a Second Detector Family
- [x] 6.6 Offline Threshold Auto-Tuning

**Implementation notes (Phase 6):**
- `6.1` implemented via `qrtool reading-rate --artifact-json`, structured JSON artifacts, dataset fingerprinting, A/B compare script (`scripts/compare_reading_rate_artifacts.py`), and CI regression gate wiring in `.github/workflows/benchmark.yml`.
- `6.2` implemented in `src/pipeline.rs` with deterministic candidate ranking, geometry confidence scoring, and `QR_DECODE_TOP_K` decode gating.
- `6.7` implemented in `src/pipeline.rs` with single-candidate-first decoding and controlled multi-candidate expansion only when confidence signals require it.
- `6.5` implemented in `src/decoder/qr_decoder/geometry.rs` with bilinear grayscale sampling plus module-size-aware adaptive sampling kernel.
- `6.4` implemented in `src/decoder/qr_decoder/geometry.rs` with timing/alignment-constrained transform refinement and quality-based transform selection.
- `6.3` implemented by adding contour fallback detector family (`src/detector/contour.rs`) and wiring it into the detection fallback flow in `src/lib.rs`.
- `6.6` implemented with offline sweep script (`scripts/tune_phase6_thresholds.py`) and checked-in profile (`docs/phase6_tuned_profile.env`).

#### 6.1 Add A/B Read-Rate Regression Harness
- **Files**: `src/bin/qrtool.rs`, `src/tools/mod.rs`, `.github/workflows/benchmark.yml`, `scripts/*` (new)
- **Deliverables**:
  - machine-readable output artifact (JSON or CSV) with run metadata and metrics
  - baseline vs candidate delta report: weighted global, per-category, runtime
  - CI regression gates (fail build on configured drop)
- **Why**: Current output is human-readable but not strict enough for regression prevention and reproducible tuning.
- **Success criteria**:
  - same dataset fingerprint required for A/B comparison
  - CI fails when weighted global drops by >1.0 percentage point
  - CI fails when median runtime regresses by >15%
- **Impact**: High (stability + faster iteration)
- **Effort**: Medium

#### 6.2 Add Candidate Scoring + Top-K Decode Gating
- **Files**: `src/lib.rs`, `src/decoder/qr_decoder.rs`
- **Deliverables**:
  - candidate score composed from geometry consistency, timing quality, alignment confidence, quiet-zone evidence
  - decode only top-K candidates per strategy (configurable)
  - telemetry: average decode attempts per image, score distributions
- **Why**: Candidate explosion still burns decode budget on weak proposals.
- **Success criteria**:
  - reduce average decode attempts/image by >=50%
  - no weighted-global regression on full-dataset run
- **Impact**: High (accuracy + speed)
- **Effort**: Medium

#### 6.7 Single-QR First, Multi-QR Expansion Second
- **Files**: `src/lib.rs`
- **Deliverables**:
  - decode strongest single candidate first
  - run multi-QR expansion only when confidence is low or multiple high-confidence groups exist
- **Why**: Most images are single-target; full multi decode by default adds noise and cost.
- **Success criteria**:
  - no drop in weighted global rate
  - median runtime within +5% vs pre-change baseline
- **Impact**: Medium
- **Effort**: Small-Medium

#### 6.5 Subpixel Sampling (Bilinear) + Adaptive Sampling Kernel
- **Files**: `src/decoder/qr_decoder.rs`
- **Deliverables**:
  - bilinear interpolation sampling path
  - kernel size derived from estimated module size (avoid fixed 3x3)
- **Why**: Rounding and fixed neighborhood cause module bleeding, especially in dense/high-version codes.
- **Success criteria**:
  - combined gain of >=5 percentage points across `high_version` + `nominal`
  - runtime regression <=10%
- **Impact**: Medium-High
- **Effort**: Medium

#### 6.4 Robust Homography Fit with Timing/Alignment Constraints
- **Files**: `src/decoder/qr_decoder.rs`, `src/utils/geometry.rs`
- **Deliverables**:
  - transform refinement using finder anchors + timing samples + alignment points
  - outlier rejection and residual score logging
- **Why**: Current transform is still brittle under perspective/curvature and drives decode failures.
- **Success criteria**:
  - combined gain of >=8 percentage points across `perspective` + `curved`
  - reduced transform-rejected attempts in telemetry
- **Impact**: High
- **Effort**: Large

#### 6.3 Add Contour-Based Detection as a Second Detector Family
- **Files**: `src/detector/contour.rs` (new), `src/detector/mod.rs`, `src/lib.rs`
- **Deliverables**:
  - contour/quadrilateral proposal path as fallback detector family
  - strict compute budget and fallback trigger rules
- **Why**: Run-length finder scanning misses some noncompliant/pathological/curved cases.
- **Success criteria**:
  - combined gain of >=5 percentage points across `noncompliant` + `pathological` + `lots`
  - median runtime regression <=15%
- **Impact**: High
- **Effort**: Large

#### 6.6 Offline Threshold Auto-Tuning
- **Files**: `scripts/*` (new), `src/lib.rs`, `src/detector/finder.rs`
- **Deliverables**:
  - script to sweep key thresholds under runtime constraints
  - checked-in tuned profile + benchmark artifact from tuning run
- **Why**: Manual threshold tuning is brittle and overfits.
- **Success criteria**:
  - reproducible tuned config from script + seed
  - >=2 percentage point weighted-global gain without breaking runtime guardrail
- **Impact**: Medium-High
- **Effort**: Medium

---

## Estimated Impact Summary

| Milestone | Reading Rate Target | Key Unlocks | Runtime Guardrail |
|-----------|---------------------|-------------|-------------------|
| Public baseline (README run `21745898128`) | 8.04% | Known starting point for comparison vs ZBar/BoofCV | Reference only |
| Phase 6.1 complete | 8.04%+ (stable) | Reproducible A/B, CI regression protection | Fail on >15% median time regression |
| Phase 6.2 + 6.7 complete | 12-18% | Better decode budget use, less candidate explosion | No weighted-global regression |
| Phase 6.5 + 6.4 complete | 18-25% | Better sampling + transform robustness (nominal/high_version/perspective/curved) | <=10% to <=15% median time regression |
| Phase 6.3 + 6.6 complete | 25%+ reliable trend | Detector diversity + tuned thresholds for difficult categories | Same guardrail enforced in CI |
| Next objective after Phase 6 | 38.95%+ | Beat ZBar on full weighted global rate | Keep fast path under 5ms target for 1MP |

## Target: Beat ZBar (38.95%) While Staying Fastest

From the public README baseline (**8.04%**), the weighted global gap to ZBar is **30.91 percentage points**. Phase 6 is focused on closing this gap with reproducible gains before additional heuristics.

**Speed constraint**: Every improvement must be evaluated against the <5ms target for 1MP images. Use tiered detection (fast path first, fallback to expensive methods) to maintain speed for common cases while handling edge cases.

---

## Recommended Implementation Order (Phase 6)

1. **Phase 6.1** - A/B regression harness + CI gates (foundation for safe iteration)
2. **Phase 6.2** - Candidate scoring + top-K decode gating
3. **Phase 6.7** - Single-QR-first decode strategy
4. **Phase 6.5** - Subpixel sampling + adaptive kernel
5. **Phase 6.4** - Robust homography fitting
6. **Phase 6.3** - Contour-based detector fallback
7. **Phase 6.6** - Offline threshold tuning and profile freeze

This order prioritizes stability first, then decode-budget efficiency, then geometric/sampling robustness, then detector diversification and final parameter tuning.

---

## Phase 7: Recovery-First Decode & Hard-Case Specialization

Phase 7 focuses on recovering near-miss decodes (where structure is mostly right but a small number of modules/geometry assumptions are wrong), and on adding dedicated fallbacks for categories that remain hard after Phase 6.

**Status:** Completed (2026-02-06)
- [x] 7.1 Soft-Decision Sampling + Erasure-Aware Reed-Solomon
- [x] 7.2 Uncertain-Module Beam Repair After Decode Failure
- [x] 7.3 Two-Finder + Timing Geometry Fallback
- [x] 7.4 Quiet-Zone Reconstruction for Noncompliant Inputs
- [x] 7.5 Piecewise Mesh Warp for Curved/Perspective Codes
- [x] 7.6 Local Radial Distortion Compensation
- [x] 7.7 Confidence-Budgeted Decode Manager
- [x] 7.8 Failure-Cluster Triage Loop (Data-Driven Tuning)

### Phase 7 Worklog Packets

Each sub-point has a dedicated work packet for sub-agent execution under `docs/worklog/`:

1. `7.1` -> `docs/worklog/phase7_01_soft_decision_rs.txt`
2. `7.2` -> `docs/worklog/phase7_02_uncertain_module_beam_repair.txt`
3. `7.3` -> `docs/worklog/phase7_03_two_finder_timing_fallback.txt`
4. `7.4` -> `docs/worklog/phase7_04_quiet_zone_reconstruction.txt`
5. `7.5` -> `docs/worklog/phase7_05_piecewise_mesh_warp.txt`
6. `7.6` -> `docs/worklog/phase7_06_radial_distortion_compensation.txt`
7. `7.7` -> `docs/worklog/phase7_07_confidence_budget_manager.txt`
8. `7.8` -> `docs/worklog/phase7_08_failure_cluster_triage.txt`

### Recommended Phase 7 Order

1. `7.1` Soft-decision + erasure RS
2. `7.4` Quiet-zone reconstruction
3. `7.3` Two-finder fallback
4. `7.7` Confidence-budgeted decode manager
5. `7.2` Uncertain-module beam repair
6. `7.5` Piecewise mesh warp
7. `7.6` Radial distortion compensation
8. `7.8` Failure-cluster triage automation

**Implementation notes (Phase 7):**
- `7.1` implemented with per-module grayscale confidence extraction, bit/codeword confidence propagation, low-confidence erasure mapping, and RS erasure fallback (`src/decoder/qr_decoder/geometry.rs`, `src/decoder/qr_decoder/payload.rs`, `src/decoder/reed_solomon.rs`).
- `7.2` implemented with bounded uncertain-module beam repair after decode failure (`src/decoder/qr_decoder/matrix_decode.rs`).
- `7.3` implemented with a strict 2-finder synthetic-third-anchor fallback path that runs only after 3-finder decode miss (`src/lib.rs`).
- `7.4` implemented with relaxed-orientation quiet-zone fallback only after strict orientation filtering fails (`src/decoder/qr_decoder/orientation.rs`, `src/decoder/qr_decoder/matrix_decode.rs`).
- `7.5` implemented with mesh-warp grayscale sampling fallback (`src/decoder/qr_decoder/geometry.rs`, `src/decoder/qr_decoder.rs`).
- `7.6` implemented with k1 radial compensation sampling fallback and activation guard from geometry distortion signal (`src/decoder/qr_decoder/geometry.rs`, `src/decoder/qr_decoder.rs`).
- `7.7` implemented with confidence-budgeted decode limits and explicit budget-skip telemetry (`src/pipeline.rs`, `src/lib.rs`).
- `7.8` implemented with failure-signature artifact enrichment and offline triage clustering script (`src/bin/qrtool.rs`, `scripts/triage_failure_clusters.py`).

---

## Phase 8: Weighted-Leverage Category Push

Phase 8 prioritizes categories that can move weighted global reading rate fastest, while preserving runtime guardrails from Phase 6/7.

**Status:** Completed (initial Phase 8 baseline on 2026-02-06)
- [x] 8.1 Region-First Multi-QR Pipeline for `lots`
- [x] 8.2 Rotation-Specialized Deskew Decode Path
- [x] 8.3 High-Version Precision Mode (v7+)
- [x] 8.4 Noncompliant/Pathological Constrained Recovery Mode
- [x] 8.5 Category-Aware Strategy Router from Telemetry
- [x] 8.6 Acceptance Calibration (False-Positive Control)
- [x] 8.7 Weighted KPI Gates and Per-Category Budgets

### Why these priorities

From current run artifacts, weighted global is dominated by a few high-volume categories:
- `lots`: 420 QR labels (largest leverage)
- `rotations`: 133
- `bright_spots`: 97
- `brightness`: 85
- `nominal`: 78
- `high_version`: high strategic value (currently zero/near-zero in many runs)

Each additional decoded QR contributes approximately `0.081` points to global weighted rate (`100 / 1232`).

### 8.1 Region-First Multi-QR Pipeline for `lots`
- **Files**: `src/lib.rs`, `src/pipeline.rs`, `src/detector/*`
- **What**:
  - spatially cluster finder/group candidates into independent regions
  - decode top-ranked candidates per region with strict dedupe (geometry + payload)
  - enforce per-region decode budget
- **Success criteria**:
  - significant lift in `lots` without exploding false positives in single-QR categories

### 8.2 Rotation-Specialized Deskew Decode Path
- **Files**: `src/decoder/qr_decoder/*.rs`, `src/lib.rs`
- **What**:
  - estimate dominant orientation from finder/timing geometry
  - deskew candidate patch before full decode
  - run only when timing confidence passes threshold
- **Success criteria**:
  - material lift in `rotations` with bounded runtime increase

### 8.3 High-Version Precision Mode (v7+)
- **Files**: `src/decoder/qr_decoder/*.rs`
- **What**:
  - v7+ dedicated path with stricter version/alignment consistency
  - denser subpixel sampling and alignment-weighted transform refinement
  - stronger rejection of inconsistent format/version combos
- **Success criteria**:
  - measurable gain in `high_version` without regressions in `nominal`

### 8.4 Noncompliant/Pathological Constrained Recovery Mode
- **Files**: `src/lib.rs`, `src/decoder/qr_decoder/*.rs`
- **What**:
  - activate only after strict path failure
  - apply relaxed quiet-zone + 2-finder + confidence-gated acceptance
  - hard cap attempts to avoid budget drain
- **Success criteria**:
  - gain in `noncompliant` and `pathological` with controlled FP rate

### 8.5 Category-Aware Strategy Router from Telemetry
- **Files**: `src/pipeline.rs`, `src/bin/qrtool.rs`
- **What**:
  - route images to specialized fallback stacks using telemetry signals:
    finder density, timing score, transform residual, confidence spread
  - avoid running all expensive paths on every image
- **Success criteria**:
  - higher weighted global with runtime guardrail maintained

### 8.6 Acceptance Calibration (False-Positive Control)
- **Files**: `src/decoder/qr_decoder/*.rs`, `src/tools/*`
- **What**:
  - add decode acceptance score combining:
    RS quality, format/version consistency, mode plausibility, content sanity
  - calibrate threshold from artifacts to reduce false positives from relaxed paths
- **Success criteria**:
  - improved precision with no net weighted-global loss

### 8.7 Weighted KPI Gates and Per-Category Budgets
- **Files**: `.github/workflows/benchmark.yml`, `scripts/*`, `src/bin/qrtool.rs`
- **What**:
  - add CI gates on weighted global and selected high-leverage categories
  - enforce per-category decode budget policy checks
  - report per-category contribution to global delta
- **Success criteria**:
  - deterministic pass/fail for Phase 8 work with clear attribution

### Recommended Phase 8 Order

1. `8.1` Region-first multi-QR (`lots`)
2. `8.2` Rotation-specialized deskew
3. `8.3` High-version precision mode
4. `8.4` Noncompliant/pathological constrained recovery
5. `8.5` Category-aware router
6. `8.6` Acceptance calibration
7. `8.7` KPI gates and contribution reporting

**Implementation notes (Phase 8 baseline):**
- `8.1` implemented via region clustering of ranked finder groups, per-region top-K decode caps, and payload/geometry dedupe in `src/pipeline.rs`.
- `8.2` implemented via bounded deskew fallback attempt path with telemetry counters (`deskew_attempts`, `deskew_successes`) in `src/decoder/qr_decoder.rs`.
- `8.3` implemented via high-version precision mode activation/counters for v7+ attempts in `src/decoder/qr_decoder.rs`.
- `8.4` implemented as fallback-only recovery attempts with explicit telemetry accounting and acceptance gating in `src/decoder/qr_decoder.rs` and `src/pipeline.rs`.
- `8.5` implemented via deterministic strategy router profiles (`fast_single`, `multi_qr_heavy`, `rotation_heavy`, `high_version_precision`, `low_contrast_recovery`) and profile telemetry in `src/pipeline.rs`.
- `8.6` implemented via acceptance scoring (RS/geometry/format-version/plausibility factors) and strict relaxed-path thresholds in `src/pipeline.rs`.
- `8.7` implemented via enhanced artifact comparison with category gates and weighted contribution reports in `scripts/compare_reading_rate_artifacts.py`, plus CI workflow wiring in `.github/workflows/benchmark.yml`.

---

## Phase 9: Hard-Case Recovery Under Strict Budget

Phase 9 focuses on improving difficult categories with bounded runtime by activating targeted recovery only after the strict path fails.

**Status:** Planned
- [ ] 9.1 Pre-binarization Ensemble (cheap-first fallback)
- [ ] 9.2 Multi-Scale Decode Schedule (1.0x -> 1.25x -> 1.5x on miss)
- [ ] 9.3 Finder Triple Re-ranking via Geometry Consistency
- [ ] 9.4 Local Contrast Normalization on Candidate ROI
- [ ] 9.5 Per-Image Decode Budget Controller (confidence lanes)
- [ ] 9.6 High-Version Subpixel Sampler + Single Refinement Pass
- [ ] 9.7 Damage-Aware Erasure Masking for RS Inputs
- [ ] 9.8 Glare/Saturation Masking for Finder and Timing Scoring
- [ ] 9.9 Category-Triggered Router v2 from Fast Image Signals
- [ ] 9.10 Failure-Signature-Driven Tuning Loop

### Phase 9 Main Points

1. Improve weak categories (`blurred`, `brightness`, `damaged`, `glare`, `bright_spots`) with category-specific recovery.
2. Keep runtime controlled by strict activation order and per-image attempt budgets.
3. Require artifact-backed wins before merging broad heuristic changes.
4. Use fast workflow for iteration and full workflow for final validation only.

### 9.1 Pre-binarization Ensemble (cheap-first fallback)
- **Files**: `src/tools/mod.rs`, `src/lib.rs`, `src/pipeline.rs`
- **What**:
  - run Otsu first, then two adaptive variants only on miss
  - stop on first successful decode
- **Success criteria**:
  - +3pp combined on `brightness` + `shadows` with <=8% median runtime regression

### 9.2 Multi-Scale Decode Schedule
- **Files**: `src/decoder/qr_decoder/geometry.rs`, `src/decoder/qr_decoder.rs`
- **What**:
  - retry failed candidates at 1.25x and 1.5x sampling scale
  - run scale retries only when module pitch/confidence indicates under-sampling
- **Success criteria**:
  - +3pp combined on `close` + `blurred` with bounded extra attempts

### 9.3 Finder Triple Re-ranking via Geometry Consistency
- **Files**: `src/detector/grouping.rs`, `src/pipeline.rs`
- **What**:
  - add timing-line agreement and module-pitch consistency to triple scoring
  - prioritize geometrically coherent triples before decode
- **Success criteria**:
  - reduced wrong-transform attempts and +2pp on `glare` + `bright_spots`

### 9.4 Local Contrast Normalization on Candidate ROI
- **Files**: `src/utils/grayscale.rs`, `src/decoder/qr_decoder/geometry.rs`
- **What**:
  - apply local normalization only to candidate ROI fallback path
  - avoid whole-image expensive enhancement
- **Success criteria**:
  - +2pp on `brightness`/`shadows` without global runtime penalty

### 9.5 Per-Image Decode Budget Controller
- **Files**: `src/pipeline.rs`, `src/lib.rs`
- **What**:
  - allocate attempts by confidence lanes: high, medium, low
  - cap total per-image attempts with explicit skip telemetry
- **Success criteria**:
  - no runtime blowups; median runtime regression <=8% while maintaining gains

### 9.6 High-Version Subpixel Sampler + Single Refinement Pass
- **Files**: `src/decoder/qr_decoder/geometry.rs`, `src/decoder/qr_decoder.rs`
- **What**:
  - use bilinear/subpixel sampler for high-version candidates
  - run one bounded transform refinement pass
- **Success criteria**:
  - +3pp on `high_version` with no regression in `nominal`

### 9.7 Damage-Aware Erasure Masking for RS Inputs
- **Files**: `src/decoder/qr_decoder/payload.rs`, `src/decoder/reed_solomon.rs`
- **What**:
  - mark uncertain modules/codewords near damage patterns as erasures
  - route to erasure-capable RS decode path
- **Success criteria**:
  - +3pp on `damaged` with controlled false-positive rate

### 9.8 Glare/Saturation Masking for Finder and Timing Scoring
- **Files**: `src/detector/finder.rs`, `src/detector/timing.rs`, `src/pipeline.rs`
- **What**:
  - suppress saturated blobs from dominating finder/timing confidence
  - run mask-aware scoring only when saturation ratio is high
- **Success criteria**:
  - +3pp combined on `glare` + `bright_spots`

### 9.9 Category-Triggered Router v2 from Fast Image Signals
- **Files**: `src/pipeline.rs`, `src/bin/qrtool.rs`
- **What**:
  - classify image quickly (blur metric, saturation %, skew estimate, density proxy)
  - dispatch to minimal recovery stack per profile
- **Success criteria**:
  - improved weighted global with same or better median runtime

### 9.10 Failure-Signature-Driven Tuning Loop
- **Files**: `scripts/triage_failure_clusters.py`, `scripts/*` (new), `src/bin/qrtool.rs`
- **What**:
  - select top weighted failure signatures from artifacts
  - tune only targeted knobs and compare against baseline artifact
- **Success criteria**:
  - each merged tuning change maps to a signature-level improvement in artifact diff

### Phase 9 Worklog Packets

1. `9.1` -> `docs/worklog/phase9_01_pre_binarization_ensemble.txt`
2. `9.2` -> `docs/worklog/phase9_02_multi_scale_decode_schedule.txt`
3. `9.3` -> `docs/worklog/phase9_03_finder_rerank_geometry.txt`
4. `9.4` -> `docs/worklog/phase9_04_local_contrast_roi_normalization.txt`
5. `9.5` -> `docs/worklog/phase9_05_per_image_budget_controller.txt`
6. `9.6` -> `docs/worklog/phase9_06_high_version_subpixel_refinement.txt`
7. `9.7` -> `docs/worklog/phase9_07_damage_aware_erasure_masking.txt`
8. `9.8` -> `docs/worklog/phase9_08_glare_saturation_masking.txt`
9. `9.9` -> `docs/worklog/phase9_09_router_v2_fast_signals.txt`
10. `9.10` -> `docs/worklog/phase9_10_failure_signature_tuning_loop.txt`

### Recommended Phase 9 Order

1. `9.5` Per-image budget controller
2. `9.1` Pre-binarization ensemble
3. `9.3` Finder triple re-ranking
4. `9.8` Glare/saturation masking
5. `9.4` Local contrast normalization
6. `9.2` Multi-scale decode schedule
7. `9.6` High-version subpixel refinement
8. `9.7` Damage-aware erasure masking
9. `9.9` Router v2 fast signals
10. `9.10` Failure-signature-driven tuning loop
