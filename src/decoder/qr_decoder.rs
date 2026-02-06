/// Main QR code decoder - wires everything together
use crate::models::{BitMatrix, Point, QRCode};

mod geometry;
mod matrix_decode;
mod orientation;
mod payload;

/// Main QR decoder that processes a detected QR region
pub struct QrDecoder;

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
