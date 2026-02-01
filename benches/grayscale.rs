use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rust_qr::utils::grayscale::{
    rgb_to_grayscale, rgb_to_grayscale_parallel, rgba_to_grayscale, rgba_to_grayscale_parallel,
};

fn bench_rgb_to_grayscale_small(c: &mut Criterion) {
    let image = vec![128u8; 100 * 100 * 3];
    c.bench_function("rgb_to_grayscale_100x100", |b| {
        b.iter(|| rgb_to_grayscale(black_box(&image), black_box(100), black_box(100)))
    });
}

fn bench_rgb_to_grayscale_medium(c: &mut Criterion) {
    let image = vec![128u8; 640 * 480 * 3];
    c.bench_function("rgb_to_grayscale_640x480", |b| {
        b.iter(|| rgb_to_grayscale(black_box(&image), black_box(640), black_box(480)))
    });
}

fn bench_rgb_to_grayscale_large(c: &mut Criterion) {
    let image = vec![128u8; 1920 * 1080 * 3];
    c.bench_function("rgb_to_grayscale_1920x1080", |b| {
        b.iter(|| rgb_to_grayscale(black_box(&image), black_box(1920), black_box(1080)))
    });
}

fn bench_rgba_to_grayscale_medium(c: &mut Criterion) {
    let image = vec![128u8; 640 * 480 * 4];
    c.bench_function("rgba_to_grayscale_640x480", |b| {
        b.iter(|| rgba_to_grayscale(black_box(&image), black_box(640), black_box(480)))
    });
}

criterion_group!(
    benches,
    bench_rgb_to_grayscale_small,
    bench_rgb_to_grayscale_medium,
    bench_rgb_to_grayscale_large,
    bench_rgba_to_grayscale_medium,
    bench_rgb_to_grayscale_parallel_small,
    bench_rgb_to_grayscale_parallel_medium,
    bench_rgb_to_grayscale_parallel_large,
    bench_rgba_to_grayscale_parallel_medium
);
criterion_main!(benches);

fn bench_rgb_to_grayscale_parallel_small(c: &mut Criterion) {
    let image = vec![128u8; 100 * 100 * 3];
    c.bench_function("rgb_to_grayscale_parallel_100x100", |b| {
        b.iter(|| rgb_to_grayscale_parallel(black_box(&image), black_box(100), black_box(100)))
    });
}

fn bench_rgb_to_grayscale_parallel_medium(c: &mut Criterion) {
    let image = vec![128u8; 640 * 480 * 3];
    c.bench_function("rgb_to_grayscale_parallel_640x480", |b| {
        b.iter(|| rgb_to_grayscale_parallel(black_box(&image), black_box(640), black_box(480)))
    });
}

fn bench_rgb_to_grayscale_parallel_large(c: &mut Criterion) {
    let image = vec![128u8; 1920 * 1080 * 3];
    c.bench_function("rgb_to_grayscale_parallel_1920x1080", |b| {
        b.iter(|| rgb_to_grayscale_parallel(black_box(&image), black_box(1920), black_box(1080)))
    });
}

fn bench_rgba_to_grayscale_parallel_medium(c: &mut Criterion) {
    let image = vec![128u8; 640 * 480 * 4];
    c.bench_function("rgba_to_grayscale_parallel_640x480", |b| {
        b.iter(|| rgba_to_grayscale_parallel(black_box(&image), black_box(640), black_box(480)))
    });
}
