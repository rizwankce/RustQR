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
use utils::binarization::{adaptive_binarize, adaptive_binarize_into, otsu_binarize, otsu_binarize_into};
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

    // Step 2: Binarize (adaptive first on large images, Otsu on small)
    let mut binary = if width >= 800 || height >= 800 {
        adaptive_binarize(&gray, width, height, 31)
    } else {
        otsu_binarize(&gray, width, height)
    };

    // Step 3: Detect finder patterns
    // Use pyramid detection for very large images (1600px+) for better performance
    let mut finder_patterns = if width >= 1600 && height >= 1600 {
        FinderDetector::detect_with_pyramid(&binary)
    } else {
        FinderDetector::detect(&binary)
    };

    // Fallback: if adaptive yields too few patterns, try Otsu (or vice-versa)
    if finder_patterns.len() < 3 {
        let fallback = if width >= 800 || height >= 800 {
            otsu_binarize(&gray, width, height)
        } else {
            adaptive_binarize(&gray, width, height, 31)
        };
        let fallback_patterns = if width >= 1600 && height >= 1600 {
            FinderDetector::detect_with_pyramid(&fallback)
        } else {
            FinderDetector::detect(&fallback)
        };
        if fallback_patterns.len() >= 3 {
            binary = fallback;
            finder_patterns = fallback_patterns;
        }
    }

    let mut results = decode_groups(&binary, &gray, width, height, &finder_patterns);

    // Final fallback: if no decode, try the other binarizer end-to-end
    if results.is_empty() {
        let fallback = if width >= 800 || height >= 800 {
            otsu_binarize(&gray, width, height)
        } else {
            adaptive_binarize(&gray, width, height, 31)
        };
        let fallback_patterns = if width >= 1600 && height >= 1600 {
            FinderDetector::detect_with_pyramid(&fallback)
        } else {
            FinderDetector::detect(&fallback)
        };
        if fallback_patterns.len() >= 3 {
            results = decode_groups(&fallback, &gray, width, height, &fallback_patterns);
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
    let module_ratio = module_size / avg_module;
    if !(0.8..=1.2).contains(&module_ratio) {
        return None;
    }

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
    if patterns.len() < 3 {
        return Vec::new();
    }

    let mut indexed: Vec<(usize, f32)> = patterns
        .iter()
        .enumerate()
        .map(|(i, p)| (i, p.module_size))
        .collect();
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    let mut bins: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut bin_min = 0.0f32;
    let bin_ratio = 1.25f32;

    for (idx, size) in indexed {
        if current.is_empty() {
            current.push(idx);
            bin_min = size;
            continue;
        }

        if size <= bin_min * bin_ratio {
            current.push(idx);
        } else {
            bins.push(current);
            current = vec![idx];
            bin_min = size;
        }
    }
    if !current.is_empty() {
        bins.push(current);
    }

    #[cfg(debug_assertions)]
    eprintln!("GROUP: Binned into {} size buckets", bins.len());

    // Try each bin and its neighbor to allow slight size mismatch
    for i in 0..bins.len() {
        let mut indices = bins[i].clone();
        if i + 1 < bins.len() {
            indices.extend_from_slice(&bins[i + 1]);
        }
        if indices.len() < 3 {
            continue;
        }
        let groups = build_groups(patterns, &indices);
        if !groups.is_empty() {
            return groups;
        }
    }

    Vec::new()
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

                if size_ratio > 1.5 {
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

fn score_and_trim_groups(
    groups: &mut Vec<Vec<usize>>,
    patterns: &[FinderPattern],
    max_groups: usize,
) {
    if groups.len() <= max_groups {
        return;
    }

    groups.sort_by(|a, b| {
        let sa = group_score(patterns, a);
        let sb = group_score(patterns, b);
        sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
    });
    groups.truncate(max_groups);
}

fn group_score(patterns: &[FinderPattern], group: &[usize]) -> f32 {
    if group.len() < 3 {
        return f32::INFINITY;
    }
    let p0 = &patterns[group[0]];
    let p1 = &patterns[group[1]];
    let p2 = &patterns[group[2]];

    let sizes = [p0.module_size, p1.module_size, p2.module_size];
    let min_size = sizes.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let max_size = sizes.iter().fold(0.0f32, |a, &b| a.max(b));
    let size_ratio = max_size / min_size;

    let d01 = p0.center.distance(&p1.center);
    let d02 = p0.center.distance(&p2.center);
    let d12 = p1.center.distance(&p2.center);
    let distances = [d01, d02, d12];
    let min_d = distances.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let max_d = distances.iter().fold(0.0f32, |a, &b| a.max(b));
    let distortion = max_d / min_d;

    // Prefer near-right angle (small cosine) and size consistency
    let a2 = d01 * d01;
    let b2 = d02 * d02;
    let c2 = d12 * d12;
    let cos_i = ((a2 + b2 - c2) / (2.0 * d01 * d02)).abs();
    let cos_j = ((a2 + c2 - b2) / (2.0 * d01 * d12)).abs();
    let cos_k = ((b2 + c2 - a2) / (2.0 * d02 * d12)).abs();
    let best_cos = cos_i.min(cos_j).min(cos_k);

    size_ratio * 2.0 + distortion + best_cos
}

fn decode_groups(
    binary: &BitMatrix,
    gray: &[u8],
    width: usize,
    height: usize,
    finder_patterns: &[FinderPattern],
) -> Vec<QRCode> {
    let mut results = Vec::new();
    let mut groups = group_finder_patterns(finder_patterns);
    score_and_trim_groups(&mut groups, finder_patterns, 40);

    #[cfg(debug_assertions)]
    eprintln!(
        "DEBUG: Found {} finder patterns, formed {} groups",
        finder_patterns.len(),
        groups.len()
    );

    for (group_idx, group) in groups.iter().enumerate() {
        if group.len() < 3 {
            continue;
        }
        #[cfg(debug_assertions)]
        eprintln!("DEBUG: Trying group {} with patterns {:?}", group_idx, group);

        if let Some((tl, tr, bl, module_size)) = order_finder_patterns(
            &finder_patterns[group[0]],
            &finder_patterns[group[1]],
            &finder_patterns[group[2]],
        ) {
            match QrDecoder::decode_with_gray(
                binary,
                gray,
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

    results
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
    let mut binary = if width >= 800 || height >= 800 {
        adaptive_binarize(image, width, height, 31)
    } else {
        otsu_binarize(image, width, height)
    };

    // Step 2: Detect finder patterns
    let mut finder_patterns = FinderDetector::detect(&binary);
    if finder_patterns.len() < 3 {
        let fallback = if width >= 800 || height >= 800 {
            otsu_binarize(image, width, height)
        } else {
            adaptive_binarize(image, width, height, 31)
        };
        let fallback_patterns = FinderDetector::detect(&fallback);
        if fallback_patterns.len() >= 3 {
            binary = fallback;
            finder_patterns = fallback_patterns;
        }
    }

    // Step 3: Group finder patterns and decode QR codes
    let mut results = decode_groups(&binary, image, width, height, &finder_patterns);

    if results.is_empty() {
        let fallback = if width >= 800 || height >= 800 {
            otsu_binarize(image, width, height)
        } else {
            adaptive_binarize(image, width, height, 31)
        };
        let fallback_patterns = FinderDetector::detect(&fallback);
        if fallback_patterns.len() >= 3 {
            results = decode_groups(&fallback, image, width, height, &fallback_patterns);
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
    // Get all buffers at once via split borrowing
    let (gray_buffer, bin_adaptive, bin_otsu, integral) = pool.get_all_buffers(width, height);

    // Step 1: Convert to grayscale using pre-allocated buffer
    rgb_to_grayscale_with_buffer(image, width, height, gray_buffer);

    // Step 2: Binarize into pooled BitMatrix buffers
    adaptive_binarize_into(gray_buffer, width, height, 31, bin_adaptive, integral);
    otsu_binarize_into(gray_buffer, width, height, bin_otsu);

    // Step 3: Detect finder patterns
    let mut finder_patterns = if width >= 800 || height >= 800 {
        FinderDetector::detect(bin_adaptive)
    } else {
        FinderDetector::detect(bin_otsu)
    };

    // Select which binary image to use for decoding (no clone needed — just a reference)
    let mut binary: &BitMatrix = if width >= 800 || height >= 800 {
        bin_adaptive
    } else {
        bin_otsu
    };

    if finder_patterns.len() < 3 {
        let fallback_patterns = if width >= 800 || height >= 800 {
            FinderDetector::detect(bin_otsu)
        } else {
            FinderDetector::detect(bin_adaptive)
        };
        if fallback_patterns.len() >= 3 {
            finder_patterns = fallback_patterns;
            binary = if width >= 800 || height >= 800 {
                bin_otsu
            } else {
                bin_adaptive
            };
        }
    }

    // Step 4: Group and decode
    let mut results = decode_groups(binary, gray_buffer, width, height, &finder_patterns);

    if results.is_empty() {
        let fallback_patterns = if width >= 800 || height >= 800 {
            FinderDetector::detect(bin_otsu)
        } else {
            FinderDetector::detect(bin_adaptive)
        };
        if fallback_patterns.len() >= 3 {
            let fallback_binary: &BitMatrix = if width >= 800 || height >= 800 {
                bin_otsu
            } else {
                bin_adaptive
            };
            results = decode_groups(fallback_binary, gray_buffer, width, height, &fallback_patterns);
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

}
