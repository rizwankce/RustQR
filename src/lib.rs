//! RustQR - World's fastest QR code scanning library
//!
//! A pure Rust QR code detection and decoding library with zero dependencies.
//! Designed for maximum speed and cross-platform compatibility.

#![allow(missing_docs)]
#![allow(clippy::missing_docs_in_private_items)]

/// Debug helpers (env-driven)
pub(crate) mod debug;
/// QR code decoding modules (error correction, format extraction, data modes)
pub mod decoder;
/// QR code detection modules (finder patterns, alignment, timing)
pub mod detector;
/// Core data structures (QRCode, BitMatrix, Point, etc.)
pub mod models;
mod pipeline;
/// CLI/bench helpers (feature-gated)
#[cfg(feature = "tools")]
pub mod tools;
/// Utility functions (grayscale, binarization, geometry)
pub mod utils;

pub use models::{BitMatrix, ECLevel, MaskPattern, Point, QRCode, Version};

/// Per-image telemetry tracking which pipeline stages succeeded or failed.
///
/// Every stage records its highest-water-mark count across all binarization
/// strategies tried (primary + fallback).
#[derive(Debug, Clone, Default)]
pub struct DetectionTelemetry {
    /// Whether binarization produced a non-empty binary matrix.
    pub binarize_ok: bool,
    /// Peak number of finder patterns detected across all binarization attempts.
    pub finder_patterns_found: usize,
    /// Peak number of valid groups (triplets) formed from finder patterns.
    pub groups_found: usize,
    /// Number of groups where a perspective transform could be built.
    pub transforms_built: usize,
    /// Number of groups where format info was extractable from the sampled grid.
    pub format_extracted: usize,
    /// Number of groups where Reed-Solomon decoding succeeded.
    pub rs_decode_ok: usize,
    /// Number of QR codes whose payload parsed into valid content.
    pub payload_decoded: usize,
    /// Number of decoder attempts made (one per transform/group decode try).
    pub decode_attempts: usize,
    /// Total candidate groups scored before trimming.
    pub candidate_groups_scored: usize,
    /// Histogram of candidate group scores:
    /// [<2.0, 2.0-<3.0, 3.0-<5.0, >=5.0]
    pub candidate_score_buckets: [usize; 4],
    /// The final detection result count.
    pub qr_codes_found: usize,
}

impl DetectionTelemetry {
    pub(crate) fn add_candidate_score(&mut self, score: f32) {
        let idx = if score < 2.0 {
            0
        } else if score < 3.0 {
            1
        } else if score < 5.0 {
            2
        } else {
            3
        };
        self.candidate_score_buckets[idx] += 1;
    }

    fn merge_high_water_from(&mut self, other: &Self) {
        self.groups_found = self.groups_found.max(other.groups_found);
        self.transforms_built = self.transforms_built.max(other.transforms_built);
        self.format_extracted = self.format_extracted.max(other.format_extracted);
        self.rs_decode_ok = self.rs_decode_ok.max(other.rs_decode_ok);
        self.payload_decoded = self.payload_decoded.max(other.payload_decoded);
        self.decode_attempts += other.decode_attempts;
        self.candidate_groups_scored += other.candidate_groups_scored;
        for i in 0..self.candidate_score_buckets.len() {
            self.candidate_score_buckets[i] += other.candidate_score_buckets[i];
        }
    }
}

use detector::contour::ContourDetector;
use detector::finder::{FinderDetector, FinderPattern};
use utils::binarization::{
    adaptive_binarize, adaptive_binarize_into, otsu_binarize, otsu_binarize_into, sauvola_binarize,
    threshold_binarize,
};
use utils::grayscale::{rgb_to_grayscale, rgb_to_grayscale_with_buffer};
use utils::memory_pool::BufferPool;

fn auto_window(width: usize, height: usize) -> usize {
    let base = (width.min(height) / 24).max(31);
    if base % 2 == 0 { base + 1 } else { base }
}

