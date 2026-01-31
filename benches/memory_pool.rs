use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rust_qr::utils::memory_pool::BufferPool;
use rust_qr::{detect, detect_with_pool};

fn bench_detect_without_pool(c: &mut Criterion) {
    let image = vec![128u8; 640 * 480 * 3];
    c.bench_function("detect_640x480_no_pool", |b| {
        b.iter(|| detect(black_box(&image), black_box(640), black_box(480)))
    });
}

fn bench_detect_with_pool(c: &mut Criterion) {
    let image = vec![128u8; 640 * 480 * 3];
    let mut pool = BufferPool::with_capacity(640 * 480);
    c.bench_function("detect_640x480_with_pool", |b| {
        b.iter(|| {
            detect_with_pool(
                black_box(&image),
                black_box(640),
                black_box(480),
                black_box(&mut pool),
            )
        })
    });
}

fn bench_detect_pool_small(c: &mut Criterion) {
    let image = vec![128u8; 100 * 100 * 3];
    let mut pool = BufferPool::with_capacity(100 * 100);
    c.bench_function("detect_100x100_with_pool", |b| {
        b.iter(|| {
            detect_with_pool(
                black_box(&image),
                black_box(100),
                black_box(100),
                black_box(&mut pool),
            )
        })
    });
}

fn bench_detect_pool_large(c: &mut Criterion) {
    let image = vec![128u8; 1920 * 1080 * 3];
    let mut pool = BufferPool::with_capacity(1920 * 1080);
    c.bench_function("detect_1920x1080_with_pool", |b| {
        b.iter(|| {
            detect_with_pool(
                black_box(&image),
                black_box(1920),
                black_box(1080),
                black_box(&mut pool),
            )
        })
    });
}

criterion_group!(
    benches,
    bench_detect_without_pool,
    bench_detect_with_pool,
    bench_detect_pool_small,
    bench_detect_pool_large
);
criterion_main!(benches);
