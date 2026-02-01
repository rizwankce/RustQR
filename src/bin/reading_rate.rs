use std::fs;
use std::path::Path;

/// Calculate QR code reading rate for a category of images
fn calculate_reading_rate(category_dir: &str) -> f64 {
    let path = Path::new(category_dir);
    if !path.exists() {
        println!("Directory not found: {}", category_dir);
        return 0.0;
    }

    let mut total = 0;
    let mut successful = 0;

    // Read all image files
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();

            // Check if it's a jpg file
            if let Some(ext) = path.extension() {
                if ext.to_string_lossy().to_lowercase() != "jpg" {
                    continue;
                }

                let file_stem = path.file_stem().unwrap().to_string_lossy();
                let txt_file = path.with_extension("txt");

                // Check if corresponding .txt file exists (ground truth)
                if txt_file.exists() {
                    total += 1;

                    // Try to detect QR code
                    if let Ok(img) = image::open(&path) {
                        let rgb_img = img.to_rgb8();
                        let (width, height) = rgb_img.dimensions();
                        let raw_pixels: Vec<u8> = rgb_img.into_raw();

                        let results = rust_qr::detect(&raw_pixels, width as usize, height as usize);

                        // Check if we found any QR codes
                        if !results.is_empty() {
                            successful += 1;
                        }
                    }
                }
            }
        }
    }

    if total == 0 {
        return 0.0;
    }

    let rate = (successful as f64 / total as f64) * 100.0;
    println!(
        "  {}: {}/{} = {:.2}%",
        category_dir, successful, total, rate
    );
    rate
}

fn main() {
    println!("RustQR QR Code Reading Rate Benchmark");
    println!("=====================================\n");

    // Categories to test (based on BoofCV/Dynamsoft dataset)
    let categories = vec![
        ("blurred", "Blurred QR codes"),
        ("bright_spots", "Bright spots/glare"),
        ("brightness", "Various brightness levels"),
        ("close", "Close-up QR codes"),
        ("curved", "Curved surface QR codes"),
        ("damaged", "Damaged QR codes"),
        ("glare", "Glare/light reflections"),
        ("high_version", "High capacity QR codes"),
        ("lots", "Many QR codes in one image"),
        ("monitor", "Standard QR codes on monitor"),
        ("nominal", "Standard/nominal conditions"),
        ("noncompliant", "Non-standard QR codes"),
        ("pathological", "Pathological cases"),
        ("perspective", "Perspective distortion"),
        ("rotations", "Rotated QR codes"),
        ("shadows", "Shadows on QR codes"),
    ];

    let mut total_rate = 0.0;
    let mut count = 0;

    for (dir, description) in categories {
        println!("Testing: {} - {}", dir, description);
        let rate = calculate_reading_rate(&format!(
            "/Users/rizwan/Downloads/qrcodes 2/detection/{}",
            dir
        ));
        total_rate += rate;
        count += 1;
    }

    if count > 0 {
        let average = total_rate / count as f64;
        println!("\n=====================================");
        println!("Average Reading Rate: {:.2}%", average);
        println!("=====================================");
    }
}