fn contrast_stretch(gray: &[u8]) -> Vec<u8> {
    if gray.is_empty() {
        return Vec::new();
    }

    let mut min_v = u8::MAX;
    let mut max_v = u8::MIN;
    for &v in gray {
        min_v = min_v.min(v);
        max_v = max_v.max(v);
    }

    if max_v <= min_v + 8 {
        return gray.to_vec();
    }

    let range = (max_v - min_v) as f32;
    gray.iter()
        .map(|&v| (((v.saturating_sub(min_v)) as f32 / range) * 255.0).round() as u8)
        .collect()
}

fn rotate_gray_45(gray: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut out = vec![255u8; width * height];
    let cx = (width as f32 - 1.0) * 0.5;
    let cy = (height as f32 - 1.0) * 0.5;
    let theta = 45.0f32.to_radians();
    let cos_t = theta.cos();
    let sin_t = theta.sin();

    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let src_x = cos_t * dx + sin_t * dy + cx;
            let src_y = -sin_t * dx + cos_t * dy + cy;
            let sx = src_x.round() as isize;
            let sy = src_y.round() as isize;
            if sx >= 0 && sy >= 0 && (sx as usize) < width && (sy as usize) < height {
                out[y * width + x] = gray[sy as usize * width + sx as usize];
            }
        }
    }

    out
}

fn run_detection_strategies(gray: &[u8], width: usize, height: usize) -> Vec<QRCode> {
    let window = auto_window(width, height);
    let otsu = otsu_binarize(gray, width, height);
    let adaptive = adaptive_binarize(gray, width, height, window);
    let sauvola_k02 = sauvola_binarize(gray, width, height, window, 0.2);
    let sauvola_k01 = sauvola_binarize(gray, width, height, window, 0.1);
    let sauvola_k03 = sauvola_binarize(gray, width, height, window, 0.3);

    let mut variants = vec![sauvola_k02, adaptive, otsu];

    let mut sorted = gray.to_vec();
    sorted.sort_unstable();
    let median = sorted[sorted.len() / 2] as i16;
    let t_dark = (median - 26).clamp(0, 255) as u8;
    let t_light = (median + 26).clamp(0, 255) as u8;
    variants.push(threshold_binarize(gray, width, height, t_dark));
    variants.push(threshold_binarize(gray, width, height, t_light));

    variants.push(sauvola_k01);
    variants.push(sauvola_k03);

    let mut results = Vec::new();
    for binary in variants {
        let finder_patterns = detect_finder_patterns(&binary, width, height);
        let decoded = if finder_patterns.len() >= 3 {
            decode_groups_with_module_aware_retry(&binary, gray, width, height, &finder_patterns)
        } else {
            Vec::new()
        };
        for qr in decoded {
            if !results.iter().any(|r: &QRCode| r.content == qr.content) {
                results.push(qr);
            }
        }
        if results.is_empty() {
            let contour_patterns = ContourDetector::detect(&binary);
            if contour_patterns.len() >= 3 {
                let contour_decoded =
                    pipeline::decode_groups(&binary, gray, width, height, &contour_patterns);
                for qr in contour_decoded {
                    if !results.iter().any(|r: &QRCode| r.content == qr.content) {
                        results.push(qr);
                    }
                }
            }
        }
        if !results.is_empty() {
            return results;
        }
    }

    results
}

fn detect_finder_patterns(binary: &BitMatrix, width: usize, height: usize) -> Vec<FinderPattern> {
    if width >= 1600 && height >= 1600 {
        FinderDetector::detect_with_pyramid(binary)
    } else {
        FinderDetector::detect(binary)
    }
}

fn adaptive_window_from_module_size(module_size: f32) -> usize {
    let base = (module_size * 7.0).round() as usize;
    let clamped = base.clamp(31, 151);
    if clamped % 2 == 0 {
        clamped + 1
    } else {
        clamped
    }
}

fn decode_groups_with_module_aware_retry(
    binary: &BitMatrix,
    gray: &[u8],
    width: usize,
    height: usize,
    finder_patterns: &[FinderPattern],
) -> Vec<QRCode> {
    let mut results = pipeline::decode_groups(binary, gray, width, height, finder_patterns);
    if !results.is_empty() || finder_patterns.len() < 3 {
        return results;
    }

    let mut module_sizes: Vec<f32> = finder_patterns.iter().map(|p| p.module_size).collect();
    module_sizes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_module = module_sizes[module_sizes.len() / 2];
    let window = adaptive_window_from_module_size(median_module);

    let retry_binary = adaptive_binarize(gray, width, height, window);
    let retry_patterns = detect_finder_patterns(&retry_binary, width, height);
    if retry_patterns.len() < 3 {
        return results;
    }

    results = pipeline::decode_groups(&retry_binary, gray, width, height, &retry_patterns);
    results
}

