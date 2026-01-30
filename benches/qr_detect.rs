use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rust_qr::{detect, detect_from_grayscale};

fn bench_detect_small(c: &mut Criterion) {
    // Create a 100x100 RGB image
    let image = vec![128u8; 100 * 100 * 3];

    c.bench_function("detect_100x100_rgb", |b| {
        b.iter(|| detect(black_box(&image), black_box(100), black_box(100)))
    });
}

fn bench_detect_medium(c: &mut Criterion) {
    // Create a 640x480 RGB image
    let image = vec![128u8; 640 * 480 * 3];

    c.bench_function("detect_640x480_rgb", |b| {
        b.iter(|| detect(black_box(&image), black_box(640), black_box(480)))
    });
}

fn bench_detect_large(c: &mut Criterion) {
    // Create a 1920x1080 RGB image
    let image = vec![128u8; 1920 * 1080 * 3];

    c.bench_function("detect_1920x1080_rgb", |b| {
        b.iter(|| detect(black_box(&image), black_box(1920), black_box(1080)))
    });
}

fn bench_detect_grayscale(c: &mut Criterion) {
    // Create a 640x480 grayscale image
    let image = vec![128u8; 640 * 480];

    c.bench_function("detect_640x480_grayscale", |b| {
        b.iter(|| detect_from_grayscale(black_box(&image), black_box(640), black_box(480)))
    });
}

criterion_group!(
    benches,
    bench_detect_small,
    bench_detect_medium,
    bench_detect_large,
    bench_detect_grayscale
);
criterion_main!(benches);
