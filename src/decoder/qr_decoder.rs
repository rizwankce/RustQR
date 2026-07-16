use crate::CancellationToken;
/// Main QR code decoder - wires everything together
use crate::models::{BitMatrix, Point, QRCode};
use std::time::Instant;

mod geometry;
mod matrix_decode;
mod orientation;
mod payload;

// Preserve the historical bottom-right hypothesis order (row-major in y,
// then x) without allocating a `Vec<Point>` for every candidate decode.
const BOTTOM_RIGHT_OFFSETS: [(f32, f32); 25] = [
    (-4.0, -4.0),
    (-2.0, -4.0),
    (0.0, -4.0),
    (2.0, -4.0),
    (4.0, -4.0),
    (-4.0, -2.0),
    (-2.0, -2.0),
    (0.0, -2.0),
    (2.0, -2.0),
    (4.0, -2.0),
    (-4.0, 0.0),
    (-2.0, 0.0),
    (0.0, 0.0),
    (2.0, 0.0),
    (4.0, 0.0),
    (-4.0, 2.0),
    (-2.0, 2.0),
    (0.0, 2.0),
    (2.0, 2.0),
    (4.0, 2.0),
    (-4.0, 4.0),
    (-2.0, 4.0),
    (0.0, 4.0),
    (2.0, 4.0),
    (4.0, 4.0),
];

// Dense routing still gives every retained candidate a strict Model 2 decode,
// but after the request's two recovery-eligible attempts it must not spend the
// complete 25-point transform sweep on each clean late candidate.  Keep the
// central hypothesis plus one module in each cardinal direction; the full
// sweep above remains available to bounded recovery attempts.
const DENSE_STRICT_BOTTOM_RIGHT_OFFSETS: [(f32, f32); 5] =
    [(0.0, 0.0), (-2.0, 0.0), (2.0, 0.0), (0.0, -2.0), (0.0, 2.0)];

/// Main QR decoder that processes a detected QR region
pub struct QrDecoder;

/// Data modes represented by the conformance corpus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatrixDataMode {
    Numeric,
    Alphanumeric,
    Byte,
    Kanji,
    Eci,
    Gs1Fnc1,
    StructuredAppend,
}

/// Structured failures from deterministic matrix decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatrixDecodeError {
    InvalidDimensions,
    VersionDimensionMismatch,
    InvalidConfidenceLength,
    ErasureModuleOutOfBounds,
    UnsupportedMode(MatrixDataMode),
    DecodeFailed,
}

