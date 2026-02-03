// Debug tool to trace QR matrix extraction and finder validation
use rust_qr::decoder::format::FormatInfo;
use rust_qr::detector::finder::FinderDetector;
use rust_qr::models::{BitMatrix, Point};
use rust_qr::utils::binarization::{adaptive_binarize, otsu_binarize};
use rust_qr::utils::geometry::PerspectiveTransform;
use rust_qr::utils::grayscale::rgb_to_grayscale;
use std::path::Path;

fn main() {
    let img_path = "benches/images/boofcv/monitor/image001.jpg";
    let path = Path::new(img_path);
    if !path.exists() {
        println!("Image not found: {}", img_path);
        return;
    }

    println!("Testing: {}", img_path);

    let img = image::open(path).expect("Failed to load image");
    let rgb_img = img.to_rgb8();
    let (width, height) = (rgb_img.width() as usize, rgb_img.height() as usize);
    let raw_pixels: Vec<u8> = rgb_img.into_raw();

    println!("Image: {}x{}", width, height);

    let gray = rgb_to_grayscale(&raw_pixels, width, height);
    let binary_adaptive = adaptive_binarize(&gray, width, height, 31);
    let binary_otsu = otsu_binarize(&gray, width, height);

    // Find large finder patterns (from Otsu - same as the test)
    let patterns = FinderDetector::detect(&binary_otsu);
    println!("Found {} patterns from Otsu", patterns.len());

    // Get the large patterns (similar module_size)
    let max_size = patterns.iter().map(|p| p.module_size).fold(0.0f32, |a, b| a.max(b));
    let large: Vec<_> = patterns.iter()
        .filter(|p| p.module_size >= max_size * 0.5 && p.module_size <= max_size * 0.8)
        .collect();

    println!("Large patterns (50-80% of max {:.1}):", max_size);
    for (i, p) in large.iter().enumerate() {
        println!("  [{}] ({:.1}, {:.1}) size={:.2}", i, p.center.x, p.center.y, p.module_size);
    }

    // The three real finder patterns should be at:
    // TL: around (720, 1482), size ~106
    // TR: around (2197, 1511), size ~104
    // BL: around (708, 2938), size ~103

    // Let's manually set them based on our analysis
    let tl = Point::new(720.9, 1481.8);
    let tr = Point::new(2197.5, 1510.9);
    let bl = Point::new(708.4, 2938.0);
    let module_size = 104.75; // average

    println!("\nUsing finder positions:");
    println!("  TL: ({:.1}, {:.1})", tl.x, tl.y);
    println!("  TR: ({:.1}, {:.1})", tr.x, tr.y);
    println!("  BL: ({:.1}, {:.1})", bl.x, bl.y);
    println!("  Module size: {:.2}", module_size);

    // Calculate BR
    let br = Point::new(
        tr.x + bl.x - tl.x,
        tr.y + bl.y - tl.y,
    );
    println!("  BR (calculated): ({:.1}, {:.1})", br.x, br.y);

    // Distance checks
    let d_tl_tr = tl.distance(&tr);
    let d_tl_bl = tl.distance(&bl);
    let d_tr_bl = tr.distance(&bl);
    println!("\nDistances:");
    println!("  TL-TR: {:.1}", d_tl_tr);
    println!("  TL-BL: {:.1}", d_tl_bl);
    println!("  TR-BL: {:.1}", d_tr_bl);

    // Estimate dimension
    let width_modules = (d_tl_tr / module_size).round() as usize;
    let dimension = width_modules + 7;
    println!("\nEstimated dimension: {} (version {})", dimension, (dimension - 17) / 4);

    // Build transform
    let src = [
        Point::new(3.5, 3.5),
        Point::new(dimension as f32 - 3.5, 3.5),
        Point::new(3.5, dimension as f32 - 3.5),
        Point::new(dimension as f32 - 3.5, dimension as f32 - 3.5),
    ];
    let dst = [tl, tr, bl, br];

    println!("\nTransform mapping:");
    println!("  src: {:?}", src);
    println!("  dst: {:?}", dst);

    let transform = PerspectiveTransform::from_points(&src, &dst);
    if transform.is_none() {
        println!("ERROR: Failed to build transform!");
        return;
    }
    let transform = transform.unwrap();

    // Extract matrix
    let qr_matrix = extract_qr_region_gray(&gray, width, height, &transform, dimension);

    println!("\nExtracted QR matrix ({}x{}):", dimension, dimension);
    print_matrix_corner(&qr_matrix, "Top-left finder (0,0):", 0, 0);
    print_matrix_corner(&qr_matrix, "Top-right finder:", dimension - 7, 0);
    print_matrix_corner(&qr_matrix, "Bottom-left finder:", 0, dimension - 7);

    // Check finder patterns
    let has_finders = has_finders_correct(&qr_matrix);
    println!("\nFinder patterns valid: {}", has_finders);

    // Try decoding
    if let Some(info) = FormatInfo::extract(&qr_matrix) {
        println!("Format info: EC={:?}, Mask={:?}", info.ec_level, info.mask_pattern);
    } else {
        println!("Failed to extract format info");
    }

    // Full detection
    let results = rust_qr::detect(&raw_pixels, width, height);
    println!("\nFull detection result: {} QR codes", results.len());
}

