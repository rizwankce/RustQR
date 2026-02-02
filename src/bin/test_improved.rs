/// Test the improved QR detector on a real image
use std::path::Path;

fn main() {
    println!("Testing Improved QR Detector");
    println!("==============================\n");

    let image_path = "benches/images/monitor/image001.jpg";

    if !Path::new(image_path).exists() {
        println!("✗ Image not found: {}", image_path);
        return;
    }

    match image::open(image_path) {
        Ok(img) => {
            let rgb_img = img.to_rgb8();
            let (width, height) = rgb_img.dimensions();
            let raw_pixels: Vec<u8> = rgb_img.into_raw();

            println!("Image: {}x{}", width, height);

            // Step 1: Convert to grayscale
            let gray = rust_qr::utils::grayscale::rgb_to_grayscale(
                &raw_pixels,
                width as usize,
                height as usize,
            );

            // Step 2: Binarize
            let binary =
                rust_qr::utils::binarization::otsu_binarize(&gray, width as usize, height as usize);

            // Step 3: Detect finder patterns
            let patterns = rust_qr::detector::finder::FinderDetector::detect(&binary);
            println!("Found {} finder patterns", patterns.len());

            // Show module size distribution
            let mut sizes: Vec<f32> = patterns.iter().map(|p| p.module_size).collect();
            sizes.sort_by(|a, b| a.partial_cmp(b).unwrap());
            println!(
                "Module sizes: min={:.1}, max={:.1}",
                sizes.first().unwrap_or(&0.0),
                sizes.last().unwrap_or(&0.0)
            );

            if patterns.len() >= 3 {
                println!("\nTop 10 patterns by module size:");
                for (i, p) in patterns.iter().take(10).enumerate() {
                    println!(
                        "  {}: size={:.1}px at ({:.0}, {:.0})",
                        i, p.module_size, p.center.x, p.center.y
                    );
                }
            }

            // Step 4: Try to detect QR codes
            println!("\nAttempting to detect QR codes...");
            let results = rust_qr::detect(&raw_pixels, width as usize, height as usize);

            if results.is_empty() {
                println!("✗ No QR codes detected");
            } else {
                println!("✓ Found {} QR code(s):", results.len());
                for (i, qr) in results.iter().enumerate() {
                    println!("  {}: {}", i + 1, qr.content);
                }
            }
        }
        Err(e) => {
            println!("✗ Failed to load image: {}", e);
        }
    }
}
