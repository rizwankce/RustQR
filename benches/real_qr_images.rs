use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use rust_qr::tools::load_rgb;
mod common;

/// Benchmark real QR code images
fn bench_real_qr_images(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_qr_images");

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

        // Benchmark this image
        group.bench_with_input(
            BenchmarkId::new("detect", &img_name),
            &(&raw_pixels, width, height),
            |b, (pixels, w, h)| {
                b.iter(|| rust_qr::detect(black_box(pixels), black_box(*w), black_box(*h)));
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_real_qr_images);
criterion_main!(benches);
