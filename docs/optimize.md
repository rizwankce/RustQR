# ScanRust Optimization Analysis

## Executive Summary

ScanRust is a pure Rust QR code scanning library targeting **<5ms detection for 1MP images** to beat industry leaders like BoofCV (~15-20ms) and ZBar (~10-15ms).

**Current Gap:** ~9x speedup needed for complex images
- Synthetic 640x480: ~1.9ms ✓ (Target met)
- Real QR images: 8-45ms (Needs optimization)

## Current Performance Baseline

### Synthetic Benchmarks
| Image Size | Time | Notes |
|------------|------|-------|
| 100x100 RGB | ~114 µs | Very fast, minimal processing |
| 640x480 RGB | ~1.9 ms | Target size, acceptable |
| 1920x1080 RGB | ~12.5 ms | Large images, needs work |
| 640x480 Grayscale | ~1.5 ms | RGB conversion overhead: ~0.4ms |

### Real QR Image Benchmarks (BoofCV Dataset)
| Image | Time | Relative |
|-------|------|----------|
| image002.jpg | 8.4 ms | Fastest - simple QR |
| image017.jpg | 10.8 ms | Good |
| image016.jpg | 12.7 ms | Moderate |
| image009.jpg | 42.4 ms | Slow - complex scene |
| image008.jpg | 44.8 ms | Slowest - multiple QRs? |

**Analysis:** Performance varies 5x based on image complexity. Larger images and scenes with multiple QR codes are significantly slower.

## Architecture Bottlenecks

### 1. Memory Access Patterns

**Current Issues:**
- Cache-unfriendly data structures
- Multiple passes over image data
- BitMatrix uses 1 byte per 8 bits (good) but random access patterns

**Impact:** Cache misses dominate runtime on large images

**Solutions:**
- Use SoA (Structure of Arrays) instead of AoS
- Process image in tiles to fit in L1/L2 cache
- Pre-allocate buffers and reuse them

### 2. Algorithm Hotspots

Based on typical QR detection flow, expected hotspots:

**A. Grayscale Conversion (10-15% of time)**
- Current: Integer multiply and shift per pixel
- Opportunity: SIMD batch processing (8-16 pixels at once)

**B. Binarization - Otsu's Method (15-20% of time)**
- Current: Histogram building + threshold calculation
- Opportunity: Parallel histogram reduction

**C. Finder Pattern Detection (30-40% of time)**
- Current: Row-by-row scanning with state machine
- Opportunity: Integral images for fast black/white counting

**D. Perspective Transform (20-30% of time)**
- Current: Per-pixel floating point calculations
- Opportunity: Fixed-point arithmetic + SIMD

### 3. Parallelization Potential

**Embarrassingly Parallel:**
- Grayscale conversion (rows independent)
- Finder pattern detection (rows independent)
- Multiple QR code decoding (QRs independent)

**Synchronization Needed:**
- Merging finder pattern candidates
- Final QR code results collection

## Optimization Strategies

### Phase 1: Low-Hanging Fruit (2-3x speedup)

#### 1.1 SIMD Grayscale Conversion
**Target:** 100x100 RGB: 114µs → ~30µs

**Implementation:**
- Process 16 pixels at once with SIMD
- Load 16 RGB pixels (48 bytes) into registers
- Extract R, G, B channels
- Multiply: R*76, G*150, B*29
- Sum and shift right by 8
- Store 16 grayscale pixels

**Complexity:** Low
**Expected Gain:** 3-4x on grayscale (10-15% total speedup)

#### 1.2 Integral Images for Binarization
**Target:** Faster Otsu threshold + adaptive thresholding

Integral images allow O(1) box sum queries:
- Build integral image in one pass: O(n)
- Local threshold queries: O(1) instead of O(k)

**Expected Gain:** 2x on binarization (10% total speedup)

#### 1.3 Early Termination Strategies
**Finder Pattern Detection:**
- Skip rows with low variance (no edges)
- Stop scanning row after finding N patterns
- Quick ratio check before full validation

**Expected Gain:** 2-3x on simple images with sparse QRs

### Phase 2: Algorithmic Improvements (2-3x speedup)

#### 2.1 Optimized Finder Pattern Detection
**Current:** Scan every row, track runs, check ratios

**Optimized Options:**
1. **Pyramid Detection:**
   - Start with 4x downscaled image
   - Find approximate locations quickly
   - Refine at full resolution only near candidates

2. **Connected Components:**
   - Find all black regions first
   - Check only regions that could be finder patterns
   - Reduces checks from O(n²) to O(k)

**Expected Gain:** 3-5x on large images

#### 2.2 Fixed-Point Perspective Transform
**Current:** Floating point per pixel
**Optimized:** Fixed-point (16.16 format)
- No FPU usage (faster on some architectures)
- Deterministic results
- Easier SIMD implementation

**Expected Gain:** 1.5-2x on transform

#### 2.3 Memory Pool / Arena Allocation
**Current:** Vec allocations throughout
**Optimized:** 
- Pre-allocate buffers at startup
- Use arena allocator for temporary structures
- Reduce malloc/free overhead

