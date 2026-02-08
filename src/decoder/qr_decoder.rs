/// Main QR code decoder - wires everything together
use crate::models::{BitMatrix, Point, QRCode};
use std::cell::RefCell;
use std::time::Instant;

mod geometry;
mod matrix_decode;
mod orientation;
mod payload;

/// Main QR decoder that processes a detected QR region
pub struct QrDecoder;

#[derive(Clone, Copy)]
pub(crate) struct DecodeCounters {
    pub deskew_attempts: usize,
    pub deskew_successes: usize,
    pub high_version_precision_attempts: usize,
    pub recovery_mode_attempts: usize,
    pub scale_retry_attempts: usize,
    pub scale_retry_successes: usize,
    pub scale_retry_skipped_by_budget: usize,
    pub hv_subpixel_attempts: usize,
    pub hv_refine_attempts: usize,
    pub hv_refine_successes: usize,
    pub rs_erasure_attempts: usize,
    pub rs_erasure_successes: usize,
    pub rs_erasure_count_hist: [usize; 4],
    pub phase11_time_budget_skips: usize,
}

impl DecodeCounters {
    const fn new() -> Self {
        Self {
            deskew_attempts: 0,
            deskew_successes: 0,
            high_version_precision_attempts: 0,
            recovery_mode_attempts: 0,
            scale_retry_attempts: 0,
            scale_retry_successes: 0,
            scale_retry_skipped_by_budget: 0,
            hv_subpixel_attempts: 0,
            hv_refine_attempts: 0,
            hv_refine_successes: 0,
            rs_erasure_attempts: 0,
            rs_erasure_successes: 0,
            rs_erasure_count_hist: [0; 4],
            phase11_time_budget_skips: 0,
        }
    }
}

impl Default for DecodeCounters {
    fn default() -> Self {
        Self::new()
    }
}

thread_local! {
    static DECODE_COUNTERS: RefCell<DecodeCounters> = const { RefCell::new(DecodeCounters::new()) };
}

pub(crate) fn reset_decode_counters() {
    DECODE_COUNTERS.with(|c| *c.borrow_mut() = DecodeCounters::new());
    payload::reset_erasure_counters();
    payload::reset_rs_erasure_global_counter();
}

pub(crate) fn take_decode_counters() -> DecodeCounters {
    let mut out = DecodeCounters::new();
    DECODE_COUNTERS.with(|c| {
        out = *c.borrow();
        *c.borrow_mut() = DecodeCounters::new();
    });
    let (rs_erasure_attempts, rs_erasure_successes, rs_erasure_count_hist) =
        payload::take_erasure_counters();
    out.rs_erasure_attempts = rs_erasure_attempts;
    out.rs_erasure_successes = rs_erasure_successes;
    out.rs_erasure_count_hist = rs_erasure_count_hist;
    out
}

