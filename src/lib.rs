//! RustQR - World's fastest QR code scanning library
//!
//! A pure Rust QR code detection and decoding library with zero dependencies.
//! Designed for maximum speed and cross-platform compatibility.

#![warn(missing_docs)]
#![allow(clippy::missing_docs_in_private_items)]

/// QR code decoding modules (error correction, format extraction, data modes)
pub mod decoder;
/// QR code detection modules (finder patterns, alignment, timing)
pub mod detector;
/// Core data structures (QRCode, BitMatrix, Point, etc.)
pub mod models;
/// Utility functions (grayscale, binarization, geometry)
pub mod utils;

pub use models::{BitMatrix, ECLevel, MaskPattern, Point, QRCode, Version};

use decoder::qr_decoder::QrDecoder;
use detector::finder::{FinderDetector, FinderPattern};
use utils::binarization::{adaptive_binarize, otsu_binarize};
use utils::grayscale::{rgb_to_grayscale, rgb_to_grayscale_with_buffer};
use utils::memory_pool::BufferPool;

/// Detect QR codes in an RGB image
///
/// # Arguments
/// * `image` - Raw RGB bytes (3 bytes per pixel)
/// * `width` - Image width in pixels
/// * `height` - Image height in pixels
///
/// # Returns
/// Vector of detected QR codes
///
/// Uses pyramid detection for large images (800px+) for better performance
pub fn detect(image: &[u8], width: usize, height: usize) -> Vec<QRCode> {
    // Step 1: Convert to grayscale
    let gray = rgb_to_grayscale(image, width, height);

    // Step 2: Binarize (use both adaptive and Otsu for robustness)
    let binary_adaptive = adaptive_binarize(&gray, width, height, 31);
    let binary_otsu = otsu_binarize(&gray, width, height);
    let binary = if width >= 800 || height >= 800 {
        binary_adaptive.clone()
    } else {
        binary_otsu.clone()
    };

    // Step 3: Detect finder patterns
    // Use pyramid detection for very large images (1600px+) for better performance
    let mut finder_patterns = Vec::new();
    let patterns_adaptive = if width >= 1600 && height >= 1600 {
        FinderDetector::detect_with_pyramid(&binary_adaptive)
    } else {
        FinderDetector::detect(&binary_adaptive)
    };
    let patterns_otsu = if width >= 1600 && height >= 1600 {
        FinderDetector::detect_with_pyramid(&binary_otsu)
    } else {
        FinderDetector::detect(&binary_otsu)
    };
    finder_patterns.extend(patterns_adaptive);
    finder_patterns.extend(patterns_otsu);

    // Step 4: Group finder patterns into potential QR codes and decode
    let mut results = Vec::new();

    // Group finder patterns by proximity and similar module size
    let groups = group_finder_patterns(&finder_patterns);

    #[cfg(debug_assertions)]
    eprintln!(
        "DEBUG: Found {} finder patterns, formed {} groups",
        finder_patterns.len(),
        groups.len()
    );

    // Try to decode each group of 3 patterns
    for (group_idx, group) in groups.iter().enumerate() {
        if group.len() >= 3 {
            #[cfg(debug_assertions)]
            eprintln!(
                "DEBUG: Trying group {} with patterns {:?}",
                group_idx, group
            );

            if let Some((tl, tr, bl, module_size)) = order_finder_patterns(
                &finder_patterns[group[0]],
                &finder_patterns[group[1]],
                &finder_patterns[group[2]],
            ) {
                match QrDecoder::decode_with_gray(
                    &binary,
                    &gray,
                    width,
                    height,
                    &tl,
                    &tr,
                    &bl,
                    module_size,
                ) {
                    Some(qr) => {
                        #[cfg(debug_assertions)]
                        eprintln!("DEBUG: Group {} decoded successfully!", group_idx);
                        results.push(qr);
                    }
                    None => {
                        #[cfg(debug_assertions)]
                        eprintln!("DEBUG: Group {} failed to decode", group_idx);
                    }
                }
            }
        }
    }

    results
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

    let dim1 = estimate_dimension_from_distance(d_tr, avg_module)?;
    let dim2 = estimate_dimension_from_distance(d_bl, avg_module)?;
    let dim = if dim1 == dim2 {
        dim1
    } else if (dim1 as isize - dim2 as isize).abs() <= 4 {
        ((dim1 + dim2) / 2).max(21)
    } else {
        return None;
    };

    let module_size = (d_tr + d_bl) / 2.0 / (dim as f32 - 7.0);

    Some((tl.center, tr.center, bl.center, module_size))
}

