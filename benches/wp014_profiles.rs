//! WP-014's allocation and in-process latency probe.
//!
//! This is deliberately a measurement harness rather than an optimization.  It
//! loads three fixed real-image lanes once, then measures only calls to the
//! public `detect` API.  `WP014_ALLOCATION_ONLY=1` emits machine-readable
//! records without running Criterion, which is what the collection script
//! uses for a reproducible baseline.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rust_qr::tools::load_rgb;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

struct CountingAllocator;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static DEALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static REALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
static DEALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        // SAFETY: Delegating to the platform allocator with the layout passed
        // to this allocator preserves `GlobalAlloc`'s contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        DEALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        DEALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        // SAFETY: `ptr` and `layout` originated from this allocator.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        REALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        DEALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        // SAFETY: `ptr` and `layout` originated from this allocator.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[derive(Clone, Copy)]
struct AllocationSnapshot {
    allocations: u64,
    deallocations: u64,
    reallocations: u64,
    allocated_bytes: u64,
    deallocated_bytes: u64,
}

fn reset_allocation_counters() {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    DEALLOCATIONS.store(0, Ordering::Relaxed);
    REALLOCATIONS.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    DEALLOCATED_BYTES.store(0, Ordering::Relaxed);
}

fn allocation_snapshot() -> AllocationSnapshot {
    AllocationSnapshot {
        allocations: ALLOCATIONS.load(Ordering::Relaxed),
        deallocations: DEALLOCATIONS.load(Ordering::Relaxed),
        reallocations: REALLOCATIONS.load(Ordering::Relaxed),
        allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        deallocated_bytes: DEALLOCATED_BYTES.load(Ordering::Relaxed),
    }
}

struct ProfileCase {
    lane: &'static str,
    path: &'static str,
    requires_success: bool,
}

const CASES: [ProfileCase; 3] = [
    ProfileCase {
        lane: "clean",
        path: "benches/images/boofcv/nominal/image005.jpg",
        requires_success: true,
    },
    ProfileCase {
        lane: "hard",
        path: "benches/images/boofcv/damaged/image002.jpg",
        requires_success: true,
    },
    // This lane is intentionally retained even though the current revision
    // does not decode it. WP-012 must establish a successful dense-scene case
    // before WP-014 may use its results for a performance claim.
    ProfileCase {
        lane: "multi_candidate",
        path: "benches/images/boofcv/lots/image001.jpg",
        requires_success: false,
    },
];

struct LoadedCase {
    case: ProfileCase,
    pixels: Vec<u8>,
    width: usize,
    height: usize,
}

fn load_case(case: ProfileCase) -> LoadedCase {
    let (pixels, width, height) =
        load_rgb(case.path).unwrap_or_else(|error| panic!("{}: {error}", case.path));
    LoadedCase {
        case,
        pixels,
        width,
        height,
    }
}

fn measure_once(case: &LoadedCase) -> (Duration, AllocationSnapshot, usize) {
    reset_allocation_counters();
    let start = Instant::now();
    let decoded = rust_qr::detect(
        black_box(&case.pixels),
        black_box(case.width),
        black_box(case.height),
    );
    let elapsed = start.elapsed();
    let decoded_count = decoded.len();
    black_box(decoded);
    (elapsed, allocation_snapshot(), decoded_count)
}

fn emit_probe(case: &LoadedCase, iterations: usize) {
    let (first_elapsed, first_allocations, first_decoded) = measure_once(case);
    if case.case.requires_success {
        assert!(first_decoded > 0, "{} must decode", case.case.path);
    }

    reset_allocation_counters();
    let warm_start = Instant::now();
    let mut decoded_total = 0usize;
    for _ in 0..iterations {
        let decoded = rust_qr::detect(
            black_box(&case.pixels),
            black_box(case.width),
            black_box(case.height),
        );
        decoded_total += decoded.len();
        black_box(decoded);
    }
    let warm_elapsed = warm_start.elapsed();
    let warm_allocations = allocation_snapshot();
    let warm_per_call_ns = warm_elapsed.as_nanos() as f64 / iterations as f64;

    println!(
        "WP014_PROFILE {{\"lane\":\"{}\",\"image\":\"{}\",\"requires_success\":{},\"first_call_ns\":{},\"first_call_decoded\":{},\"first_call_allocations\":{},\"first_call_reallocations\":{},\"first_call_allocated_bytes\":{},\"warm_iterations\":{},\"warm_total_ns\":{},\"warm_per_call_ns\":{:.3},\"warm_decoded_total\":{},\"warm_allocations\":{},\"warm_deallocations\":{},\"warm_reallocations\":{},\"warm_allocated_bytes\":{},\"warm_deallocated_bytes\":{}}}",
        case.case.lane,
        case.case.path,
        case.case.requires_success,
        first_elapsed.as_nanos(),
        first_decoded,
        first_allocations.allocations,
        first_allocations.reallocations,
        first_allocations.allocated_bytes,
        iterations,
        warm_elapsed.as_nanos(),
        warm_per_call_ns,
        decoded_total,
        warm_allocations.allocations,
        warm_allocations.deallocations,
        warm_allocations.reallocations,
        warm_allocations.allocated_bytes,
        warm_allocations.deallocated_bytes,
    );
}

fn bench_profiles(c: &mut Criterion) {
    let cases: Vec<_> = CASES.into_iter().map(load_case).collect();
    if std::env::var_os("WP014_ALLOCATION_ONLY").is_some() {
        let iterations = std::env::var("WP014_PROFILE_ITERATIONS")
            .ok()
            .and_then(|value| value.parse().ok())
            .filter(|value: &usize| *value > 0)
            .unwrap_or(5);
        for case in &cases {
            emit_probe(case, iterations);
        }
        return;
    }

    for case in &cases {
        let benchmark_name = format!("wp014/detect/{}", case.case.lane);
        c.bench_function(&benchmark_name, |b| {
            b.iter(|| {
                rust_qr::detect(
                    black_box(&case.pixels),
                    black_box(case.width),
                    black_box(case.height),
                )
            })
        });
    }
}

criterion_group!(benches, bench_profiles);
criterion_main!(benches);