impl QrDecoder {
    /// Decode a QR code from a binary matrix and finder pattern locations
    pub fn decode(
        matrix: &BitMatrix,
        top_left: &Point,
        top_right: &Point,
        bottom_left: &Point,
        module_size: f32,
    ) -> Option<QRCode> {
        if cfg!(debug_assertions) && crate::debug::debug_enabled() {
            eprintln!("    DECODE: module_size={:.2}", module_size);
        }

        // Calculate the bottom-right corner
        let bottom_right = Self::calculate_bottom_right(top_left, top_right, bottom_left)?;
        if cfg!(debug_assertions) && crate::debug::debug_enabled() {
            eprintln!(
                "    DECODE: bottom_right=({:.1}, {:.1})",
                bottom_right.x, bottom_right.y
            );
        }

        // Determine QR code dimension (version) estimate
        let estimated_dimension =
            Self::estimate_dimension(top_left, top_right, &bottom_right, module_size)?;
        if cfg!(debug_assertions) && crate::debug::debug_enabled() {
            eprintln!("    DECODE: estimated_dimension={}", estimated_dimension);
        }

        let estimated_version = ((estimated_dimension - 17) / 4) as i32;
        let candidates = Self::version_candidates(estimated_version);

        let mut br_candidates = Vec::new();
        let step = module_size.max(1.0) * 2.0;
        for dy in [-4.0f32, -2.0, 0.0, 2.0, 4.0] {
            for dx in [-4.0f32, -2.0, 0.0, 2.0, 4.0] {
                br_candidates.push(Point::new(
                    bottom_right.x + dx * step,
                    bottom_right.y + dy * step,
                ));
            }
        }

        for version_num in candidates {
            if version_num >= 7 {
                DECODE_COUNTERS.with(|c| c.borrow_mut().high_version_precision_attempts += 1);
            }
            let dimension = 17 + 4 * version_num as usize;
            for br in &br_candidates {
                let transform =
                    match Self::build_transform(top_left, top_right, bottom_left, br, dimension) {
                        Some(t) => t,
                        None => continue,
                    };
                let transform = Self::refine_transform_with_alignment(
                    matrix,
                    &transform,
                    version_num,
                    dimension,
                    module_size,
                    top_left,
                    top_right,
                    bottom_left,
                )
                .unwrap_or(transform);
                let qr_matrix =
                    Self::extract_qr_region_with_transform(matrix, &transform, dimension);

                if !orientation::validate_timing_patterns(&qr_matrix) {
                    continue;
                }

                if let Some(qr) = Self::decode_from_matrix(&qr_matrix, version_num) {
                    return Some(qr);
                }

                // Try inverted grid (binarization might be flipped)
                let inverted = orientation::invert_matrix(&qr_matrix);
                if let Some(qr) = Self::decode_from_matrix(&inverted, version_num) {
                    return Some(qr);
                }
            }
        }

        None
    }

