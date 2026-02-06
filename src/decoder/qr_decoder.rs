/// Main QR code decoder - wires everything together
use crate::models::{BitMatrix, Point, QRCode};
use std::sync::atomic::{AtomicUsize, Ordering};

mod geometry;
mod matrix_decode;
mod orientation;
mod payload;

/// Main QR decoder that processes a detected QR region
pub struct QrDecoder;

#[derive(Clone, Copy, Default)]
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
}

static DESKEW_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
static DESKEW_SUCCESSES: AtomicUsize = AtomicUsize::new(0);
static HIGH_VERSION_PRECISION_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
static RECOVERY_MODE_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
static SCALE_RETRY_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
static SCALE_RETRY_SUCCESSES: AtomicUsize = AtomicUsize::new(0);
static SCALE_RETRY_SKIPPED_BY_BUDGET: AtomicUsize = AtomicUsize::new(0);
static HV_SUBPIXEL_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
static HV_REFINE_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
static HV_REFINE_SUCCESSES: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn reset_decode_counters() {
    DESKEW_ATTEMPTS.store(0, Ordering::Relaxed);
    DESKEW_SUCCESSES.store(0, Ordering::Relaxed);
    HIGH_VERSION_PRECISION_ATTEMPTS.store(0, Ordering::Relaxed);
    RECOVERY_MODE_ATTEMPTS.store(0, Ordering::Relaxed);
    SCALE_RETRY_ATTEMPTS.store(0, Ordering::Relaxed);
    SCALE_RETRY_SUCCESSES.store(0, Ordering::Relaxed);
    SCALE_RETRY_SKIPPED_BY_BUDGET.store(0, Ordering::Relaxed);
    HV_SUBPIXEL_ATTEMPTS.store(0, Ordering::Relaxed);
    HV_REFINE_ATTEMPTS.store(0, Ordering::Relaxed);
    HV_REFINE_SUCCESSES.store(0, Ordering::Relaxed);
    payload::reset_erasure_counters();
}

pub(crate) fn take_decode_counters() -> DecodeCounters {
    let (rs_erasure_attempts, rs_erasure_successes, rs_erasure_count_hist) =
        payload::take_erasure_counters();
    DecodeCounters {
        deskew_attempts: DESKEW_ATTEMPTS.swap(0, Ordering::Relaxed),
        deskew_successes: DESKEW_SUCCESSES.swap(0, Ordering::Relaxed),
        high_version_precision_attempts: HIGH_VERSION_PRECISION_ATTEMPTS.swap(0, Ordering::Relaxed),
        recovery_mode_attempts: RECOVERY_MODE_ATTEMPTS.swap(0, Ordering::Relaxed),
        scale_retry_attempts: SCALE_RETRY_ATTEMPTS.swap(0, Ordering::Relaxed),
        scale_retry_successes: SCALE_RETRY_SUCCESSES.swap(0, Ordering::Relaxed),
        scale_retry_skipped_by_budget: SCALE_RETRY_SKIPPED_BY_BUDGET.swap(0, Ordering::Relaxed),
        hv_subpixel_attempts: HV_SUBPIXEL_ATTEMPTS.swap(0, Ordering::Relaxed),
        hv_refine_attempts: HV_REFINE_ATTEMPTS.swap(0, Ordering::Relaxed),
        hv_refine_successes: HV_REFINE_SUCCESSES.swap(0, Ordering::Relaxed),
        rs_erasure_attempts,
        rs_erasure_successes,
        rs_erasure_count_hist,
    }
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
                HIGH_VERSION_PRECISION_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
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
    ) -> Option<QRCode> {
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
                let transform = Self::refine_transform_with_alignment(
                    binary,
                    &transform,
                    version_num,
                    dimension,
                    module_size,
                    top_left,
                    top_right,
                    bottom_left,
                )
                .unwrap_or(transform);

                let (qr_matrix, module_confidence) =
                    Self::extract_qr_region_gray_with_transform_and_confidence(
                        gray, width, height, &transform, dimension,
                    );
                if version_num >= 7 {
                    HV_SUBPIXEL_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
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
                if should_scale_retry {
                    for &scale in &[1.25f32, 1.5f32] {
                        SCALE_RETRY_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
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
                            SCALE_RETRY_SUCCESSES.fetch_add(1, Ordering::Relaxed);
                            return Some(qr);
                        }
                        let scaled_inverted = orientation::invert_matrix(&scaled_matrix);
                        if let Some(qr) = Self::decode_from_matrix_with_confidence(
                            &scaled_inverted,
                            version_num,
                            &scaled_conf,
                        ) {
                            SCALE_RETRY_SUCCESSES.fetch_add(1, Ordering::Relaxed);
                            return Some(qr);
                        }
                    }
                } else {
                    SCALE_RETRY_SKIPPED_BY_BUDGET.fetch_add(1, Ordering::Relaxed);
                }

                if version_num >= 7 {
                    HV_REFINE_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
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
                                HV_REFINE_SUCCESSES.fetch_add(1, Ordering::Relaxed);
                                return Some(qr);
                            }
                        }
                    }
                }

                // Rotation-specialized deskew fallback: apply a bounded mesh warp variant
                // only after strict decode misses.
                if version_num >= 2 {
                    DESKEW_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
                    let (deskew_matrix, deskew_conf) = Self::extract_qr_region_gray_with_mesh_warp(
                        gray, width, height, &transform, dimension,
                    );
                    if orientation::validate_timing_patterns(&deskew_matrix) {
                        if let Some(qr) = Self::decode_from_matrix_with_confidence(
                            &deskew_matrix,
                            version_num,
                            &deskew_conf,
                        ) {
                            DESKEW_SUCCESSES.fetch_add(1, Ordering::Relaxed);
                            return Some(qr);
                        }
                    }
                }

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

                let qr_matrix =
                    Self::extract_qr_region_with_transform(binary, &transform, dimension);
                if !orientation::validate_timing_patterns(&qr_matrix) {
                    continue;
                }
                RECOVERY_MODE_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
                if let Some(qr) = Self::decode_from_matrix(&qr_matrix, version_num) {
                    return Some(qr);
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
