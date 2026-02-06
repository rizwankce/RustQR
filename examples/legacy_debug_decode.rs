// Debug test to trace through QR decoder step by step
use rust_qr::decoder::format::FormatInfo;
use rust_qr::detector::finder::FinderDetector;
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
    use rust_qr::utils::binarization::adaptive_binarize;
    use rust_qr::utils::binarization::otsu_binarize;
    use rust_qr::utils::grayscale::rgb_to_grayscale;
    let gray = rgb_to_grayscale(&raw_pixels, width as usize, height as usize);
    let binary = if width >= 800 || height >= 800 {
        adaptive_binarize(&gray, width as usize, height as usize, 31)
    } else {
        otsu_binarize(&gray, width as usize, height as usize)
    };

    // Find patterns
    let patterns = FinderDetector::detect(&binary);
    println!("Found {} finder patterns\n", patterns.len());

    // Show first 10 patterns
    for (i, pattern) in patterns.iter().enumerate().take(patterns.len().min(10)) {
        println!(
            "Pattern {}: ({:.1}, {:.1}) size={:.2}",
            i, pattern.center.x, pattern.center.y, pattern.module_size
        );
    }

    // If we have hand-labeled corner points, try a direct format decode from those corners
    let points_path = Path::new("benches/images/monitor/image001.txt");
    if points_path.exists() {
        if let Ok(content) = std::fs::read_to_string(points_path) {
            let mut vals = Vec::new();
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                for tok in line.split_whitespace() {
                    if let Ok(v) = tok.parse::<f32>() {
                        vals.push(v);
                    }
                }
            }
            if vals.len() >= 8 {
                let mut pts = [
                    Point::new(vals[0], vals[1]),
                    Point::new(vals[2], vals[3]),
                    Point::new(vals[4], vals[5]),
                    Point::new(vals[6], vals[7]),
                ];
                pts.sort_by(|a, b| (a.x + a.y).partial_cmp(&(b.x + b.y)).unwrap());
                let top_left = pts[0];
                let bottom_right = pts[3];
                let others = [pts[1], pts[2]];
                let top_right = if others[0].x > others[1].x {
                    others[0]
                } else {
                    others[1]
                };
                let bottom_left = if others[0].x > others[1].x {
                    others[1]
                } else {
                    others[0]
                };

                println!("\n--- Testing with hand-labeled corners ---");
                println!(
                    "TL=({:.1},{:.1}) TR=({:.1},{:.1}) BL=({:.1},{:.1}) BR=({:.1},{:.1})",
                    top_left.x,
                    top_left.y,
                    top_right.x,
                    top_right.y,
                    bottom_left.x,
                    bottom_left.y,
                    bottom_right.x,
                    bottom_right.y
                );

                for version in 1..=40u8 {
                    let dimension = 17 + 4 * version as usize;
                    let src = [
                        Point::new(0.0, 0.0),
                        Point::new(dimension as f32 - 1.0, 0.0),
                        Point::new(dimension as f32 - 1.0, dimension as f32 - 1.0),
                        Point::new(0.0, dimension as f32 - 1.0),
                    ];
                    let dst = [top_left, top_right, bottom_right, bottom_left];
                    let transform = match PerspectiveTransform::from_points(&src, &dst) {
                        Some(t) => t,
                        None => continue,
                    };

                    let mut qr_matrix = BitMatrix::new(dimension, dimension);
                    for y in 0..dimension {
                        for x in 0..dimension {
                            let p = Point::new(x as f32, y as f32);
                            let img_point = transform.transform(&p);
                            let img_x = img_point.x.floor() as isize;
                            let img_y = img_point.y.floor() as isize;
                            if img_x >= 0
                                && img_y >= 0
                                && (img_x as usize) < binary.width()
                                && (img_y as usize) < binary.height()
                            {
                                qr_matrix.set(x, y, binary.get(img_x as usize, img_y as usize));
                            }
                        }
                    }

                    if let Some(info) = FormatInfo::extract(&qr_matrix) {
                        println!(
                            "Version {} format: EC={:?} Mask={:?}",
                            version, info.ec_level, info.mask_pattern
                        );
                        break;
                    }
                }
            }
        }
    }

    println!("\n--- Full library detection ---");
    let results = rust_qr::detect(&raw_pixels, width as usize, height as usize);
    println!("Found {} QR codes", results.len());
}