    /// Decode using grayscale sampling to build the QR matrix (more robust for real photos).
    /// Skips expensive subpixel refinement for very blurry images (blur < threshold).
    #[allow(clippy::too_many_arguments)]
    pub fn decode_with_gray(
        binary: &BitMatrix,
        gray: &[u8],
        width: usize,
        height: usize,
        top_left: &Point,
        top_right: &Point,
        bottom_left: &Point,
        module_size: f32,
        allow_heavy_recovery: bool,
        blur_metric: f32,
    ) -> Option<QRCode> {
        let started = Instant::now();
        let candidate_budget_ms = crate::decoder::config::candidate_time_budget_ms();
        let budget_exhausted = || started.elapsed().as_millis() as u64 >= candidate_budget_ms;
        let bottom_right = Self::calculate_bottom_right(top_left, top_right, bottom_left)?;
        let mut br_candidates = Vec::new();
        let step = module_size.max(1.0) * 2.0;
        for dy in [-4.0f32, -2.0, 0.0, 2.0, 4.0] {
            for dx in [-4.0f32, -2.0, 0.0, 2.0, 4.0] {
                br_candidates.push(Point::new(
                    bottom_right.x + dx * step,
                    bottom_right.y + dy * step,
                ));
            }
        }
        let estimated_dimension =
            Self::estimate_dimension(top_left, top_right, &bottom_right, module_size)?;

        let estimated_version = ((estimated_dimension - 17) / 4) as i32;
        let candidates = Self::version_candidates(estimated_version);

        for version_num in candidates {
            let dimension = 17 + 4 * version_num as usize;
            for br in &br_candidates {
                let transform =
                    match Self::build_transform(top_left, top_right, bottom_left, br, dimension) {
                        Some(t) => t,
                        None => continue,
                    };
                // Skip expensive subpixel refinement for very blurry images
                let blur_threshold = crate::decoder::config::blur_disable_recovery_threshold();
                let should_do_subpixel = version_num >= 7 && blur_metric >= blur_threshold;
                
                let transform = if should_do_subpixel {
                    Self::refine_transform_with_alignment(
                        binary,
                        &transform,
                        version_num,
                        dimension,
                        module_size,
                        top_left,
                        top_right,
                        bottom_left,
                    )
                    .unwrap_or(transform)
                } else {
                    transform
                };

                let (qr_matrix, module_confidence) =
                    Self::extract_qr_region_gray_with_transform_and_confidence(
                        gray, width, height, &transform, dimension,
                    );
                if should_do_subpixel {
                    DECODE_COUNTERS.with(|c| c.borrow_mut().hv_subpixel_attempts += 1);
                }
                if !orientation::validate_timing_patterns(&qr_matrix) {
                    continue;
                }

                if let Some(qr) = Self::decode_from_matrix_with_confidence(
                    &qr_matrix,
                    version_num,
                    &module_confidence,
                ) {
                    return Some(qr);
                }

                let inverted = orientation::invert_matrix(&qr_matrix);
                if let Some(qr) = Self::decode_from_matrix_with_confidence(
                    &inverted,
                    version_num,
                    &module_confidence,
                ) {
                    return Some(qr);
                }

                let should_scale_retry = module_size <= 2.4 || version_num >= 7 || dimension >= 85;
                if allow_heavy_recovery && should_scale_retry && !budget_exhausted() {
                    for &scale in &[1.25f32, 1.5f32] {
                        if budget_exhausted() {
                            DECODE_COUNTERS.with(|c| c.borrow_mut().phase11_time_budget_skips += 1);
                            break;
                        }
                        DECODE_COUNTERS.with(|c| c.borrow_mut().scale_retry_attempts += 1);
                        let (scaled_matrix, scaled_conf) =
                            Self::extract_qr_region_gray_with_transform_and_confidence_scaled(
                                gray, width, height, &transform, dimension, scale,
                            );
                        if !orientation::validate_timing_patterns(&scaled_matrix) {
                            continue;
                        }
                        if let Some(qr) = Self::decode_from_matrix_with_confidence(
                            &scaled_matrix,
                            version_num,
                            &scaled_conf,
                        ) {
                            DECODE_COUNTERS.with(|c| c.borrow_mut().scale_retry_successes += 1);
                            return Some(qr);
                        }
                        let scaled_inverted = orientation::invert_matrix(&scaled_matrix);
                        if let Some(qr) = Self::decode_from_matrix_with_confidence(
                            &scaled_inverted,
                            version_num,
                            &scaled_conf,
                        ) {
                            DECODE_COUNTERS.with(|c| c.borrow_mut().scale_retry_successes += 1);
                            return Some(qr);
                        }
                    }
                } else {
                    DECODE_COUNTERS.with(|c| c.borrow_mut().scale_retry_skipped_by_budget += 1);
                }

                if allow_heavy_recovery && version_num >= 7 && !budget_exhausted() {
                    DECODE_COUNTERS.with(|c| c.borrow_mut().hv_refine_attempts += 1);
                    if let Some(refined_hv_transform) = Self::refine_transform_with_alignment(
                        binary,
                        &transform,
                        version_num,
                        dimension,
                        (module_size * 0.9).max(1.0),
                        top_left,
                        top_right,
                        bottom_left,
                    ) {
                        let (hv_matrix, hv_conf) =
                            Self::extract_qr_region_gray_with_transform_and_confidence_scaled(
                                gray,
                                width,
                                height,
                                &refined_hv_transform,
                                dimension,
                                1.35,
                            );
                        if orientation::validate_timing_patterns(&hv_matrix) {
                            if let Some(qr) = Self::decode_from_matrix_with_confidence(
                                &hv_matrix,
                                version_num,
                                &hv_conf,
                            ) {
                                DECODE_COUNTERS.with(|c| c.borrow_mut().hv_refine_successes += 1);
                                return Some(qr);
                            }
                        }
                    }
                }

                // Rotation-specialized deskew fallback: apply a bounded mesh warp variant
                // only after strict decode misses.
                if allow_heavy_recovery && version_num >= 2 && !budget_exhausted() {
                    DECODE_COUNTERS.with(|c| c.borrow_mut().deskew_attempts += 1);
                    let (deskew_matrix, deskew_conf) = Self::extract_qr_region_gray_with_mesh_warp(
                        gray, width, height, &transform, dimension,
                    );
                    if orientation::validate_timing_patterns(&deskew_matrix) {
                        if let Some(qr) = Self::decode_from_matrix_with_confidence(
                            &deskew_matrix,
                            version_num,
                            &deskew_conf,
                        ) {
                            DECODE_COUNTERS.with(|c| c.borrow_mut().deskew_successes += 1);
                            return Some(qr);
                        }
                    }
                }

                if allow_heavy_recovery && !budget_exhausted() {
                    let (mesh_matrix, mesh_conf) = Self::extract_qr_region_gray_with_mesh_warp(
                        gray, width, height, &transform, dimension,
                    );
                    if orientation::validate_timing_patterns(&mesh_matrix) {
                        if let Some(qr) = Self::decode_from_matrix_with_confidence(
                            &mesh_matrix,
                            version_num,
                            &mesh_conf,
                        ) {
                            return Some(qr);
                        }
                    }
                }

                if allow_heavy_recovery && !budget_exhausted() {
                    if let Some((radial_matrix, radial_conf)) =
                        Self::extract_qr_region_gray_with_radial_compensation(
                            gray, width, height, &transform, dimension,
                        )
                    {
                        if orientation::validate_timing_patterns(&radial_matrix) {
                            if let Some(qr) = Self::decode_from_matrix_with_confidence(
                                &radial_matrix,
                                version_num,
                                &radial_conf,
                            ) {
                                return Some(qr);
                            }
                        }
                    }
                }

                if allow_heavy_recovery && !budget_exhausted() {
                    let qr_matrix =
                        Self::extract_qr_region_with_transform(binary, &transform, dimension);
                    if !orientation::validate_timing_patterns(&qr_matrix) {
                        continue;
                    }
                    DECODE_COUNTERS.with(|c| c.borrow_mut().recovery_mode_attempts += 1);
                    if let Some(qr) = Self::decode_from_matrix(&qr_matrix, version_num) {
                        return Some(qr);
                    }
                } else if budget_exhausted() {
                    DECODE_COUNTERS.with(|c| c.borrow_mut().phase11_time_budget_skips += 1);
                }
            }
        }

        None
    }

