// Comprehensive diagnostic tool to trace QR detection pipeline
use rust_qr::detector::finder::{FinderDetector, FinderPattern};
use rust_qr::models::{BitMatrix, Point};
use rust_qr::utils::binarization::{adaptive_binarize, otsu_binarize};
use rust_qr::utils::grayscale::rgb_to_grayscale;
use std::path::Path;

fn main() {
    // Test with a specific image
    let test_images = [
        "benches/images/boofcv/monitor/image001.jpg",
        "benches/images/boofcv/nominal/image001.jpg",
    ];

    for img_path in &test_images {
        let path = Path::new(img_path);
        if !path.exists() {
            println!("Image not found: {}", img_path);
            continue;
        }

        println!("\n============================================================");
        println!("DIAGNOSING: {}", img_path);
        println!("============================================================\n");

        diagnose_image(img_path);
    }
}

fn diagnose_image(img_path: &str) {
    // Load image
    let img = match image::open(img_path) {
        Ok(img) => img,
        Err(e) => {
            println!("Failed to open image: {}", e);
            return;
        }
    };

    let rgb_img = img.to_rgb8();
    let (width, height) = (rgb_img.width() as usize, rgb_img.height() as usize);
    let raw_pixels: Vec<u8> = rgb_img.into_raw();

    println!("Step 1: Image loaded - {}x{} pixels", width, height);

    // Step 2: Convert to grayscale
    let gray = rgb_to_grayscale(&raw_pixels, width, height);
    println!("Step 2: Converted to grayscale - {} bytes", gray.len());

    // Step 3: Binarization
    let binary_adaptive = adaptive_binarize(&gray, width, height, 31);
    let binary_otsu = otsu_binarize(&gray, width, height);
    println!("Step 3: Binarization complete");
    println!("  - Adaptive binarize: {}x{}", binary_adaptive.width(), binary_adaptive.height());
    println!("  - Otsu binarize: {}x{}", binary_otsu.width(), binary_otsu.height());

    // Step 4: Finder pattern detection
    println!("\nStep 4: Finder pattern detection");

    let patterns_adaptive = FinderDetector::detect(&binary_adaptive);
    println!("  - Adaptive: {} patterns found", patterns_adaptive.len());
    for (i, p) in patterns_adaptive.iter().enumerate().take(10) {
        println!("    [{:2}] center=({:6.1}, {:6.1}), module_size={:.2}",
                 i, p.center.x, p.center.y, p.module_size);
    }

    let patterns_otsu = FinderDetector::detect(&binary_otsu);
    println!("  - Otsu: {} patterns found", patterns_otsu.len());
    for (i, p) in patterns_otsu.iter().enumerate().take(10) {
        println!("    [{:2}] center=({:6.1}, {:6.1}), module_size={:.2}",
                 i, p.center.x, p.center.y, p.module_size);
    }

    // Combine patterns
    let mut all_patterns = Vec::new();
    all_patterns.extend(patterns_adaptive);
    all_patterns.extend(patterns_otsu);
    println!("  - Combined: {} patterns", all_patterns.len());

    // Filter large patterns (likely real QR codes)
    let max_size = all_patterns.iter().map(|p| p.module_size).fold(0.0f32, |a, b| a.max(b));
    let large_patterns: Vec<_> = all_patterns.iter()
        .filter(|p| p.module_size >= max_size * 0.5)
        .collect();
    println!("  - Large patterns (>=50% of max {:.1}): {}", max_size, large_patterns.len());
    for (i, p) in large_patterns.iter().enumerate() {
        println!("    [{:2}] center=({:6.1}, {:6.1}), module_size={:.2}",
                 i, p.center.x, p.center.y, p.module_size);
    }

    // Step 5: Pattern grouping
    println!("\nStep 5: Pattern grouping");
    let groups = group_finder_patterns_diagnostic(&all_patterns);
    println!("  - {} groups formed", groups.len());

    // Step 6: Try decoding each group
    println!("\nStep 6: Attempting to decode each group");

    let binary = if width >= 800 || height >= 800 {
        &binary_adaptive
    } else {
        &binary_otsu
    };

    for (group_idx, group) in groups.iter().enumerate() {
        println!("\n  Group {}:", group_idx);
        let p0 = &all_patterns[group[0]];
        let p1 = &all_patterns[group[1]];
        let p2 = &all_patterns[group[2]];
        println!("    Patterns: [{:2}] ({:.1}, {:.1}), [{:2}] ({:.1}, {:.1}), [{:2}] ({:.1}, {:.1})",
                 group[0], p0.center.x, p0.center.y,
                 group[1], p1.center.x, p1.center.y,
                 group[2], p2.center.x, p2.center.y);
        println!("    Module sizes: {:.2}, {:.2}, {:.2}",
                 p0.module_size, p1.module_size, p2.module_size);

        // Try to order patterns
        if let Some((tl, tr, bl, module_size)) = order_finder_patterns(p0, p1, p2) {
            println!("    Ordered: TL=({:.1}, {:.1}), TR=({:.1}, {:.1}), BL=({:.1}, {:.1})",
                     tl.x, tl.y, tr.x, tr.y, bl.x, bl.y);
            println!("    Avg module size: {:.2}", module_size);

            // Calculate distances
            let d_tr = tl.distance(&tr);
            let d_bl = tl.distance(&bl);
            println!("    Distances: TL-TR={:.1}, TL-BL={:.1}", d_tr, d_bl);

            // Estimate dimension
            let dim1 = estimate_dimension(d_tr, module_size);
            let dim2 = estimate_dimension(d_bl, module_size);
            println!("    Estimated dimensions: {} x {} (from TR), {} x {} (from BL)",
                     dim1.unwrap_or(0), dim1.unwrap_or(0),
                     dim2.unwrap_or(0), dim2.unwrap_or(0));

            // Try to decode
            let result = rust_qr::detect(&raw_pixels, width, height);
            println!("    Full detection result: {} QR codes found", result.len());

        } else {
            println!("    FAILED: Could not order finder patterns");
        }
    }

    // Final detection attempt
    println!("\nStep 7: Full library detection");
    let results = rust_qr::detect(&raw_pixels, width, height);
    println!("  Final result: {} QR codes detected", results.len());
    for (i, qr) in results.iter().enumerate() {
        println!("    [{}] version={:?}, content={}", i, qr.version, qr.content);
    }
}