fn run_fast_path(gray: &[u8], width: usize, height: usize) -> Vec<QRCode> {
    // Fast path: one cheap global threshold pass only.
    let binary = otsu_binarize(gray, width, height);
    let finder_patterns = detect_finder_patterns(&binary, width, height);
    if finder_patterns.len() < 3 {
        return Vec::new();
    }
    pipeline::decode_groups(&binary, gray, width, height, &finder_patterns)
}

fn run_detection_with_phase4_fallbacks(gray: &[u8], width: usize, height: usize) -> Vec<QRCode> {
    let mut results = run_detection_strategies(gray, width, height);
    if !results.is_empty() {
        return results;
    }

    let enhanced = contrast_stretch(gray);
    results = run_detection_strategies(&enhanced, width, height);
    if !results.is_empty() {
        return results;
    }

    let rotated = rotate_gray_45(gray, width, height);
    run_detection_strategies(&rotated, width, height)
}

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
    let fast = run_fast_path(&gray, width, height);
    if !fast.is_empty() {
        return fast;
    }

    run_detection_with_phase4_fallbacks(&gray, width, height)
}

/// Detect QR codes in an RGB image, returning telemetry about which pipeline
/// stages succeeded or failed. This is intended for benchmark diagnostics.
///
/// The telemetry records the high-water-mark across all binarization attempts
/// so callers can determine *where* the pipeline stalls for a given image.
pub fn detect_with_telemetry(
    image: &[u8],
    width: usize,
    height: usize,
) -> (Vec<QRCode>, DetectionTelemetry) {
    let mut tel = DetectionTelemetry::default();

    // Step 1: Convert to grayscale
    let gray = rgb_to_grayscale(image, width, height);

    // Step 2: Binarize (adaptive first on large images, Otsu on small)
    let mut binary = if width >= 800 || height >= 800 {
        adaptive_binarize(&gray, width, height, 31)
    } else {
        otsu_binarize(&gray, width, height)
    };
    tel.binarize_ok = true;

    // Step 3: Detect finder patterns
    let mut finder_patterns = if width >= 1600 && height >= 1600 {
        FinderDetector::detect_with_pyramid(&binary)
    } else {
        FinderDetector::detect(&binary)
    };
    tel.finder_patterns_found = finder_patterns.len();

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
        tel.finder_patterns_found = tel.finder_patterns_found.max(fallback_patterns.len());
        if fallback_patterns.len() >= 3 {
            binary = fallback;
            finder_patterns = fallback_patterns;
        }
    }

    let (mut results, decode_tel) =
        pipeline::decode_groups_with_telemetry(&binary, &gray, width, height, &finder_patterns);
    tel.merge_high_water_from(&decode_tel);

    // Sauvola fallback: adapts to local contrast (handles shadows/glare)
    if results.is_empty() {
        let sauvola = sauvola_binarize(&gray, width, height, 31, 0.2);
        let sauvola_patterns = if width >= 1600 && height >= 1600 {
            FinderDetector::detect_with_pyramid(&sauvola)
        } else {
            FinderDetector::detect(&sauvola)
        };
        tel.finder_patterns_found = tel.finder_patterns_found.max(sauvola_patterns.len());
        if sauvola_patterns.len() >= 3 {
            let (sv_results, sv_tel) = pipeline::decode_groups_with_telemetry(
                &sauvola,
                &gray,
                width,
                height,
                &sauvola_patterns,
            );
            tel.merge_high_water_from(&sv_tel);
            results = sv_results;
        }
    }

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
        tel.finder_patterns_found = tel.finder_patterns_found.max(fallback_patterns.len());
        if fallback_patterns.len() >= 3 {
            let (fb_results, fb_tel) = pipeline::decode_groups_with_telemetry(
                &fallback,
                &gray,
                width,
                height,
                &fallback_patterns,
            );
            tel.merge_high_water_from(&fb_tel);
            results = fb_results;
        }
    }

    tel.qr_codes_found = results.len();
    (results, tel)
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
    let fast = run_fast_path(image, width, height);
    if !fast.is_empty() {
        return fast;
    }

    run_detection_with_phase4_fallbacks(image, width, height)
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

    // Fast path: one Otsu pass and decode.
    let fast = run_fast_path(gray_buffer, width, height);
    if !fast.is_empty() {
        return fast;
    }

    // Slow path: additional strategies.
    // Step 2: Binarize into pooled BitMatrix buffers
    adaptive_binarize_into(gray_buffer, width, height, 31, bin_adaptive, integral);
    otsu_binarize_into(gray_buffer, width, height, bin_otsu);

    // Step 3: Detect finder patterns
    let mut finder_patterns = if width >= 800 || height >= 800 {
        detect_finder_patterns(bin_adaptive, width, height)
    } else {
        detect_finder_patterns(bin_otsu, width, height)
    };

    // Select which binary image to use for decoding (no clone needed — just a reference)
    let mut binary: &BitMatrix = if width >= 800 || height >= 800 {
        bin_adaptive
    } else {
        bin_otsu
    };

    if finder_patterns.len() < 3 {
        let fallback_patterns = if width >= 800 || height >= 800 {
            detect_finder_patterns(bin_otsu, width, height)
        } else {
            detect_finder_patterns(bin_adaptive, width, height)
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
    let mut results =
        decode_groups_with_module_aware_retry(binary, gray_buffer, width, height, &finder_patterns);

    // Sauvola fallback: adapts to local contrast (handles shadows/glare)
    if results.is_empty() {
        let sauvola = sauvola_binarize(gray_buffer, width, height, 31, 0.2);
        let sauvola_patterns = detect_finder_patterns(&sauvola, width, height);
        if sauvola_patterns.len() >= 3 {
            results = decode_groups_with_module_aware_retry(
                &sauvola,
                gray_buffer,
                width,
                height,
                &sauvola_patterns,
            );
        }
    }

    if results.is_empty() {
        let fallback_patterns = if width >= 800 || height >= 800 {
            detect_finder_patterns(bin_otsu, width, height)
        } else {
            detect_finder_patterns(bin_adaptive, width, height)
        };
        if fallback_patterns.len() >= 3 {
            let fallback_binary: &BitMatrix = if width >= 800 || height >= 800 {
                bin_otsu
            } else {
                bin_adaptive
            };
            results = decode_groups_with_module_aware_retry(
                fallback_binary,
                gray_buffer,
                width,
                height,
                &fallback_patterns,
            );
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
    use image::GenericImageView;
    use std::env;

    fn test_max_dim(default: u32) -> u32 {
        match env::var("QR_MAX_DIM") {
            Ok(val) => match val.trim().parse::<u32>() {
                Ok(0) => u32::MAX,
                Ok(v) => v,
                Err(_) => default,
            },
            Err(_) => default,
        }
    }

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
        let (orig_w, orig_h) = img.dimensions();
        let max_dim = orig_w.max(orig_h);
        // Keep this smoke test fast in default `cargo test` runs.
        // Callers can still override with QR_MAX_DIM.
        let max_dim_limit = test_max_dim(800);
        let rgb_img = if max_dim > max_dim_limit {
            let scale = max_dim_limit as f32 / max_dim as f32;
            let new_w = (orig_w as f32 * scale).round().max(1.0) as u32;
            let new_h = (orig_h as f32 * scale).round().max(1.0) as u32;
            println!(
                "Downscaling image for test from {}x{} to {}x{}",
                orig_w, orig_h, new_w, new_h
            );
            let resized = img.resize(new_w, new_h, image::imageops::FilterType::Triangle);
            resized.to_rgb8()
        } else {
            img.to_rgb8()
        };
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
        let groups = pipeline::group_finder_patterns(&patterns);
        println!("Formed {} valid groups of 3 patterns", groups.len());

        // Assert at least something to make the test fail visibly if we find nothing
        assert!(
            !patterns.is_empty(),
            "Expected to find at least 3 finder patterns, found {}",
            patterns.len()
        );
    }
}
