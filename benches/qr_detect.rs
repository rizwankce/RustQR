use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rust_qr::{detect, detect_from_grayscale};

fn bench_detect_small(c: &mut Criterion) {
    let image = vec![128u8; 100 * 100 * 3];
    c.bench_function("detect_100x100_rgb", |b| {
        b.iter(|| detect(black_box(&image), black_box(100), black_box(100)))
    });
}

fn bench_detect_medium(c: &mut Criterion) {
    let image = vec![128u8; 640 * 480 * 3];
    c.bench_function("detect_640x480_rgb", |b| {
        b.iter(|| detect(black_box(&image), black_box(640), black_box(480)))
    });
}

fn bench_detect_large(c: &mut Criterion) {
    let image = vec![128u8; 1920 * 1080 * 3];
    c.bench_function("detect_1920x1080_rgb", |b| {
        b.iter(|| detect(black_box(&image), black_box(1920), black_box(1080)))
    });
}

fn bench_detect_grayscale(c: &mut Criterion) {
    let image = vec![128u8; 640 * 480];
    c.bench_function("detect_640x480_grayscale", |b| {
        b.iter(|| detect_from_grayscale(black_box(&image), black_box(640), black_box(480)))
    });
}

// Connected components detection benchmarks
fn bench_detect_cc_medium(c: &mut Criterion) {
    use rust_qr::detector::finder::FinderDetector;
    use rust_qr::utils::binarization::otsu_binarize;
    use rust_qr::utils::grayscale::rgb_to_grayscale;

    let image = vec![128u8; 640 * 480 * 3];
    let gray = rgb_to_grayscale(&image, 640, 480);
    let binary = otsu_binarize(&gray, 640, 480);

    c.bench_function("detect_cc_640x480", |b| {
        b.iter(|| FinderDetector::detect_with_connected_components(black_box(&binary)))
    });
}

fn bench_detect_cc_large(c: &mut Criterion) {
    use rust_qr::detector::finder::FinderDetector;
    use rust_qr::utils::binarization::otsu_binarize;
    use rust_qr::utils::grayscale::rgb_to_grayscale;

    let image = vec![128u8; 1920 * 1080 * 3];
    let gray = rgb_to_grayscale(&image, 1920, 1080);
    let binary = otsu_binarize(&gray, 1920, 1080);

    c.bench_function("detect_cc_1920x1080", |b| {
        b.iter(|| FinderDetector::detect_with_connected_components(black_box(&binary)))
    });
}

criterion_group!(
    benches,
    bench_detect_small,
    bench_detect_medium,
    bench_detect_large,
    bench_detect_grayscale,
    bench_detect_cc_medium,
    bench_detect_cc_large
);
criterion_main!(benches);