fn group_finder_patterns_diagnostic(patterns: &[FinderPattern]) -> Vec<Vec<usize>> {
    let mut groups: Vec<Vec<usize>> = Vec::new();

    if patterns.len() < 3 {
        println!("    Not enough patterns (<3)");
        return groups;
    }

    let max_size = patterns.iter().fold(0.0f32, |a, p| a.max(p.module_size));

    // Try large patterns first
    let large_indices: Vec<usize> = patterns
        .iter()
        .enumerate()
        .filter(|(_, p)| p.module_size >= max_size * 0.5)
        .map(|(i, _)| i)
        .collect();

    println!("    Large pattern indices (>=50% of {:.1}): {:?}", max_size, large_indices);

    if large_indices.len() >= 3 {
        let groups_large = build_groups_diagnostic(patterns, &large_indices);
        if !groups_large.is_empty() {
            return groups_large;
        }
    }

    // Fallback to median-based
    let mut sizes: Vec<f32> = patterns.iter().map(|p| p.module_size).collect();
    sizes.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_size = sizes[sizes.len() / 2];

    let valid_indices: Vec<usize> = patterns
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            let ratio = p.module_size / median_size;
            ratio >= 0.5 && ratio <= 2.0
        })
        .map(|(i, _)| i)
        .collect();

    println!("    Median-based indices (median={:.1}): {} patterns", median_size, valid_indices.len());

    if valid_indices.len() >= 3 {
        groups = build_groups_diagnostic(patterns, &valid_indices);
    }

    groups
}

