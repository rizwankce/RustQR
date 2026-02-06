// Simple debug test for QR detection
use std::path::Path;

fn main() {
    let test_image = Path::new("benches/images/monitor/image001.jpg");

    if !test_image.exists() {
        println!("Test image not found: {}", test_image.display());
        return;
    }

    println!("Testing with: {}", test_image.display());

    // Load image
    let img = image::open(test_image).expect("Failed to open image");
    let rgb_img = img.to_rgb8();
    let (width, height) = rgb_img.dimensions();
    let raw_pixels: Vec<u8> = rgb_img.into_raw();

    println!("Image size: {}x{}", width, height);
    println!(
        "Total pixels: {} ({} bytes)",
        width * height,
        raw_pixels.len()
    );

    // Step 1: Convert to grayscale using the library function
    use rust_qr::utils::grayscale::rgb_to_grayscale;
    let gray = rgb_to_grayscale(&raw_pixels, width as usize, height as usize);

    // Check grayscale range
    let min_gray = *gray.iter().min().unwrap();
    let max_gray = *gray.iter().max().unwrap();
    let avg_gray = gray.iter().map(|&v| v as u32).sum::<u32>() / gray.len() as u32;
    println!(
        "Grayscale range: {}-{}, average: {}",
        min_gray, max_gray, avg_gray
    );

    // Step 2: Binarize and check
    use rust_qr::utils::binarization::otsu_binarize;
    let binary = otsu_binarize(&gray, width as usize, height as usize);
    println!("Binary matrix size: {}x{}", binary.width(), binary.height());

    // Count black pixels
    let mut black_count = 0;
    for y in 0..binary.height() {
        for x in 0..binary.width() {
            if binary.get(x, y) {
                black_count += 1;
            }
        }
    }
    println!(
        "Black pixels: {} / {} ({:.1}%)",
        black_count,
        binary.width() * binary.height(),
        100.0 * black_count as f64 / (binary.width() * binary.height()) as f64
    );

    // Step 3: Try to detect finder patterns
    use rust_qr::detector::finder::FinderDetector;
    let finder_patterns = FinderDetector::detect(&binary);
    println!("Found {} finder patterns", finder_patterns.len());

    for (i, pattern) in finder_patterns.iter().enumerate() {
        println!(
            "  Pattern {}: center=({:.1}, {:.1}), module_size={:.2}",
            i, pattern.center.x, pattern.center.y, pattern.module_size
        );
    }

    // Debug: Test grouping manually
    println!("\n--- Manual grouping test ---");
    let mut groups_found = 0;
    for i in 0..finder_patterns.len() {
        for j in (i + 1)..finder_patterns.len() {
            for k in (j + 1)..finder_patterns.len() {
                let size_ratio_ij = finder_patterns[i].module_size / finder_patterns[j].module_size;
                let size_ratio_ik = finder_patterns[i].module_size / finder_patterns[k].module_size;

                if (0.7..=1.4).contains(&size_ratio_ij) && (0.7..=1.4).contains(&size_ratio_ik) {
                    // Check distances
                    let d_ij = finder_patterns[i]
                        .center
                        .distance(&finder_patterns[j].center);
                    let d_ik = finder_patterns[i]
                        .center
                        .distance(&finder_patterns[k].center);
                    let d_jk = finder_patterns[j]
                        .center
                        .distance(&finder_patterns[k].center);

                    // Check if they form a reasonable triangle (QR finder pattern triangle)
                    // The three corners should be roughly 7 modules apart in the image
                    let avg_module = (finder_patterns[i].module_size
                        + finder_patterns[j].module_size
                        + finder_patterns[k].module_size)
                        / 3.0;

                    // For a QR code, distances should be roughly similar (within 2:1 ratio)
                    let max_d = d_ij.max(d_ik).max(d_jk);
                    let min_d = d_ij.min(d_ik).min(d_jk);

                    if max_d / min_d < 2.0 && min_d > avg_module * 7.0 {
                        groups_found += 1;
                        println!(
                            "  Group {}: patterns {}, {}, {} - module sizes: {:.2}, {:.2}, {:.2}",
                            groups_found,
                            i,
                            j,
                            k,
                            finder_patterns[i].module_size,
                            finder_patterns[j].module_size,
                            finder_patterns[k].module_size
                        );
                        println!(
                            "    Distances: {:.1}, {:.1}, {:.1} (avg module: {:.2})",
                            d_ij, d_ik, d_jk, avg_module
                        );
                    }
                }
            }
        }
    }
    println!("  Found {} valid groups of 3", groups_found);

    // Step 4: Try full detection
    let results = rust_qr::detect(&raw_pixels, width as usize, height as usize);
    println!("\nFull detection found {} QR codes", results.len());

    for (i, qr) in results.iter().enumerate() {
        println!(
            "  QR {}: version={:?}, error_correction={:?}, mask={:?}, content={}",
            i, qr.version, qr.error_correction, qr.mask_pattern, qr.content
        );
    }
}
