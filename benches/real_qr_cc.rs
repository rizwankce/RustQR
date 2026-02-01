use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rust_qr::detector::finder::FinderDetector;
use rust_qr::utils::binarization::otsu_binarize;
use rust_qr::utils::grayscale::rgb_to_grayscale;
use std::fs;
use std::path::Path;

/// Benchmark connected components detection on real QR code images
fn bench_real_qr_connected_components(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_qr_connected_components");

    // Find all test images
    let image_dir = Path::new("benches/images/monitor");
    if !image_dir.exists() {
        println!("Warning: No test images found at {:?}", image_dir);
        return;
    }

    let entries = fs::read_dir(image_dir).unwrap();
    let mut image_count = 0;

    for entry in entries {
        let entry = entry.unwrap();
        let path = entry.path();

        if let Some(ext) = path.extension() {
            let ext = ext.to_string_lossy().to_lowercase();
            if ext == "png" || ext == "jpg" || ext == "jpeg" {
                if image_count >= 5 {
                    // Limit to 5 images for reasonable benchmark time
                    break;
                }

                let img_name = path.file_name().unwrap().to_string_lossy();

                // Load image
                let img = image::open(&path).unwrap();
                let rgb_img = img.to_rgb8();
                let (width, height) = rgb_img.dimensions();
                let raw_pixels: Vec<u8> = rgb_img.into_raw();

                // Preprocess: convert to grayscale and binarize
                let gray = rgb_to_grayscale(&raw_pixels, width as usize, height as usize);
                let binary = otsu_binarize(&gray, width as usize, height as usize);

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
                        b.iter(|| {
                            FinderDetector::detect_with_connected_components(black_box(binary))
                        });
                    },
                );

                image_count += 1;
            }
        }
    }

    group.finish();
}

criterion_group!(benches, bench_real_qr_connected_components);
criterion_main!(benches);
