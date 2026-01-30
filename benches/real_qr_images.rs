use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use std::fs;
use std::path::Path;

/// Benchmark real QR code images
fn bench_real_qr_images(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_qr_images");

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

                // Benchmark this image
                group.bench_with_input(
                    BenchmarkId::new("detect", &img_name),
                    &(&raw_pixels, width as usize, height as usize),
                    |b, (pixels, w, h)| {
                        b.iter(|| rust_qr::detect(black_box(pixels), black_box(*w), black_box(*h)));
                    },
                );

                image_count += 1;
            }
        }
    }

    group.finish();
}

criterion_group!(benches, bench_real_qr_images);
criterion_main!(benches);