    fn calculate_bottom_right(
        top_left: &Point,
        top_right: &Point,
        bottom_left: &Point,
    ) -> Option<Point> {
        geometry::calculate_bottom_right(top_left, top_right, bottom_left)
    }

    fn estimate_dimension(
        top_left: &Point,
        top_right: &Point,
        bottom_right: &Point,
        module_size: f32,
    ) -> Option<usize> {
        geometry::estimate_dimension(top_left, top_right, bottom_right, module_size)
    }

    fn version_candidates(estimated_version: i32) -> Vec<u8> {
        geometry::version_candidates(estimated_version)
    }

    #[allow(dead_code)]
    fn extract_qr_region(
        matrix: &BitMatrix,
        top_left: &Point,
        top_right: &Point,
        bottom_left: &Point,
        bottom_right: &Point,
        dimension: usize,
    ) -> Option<BitMatrix> {
        let transform =
            Self::build_transform(top_left, top_right, bottom_left, bottom_right, dimension)?;
        Some(Self::extract_qr_region_with_transform(
            matrix, &transform, dimension,
        ))
    }

    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    fn extract_qr_region_gray(
        gray: &[u8],
        width: usize,
        height: usize,
        top_left: &Point,
        top_right: &Point,
        bottom_left: &Point,
        bottom_right: &Point,
        dimension: usize,
    ) -> Option<BitMatrix> {
        let transform =
            Self::build_transform(top_left, top_right, bottom_left, bottom_right, dimension)?;
        Some(Self::extract_qr_region_gray_with_transform(
            gray, width, height, &transform, dimension,
        ))
    }

