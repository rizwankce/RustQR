/// Test decoder with a real QR code image
/// This uses actual images from the benchmark dataset
use std::path::Path;

fn main() {
    println!("Real QR Image Decoder Test");
    println!("===========================\n");

    // Test with a real QR image from the benchmark dataset
    let test_images = vec![
        "benches/images/monitor/image001.jpg",
        "benches/images/monitor/image002.jpg",
    ];

    for image_path in test_images {
        println!("\nTesting: {}", image_path);

        if !Path::new(image_path).exists() {
            println!("  ✗ Image not found");
            continue;
        }

        match image::open(image_path) {
            Ok(img) => {
                let rgb_img = img.to_rgb8();
                let (width, height) = rgb_img.dimensions();
                let raw_pixels: Vec<u8> = rgb_img.into_raw();

                println!("  Image size: {}x{}", width, height);

                // Try to detect
                let results = rust_qr::detect(&raw_pixels, width as usize, height as usize);

                if results.is_empty() {
                    println!("  ✗ No QR codes detected");

                    // Debug: Try just finder pattern detection
                    let gray = rust_qr::utils::grayscale::rgb_to_grayscale(
                        &raw_pixels,
                        width as usize,
                        height as usize,
                    );
                    let binary = rust_qr::utils::binarization::otsu_binarize(
                        &gray,
                        width as usize,
                        height as usize,
                    );
                    let patterns = rust_qr::detector::finder::FinderDetector::detect(&binary);

                    println!("    Found {} finder patterns", patterns.len());
                    println!("    All patterns:");
                    for (i, p) in patterns.iter().enumerate() {
                        println!(
                            "      {}: center=({:.1}, {:.1}), size={:.1}",
                            i, p.center.x, p.center.y, p.module_size
                        );
                    }
                } else {
                    println!("  ✓ Found {} QR code(s):", results.len());
                    for (i, qr) in results.iter().enumerate() {
                        println!("    {}: {}", i + 1, qr.content);
                    }
                }
            }
            Err(e) => {
                println!("  ✗ Failed to load image: {}", e);
            }
        }
    }

    println!("\n===========================");
    println!("Test complete");
}
