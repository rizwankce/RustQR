// Debug test to trace through QR decoder step by step
use rust_qr::decoder::format::FormatInfo;
use rust_qr::decoder::qr_decoder::QrDecoder;
use rust_qr::detector::finder::{FinderDetector, FinderPattern};
use rust_qr::models::{BitMatrix, Point};
use rust_qr::utils::geometry::PerspectiveTransform;
use std::path::Path;

fn main() {
    let test_image = Path::new("benches/images/monitor/image001.jpg");

    if !test_image.exists() {
        println!("Test image not found: {}", test_image.display());
        return;
    }

    println!("Testing with: {}\n", test_image.display());

    // Load image
    let img = image::open(test_image).expect("Failed to open image");
    let rgb_img = img.to_rgb8();
    let (width, height) = rgb_img.dimensions();
    let raw_pixels: Vec<u8> = rgb_img.into_raw();

    println!("Image: {}x{} ({} pixels)", width, height, width * height);

    // Convert and binarize
    use rust_qr::utils::binarization::otsu_binarize;
    use rust_qr::utils::grayscale::rgb_to_grayscale;
    let gray = rgb_to_grayscale(&raw_pixels, width as usize, height as usize);
    let binary = otsu_binarize(&gray, width as usize, height as usize);

    // Find patterns
    let patterns = FinderDetector::detect(&binary);
    println!("Found {} finder patterns\n", patterns.len());

    // Show first 10 patterns
    for i in 0..patterns.len().min(10) {
        println!(
            "Pattern {}: ({:.1}, {:.1}) size={:.2}",
            i, patterns[i].center.x, patterns[i].center.y, patterns[i].module_size
        );
    }

    if patterns.len() >= 3 {
        // Try first 3 patterns manually
        let tl = &patterns[0].center;
        let tr = &patterns[1].center;
        let bl = &patterns[2].center;

        println!("\n--- Testing with first 3 patterns ---");
        println!(
            "TL: ({:.1}, {:.1}), TR: ({:.1}, {:.1}), BL: ({:.1}, {:.1})",
            tl.x, tl.y, tr.x, tr.y, bl.x, bl.y
        );

        // Step 1: Estimate module size
        let d12 = tl.distance(tr);
        let d13 = tl.distance(bl);
        let avg_dist = (d12 + d13) / 2.0;
        let module_size = avg_dist / 7.0;
        println!(
            "Distances: d12={:.1}, d13={:.1}, avg={:.1}",
            d12, d13, avg_dist
        );
        println!("Module size: {:.2}", module_size);

        // Step 2: Calculate bottom-right
        let br_x = tr.x + bl.x - tl.x;
        let br_y = tr.y + bl.y - tl.y;
        println!("Calculated BR: ({:.1}, {:.1})", br_x, br_y);

        // Step 3: Estimate dimension
        let width_pixels = tl.distance(tr);
        let width_modules = (width_pixels / module_size).round() as usize;
        let dimension = width_modules + 7;
        println!(
            "Width in pixels: {:.1}, modules: {}, dimension: {}",
            width_pixels, width_modules, dimension
        );

        if dimension < 21 {
            println!("ERROR: dimension {} is too small (min 21)", dimension);
        } else {
            // Step 4: Try perspective transform
            let src = [
                Point::new(3.5, 3.5),
                Point::new(dimension as f32 - 3.5, 3.5),
                Point::new(3.5, dimension as f32 - 3.5),
                Point::new(dimension as f32 - 3.5, dimension as f32 - 3.5),
            ];
            let dst = [*tl, *tr, *bl, Point::new(br_x, br_y)];

            match PerspectiveTransform::from_points(&dst, &src) {
                Some(transform) => {
                    println!("Perspective transform: OK");

                    // Step 5: Extract and try format info
                    let mut qr_matrix = BitMatrix::new(dimension, dimension);
                    for y in 0..dimension {
                        for x in 0..dimension {
                            let module_center = Point::new(x as f32 + 0.5, y as f32 + 0.5);
                            let img_point = transform.transform(&module_center);
                            let img_x = img_point.x as usize;
                            let img_y = img_point.y as usize;
                            if img_x < binary.width() && img_y < binary.height() {
                                qr_matrix.set(x, y, binary.get(img_x, img_y));
                            }
                        }
                    }

                    println!("Extracted {}x{} matrix", dimension, dimension);

                    // Try format extraction
                    match FormatInfo::extract(&qr_matrix) {
                        Some(format) => {
                            println!(
                                "Format extracted: EC={:?}, Mask={:?}",
                                format.ec_level, format.mask_pattern
                            );
                        }
                        None => {
                            println!("ERROR: Format extraction failed!");

                            // Debug: show what bits were read
                            if dimension >= 21 {
                                print!("Row 8, cols 0-7 (skip 6): ");
                                for col in 0..8 {
                                    if col == 6 {
                                        print!("X ");
                                        continue;
                                    }
                                    print!("{} ", if qr_matrix.get(col, 8) { "B" } else { "W" });
                                }
                                println!();

                                print!("Col 8, rows 0-7 (skip 6, bottom-up): ");
                                for row in (0..8).rev() {
                                    if row == 6 {
                                        print!("X ");
                                        continue;
                                    }
                                    print!("{} ", if qr_matrix.get(8, row) { "B" } else { "W" });
                                }
                                println!();
                            }
                        }
                    }
                }
                None => {
                    println!("ERROR: Perspective transform failed!");
                }
            }
        }
    }

    println!("\n--- Full library detection ---");
    let results = rust_qr::detect(&raw_pixels, width as usize, height as usize);
    println!("Found {} QR codes", results.len());
}