/// Request-scoped erasure evidence for deterministic matrix decoding.
///
/// Confidence values are row-major, with `0` identifying an erased module and
/// non-zero values identifying known modules. Explicit coordinates use `(x, y)`.
#[derive(Debug, Clone, Copy)]
pub enum MatrixErasureEvidence<'a> {
    ModuleConfidence(&'a [u8]),
    ErasedModules(&'a [(usize, usize)]),
}

#[derive(Clone, Copy)]
pub(crate) struct DecodeCounters {
    /// Exact BCH format candidates accepted from a sampled matrix.
    pub format_bch_candidates: usize,
    /// BCH distances of accepted format candidates: [0, 1, 2, 3].
    pub format_bch_distance_hist: [usize; 4],
    /// Candidate payload paths that reached Reed-Solomon correction.
    pub rs_candidate_attempts: usize,
    /// Candidate payload paths rejected because QR remainder bits were non-zero.
    pub nonzero_remainder_bit_rejections: usize,
    /// Individual Reed-Solomon block decodes attempted.
    pub rs_block_attempts: usize,
    /// Individual Reed-Solomon blocks corrected successfully.
    pub rs_block_successes: usize,
    /// Individual Reed-Solomon blocks that remained uncorrectable.
    pub rs_block_failures: usize,
    pub deskew_attempts: usize,
    pub deskew_successes: usize,
    pub high_version_precision_attempts: usize,
    pub recovery_mode_attempts: usize,
    /// Strict canonical matrix payload decodes attempted before recovery.
    pub strict_matrix_payload_attempts: usize,
    /// Matrix payload decodes attempted by the bounded recovery frontier.
    pub matrix_recovery_payload_attempts: usize,
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
    pub unsupported_payloads: usize,
    /// Sampled matrices rejected by the fixed timing-pattern gate.
    pub timing_pattern_rejections: usize,
    /// Sum of horizontal timing alternation ratios for rejected matrices.
    pub timing_pattern_horizontal_ratio_sum: f32,
    /// Sum of vertical timing alternation ratios for rejected matrices.
    pub timing_pattern_vertical_ratio_sum: f32,
}

/// Mutable state owned by one decode request.
///
/// This deliberately travels with the call chain instead of residing in a
/// process global or thread-local slot: concurrent requests must not consume
/// each other's recovery budget or report one another's counters.
pub(crate) struct DecodeRequestContext {
    deadline: Option<Instant>,
    cancellation: Option<CancellationToken>,
    erasure_attempts_remaining: usize,
    counters: DecodeCounters,
}

impl DecodeRequestContext {
    pub(crate) const fn new(erasure_attempt_limit: usize) -> Self {
        Self {
            deadline: None,
            cancellation: None,
            erasure_attempts_remaining: erasure_attempt_limit,
            counters: DecodeCounters::new(),
        }
    }

    /// Build a request context with a cooperative deadline and cancellation
    /// token. Both controls are request-owned and therefore safe to use from
    /// concurrent callers.
    pub(crate) fn with_deadline_and_cancellation(
        erasure_attempt_limit: usize,
        deadline: Instant,
        cancellation: Option<CancellationToken>,
    ) -> Self {
        Self {
            deadline: Some(deadline),
            cancellation,
            erasure_attempts_remaining: erasure_attempt_limit,
            counters: DecodeCounters::new(),
        }
    }

    pub(crate) fn deadline_expired(&self) -> bool {
        self.cancellation
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
            || self
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
    }

    pub(crate) const fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    pub(crate) fn try_consume_erasure_attempt(&mut self) -> bool {
        if self.erasure_attempts_remaining == 0 {
            return false;
        }
        self.erasure_attempts_remaining -= 1;
        true
    }

    pub(crate) fn counters(&self) -> DecodeCounters {
        self.counters
    }

    pub(crate) fn counters_mut(&mut self) -> &mut DecodeCounters {
        &mut self.counters
    }
}

impl Default for DecodeRequestContext {
    fn default() -> Self {
        // Preserve the established non-recovery behaviour for APIs which do
        // not opt into an options-bearing request context.
        Self::new(0)
    }
}

impl DecodeCounters {
    const fn new() -> Self {
        Self {
            format_bch_candidates: 0,
            format_bch_distance_hist: [0; 4],
            rs_candidate_attempts: 0,
            nonzero_remainder_bit_rejections: 0,
            rs_block_attempts: 0,
            rs_block_successes: 0,
            rs_block_failures: 0,
            deskew_attempts: 0,
            deskew_successes: 0,
            high_version_precision_attempts: 0,
            recovery_mode_attempts: 0,
            strict_matrix_payload_attempts: 0,
            matrix_recovery_payload_attempts: 0,
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
            unsupported_payloads: 0,
            timing_pattern_rejections: 0,
            timing_pattern_horizontal_ratio_sum: 0.0,
            timing_pattern_vertical_ratio_sum: 0.0,
        }
    }
}

impl Default for DecodeCounters {
    fn default() -> Self {
        Self::new()
    }
}

impl QrDecoder {
    /// Decode an already sampled Model 2 module matrix without image detection.
    ///
    /// Callers that know the encoded mode can use [`Self::decode_matrix_for_mode`]
    /// to document the fixture's primary payload mode.
    pub fn decode_matrix(
        qr_matrix: &BitMatrix,
        version_num: u8,
    ) -> Result<QRCode, MatrixDecodeError> {
        if qr_matrix.width() != qr_matrix.height()
            || version_num == 0
            || version_num > 40
            || qr_matrix.width() < 21
        {
            return Err(MatrixDecodeError::InvalidDimensions);
        }
        if qr_matrix.width() != 17 + 4 * version_num as usize {
            return Err(MatrixDecodeError::VersionDimensionMismatch);
        }
        // The deterministic matrix entry point is also the conformance entry
        // point. Do not let the image-recovery decoder's brute-force format or
        // version fallbacks turn structurally invalid symbols into successes.
        if crate::decoder::format::FormatInfo::extract(qr_matrix).is_none() {
            return Err(MatrixDecodeError::DecodeFailed);
        }
        if version_num >= 7
            && crate::decoder::version::VersionInfo::extract(qr_matrix) != Some(version_num)
        {
            return Err(MatrixDecodeError::DecodeFailed);
        }
        Self::decode_from_matrix(qr_matrix, version_num).ok_or(MatrixDecodeError::DecodeFailed)
    }

    /// Decode a matrix fixture while enforcing the decoder's advertised mode support.
    pub fn decode_matrix_for_mode(
        qr_matrix: &BitMatrix,
        version_num: u8,
        mode: MatrixDataMode,
    ) -> Result<QRCode, MatrixDecodeError> {
        match mode {
            MatrixDataMode::Numeric
            | MatrixDataMode::Alphanumeric
            | MatrixDataMode::Byte
            | MatrixDataMode::Kanji
            | MatrixDataMode::Eci
            | MatrixDataMode::Gs1Fnc1
            | MatrixDataMode::StructuredAppend => Self::decode_matrix(qr_matrix, version_num),
        }
    }

    /// Decode a sampled Model 2 matrix using known module erasures.
    ///
    /// Evidence is validated and mapped through the QR data traversal into
    /// per-block codeword erasures. The request uses no global configuration,
    /// counters, or mutable decoder state.
    pub fn decode_matrix_with_erasures(
        qr_matrix: &BitMatrix,
        version_num: u8,
        evidence: MatrixErasureEvidence<'_>,
    ) -> Result<QRCode, MatrixDecodeError> {
        if qr_matrix.width() != qr_matrix.height()
            || version_num == 0
            || version_num > 40
            || qr_matrix.width() < 21
        {
            return Err(MatrixDecodeError::InvalidDimensions);
        }
        if qr_matrix.width() != 17 + 4 * version_num as usize {
            return Err(MatrixDecodeError::VersionDimensionMismatch);
        }

        let dimension = qr_matrix.width();
        let confidence = match evidence {
            MatrixErasureEvidence::ModuleConfidence(values) => {
                if values.len() != dimension * dimension {
                    return Err(MatrixDecodeError::InvalidConfidenceLength);
                }
                values.to_vec()
            }
            MatrixErasureEvidence::ErasedModules(modules) => {
                let mut values = vec![u8::MAX; dimension * dimension];
                for &(x, y) in modules {
                    if x >= dimension || y >= dimension {
                        return Err(MatrixDecodeError::ErasureModuleOutOfBounds);
                    }
                    values[y * dimension + x] = 0;
                }
                values
            }
        };

        let format_info = crate::decoder::format::FormatInfo::extract(qr_matrix)
            .ok_or(MatrixDecodeError::DecodeFailed)?;
        if version_num >= 7
            && crate::decoder::version::VersionInfo::extract(qr_matrix) != Some(version_num)
        {
            return Err(MatrixDecodeError::DecodeFailed);
        }
        payload::try_decode_single_deterministic_erasures(
            qr_matrix,
            version_num,
            &format_info,
            &confidence,
        )
        .ok_or(MatrixDecodeError::DecodeFailed)
    }

    /// Decode a QR code from a binary matrix and finder pattern locations
    pub fn decode(
        matrix: &BitMatrix,
        top_left: &Point,
        top_right: &Point,
        bottom_left: &Point,
        module_size: f32,
    ) -> Option<QRCode> {
        let mut context = DecodeRequestContext::default();
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
        let (candidates, candidate_count) = Self::version_candidates(estimated_version);
        let step = module_size.max(1.0) * 2.0;
        for &version_num in &candidates[..candidate_count] {
            if version_num >= 7 {
                context.counters_mut().high_version_precision_attempts += 1;
            }
            let dimension = 17 + 4 * version_num as usize;
            for &(dx, dy) in &BOTTOM_RIGHT_OFFSETS {
                let br = Point::new(bottom_right.x + dx * step, bottom_right.y + dy * step);
                let transform =
                    match Self::build_transform(top_left, top_right, bottom_left, &br, dimension) {
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

                if let Some(qr) =
                    Self::decode_from_matrix_in_context(&qr_matrix, version_num, &mut context)
                {
                    return Some(Self::with_position(qr, &transform, dimension));
                }

                // Try inverted grid (binarization might be flipped)
                let inverted = orientation::invert_matrix(&qr_matrix);
                if let Some(qr) =
                    Self::decode_from_matrix_in_context(&inverted, version_num, &mut context)
                {
                    return Some(Self::with_position(qr, &transform, dimension));
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
        allow_heavy_recovery: bool,
    ) -> Option<QRCode> {
        let mut context = DecodeRequestContext::default();
        Self::decode_with_gray_in_context(
            binary,
            gray,
            width,
            height,
            top_left,
            top_right,
            bottom_left,
            module_size,
            allow_heavy_recovery,
            allow_heavy_recovery,
            &mut context,
        )
    }

    /// Decode a sampled candidate while charging recovery work to one request.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn decode_with_gray_in_context(
        binary: &BitMatrix,
        gray: &[u8],
        width: usize,
        height: usize,
        top_left: &Point,
        top_right: &Point,
        bottom_left: &Point,
        module_size: f32,
        allow_heavy_recovery: bool,
        allow_matrix_recovery: bool,
        context: &mut DecodeRequestContext,
    ) -> Option<QRCode> {
        let started = Instant::now();
        let candidate_budget_ms = crate::decoder::config::candidate_time_budget_ms();
        let budget_exhausted = || started.elapsed().as_millis() as u64 >= candidate_budget_ms;
        let bottom_right = Self::calculate_bottom_right(top_left, top_right, bottom_left)?;
        let step = module_size.max(1.0) * 2.0;
        let estimated_dimension =
            Self::estimate_dimension(top_left, top_right, &bottom_right, module_size)?;

        let estimated_version = ((estimated_dimension - 17) / 4) as i32;
        let (candidates, candidate_count) = Self::version_candidates(estimated_version);

        let bottom_right_offsets: &[(f32, f32)] = if allow_matrix_recovery {
            &BOTTOM_RIGHT_OFFSETS
        } else {
            &DENSE_STRICT_BOTTOM_RIGHT_OFFSETS
        };

        for &version_num in &candidates[..candidate_count] {
            let dimension = 17 + 4 * version_num as usize;
            for &(dx, dy) in bottom_right_offsets {
                let br = Point::new(bottom_right.x + dx * step, bottom_right.y + dy * step);
                let transform =
                    match Self::build_transform(top_left, top_right, bottom_left, &br, dimension) {
                        Some(t) => t,
                        None => continue,
                    };
                let transform = geometry::refine_transform_with_timing_and_alignment(
                    binary,
                    Some(gray),
                    width,
                    height,
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
                    context.counters_mut().hv_subpixel_attempts += 1;
                }
                if !orientation::validate_timing_patterns(&qr_matrix) {
                    context.counters_mut().timing_pattern_rejections += 1;
                    if let Some((horizontal, vertical)) =
                        orientation::timing_pattern_ratios(&qr_matrix)
                    {
                        context.counters_mut().timing_pattern_horizontal_ratio_sum += horizontal;
                        context.counters_mut().timing_pattern_vertical_ratio_sum += vertical;
                    }
                    continue;
                }

                if let Some(qr) =
                    matrix_decode::decode_from_matrix_with_confidence_in_context_with_recovery(
                        &qr_matrix,
                        version_num,
                        &module_confidence,
                        allow_matrix_recovery,
                        context,
                    )
                {
                    return Some(Self::with_position(qr, &transform, dimension));
                }

                let inverted = orientation::invert_matrix(&qr_matrix);
                if let Some(qr) =
                    matrix_decode::decode_from_matrix_with_confidence_in_context_with_recovery(
                        &inverted,
                        version_num,
                        &module_confidence,
                        allow_matrix_recovery,
                        context,
                    )
                {
                    return Some(Self::with_position(qr, &transform, dimension));
                }

                if allow_heavy_recovery && !budget_exhausted() {
                    let jitter_offsets: [(f32, f32); 4] =
                        [(0.25, 0.0), (-0.25, 0.0), (0.0, 0.25), (0.0, -0.25)];
                    for &(jx, jy) in &jitter_offsets {
                        if budget_exhausted() {
                            break;
                        }
                        let jittered_transform =
                            transform.translated(jx * module_size, jy * module_size);
                        let (jit_matrix, jit_conf) =
                            Self::extract_qr_region_gray_with_transform_and_confidence(
                                gray,
                                width,
                                height,
                                &jittered_transform,
                                dimension,
                            );
                        if !orientation::validate_timing_patterns(&jit_matrix) {
                            continue;
                        }
                        if let Some(qr) = Self::decode_from_matrix_with_confidence_in_context(
                            &jit_matrix,
                            version_num,
                            &jit_conf,
                            context,
                        ) {
                            return Some(Self::with_position(qr, &jittered_transform, dimension));
                        }
                    }
                }

                let _should_scale_retry = module_size <= 2.4 || version_num >= 7 || dimension >= 85;
                if false {
                    // Scale retries disabled (0/455 success rate in benchmarks)
                    for &scale in &[1.25f32, 1.5f32] {
                        if budget_exhausted() {
                            context.counters_mut().phase11_time_budget_skips += 1;
                            break;
                        }
                        context.counters_mut().scale_retry_attempts += 1;
                        let (scaled_matrix, scaled_conf) =
                            Self::extract_qr_region_gray_with_transform_and_confidence_scaled(
                                gray, width, height, &transform, dimension, scale,
                            );
                        if !orientation::validate_timing_patterns(&scaled_matrix) {
                            continue;
                        }
                        if let Some(qr) = Self::decode_from_matrix_with_confidence_in_context(
                            &scaled_matrix,
                            version_num,
                            &scaled_conf,
                            context,
                        ) {
                            context.counters_mut().scale_retry_successes += 1;
                            return Some(Self::with_position(qr, &transform, dimension));
                        }
                        let scaled_inverted = orientation::invert_matrix(&scaled_matrix);
                        if let Some(qr) = Self::decode_from_matrix_with_confidence_in_context(
                            &scaled_inverted,
                            version_num,
                            &scaled_conf,
                            context,
                        ) {
                            context.counters_mut().scale_retry_successes += 1;
                            return Some(Self::with_position(qr, &transform, dimension));
                        }
                    }
                }

                if allow_heavy_recovery && version_num >= 7 && !budget_exhausted() {
                    context.counters_mut().hv_refine_attempts += 1;
                    if let Some(refined_hv_transform) =
                        geometry::refine_transform_with_timing_and_alignment(
                            binary,
                            Some(gray),
                            width,
                            height,
                            &transform,
                            version_num,
                            dimension,
                            (module_size * 0.9).max(1.0),
                            top_left,
                            top_right,
                            bottom_left,
                        )
                    {
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
                            if let Some(qr) = Self::decode_from_matrix_with_confidence_in_context(
                                &hv_matrix,
                                version_num,
                                &hv_conf,
                                context,
                            ) {
                                context.counters_mut().hv_refine_successes += 1;
                                return Some(Self::with_position(
                                    qr,
                                    &refined_hv_transform,
                                    dimension,
                                ));
                            }
                        }
                    }
                }

                // Rotation-specialized deskew fallback: apply a bounded mesh warp variant
                // only after strict decode misses.
                if false {
                    // Deskew disabled (0/1038 success rate in benchmarks)
                    let (deskew_matrix, deskew_conf) = Self::extract_qr_region_gray_with_mesh_warp(
                        gray, width, height, &transform, dimension,
                    );
                    if orientation::validate_timing_patterns(&deskew_matrix) {
                        if let Some(qr) = Self::decode_from_matrix_with_confidence_in_context(
                            &deskew_matrix,
                            version_num,
                            &deskew_conf,
                            context,
                        ) {
                            context.counters_mut().deskew_successes += 1;
                            return Some(Self::with_position(qr, &transform, dimension));
                        }
                    }
                }

                if false {
                    // Duplicate mesh warp disabled (0 success rate in benchmarks)
                    let (mesh_matrix, mesh_conf) = Self::extract_qr_region_gray_with_mesh_warp(
                        gray, width, height, &transform, dimension,
                    );
                    if orientation::validate_timing_patterns(&mesh_matrix) {
                        if let Some(qr) = Self::decode_from_matrix_with_confidence_in_context(
                            &mesh_matrix,
                            version_num,
                            &mesh_conf,
                            context,
                        ) {
                            return Some(Self::with_position(qr, &transform, dimension));
                        }
                    }
                }

                if false {
                    // Radial compensation disabled (0 success rate in benchmarks)
                    if let Some((radial_matrix, radial_conf)) =
                        Self::extract_qr_region_gray_with_radial_compensation(
                            gray, width, height, &transform, dimension,
                        )
                    {
                        if orientation::validate_timing_patterns(&radial_matrix) {
                            if let Some(qr) = Self::decode_from_matrix_with_confidence_in_context(
                                &radial_matrix,
                                version_num,
                                &radial_conf,
                                context,
                            ) {
                                return Some(Self::with_position(qr, &transform, dimension));
                            }
                        }
                    }
                }

                if false {
                    // Binary-only recovery disabled (0 success rate in benchmarks)
                    let qr_matrix =
                        Self::extract_qr_region_with_transform(binary, &transform, dimension);
                    if !orientation::validate_timing_patterns(&qr_matrix) {
                        continue;
                    }
                    context.counters_mut().recovery_mode_attempts += 1;
                    if let Some(qr) =
                        Self::decode_from_matrix_in_context(&qr_matrix, version_num, context)
                    {
                        return Some(Self::with_position(qr, &transform, dimension));
                    }
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

    fn version_candidates(estimated_version: i32) -> ([u8; 5], usize) {
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

    fn with_position(
        mut qr: QRCode,
        transform: &crate::utils::geometry::PerspectiveTransform,
        dimension: usize,
    ) -> QRCode {
        let dimension = dimension as f32;
        qr.position = [
            transform.transform(&Point::new(0.0, 0.0)),
            transform.transform(&Point::new(dimension, 0.0)),
            transform.transform(&Point::new(dimension, dimension)),
            transform.transform(&Point::new(0.0, dimension)),
        ];
        qr
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

    pub(crate) fn decode_from_matrix_in_context(
        qr_matrix: &BitMatrix,
        version_num: u8,
        context: &mut DecodeRequestContext,
    ) -> Option<QRCode> {
        matrix_decode::decode_from_matrix_in_context(qr_matrix, version_num, context)
    }

    #[allow(dead_code)]
    pub(crate) fn decode_from_matrix_with_confidence(
        qr_matrix: &BitMatrix,
        version_num: u8,
        module_confidence: &[u8],
    ) -> Option<QRCode> {
        matrix_decode::decode_from_matrix_with_confidence(qr_matrix, version_num, module_confidence)
    }

    pub(crate) fn decode_from_matrix_with_confidence_in_context(
        qr_matrix: &BitMatrix,
        version_num: u8,
        module_confidence: &[u8],
        context: &mut DecodeRequestContext,
    ) -> Option<QRCode> {
        matrix_decode::decode_from_matrix_with_confidence_in_context(
            qr_matrix,
            version_num,
            module_confidence,
            context,
        )
    }
}

#[cfg(test)]
#[allow(clippy::needless_range_loop)]
mod tests;
