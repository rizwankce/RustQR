# Profile-Guided Optimization (PGO) Guide

## Overview

Profile-Guided Optimization (PGO) uses runtime profiling data to make better optimization decisions:
- **Better branch prediction** - Hot/cold path separation
- **Smarter inlining** - Frequently called functions inlined
- **Improved cache layout** - Code reordered based on execution patterns
- **Expected gain:** 10-20% performance improvement

## Requirements

- LLVM/Clang with PGO support (included in modern Rust toolchains)
- `cargo-pgo` tool: `cargo install cargo-pgo`

## Usage

### Step 1: Build with Instrumentation

```bash
cargo pgo run --release
```

This:
- Compiles with profiling instrumentation
- Runs tests/benchmarks to collect profile data
- Saves `.profraw` files to `target/pgo-profiles/`

### Step 2: Build Optimized Version

```bash
cargo pgo optimize
```

This:
- Merges all `.profraw` files into a single `.profdata` file
- Recompiles using the profile data for optimization
- Creates optimized binary/library in `target/pgo-profiles/`

## Results for RustQR

**Profile Data Collected:**
- 2 profile files (47.34 KiB total)
- Merged into: `merged-*.profdata`
- Coverage: Library tests and benchmarks

**Optimized Library:**
- Location: `target/aarch64-apple-darwin/release/librust_qr.rlib`
- Size: 436 KB
- Built with: Rust 1.85.0-nightly, PGO optimizations enabled

## Using PGO in Production

### For Applications Using RustQR

If you're building an application that uses RustQR, you can apply PGO to your entire application:

```bash
# In your application directory
cargo pgo run --release
cargo pgo optimize
```

This will optimize your binary including all dependencies (RustQR included).

### For Library Distribution

PGO-optimized libraries provide best performance when:
1. The library is used in similar workloads as the profiling run
2. The final binary is also compiled with LTO (Link Time Optimization)

## Expected Performance Impact

Based on typical PGO results for similar workloads:
- **Branch prediction:** 5-10% improvement
- **Inlining decisions:** 3-7% improvement  
- **Cache locality:** 2-5% improvement
- **Total expected:** 10-20% improvement

**Note:** Actual improvement depends on workload characteristics and code patterns.

## Limitations

1. **Profile-specific:** Optimizations are based on the profiled workload
2. **Platform-specific:** Profile data is architecture-specific (ARM64, x86_64)
3. **Library crates:** Benefits most visible in final binaries, not standalone benchmarks

## Further Reading

- [LLVM PGO Documentation](https://llvm.org/docs/HowToBuildWithPGO.html)
- [cargo-pgo README](https://github.com/Kobzol/cargo-pgo)
- [Rust PGO RFC](https://rust-lang.github.io/rfcs/2952-cargo-profile-guides.html)