fn build_groups_diagnostic(patterns: &[FinderPattern], indices: &[usize]) -> Vec<Vec<usize>> {
    let mut groups = Vec::new();
    let mut used = vec![false; patterns.len()];

    for idx_i in 0..indices.len() {
        let i = indices[idx_i];
        if used[i] { continue; }
        for idx_j in (idx_i + 1)..indices.len() {
            let j = indices[idx_j];
            if used[j] { continue; }
            for idx_k in (idx_j + 1)..indices.len() {
                let k = indices[idx_k];
                if used[k] { continue; }

                let pi = &patterns[i];
                let pj = &patterns[j];
                let pk = &patterns[k];

                // Size check
                let sizes = [pi.module_size, pj.module_size, pk.module_size];
                let min_size = sizes.iter().fold(f32::INFINITY, |a, &b| a.min(b));
                let max_size = sizes.iter().fold(0.0f32, |a, &b| a.max(b));
                let size_ratio = max_size / min_size;

                if size_ratio < 0.33 || size_ratio > 3.0 {
                    continue;
                }

                // Distance checks
                let d_ij = pi.center.distance(&pj.center);
                let d_ik = pi.center.distance(&pk.center);
                let d_jk = pj.center.distance(&pk.center);

                let distances = [d_ij, d_ik, d_jk];
                let min_d = distances.iter().fold(f32::INFINITY, |a, &b| a.min(b));
                let max_d = distances.iter().fold(0.0f32, |a, &b| a.max(b));

                let avg_module = (pi.module_size + pj.module_size + pk.module_size) / 3.0;
                if min_d < avg_module * 3.0 {
                    continue;
                }
                if max_d > 3000.0 {
                    continue;
                }
                let distortion_ratio = max_d / min_d;
                if distortion_ratio > 5.0 {
                    continue;
                }

                // Angle check
                let a2 = d_ij * d_ij;
                let b2 = d_ik * d_ik;
                let c2 = d_jk * d_jk;

                let cos_i = (a2 + b2 - c2) / (2.0 * d_ij * d_ik);
                let cos_j = (a2 + c2 - b2) / (2.0 * d_ij * d_jk);
                let cos_k = (b2 + c2 - a2) / (2.0 * d_ik * d_jk);
                let has_right_angle = cos_i.abs() < 0.3 || cos_j.abs() < 0.3 || cos_k.abs() < 0.3;
                if !has_right_angle {
                    continue;
                }

                groups.push(vec![i, j, k]);
                used[i] = true;
                used[j] = true;
                used[k] = true;
                break;
            }
        }
    }

    groups
}

fn order_finder_patterns(
    a: &FinderPattern,
    b: &FinderPattern,
    c: &FinderPattern,
) -> Option<(Point, Point, Point, f32)> {
    let patterns = [a, b, c];

    if patterns.iter().any(|p| p.module_size < 2.0) {
        return None;
    }

    // Find the right-angle corner (top-left)
    let mut best_idx = 0usize;
    let mut best_cos = f32::INFINITY;
    for i in 0..3 {
        let p = &patterns[i].center;
        let p1 = &patterns[(i + 1) % 3].center;
        let p2 = &patterns[(i + 2) % 3].center;

        let v1x = p1.x - p.x;
        let v1y = p1.y - p.y;
        let v2x = p2.x - p.x;
        let v2y = p2.y - p.y;
        let dot = v1x * v2x + v1y * v2y;
        let denom = (v1x * v1x + v1y * v1y).sqrt() * (v2x * v2x + v2y * v2y).sqrt();
        if denom == 0.0 {
            continue;
        }
        let cos = (dot / denom).abs();
        if cos < best_cos {
            best_cos = cos;
            best_idx = i;
        }
    }

    let tl = patterns[best_idx];
    let p1 = patterns[(best_idx + 1) % 3];
    let p2 = patterns[(best_idx + 2) % 3];

    let v1x = p1.center.x - tl.center.x;
    let v1y = p1.center.y - tl.center.y;
    let v2x = p2.center.x - tl.center.x;
    let v2y = p2.center.y - tl.center.y;
    let cross = v1x * v2y - v1y * v2x;

    let (tr, bl) = if cross > 0.0 { (p1, p2) } else { (p2, p1) };
    let avg_module = (tl.module_size + tr.module_size + bl.module_size) / 3.0;
    let d_tr = tl.center.distance(&tr.center);
    let d_bl = tl.center.distance(&bl.center);

    let dim1 = estimate_dimension(d_tr, avg_module)?;
    let dim2 = estimate_dimension(d_bl, avg_module)?;
    let dim = if dim1 == dim2 {
        dim1
    } else if (dim1 as isize - dim2 as isize).abs() <= 4 {
        ((dim1 + dim2) / 2).max(21)
    } else {
        println!("      FAIL: Dimension mismatch: {} vs {}", dim1, dim2);
        return None;
    };

    let module_size = (d_tr + d_bl) / 2.0 / (dim as f32 - 7.0);

    Some((tl.center, tr.center, bl.center, module_size))
}

fn estimate_dimension(distance: f32, module_size: f32) -> Option<usize> {
    if module_size <= 0.0 {
        return None;
    }
    let raw_dim = distance / module_size + 7.0;
    if raw_dim < 21.0 {
        return None;
    }
    let version = ((raw_dim - 17.0) / 4.0).round() as i32;
    if !(1..=40).contains(&version) {
        return None;
    }
    Some(17 + 4 * version as usize)
}