fn extract_qr_region_gray(
    gray: &[u8],
    width: usize,
    height: usize,
    transform: &PerspectiveTransform,
    dimension: usize,
) -> BitMatrix {
    let mut samples: Vec<u8> = Vec::with_capacity(dimension * dimension);
    for y in 0..dimension {
        for x in 0..dimension {
            let module_center = Point::new(x as f32 + 0.5, y as f32 + 0.5);
            let img_point = transform.transform(&module_center);
            let img_x = img_point.x.round() as isize;
            let img_y = img_point.y.round() as isize;

            let mut sum = 0u32;
            let mut count = 0u32;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let sx = img_x + dx;
                    let sy = img_y + dy;
                    if sx >= 0 && sy >= 0 && (sx as usize) < width && (sy as usize) < height {
                        let idx = sy as usize * width + sx as usize;
                        sum += gray[idx] as u32;
                        count += 1;
                    }
                }
            }
            let avg = if count > 0 { (sum / count) as u8 } else { 255u8 };
            samples.push(avg);
        }
    }

    let mut sorted = samples.clone();
    sorted.sort_unstable();
    let threshold = sorted[sorted.len() / 2];
    println!("Binarization threshold (median): {}", threshold);

    let mut result = BitMatrix::new(dimension, dimension);
    for y in 0..dimension {
        for x in 0..dimension {
            let idx = y * dimension + x;
            result.set(x, y, samples[idx] < threshold);
        }
    }

    result
}

fn print_matrix_corner(matrix: &BitMatrix, label: &str, ox: usize, oy: usize) {
    println!("{}", label);
    for y in 0..7.min(matrix.height() - oy) {
        print!("  ");
        for x in 0..7.min(matrix.width() - ox) {
            let ch = if matrix.get(ox + x, oy + y) { '#' } else { '.' };
            print!("{}", ch);
        }
        println!();
    }
}

fn has_finders_correct(matrix: &BitMatrix) -> bool {
    let dim = matrix.width();
    if dim < 21 || matrix.height() < 21 {
        return false;
    }

    let finder_checks: [(usize, usize, bool); 7] = [
        (0, 0, true),   // top-left corner
        (6, 0, true),   // top-right corner
        (0, 6, true),   // bottom-left corner
        (6, 6, true),   // bottom-right corner
        (3, 3, true),   // center
        (1, 1, false),  // inner white ring
        (2, 2, true),   // inner black ring
    ];

    let origins = [
        (0, 0),
        (dim - 7, 0),
        (0, dim - 7),
    ];

    let mut mismatches = 0;
    for &(ox, oy) in &origins {
        println!("  Checking finder at ({}, {}):", ox, oy);
        for &(dx, dy, expected) in &finder_checks {
            let x = ox + dx;
            let y = oy + dy;
            if x >= dim || y >= matrix.height() {
                return false;
            }
            let actual = matrix.get(x, y);
            if actual != expected {
                mismatches += 1;
                println!("    MISMATCH at ({}, {}): expected {}, got {}", x, y, expected, actual);
            }
        }
    }

    println!("  Total mismatches: {}", mismatches);
    mismatches <= 3
}