fn estimate_dimension_from_distance(distance: f32, module_size: f32) -> Option<usize> {
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

/// Simplified finder pattern grouping with relaxed constraints
fn group_finder_patterns(patterns: &[FinderPattern]) -> Vec<Vec<usize>> {
    let mut groups: Vec<Vec<usize>> = Vec::new();

    if patterns.len() < 3 {
        return groups;
    }

    let max_size = patterns
        .iter()
        .fold(0.0f32, |a, p| a.max(p.module_size));

    let large_indices: Vec<usize> = patterns
        .iter()
        .enumerate()
        .filter(|(_, p)| p.module_size >= max_size * 0.5)
        .map(|(i, _)| i)
        .collect();

    if large_indices.len() >= 3 {
        eprintln!(
            "GROUP: Trying large-pattern cluster: {} patterns (max={:.1})",
            large_indices.len(),
            max_size
        );
        let mut groups_large = build_groups(patterns, &large_indices);
        if !groups_large.is_empty() {
            return groups_large;
        }
    }

    // Fallback to median-based cluster
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

    eprintln!(
        "GROUP: Starting with {} patterns, {} after size filtering (median={:.1})",
        patterns.len(),
        valid_indices.len(),
        median_size
    );

    if valid_indices.len() < 3 {
        return groups;
    }

    groups = build_groups(patterns, &valid_indices);
    eprintln!("GROUP: Finished with {} groups", groups.len());
    groups
}

fn build_groups(patterns: &[FinderPattern], indices: &[usize]) -> Vec<Vec<usize>> {
    let mut groups = Vec::new();
    let mut used = vec![false; patterns.len()];

    for idx_i in 0..indices.len() {
        let i = indices[idx_i];
        if used[i] {
            continue;
        }
        for idx_j in (idx_i + 1)..indices.len() {
            let j = indices[idx_j];
            if used[j] {
                continue;
            }
            for idx_k in (idx_j + 1)..indices.len() {
                let k = indices[idx_k];
                if used[k] {
                    continue;
                }

                let pi = &patterns[i];
                let pj = &patterns[j];
                let pk = &patterns[k];

                let sizes = [pi.module_size, pj.module_size, pk.module_size];
                let min_size = sizes.iter().fold(f32::INFINITY, |a, &b| a.min(b));
                let max_size = sizes.iter().fold(0.0f32, |a, &b| a.max(b));
                let size_ratio = max_size / min_size;

                if size_ratio < 0.33 || size_ratio > 3.0 {
                    continue;
                }

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

/// Detect QR codes from a pre-computed grayscale image
///
/// # Arguments
/// * `image` - Grayscale bytes (1 byte per pixel)
/// * `width` - Image width in pixels
/// * `height` - Image height in pixels
///
/// # Returns
/// Vector of detected QR codes
pub fn detect_from_grayscale(image: &[u8], width: usize, height: usize) -> Vec<QRCode> {
    // Step 1: Binarize
    let binary_adaptive = adaptive_binarize(image, width, height, 31);
    let binary_otsu = otsu_binarize(image, width, height);
    let binary = if width >= 800 || height >= 800 {
        binary_adaptive.clone()
    } else {
        binary_otsu.clone()
    };

    // Step 2: Detect finder patterns
    let mut finder_patterns = Vec::new();
    finder_patterns.extend(FinderDetector::detect(&binary_adaptive));
    finder_patterns.extend(FinderDetector::detect(&binary_otsu));

    // Step 3: Group finder patterns and decode QR codes
    let mut results = Vec::new();

    let groups = group_finder_patterns(&finder_patterns);

    for group in groups {
        if group.len() >= 3 {
            if let Some((tl, tr, bl, module_size)) = order_finder_patterns(
                &finder_patterns[group[0]],
                &finder_patterns[group[1]],
                &finder_patterns[group[2]],
            ) {
                if let Some(qr) = QrDecoder::decode_with_gray(
                    &binary,
                    image,
                    width,
                    height,
                    &tl,
                    &tr,
                    &bl,
                    module_size,
                ) {
                    results.push(qr);
                }
            }
        }
    }

    results
}

/// Detect QR codes using a reusable buffer pool (faster for batch processing)
///
/// This version uses pre-allocated buffers to avoid repeated memory allocations.
/// Use this when processing multiple images of similar size.
///
/// # Example
/// ```
/// use rust_qr::utils::memory_pool::BufferPool;
///
/// let mut pool = BufferPool::new();
/// let image = vec![0u8; 640 * 480 * 3]; // RGB image buffer
/// let codes = rust_qr::detect_with_pool(&image, 640, 480, &mut pool);
/// ```
pub fn detect_with_pool(
    image: &[u8],
    width: usize,
    height: usize,
    pool: &mut BufferPool,
) -> Vec<QRCode> {
    let pixel_count = width * height;

    // Step 1: Convert to grayscale using pre-allocated buffer
    let gray_buffer = pool.get_grayscale_buffer(pixel_count);
    rgb_to_grayscale_with_buffer(image, width, height, gray_buffer);

    // Step 2: Binarize (creates new BitMatrix - could also be pooled)
    let binary_adaptive = adaptive_binarize(gray_buffer, width, height, 31);
    let binary_otsu = otsu_binarize(gray_buffer, width, height);
    let binary = if width >= 800 || height >= 800 {
        binary_adaptive.clone()
    } else {
        binary_otsu.clone()
    };

    // Step 3: Detect finder patterns
    let mut finder_patterns = Vec::new();
    finder_patterns.extend(FinderDetector::detect(&binary_adaptive));
    finder_patterns.extend(FinderDetector::detect(&binary_otsu));

    // Step 4: Group and decode
    let mut results = Vec::new();

    let groups = group_finder_patterns(&finder_patterns);

    for group in groups {
        if group.len() >= 3 {
            if let Some((tl, tr, bl, module_size)) = order_finder_patterns(
                &finder_patterns[group[0]],
                &finder_patterns[group[1]],
                &finder_patterns[group[2]],
            ) {
                if let Some(qr) = QrDecoder::decode_with_gray(
                    &binary,
                    gray_buffer,
                    width,
                    height,
                    &tl,
                    &tr,
                    &bl,
                    module_size,
                ) {
                    results.push(qr);
                }
            }
        }
    }

    results
}

/// Detector with configuration options and optional buffer pool
pub struct Detector {
    /// Optional buffer pool for memory reuse
    pool: Option<BufferPool>,
}

impl Detector {
    /// Create a new detector with default settings
    pub fn new() -> Self {
        Self { pool: None }
    }

    /// Create a detector with buffer pooling enabled
    pub fn with_pool() -> Self {
        Self {
            pool: Some(BufferPool::new()),
        }
    }

    /// Create a detector with a specific pool capacity
    pub fn with_pool_capacity(capacity: usize) -> Self {
        Self {
            pool: Some(BufferPool::with_capacity(capacity)),
        }
    }

    /// Detect QR codes in an image
    pub fn detect(&mut self, image: &[u8], width: usize, height: usize) -> Vec<QRCode> {
        match &mut self.pool {
            Some(pool) => detect_with_pool(image, width, height, pool),
            None => detect(image, width, height),
        }
    }

    /// Detect a single QR code (faster if you know there's only one)
    pub fn detect_single(&mut self, image: &[u8], width: usize, height: usize) -> Option<QRCode> {
        let codes = self.detect(image, width, height);
        codes.into_iter().next()
    }

    /// Clear the internal buffer pool (keeps capacity)
    pub fn clear_pool(&mut self) {
        if let Some(pool) = &mut self.pool {
            pool.clear();
        }
    }
}

impl Default for Detector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::qr_decoder::QrDecoder;
    use crate::utils::binarization::adaptive_binarize;
    use crate::utils::geometry::PerspectiveTransform;
    use crate::utils::grayscale::rgb_to_grayscale;
    use std::fs;
    use std::path::Path;

    #[test]
    fn test_detect_empty() {
        // Test with empty image
        let image = vec![0u8; 300]; // 10x10 RGB
        let codes = detect(&image, 10, 10);
        assert!(codes.is_empty());
    }

    #[test]
    fn test_real_qr() {
        // Load a real QR code image and see how many finder patterns we detect
        let img_path = "benches/images/boofcv/monitor/image001.jpg";
        let img = image::open(img_path).expect("Failed to load image");
        let rgb_img = img.to_rgb8();
        let (width, height) = (rgb_img.width() as usize, rgb_img.height() as usize);

        println!("Loaded image: {}x{} pixels", width, height);

        // Convert to flat RGB buffer
        let rgb_bytes: Vec<u8> = rgb_img.into_raw();

        // Convert to grayscale
        let gray = rgb_to_grayscale(&rgb_bytes, width, height);
        println!("Converted to grayscale: {} bytes", gray.len());

        // Binarize
        let binary = otsu_binarize(&gray, width, height);
        println!("Binarized: {}x{} matrix", binary.width(), binary.height());

        // Detect finder patterns
        let patterns = FinderDetector::detect(&binary);
        println!("Found {} finder patterns:", patterns.len());

        for (i, p) in patterns.iter().enumerate() {
            println!(
                "  Pattern {}: center=({:.1}, {:.1}), module_size={:.2}",
                i, p.center.x, p.center.y, p.module_size
            );
        }

        // Also try grouping to see how many valid groups we get
        let groups = group_finder_patterns(&patterns);
        println!("Formed {} valid groups of 3 patterns", groups.len());

        // Assert at least something to make the test fail visibly if we find nothing
        assert!(
            !patterns.is_empty(),
            "Expected to find at least 3 finder patterns, found {}",
            patterns.len()
        );
    }

    fn order_points(points: &[Point; 4]) -> (Point, Point, Point, Point) {
        let mut tl = points[0];
        let mut tr = points[0];
        let mut br = points[0];
        let mut bl = points[0];

        let mut min_sum = f32::INFINITY;
        let mut max_sum = f32::NEG_INFINITY;
        let mut min_diff = f32::INFINITY;
        let mut max_diff = f32::NEG_INFINITY;

        for &p in points.iter() {
            let sum = p.x + p.y;
            let diff = p.x - p.y;
            if sum < min_sum {
                min_sum = sum;
                tl = p;
            }
            if sum > max_sum {
                max_sum = sum;
                br = p;
            }
            if diff < min_diff {
                min_diff = diff;
                bl = p;
            }
            if diff > max_diff {
                max_diff = diff;
                tr = p;
            }
        }

        (tl, tr, br, bl)
    }

    fn load_points(txt_path: &str) -> Option<[Point; 4]> {
        let content = fs::read_to_string(txt_path).ok()?;
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
        if vals.len() < 8 {
            return None;
        }
        Some([
            Point::new(vals[0], vals[1]),
            Point::new(vals[2], vals[3]),
            Point::new(vals[4], vals[5]),
            Point::new(vals[6], vals[7]),
        ])
    }

    #[test]
    fn test_real_qr_decode_image001() {
        let image_path = "benches/images/boofcv/monitor/image001.jpg";
        let points_path = "benches/images/boofcv/monitor/image001.txt";

        assert!(
            Path::new(image_path).exists(),
            "Missing image: {}",
            image_path
        );
        assert!(
            Path::new(points_path).exists(),
            "Missing points: {}",
            points_path
        );

        let points = load_points(points_path).expect("Failed to parse points");
        let (tl, tr, br, bl) = order_points(&points);

        let img = image::open(image_path).expect("Failed to open image");
        let rgb = img.to_rgb8();
        let (width, height) = rgb.dimensions();
        let raw = rgb.into_raw();
        let gray = rgb_to_grayscale(&raw, width as usize, height as usize);
        let binary = adaptive_binarize(&gray, width as usize, height as usize, 31);

        let offsets = [0.0f32, 0.5, 1.0];
        let expected = "4376471154038";
        // 13-digit numeric fits in Version 1-M; try a small range to keep the test fast
        for version in 1..=3u8 {
            let dimension = 17 + 4 * version as usize;
            for &offset in &offsets {
                let src_min = offset;
                let src_max = dimension as f32 - offset;
                let src = [
                    Point::new(src_min, src_min),
                    Point::new(src_max, src_min),
                    Point::new(src_max, src_max),
                    Point::new(src_min, src_max),
                ];
                let dst = [tl, tr, br, bl];
                let transform = match PerspectiveTransform::from_points(&src, &dst) {
                    Some(t) => t,
                    None => continue,
                };

                let tl_f = transform.transform(&Point::new(3.5, 3.5));
                let tr_f = transform.transform(&Point::new(dimension as f32 - 3.5, 3.5));
                let bl_f = transform.transform(&Point::new(3.5, dimension as f32 - 3.5));
                let module_size = tl_f.distance(&tr_f) / (dimension as f32 - 7.0);

                if let Some(qr) = QrDecoder::decode_with_gray(
                    &binary,
                    &gray,
                    width as usize,
                    height as usize,
                    &tl_f,
                    &tr_f,
                    &bl_f,
                    module_size,
                ) {
                    if qr.content == expected {
                        return;
                    }
                }
            }
        }

        panic!("Expected payload not decoded: {}", expected);
    }

    #[test]
    fn test_smoke_real_images() {
        // Quick smoke test: run full detect() pipeline on a few nominal images
        // Just check that we find at least one QR code (no content check)
        let base = "benches/images/boofcv/nominal";
        if !Path::new(base).exists() {
            eprintln!("Skipping: benchmark images not found");
            return;
        }

        let images = ["image001.jpg", "image002.jpg", "image003.jpg"];
        let mut decoded = 0;
        for name in &images {
            let path = format!("{}/{}", base, name);
            if !Path::new(&path).exists() {
                continue;
            }
            let img = image::open(&path).expect("Failed to open image");
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            let raw = rgb.into_raw();
            let results = detect(&raw, w as usize, h as usize);
            if !results.is_empty() {
                decoded += 1;
                eprintln!("  {}: decoded {} QR code(s), first content={:?}",
                    name, results.len(), &results[0].content);
            } else {
                eprintln!("  {}: no QR codes found", name);
            }
        }
        eprintln!("Decoded {}/{} images", decoded, images.len());
        // At least one should decode if our fixes are working
        assert!(decoded > 0, "Expected to decode at least 1 real image");
    }
}