**Expected Gain:** 10-20% overall

### Phase 3: Advanced Optimizations (2-4x speedup)

#### 3.1 Parallel Processing

**Multi-threading Opportunities:**
1. **Image tiling:** Split image into N tiles, process in parallel
2. **Pipeline parallelism:** 
   - Thread 1: Grayscale + binarization
   - Thread 2: Finder detection
   - Thread 3: QR decoding
3. **QR-level parallelism:** Decode multiple QRs simultaneously

**Implementation Options:**
- Rayon for data parallelism
- Crossbeam for channels/pipelines
- std::thread for manual control

**Trade-offs:**
- Overhead for small images (<100ms)
- Worth it for 1MP+ images and multiple QRs
- Memory bandwidth limit on high core counts

**Expected Gain:** 2-4x on multi-core systems (4+ cores)

#### 3.2 GPU Acceleration (Future)

**Suitable for:**
- Massive batch processing (100+ images)
- Real-time video processing
- Very large images (4K+)

**Not Worth It:**
- Single image processing (transfer overhead)
- Simple detection scenarios

**Libraries:**
- wgpu (cross-platform WebGPU)
- Vulkan compute shaders
- Metal Performance Shaders (Apple)
- CUDA (NVIDIA only)

#### 3.3 Profile-Guided Optimization (PGO)

**Build Process:**
1. Compile with instrumentation
2. Run benchmark suite (real workloads)
3. Recompile using profile data
4. Better branch prediction, inlining decisions

**Expected Gain:** 10-20% overall

**Cargo Command:**
```bash
cargo pgo run bench
cargo pgo optimize
```

## Component-Specific Optimizations

### BitMatrix

**Current:** 
- 1 byte per 8 bits (good compression)
- Random access patterns

**Optimized:**
- Use SIMD to check 64 bits at once (_mm_movemask_epi8)
- Align to cache lines (64 bytes)
- Store in tiles for locality
- Use popcount for fast bit counting

### Reed-Solomon Decoder

**Current:**
- GF(256) with log/exp tables
- Berlekamp-Massey algorithm

**Optimized:**
- SIMD for syndrome calculation (process multiple syndromes at once)
- Lookup tables in L1 cache (keep hot)
- Specialized versions for common ECC levels (L, M, Q, H)
- Early exit when no errors detected

### Perspective Transform

**Current:**
- Per-pixel floating point
- Matrix multiplication per sample

**Optimized:**
- Fixed-point 16.16 arithmetic
- Pre-compute inverse transform (map image to QR, not QR to image)
- Bilinear interpolation in fixed-point
- SIMD for 4 pixels at once

### Finder Pattern Detection

**Current:**
- Row-by-row state machine
- O(n²) complexity for n×n image

**Optimized:**
- Integral image for fast ratio checking
- Skip uniform rows (variance check)
- Spatial indexing for found patterns
- Connected components for large black regions

## Recommended Implementation Order

### Week 1: Quick Wins (Target: 2x speedup)
1. **SIMD Grayscale** - Easy, 10-15% gain
2. **Integral Images** - Medium, 10% gain  
3. **Early Termination** - Easy, variable gain
4. **Memory Pools** - Medium, 10-20% gain

### Week 2: Algorithm Improvements (Target: 2-3x speedup)
1. **Finder Pattern Pyramid** - Hard, 3-5x on large images
2. **Fixed-Point Transform** - Medium, 1.5-2x
3. **Connected Components** - Hard, 2-3x

### Week 3: Parallelization (Target: 2-4x speedup)
1. **Image Tiling** - Medium, scales with cores
2. **Pipeline Parallelism** - Medium, better throughput
3. **Rayon Integration** - Easy, drop-in parallelism

### Week 4: Advanced (Target: 1.5-2x speedup)
1. **SIMD RS Decoder** - Hard, specialized
2. **PGO** - Easy, compile-time optimization
3. **Profile and Iterate** - Ongoing

## Expected Total Speedup

**Cumulative:** 2×2.5×3×1.5 = **22.5x theoretical**

**Realistic:** Accounting for Amdahl's Law and overlap: **6-10x**

**Target Achievement:** 
- Current: 44ms (worst case)
- Optimized: 4.4-7.3ms ✓ **Meets <5ms target**

## Key Success Metrics

1. **640x480 RGB: <2ms** (currently 1.9ms, maintain or improve)
2. **1MP images: <5ms** (currently ~12-45ms, need 3-9x)
3. **Batch processing:** Scale linearly with cores
4. **Memory usage:** <50MB peak

## Risk Factors

1. **SIMD portability** - x86_64 vs ARM64 differences
2. **Parallel overhead** - Small images might be slower
3. **Complexity increase** - Harder to maintain
4. **Compile times** - PGO adds build steps

## Conclusion

The path to "world's fastest" is clear:
1. SIMD for pixel operations (grayscale, binarization)
2. Algorithm improvements (finder patterns, connected components)
3. Parallel processing (multi-core utilization)
4. Profile-guided optimization (compiler magic)

**With focused effort over 4 weeks, achieving <5ms for 1MP images is realistic.**
