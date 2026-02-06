use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use rust_qr::detector::finder::FinderDetector;
use rust_qr::tools::{binarize_otsu, load_rgb, to_grayscale};
mod common;

/// Benchmark connected components detection on real QR code images
fn bench_real_qr_connected_components(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_qr_connected_components");

    let (image_root, images) = common::collect_dataset_images();
    if !image_root.exists() {
        println!("Warning: No test images found at {:?}", image_root);
        return;
    }

    if images.is_empty() {
        println!("Warning: No test images found under {:?}", image_root);
        return;
    }

    for path in images {
        let img_name = path
            .strip_prefix(&image_root)
            .unwrap_or(&path)
            .to_string_lossy();

        // Load image
        let (raw_pixels, width, height) = match load_rgb(&path) {
            Ok(result) => result,
            Err(err) => {
                println!("Warning: Failed to load {}: {}", path.display(), err);
                continue;
            }
        };

        // Preprocess: convert to grayscale and binarize
        let gray = to_grayscale(&raw_pixels, width, height);
        let binary = binarize_otsu(&gray, width, height);

        // Benchmark regular detection
        group.bench_with_input(
            BenchmarkId::new("regular_detect", &img_name),
            &binary,
            |b, binary| {
                b.iter(|| FinderDetector::detect(black_box(binary)));
            },
        );

        // Benchmark connected components detection
        group.bench_with_input(
            BenchmarkId::new("connected_components", &img_name),
            &binary,
            |b, binary| {
                b.iter(|| FinderDetector::detect_with_connected_components(black_box(binary)));
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_real_qr_connected_components);
criterion_main!(benches);
