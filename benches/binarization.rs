use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rust_qr::utils::binarization::{adaptive_binarize, otsu_binarize, threshold_binarize};

fn bench_otsu_binarize_small(c: &mut Criterion) {
    let gray = vec![128u8; 100 * 100];
    c.bench_function("otsu_binarize_100x100", |b| {
        b.iter(|| otsu_binarize(black_box(&gray), black_box(100), black_box(100)))
    });
}

fn bench_otsu_binarize_medium(c: &mut Criterion) {
    let gray = vec![128u8; 640 * 480];
    c.bench_function("otsu_binarize_640x480", |b| {
        b.iter(|| otsu_binarize(black_box(&gray), black_box(640), black_box(480)))
    });
}

fn bench_otsu_binarize_large(c: &mut Criterion) {
    let gray = vec![128u8; 1920 * 1080];
    c.bench_function("otsu_binarize_1920x1080", |b| {
        b.iter(|| otsu_binarize(black_box(&gray), black_box(1920), black_box(1080)))
    });
}

fn bench_adaptive_binarize_medium(c: &mut Criterion) {
    let gray = vec![128u8; 640 * 480];
    c.bench_function("adaptive_binarize_640x480", |b| {
        b.iter(|| {
            adaptive_binarize(
                black_box(&gray),
                black_box(640),
                black_box(480),
                black_box(15),
            )
        })
    });
}

fn bench_threshold_binarize_medium(c: &mut Criterion) {
    let gray = vec![128u8; 640 * 480];
    c.bench_function("threshold_binarize_640x480", |b| {
        b.iter(|| {
            threshold_binarize(
                black_box(&gray),
                black_box(640),
                black_box(480),
                black_box(128),
            )
        })
    });
}

criterion_group!(
    benches,
    bench_otsu_binarize_small,
    bench_otsu_binarize_medium,
    bench_otsu_binarize_large,
    bench_adaptive_binarize_medium,
    bench_threshold_binarize_medium
);
criterion_main!(benches);