    fn build_transform(
        top_left: &Point,
        top_right: &Point,
        bottom_left: &Point,
        bottom_right: &Point,
        dimension: usize,
    ) -> Option<crate::utils::geometry::PerspectiveTransform> {
        geometry::build_transform(top_left, top_right, bottom_left, bottom_right, dimension)
    }

    fn extract_qr_region_with_transform(
        matrix: &BitMatrix,
        transform: &crate::utils::geometry::PerspectiveTransform,
        dimension: usize,
    ) -> BitMatrix {
        geometry::extract_qr_region_with_transform(matrix, transform, dimension)
    }

    fn extract_qr_region_gray_with_transform(
        gray: &[u8],
        width: usize,
        height: usize,
        transform: &crate::utils::geometry::PerspectiveTransform,
        dimension: usize,
    ) -> BitMatrix {
        geometry::extract_qr_region_gray_with_transform(gray, width, height, transform, dimension)
    }

    fn extract_qr_region_gray_with_transform_and_confidence(
        gray: &[u8],
        width: usize,
        height: usize,
        transform: &crate::utils::geometry::PerspectiveTransform,
        dimension: usize,
    ) -> (BitMatrix, Vec<u8>) {
        geometry::extract_qr_region_gray_with_transform_and_confidence(
            gray, width, height, transform, dimension,
        )
    }

    fn extract_qr_region_gray_with_transform_and_confidence_scaled(
        gray: &[u8],
        width: usize,
        height: usize,
        transform: &crate::utils::geometry::PerspectiveTransform,
        dimension: usize,
        sample_scale: f32,
    ) -> (BitMatrix, Vec<u8>) {
        geometry::extract_qr_region_gray_with_transform_and_confidence_scaled(
            gray,
            width,
            height,
            transform,
            dimension,
            sample_scale,
        )
    }

    fn extract_qr_region_gray_with_mesh_warp(
        gray: &[u8],
        width: usize,
        height: usize,
        transform: &crate::utils::geometry::PerspectiveTransform,
        dimension: usize,
    ) -> (BitMatrix, Vec<u8>) {
        geometry::extract_qr_region_gray_with_mesh_warp(gray, width, height, transform, dimension)
    }

    fn extract_qr_region_gray_with_radial_compensation(
        gray: &[u8],
        width: usize,
        height: usize,
        transform: &crate::utils::geometry::PerspectiveTransform,
        dimension: usize,
    ) -> Option<(BitMatrix, Vec<u8>)> {
        geometry::extract_qr_region_gray_with_radial_compensation(
            gray, width, height, transform, dimension,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn refine_transform_with_alignment(
        binary: &BitMatrix,
        transform: &crate::utils::geometry::PerspectiveTransform,
        version_num: u8,
        dimension: usize,
        module_size: f32,
        top_left: &Point,
        top_right: &Point,
        bottom_left: &Point,
    ) -> Option<crate::utils::geometry::PerspectiveTransform> {
        geometry::refine_transform_with_alignment(
            binary,
            transform,
            version_num,
            dimension,
            module_size,
            top_left,
            top_right,
            bottom_left,
        )
    }

    pub(crate) fn decode_from_matrix(qr_matrix: &BitMatrix, version_num: u8) -> Option<QRCode> {
        matrix_decode::decode_from_matrix(qr_matrix, version_num)
    }

    pub(crate) fn decode_from_matrix_with_confidence(
        qr_matrix: &BitMatrix,
        version_num: u8,
        module_confidence: &[u8],
    ) -> Option<QRCode> {
        matrix_decode::decode_from_matrix_with_confidence(qr_matrix, version_num, module_confidence)
    }
}

#[cfg(test)]
#[allow(clippy::needless_range_loop)]
mod tests;
