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

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// A cooperative cancellation handle for one decoding request.
///
/// Clone this handle before passing it to [`DecoderOptions::with_cancellation`]
/// and call [`Self::cancel`] from another thread when the request is no longer
/// needed. Cancellation is checked at the same bounded checkpoints as a
/// request deadline; it cannot interrupt a single in-flight CPU operation.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    /// Create a token that has not been cancelled.
    pub fn new() -> Self {
        Self::default()
    }

    /// Cooperatively cancel every request configured with a clone of this token.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    /// Whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// Recovery effort selected for a decoding request.
///
/// Presets select both a cooperative deadline and a maximum number of image
/// candidate decode attempts. This keeps recovery effort local to the request
/// rather than inheriting ambient process configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DecoderPreset {
    /// Prefer a quick answer for interactive scanning.
    Fast,
    /// Use the normal recovery budget.
    #[default]
    Balanced,
    /// Allow additional time for batch or archival scanning.
    Exhaustive,
}

/// Immutable options for one decoding request.
///
/// Construct with a preset and use the builder methods to override only the
/// fields relevant to that request.  Options are never read from environment
/// variables and may be freely shared between concurrent callers.
#[derive(Debug, Clone)]
pub struct DecoderOptions {
    preset: DecoderPreset,
    deadline: std::time::Duration,
    candidate_limit: usize,
    erasure_attempt_limit: usize,
    diagnostics: bool,
    cancellation: Option<CancellationToken>,
}

impl DecoderOptions {
    /// Start with the named recovery preset.
    pub fn with_preset(preset: DecoderPreset) -> Self {
        let (deadline, candidate_limit, erasure_attempt_limit) = match preset {
            DecoderPreset::Fast => (std::time::Duration::from_millis(250), 16, 4),
            DecoderPreset::Balanced => (std::time::Duration::from_secs(2), 128, 16),
            DecoderPreset::Exhaustive => (std::time::Duration::from_secs(10), 512, 64),
        };
        Self {
            preset,
            deadline,
            candidate_limit,
            erasure_attempt_limit,
            diagnostics: false,
            cancellation: None,
        }
    }

    /// Set a cooperative deadline for this request.
    pub fn with_deadline(mut self, deadline: std::time::Duration) -> Self {
        self.deadline = deadline;
        self
    }

    /// Set the maximum number of candidate decode attempts for this request.
    ///
    /// A value of zero performs no candidate decoding. This is useful for
    /// callers that want to exercise only cheap detection stages.
    pub fn with_candidate_limit(mut self, candidate_limit: usize) -> Self {
        self.candidate_limit = candidate_limit;
        self
    }

    /// Set the maximum number of confidence-guided Reed-Solomon erasure
    /// recovery attempts for this request.
    ///
    /// The budget is shared by all candidate matrices within the request; a
    /// value of zero disables this optional recovery path.
    pub fn with_erasure_attempt_limit(mut self, erasure_attempt_limit: usize) -> Self {
        self.erasure_attempt_limit = erasure_attempt_limit;
        self
    }

    /// Request stage diagnostics in the returned result.
    pub fn with_diagnostics(mut self, enabled: bool) -> Self {
        self.diagnostics = enabled;
        self
    }

    /// Stop this request when the supplied token is cancelled.
    ///
    /// The token is deliberately owned by the options value rather than held
    /// in global state, so cancellation cannot affect unrelated requests.
    pub fn with_cancellation(mut self, cancellation: CancellationToken) -> Self {
        self.cancellation = Some(cancellation);
        self
    }

    /// The selected recovery preset.
    pub const fn preset(&self) -> DecoderPreset {
        self.preset
    }

    /// The cooperative request deadline.
    pub const fn deadline(&self) -> std::time::Duration {
        self.deadline
    }

    /// Maximum candidate decode attempts available to this request.
    pub const fn candidate_limit(&self) -> usize {
        self.candidate_limit
    }

    /// Maximum confidence-guided RS erasure recovery attempts for this request.
    pub const fn erasure_attempt_limit(&self) -> usize {
        self.erasure_attempt_limit
    }

    /// Whether diagnostics are collected for this request.
    pub const fn diagnostics_enabled(&self) -> bool {
        self.diagnostics
    }

    /// Whether the request's cancellation token has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancellation
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
    }

    fn cancellation(&self) -> Option<CancellationToken> {
        self.cancellation.clone()
    }
}

impl Default for DecoderOptions {
    fn default() -> Self {
        Self::with_preset(DecoderPreset::Balanced)
    }
}

/// The furthest stage reached by a request that did not decode a QR code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureStage {
    /// The image description was invalid.
    InvalidInput,
    /// The cooperative deadline elapsed.
    Timeout,
    /// The caller cancelled the request through its request-scoped token.
    Cancelled,
    /// No finder-pattern candidate was detected.
    Detection,
    /// Candidates were found but no usable geometry was built.
    Geometry,
    /// Geometry was sampled but error correction never succeeded.
    ReedSolomon,
    /// Error correction succeeded but no payload could be parsed.
    Payload,
    /// Error correction succeeded but the payload contains a QR mode this
    /// library does not support.
    UnsupportedContent,
}

/// Optional evidence collected for a request.
#[derive(Debug, Clone)]
pub struct RequestDiagnostics {
    /// The final stage, absent when at least one code decoded.
    pub failure_stage: Option<FailureStage>,
    /// Detailed counters collected only when diagnostics were requested.
    pub telemetry: Option<DetectionTelemetry>,
}

/// Result of [`try_detect_with_options`].
#[derive(Debug, Clone)]
pub struct DetectionResult {
    /// Codes decoded from the input.
    pub codes: Vec<QRCode>,
    /// Optional structured diagnostic evidence.
    pub diagnostics: RequestDiagnostics,
}

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
    /// Number of sampled matrix candidates with BCH-valid format information.
    ///
    /// This is observed decoder evidence, not an inferred failure label.
    pub format_extracted: usize,
    /// BCH distances of observed format candidates: [0, 1, 2, 3].
    pub format_bch_distance_hist: [usize; 4],
    /// Candidate payload paths that reached Reed-Solomon correction.
    pub rs_candidate_attempts: usize,
    /// Candidate paths rejected before Reed-Solomon because QR remainder bits
    /// were non-zero after unmasking.
    pub nonzero_remainder_bit_rejections: usize,
    /// Individual Reed-Solomon block decodes attempted.
    pub rs_block_attempts: usize,
    /// Individual Reed-Solomon blocks corrected successfully.
    pub rs_block_successes: usize,
    /// Individual Reed-Solomon blocks that remained uncorrectable.
    pub rs_block_failures: usize,
    /// Number of groups where Reed-Solomon decoding succeeded.
    pub rs_decode_ok: usize,
    /// Number of QR codes whose payload parsed into valid content.
    pub payload_decoded: usize,
    /// Number of error-corrected payloads rejected because they use an
    /// unsupported QR content mode.
    pub unsupported_content: usize,
    /// Number of decoder attempts made (one per transform/group decode try).
    pub decode_attempts: usize,
    /// Total candidate groups scored before trimming.
    pub candidate_groups_scored: usize,
    /// Histogram of candidate group scores:
    /// [<2.0, 2.0-<3.0, 3.0-<5.0, >=5.0]
    pub candidate_score_buckets: [usize; 4],
    /// The final detection result count.
    pub qr_codes_found: usize,
    /// Number of candidate decodes skipped due to decode budget limits.
    pub budget_skips: usize,
    /// Decode attempts consumed in the high-confidence lane.
    pub budget_lane_high: usize,
    /// Decode attempts consumed in the medium-confidence lane.
    pub budget_lane_medium: usize,
    /// Decode attempts consumed in the low-confidence lane.
    pub budget_lane_low: usize,
    /// Fallback transition count from Otsu to adaptive(31).
    pub bin_fallback_otsu_to_adaptive31: usize,
    /// Fallback transition count from adaptive(31) to adaptive(21).
    pub bin_fallback_adaptive31_to_adaptive21: usize,
    /// Number of successful decodes that happened on fallback binarization.
    pub bin_fallback_successes: usize,
    /// Whether geometry rerank path was active for this image.
    pub rerank_enabled: bool,
    /// Number of top-1 reranked candidate decode attempts.
    pub rerank_top1_attempts: usize,
    /// Number of successful decodes from top-1 reranked candidate.
    pub rerank_top1_successes: usize,
    /// Candidate groups rejected during rerank due to transform/order failures.
    pub rerank_transform_reject_count: usize,
    /// Whether saturation-aware scoring was enabled for this image.
    pub saturation_mask_enabled: bool,
    /// Image-level saturation coverage ratio when mask path was enabled.
    pub saturation_mask_coverage: f32,
    /// Successful decodes influenced by saturation-aware scoring.
    pub saturation_mask_decode_successes: usize,
    /// Number of ROI normalization fallback attempts.
    pub roi_norm_attempts: usize,
    /// Number of successful decodes from ROI normalization fallback.
    pub roi_norm_successes: usize,
    /// Number of times ROI normalization fallback was skipped.
    pub roi_norm_skipped: usize,
    /// Number of times 2-finder fallback path was attempted.
    pub two_finder_attempts: usize,
    /// Number of successful decodes from 2-finder fallback path.
    pub two_finder_successes: usize,
    /// Strategy profile selected by category-aware router.
    pub strategy_profile: String,
    /// Number of spatial regions considered for region-first multi-QR decode.
    pub regions_considered: usize,
    /// Whether router enabled multi-region decode for this image.
    pub router_multi_region: bool,
    /// Number of successful decodes from region-routed candidates.
    pub router_region_decodes: usize,
    /// Fast-signal blur metric used by router v2.
    pub router_blur_metric: f32,
    /// Fast-signal saturation ratio used by router v2.
    pub router_saturation_ratio: f32,
    /// Fast-signal skew estimate in degrees used by router v2.
    pub router_skew_estimate_deg: f32,
    /// Fast-signal region density proxy used by router v2.
    pub router_region_density_proxy: f32,
    /// Number of decodes rejected by acceptance calibration threshold.
    pub acceptance_rejected: usize,
    /// Number of deskew decode attempts.
    pub deskew_attempts: usize,
    /// Number of successful deskew decode recoveries.
    pub deskew_successes: usize,
    /// Number of high-version precision mode decode attempts.
    pub high_version_precision_attempts: usize,
    /// Number of recovery-mode decode attempts.
    pub recovery_mode_attempts: usize,
    /// Number of multi-scale retry decode attempts.
    pub scale_retry_attempts: usize,
    /// Number of successful multi-scale retries.
    pub scale_retry_successes: usize,
    /// Number of candidates skipped from multi-scale retry due to budget/guardrails.
    pub scale_retry_skipped_by_budget: usize,
    /// Number of high-version subpixel precision attempts.
    pub hv_subpixel_attempts: usize,
    /// Number of high-version refinement attempts.
    pub hv_refine_attempts: usize,
    /// Number of successful high-version refinement decodes.
    pub hv_refine_successes: usize,
    /// Number of RS erasure decode attempts.
    pub rs_erasure_attempts: usize,
    /// Number of successful RS erasure decodes.
    pub rs_erasure_successes: usize,
    /// RS erasure count histogram buckets: [1, 2-3, 4-6, 7+].
    pub rs_erasure_count_hist: [usize; 4],
    /// Number of candidate decode branches skipped by phase 9.11 time budget.
    pub phase11_time_budget_skips: usize,
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
        for i in 0..self.format_bch_distance_hist.len() {
            self.format_bch_distance_hist[i] += other.format_bch_distance_hist[i];
        }
        self.rs_candidate_attempts += other.rs_candidate_attempts;
        self.nonzero_remainder_bit_rejections += other.nonzero_remainder_bit_rejections;
        self.rs_block_attempts += other.rs_block_attempts;
        self.rs_block_successes += other.rs_block_successes;
        self.rs_block_failures += other.rs_block_failures;
        self.rs_decode_ok = self.rs_decode_ok.max(other.rs_decode_ok);
        self.payload_decoded = self.payload_decoded.max(other.payload_decoded);
        self.unsupported_content += other.unsupported_content;
        self.decode_attempts += other.decode_attempts;
        self.candidate_groups_scored += other.candidate_groups_scored;
        self.budget_skips += other.budget_skips;
        self.budget_lane_high += other.budget_lane_high;
        self.budget_lane_medium += other.budget_lane_medium;
        self.budget_lane_low += other.budget_lane_low;
        self.bin_fallback_otsu_to_adaptive31 += other.bin_fallback_otsu_to_adaptive31;
        self.bin_fallback_adaptive31_to_adaptive21 += other.bin_fallback_adaptive31_to_adaptive21;
        self.bin_fallback_successes += other.bin_fallback_successes;
        self.rerank_enabled = self.rerank_enabled || other.rerank_enabled;
        self.rerank_top1_attempts += other.rerank_top1_attempts;
        self.rerank_top1_successes += other.rerank_top1_successes;
        self.rerank_transform_reject_count += other.rerank_transform_reject_count;
        self.saturation_mask_enabled =
            self.saturation_mask_enabled || other.saturation_mask_enabled;
        self.saturation_mask_coverage = self
            .saturation_mask_coverage
            .max(other.saturation_mask_coverage);
        self.saturation_mask_decode_successes += other.saturation_mask_decode_successes;
        self.roi_norm_attempts += other.roi_norm_attempts;
        self.roi_norm_successes += other.roi_norm_successes;
        self.roi_norm_skipped += other.roi_norm_skipped;
        self.two_finder_attempts += other.two_finder_attempts;
        self.two_finder_successes += other.two_finder_successes;
        self.regions_considered = self.regions_considered.max(other.regions_considered);
        self.router_multi_region = self.router_multi_region || other.router_multi_region;
        self.router_region_decodes += other.router_region_decodes;
        self.router_blur_metric = self.router_blur_metric.max(other.router_blur_metric);
        self.router_saturation_ratio = self
            .router_saturation_ratio
            .max(other.router_saturation_ratio);
        self.router_skew_estimate_deg = self
            .router_skew_estimate_deg
            .max(other.router_skew_estimate_deg);
        self.router_region_density_proxy = self
            .router_region_density_proxy
            .max(other.router_region_density_proxy);
        self.acceptance_rejected += other.acceptance_rejected;
        self.deskew_attempts += other.deskew_attempts;
        self.deskew_successes += other.deskew_successes;
        self.high_version_precision_attempts += other.high_version_precision_attempts;
        self.recovery_mode_attempts += other.recovery_mode_attempts;
        self.scale_retry_attempts += other.scale_retry_attempts;
        self.scale_retry_successes += other.scale_retry_successes;
        self.scale_retry_skipped_by_budget += other.scale_retry_skipped_by_budget;
        self.hv_subpixel_attempts += other.hv_subpixel_attempts;
        self.hv_refine_attempts += other.hv_refine_attempts;
        self.hv_refine_successes += other.hv_refine_successes;
        self.rs_erasure_attempts += other.rs_erasure_attempts;
        self.rs_erasure_successes += other.rs_erasure_successes;
        for i in 0..self.rs_erasure_count_hist.len() {
            self.rs_erasure_count_hist[i] += other.rs_erasure_count_hist[i];
        }
        self.phase11_time_budget_skips += other.phase11_time_budget_skips;
        if self.strategy_profile.is_empty() && !other.strategy_profile.is_empty() {
            self.strategy_profile = other.strategy_profile.clone();
        }
        for i in 0..self.candidate_score_buckets.len() {
            self.candidate_score_buckets[i] += other.candidate_score_buckets[i];
        }
    }
}

use decoder::qr_decoder::DecodeRequestContext;
use detector::contour::ContourDetector;
use detector::finder::{FinderDetector, FinderPattern};
use utils::binarization::{
    adaptive_binarize, adaptive_binarize_into, gamma_correct, invert_gray, otsu_binarize,
    otsu_binarize_into, sauvola_binarize, sauvola_binarize_bright, threshold_binarize,
};
use utils::grayscale::{
    normalize_roi_local_contrast, rgb_to_grayscale, rgb_to_grayscale_with_buffer,
};
use utils::memory_pool::BufferPool;

/// Pixel layout for an [`ImageInput`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// One luminance byte per pixel.
    Grayscale,
    /// Three interleaved bytes per pixel, in red-green-blue order.
    Rgb,
    /// Four interleaved bytes per pixel, in red-green-blue-alpha order.
    Rgba,
}

impl PixelFormat {
    const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Grayscale => 1,
            Self::Rgb => 3,
            Self::Rgba => 4,
        }
    }
}

/// A borrowed image supplied to [`try_detect`].
#[derive(Debug, Clone, Copy)]
pub struct ImageInput<'a> {
    pub data: &'a [u8],
    pub width: usize,
    pub height: usize,
    pub pixel_format: PixelFormat,
    /// Bytes between the starts of adjacent rows. `None` means tightly packed.
    pub stride: Option<usize>,
}

impl<'a> ImageInput<'a> {
    /// Create a tightly packed image input.
    pub const fn new(
        data: &'a [u8],
        width: usize,
        height: usize,
        pixel_format: PixelFormat,
    ) -> Self {
        Self {
            data,
            width,
            height,
            pixel_format,
            stride: None,
        }
    }

    /// Set the number of bytes between adjacent row starts.
    pub const fn with_stride(mut self, stride: usize) -> Self {
        self.stride = Some(stride);
        self
    }
}

/// Validation failures returned by fallible image entry points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputError {
    /// Width and height must both be non-zero.
    ZeroDimension,
    /// A dimension, channel, or stride calculation overflowed `usize`.
    DimensionOverflow,
    /// The row stride cannot contain one complete row.
    InvalidStride { stride: usize, minimum: usize },
    /// The input slice does not contain all addressed pixels.
    BufferTooShort { required: usize, actual: usize },
}

impl std::fmt::Display for InputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroDimension => write!(f, "image width and height must be non-zero"),
            Self::DimensionOverflow => write!(f, "image dimensions overflow addressable memory"),
            Self::InvalidStride { stride, minimum } => {
                write!(
                    f,
                    "image stride {stride} is smaller than row size {minimum}"
                )
            }
            Self::BufferTooShort { required, actual } => write!(
                f,
                "image buffer is too short: requires {required} bytes, got {actual}"
            ),
        }
    }
}

impl std::error::Error for InputError {}

fn validate_input(input: ImageInput<'_>) -> Result<(usize, usize), InputError> {
    if input.width == 0 || input.height == 0 {
        return Err(InputError::ZeroDimension);
    }
    let row_bytes = input
        .width
        .checked_mul(input.pixel_format.bytes_per_pixel())
        .ok_or(InputError::DimensionOverflow)?;
    let stride = input.stride.unwrap_or(row_bytes);
    if stride < row_bytes {
        return Err(InputError::InvalidStride {
            stride,
            minimum: row_bytes,
        });
    }
    let required = (input.height - 1)
        .checked_mul(stride)
        .and_then(|offset| offset.checked_add(row_bytes))
        .ok_or(InputError::DimensionOverflow)?;
    if input.data.len() < required {
        return Err(InputError::BufferTooShort {
            required,
            actual: input.data.len(),
        });
    }
    input
        .width
        .checked_mul(input.height)
        .ok_or(InputError::DimensionOverflow)?;
    Ok((row_bytes, stride))
}

fn input_to_grayscale(input: ImageInput<'_>) -> Result<Vec<u8>, InputError> {
    let (row_bytes, stride) = validate_input(input)?;
    let pixel_count = input
        .width
        .checked_mul(input.height)
        .ok_or(InputError::DimensionOverflow)?;
    let mut gray = Vec::new();
    gray.try_reserve_exact(pixel_count)
        .map_err(|_| InputError::DimensionOverflow)?;
    gray.resize(pixel_count, 0);

    for y in 0..input.height {
        let source_start = y.checked_mul(stride).ok_or(InputError::DimensionOverflow)?;
        let source = &input.data[source_start..source_start + row_bytes];
        let output = &mut gray[y * input.width..(y + 1) * input.width];
        match input.pixel_format {
            PixelFormat::Grayscale => output.copy_from_slice(source),
            PixelFormat::Rgb => {
                for (dst, pixel) in output.iter_mut().zip(source.chunks_exact(3)) {
                    *dst = ((76 * pixel[0] as u16 + 150 * pixel[1] as u16 + 29 * pixel[2] as u16)
                        >> 8) as u8;
                }
            }
            PixelFormat::Rgba => {
                for (dst, pixel) in output.iter_mut().zip(source.chunks_exact(4)) {
                    *dst = ((76 * pixel[0] as u16 + 150 * pixel[1] as u16 + 29 * pixel[2] as u16)
                        >> 8) as u8;
                }
            }
        }
    }
    Ok(gray)
}

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

    if max_v <= min_v.saturating_add(8) {
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

/// Check if image is overexposed (too bright) based on histogram analysis
fn is_overexposed(gray: &[u8]) -> bool {
    if gray.is_empty() {
        return false;
    }

    let mut histogram = [0u32; 256];
    for &pixel in gray {
        histogram[pixel as usize] += 1;
    }

    let total = gray.len() as u32;

    // Check if significant portion of pixels are very bright
    let bright_pixels: u32 = histogram[200..].iter().sum();
    let bright_ratio = bright_pixels as f32 / total as f32;

    // Check if median is in the bright range
    let mut cumulative = 0u32;
    let mut median = 128u8;
    for (i, &count) in histogram.iter().enumerate() {
        cumulative += count;
        if cumulative * 2 >= total {
            median = i as u8;
            break;
        }
    }

    // Image is overexposed if:
    // - More than 20% of pixels are in the very bright range (>200)
    // - OR median is above 180 (most pixels are bright)
    bright_ratio > 0.20 || median > 180
}

fn budgeted_decode(
    binary: &BitMatrix,
    gray: &[u8],
    width: usize,
    height: usize,
    patterns: &[FinderPattern],
    remaining: &mut usize,
) -> Vec<QRCode> {
    if *remaining == 0 {
        return Vec::new();
    }
    // A normal recovery lane is intentionally compact, but once the finder
    // stage has entered pipeline's spatial-routing regime, 24 trials cannot
    // possibly cover a dense scene.  The request's remaining candidate budget
    // is still the strict upper bound (128 by default), so this does not turn
    // a brightness fallback into an unbounded scan.
    let cap = if patterns.len() > 12 {
        *remaining
    } else {
        (*remaining).min(24)
    };
    let (decoded, tel) =
        pipeline::decode_groups_with_telemetry_limited(binary, gray, width, height, patterns, cap);
    *remaining = remaining.saturating_sub(tel.decode_attempts);
    decoded
}

/// Run brightness-specific detection for overexposed images
/// Simplified and optimized to avoid timeouts
fn run_brightness_detection<F: Fn() -> bool>(
    gray: &[u8],
    width: usize,
    height: usize,
    is_expired: &F,
    remaining: &mut usize,
) -> Vec<QRCode> {
    let window = auto_window(width, height);

    let gamma_corrected = gamma_correct(gray, 0.6);
    let gamma_otsu = otsu_binarize(&gamma_corrected, width, height);
    let patterns = detect_finder_patterns(&gamma_otsu, width, height);
    if patterns.len() >= 3 && patterns.len() <= FINDER_PATTERN_THRESHOLD {
        let decoded = budgeted_decode(&gamma_otsu, gray, width, height, &patterns, remaining);
        if !decoded.is_empty() {
            return decoded;
        }
    }

    if is_expired() || *remaining == 0 {
        return Vec::new();
    }

    if patterns.len() >= 2 {
        let contour_patterns = ContourDetector::detect(&gamma_otsu);
        if contour_patterns.len() >= 3 {
            let decoded = budgeted_decode(
                &gamma_otsu,
                gray,
                width,
                height,
                &contour_patterns,
                remaining,
            );
            if !decoded.is_empty() {
                return decoded;
            }
        }
    }

    if is_expired() || *remaining == 0 {
        return Vec::new();
    }

    let inverted = invert_gray(gray);
    let inv_otsu = otsu_binarize(&inverted, width, height);
    let inv_patterns = detect_finder_patterns(&inv_otsu, width, height);
    if inv_patterns.len() >= 3 && inv_patterns.len() <= FINDER_PATTERN_THRESHOLD {
        let decoded = budgeted_decode(&inv_otsu, gray, width, height, &inv_patterns, remaining);
        if !decoded.is_empty() {
            return decoded;
        }
    }

    if is_expired() || *remaining == 0 {
        return Vec::new();
    }

    if inv_patterns.len() >= 2 {
        let contour_patterns = ContourDetector::detect(&inv_otsu);
        if contour_patterns.len() >= 3 {
            let decoded =
                budgeted_decode(&inv_otsu, gray, width, height, &contour_patterns, remaining);
            if !decoded.is_empty() {
                return decoded;
            }
        }
    }

    if is_expired() || *remaining == 0 {
        return Vec::new();
    }

    let sauvola = sauvola_binarize_bright(gray, width, height, window);
    let sauv_patterns = detect_finder_patterns(&sauvola, width, height);
    if sauv_patterns.len() >= 3 && sauv_patterns.len() <= FINDER_PATTERN_THRESHOLD {
        let decoded = budgeted_decode(&sauvola, gray, width, height, &sauv_patterns, remaining);
        if !decoded.is_empty() {
            return decoded;
        }
    }

    Vec::new()
}

/// Threshold for early termination when too many finder patterns are detected.
///
/// The pipeline now switches to a spatial, bounded grouping path above twelve
/// proposals, so a real dense scene must not be diverted before that path can
/// route its local triples.  The value is deliberately finite: grouping keeps
/// at most 128 candidates and the request-wide decode budget still caps the
/// expensive work.
const FINDER_PATTERN_THRESHOLD: usize = 384;

fn histogram_median(gray: &[u8]) -> u8 {
    let mut hist = [0u32; 256];
    for &v in gray {
        hist[v as usize] += 1;
    }
    let half = gray.len() as u32 / 2;
    let mut cum = 0u32;
    for (i, &c) in hist.iter().enumerate() {
        cum += c;
        if cum >= half {
            return i as u8;
        }
    }
    128
}

fn run_detection_strategies<F: Fn() -> bool>(
    gray: &[u8],
    width: usize,
    height: usize,
    is_expired: &F,
) -> Vec<QRCode> {
    let window = auto_window(width, height);
    let large_window = (window * 2).clamp(63, 255);
    let median = histogram_median(gray) as i16;
    let t_dark = (median - 26).clamp(0, 255) as u8;
    let t_light = (median + 26).clamp(0, 255) as u8;
    let has_large = large_window != window;

    type BinFn<'a> = Box<dyn FnOnce() -> BitMatrix + 'a>;
    let variant_builders: Vec<(&str, BinFn)> = vec![
        (
            "sauvola_k02",
            Box::new(|| sauvola_binarize(gray, width, height, window, 0.2)),
        ),
        (
            "adaptive",
            Box::new(|| adaptive_binarize(gray, width, height, window)),
        ),
        ("otsu", Box::new(|| otsu_binarize(gray, width, height))),
        (
            "thresh_dark",
            Box::new(|| threshold_binarize(gray, width, height, t_dark)),
        ),
        (
            "thresh_light",
            Box::new(|| threshold_binarize(gray, width, height, t_light)),
        ),
        (
            "sauvola_k01",
            Box::new(|| sauvola_binarize(gray, width, height, window, 0.1)),
        ),
        (
            "sauvola_k03",
            Box::new(|| sauvola_binarize(gray, width, height, window, 0.3)),
        ),
    ];

    let large_builders: Vec<(&str, BinFn)> = if has_large {
        vec![
            (
                "sauvola_lw",
                Box::new(|| sauvola_binarize(gray, width, height, large_window, 0.2)),
            ),
            (
                "adaptive_lw",
                Box::new(|| adaptive_binarize(gray, width, height, large_window)),
            ),
        ]
    } else {
        vec![]
    };

    let all_builders = variant_builders.into_iter().chain(large_builders);

    let mut results = Vec::new();
    for (_name, build_fn) in all_builders {
        if is_expired() {
            break;
        }

        let binary = build_fn();
        let finder_patterns = detect_finder_patterns(&binary, width, height);

        let decoded = if finder_patterns.len() > FINDER_PATTERN_THRESHOLD {
            Vec::new()
        } else if finder_patterns.len() >= 2 {
            decode_groups_with_module_aware_retry(&binary, gray, width, height, &finder_patterns)
        } else {
            Vec::new()
        };
        let finder_decode_failed = decoded.is_empty();
        for qr in decoded {
            if !results.iter().any(|r: &QRCode| r.content == qr.content) {
                results.push(qr);
            }
        }
        if (results.is_empty()
            || (finder_patterns.len() >= 2 && finder_decode_failed)
            || finder_patterns.len() > FINDER_PATTERN_THRESHOLD)
            && !is_expired()
        {
            let contour_patterns = ContourDetector::detect(&binary);
            if contour_patterns.len() >= 2 {
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum BinarizationPolicy {
    Otsu,
    Adaptive31,
    Adaptive21,
}

fn initial_policy(width: usize, height: usize) -> BinarizationPolicy {
    if width >= 800 || height >= 800 {
        BinarizationPolicy::Adaptive31
    } else {
        BinarizationPolicy::Otsu
    }
}

fn phase9_binarization_sequence(width: usize, height: usize) -> Vec<BinarizationPolicy> {
    let strict = initial_policy(width, height);
    let mut sequence = vec![strict];
    for policy in [
        BinarizationPolicy::Otsu,
        BinarizationPolicy::Adaptive31,
        BinarizationPolicy::Adaptive21,
    ] {
        if !sequence.contains(&policy) {
            sequence.push(policy);
        }
    }
    sequence
}

fn binarize_with_policy(
    gray: &[u8],
    width: usize,
    height: usize,
    policy: BinarizationPolicy,
) -> BitMatrix {
    match policy {
        BinarizationPolicy::Otsu => otsu_binarize(gray, width, height),
        BinarizationPolicy::Adaptive31 => adaptive_binarize(gray, width, height, 31),
        BinarizationPolicy::Adaptive21 => adaptive_binarize(gray, width, height, 21),
    }
}

fn image_decode_attempt_budget() -> usize {
    decoder::config::image_decode_attempt_budget()
}

fn record_binarization_transition(
    tel: &mut DetectionTelemetry,
    from: BinarizationPolicy,
    to: BinarizationPolicy,
) {
    if from == BinarizationPolicy::Otsu && to == BinarizationPolicy::Adaptive31 {
        tel.bin_fallback_otsu_to_adaptive31 += 1;
    } else if from == BinarizationPolicy::Adaptive31 && to == BinarizationPolicy::Adaptive21 {
        tel.bin_fallback_adaptive31_to_adaptive21 += 1;
    }
}

fn grayscale_contrast_span(gray: &[u8]) -> u8 {
    if gray.is_empty() {
        return 0;
    }
    let mut min_v = u8::MAX;
    let mut max_v = u8::MIN;
    for &v in gray {
        min_v = min_v.min(v);
        max_v = max_v.max(v);
    }
    max_v.saturating_sub(min_v)
}

fn finder_roi_bounds(
    finder_patterns: &[FinderPattern],
    width: usize,
    height: usize,
) -> Option<(usize, usize, usize, usize)> {
    if finder_patterns.len() < 3 {
        return None;
    }
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = 0.0f32;
    let mut max_y = 0.0f32;
    let mut avg_module = 0.0f32;

    for p in finder_patterns.iter().take(6) {
        min_x = min_x.min(p.center.x);
        min_y = min_y.min(p.center.y);
        max_x = max_x.max(p.center.x);
        max_y = max_y.max(p.center.y);
        avg_module += p.module_size.max(1.0);
    }
    avg_module /= finder_patterns.len().min(6) as f32;
    let pad = (avg_module * 20.0).clamp(16.0, 220.0);
    let x0 = (min_x - pad).floor().max(0.0) as usize;
    let y0 = (min_y - pad).floor().max(0.0) as usize;
    let x1 = (max_x + pad).ceil().min(width as f32) as usize;
    let y1 = (max_y + pad).ceil().min(height as f32) as usize;
    if x0 >= x1 || y0 >= y1 {
        None
    } else {
        Some((x0, y0, x1, y1))
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
    if !results.is_empty() {
        return results;
    }

    if finder_patterns.len() == 2 {
        return decode_two_finder_fallback(binary, gray, width, height, finder_patterns);
    }
    if finder_patterns.len() < 3 {
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

fn decode_two_finder_fallback(
    binary: &BitMatrix,
    gray: &[u8],
    width: usize,
    height: usize,
    finder_patterns: &[FinderPattern],
) -> Vec<QRCode> {
    decode_two_finder_fallback_limited(binary, gray, width, height, finder_patterns, None, None)
}

fn decode_two_finder_fallback_limited(
    binary: &BitMatrix,
    gray: &[u8],
    width: usize,
    height: usize,
    finder_patterns: &[FinderPattern],
    mut remaining_attempts: Option<&mut usize>,
    mut telemetry: Option<&mut DetectionTelemetry>,
) -> Vec<QRCode> {
    if finder_patterns.len() < 2 {
        return Vec::new();
    }
    let a = &finder_patterns[0];
    let b = &finder_patterns[1];
    let vx = b.center.x - a.center.x;
    let vy = b.center.y - a.center.y;
    let len = (vx * vx + vy * vy).sqrt();
    if len < 6.0 {
        return Vec::new();
    }
    let nx = -vy / len;
    let ny = vx / len;
    let span = len;
    let module = ((a.module_size + b.module_size) * 0.5).max(1.0);

    let candidates = [
        Point::new(a.center.x + nx * span, a.center.y + ny * span),
        Point::new(b.center.x + nx * span, b.center.y + ny * span),
        Point::new(a.center.x - nx * span, a.center.y - ny * span),
        Point::new(b.center.x - nx * span, b.center.y - ny * span),
    ];

    for c in candidates {
        if c.x < 0.0 || c.y < 0.0 || c.x >= width as f32 || c.y >= height as f32 {
            continue;
        }
        if let Some(remaining) = remaining_attempts.as_deref_mut() {
            if *remaining == 0 {
                if let Some(tel) = telemetry.as_deref_mut() {
                    tel.budget_skips += 1;
                }
                break;
            }
        }
        let synthetic = FinderPattern {
            center: c,
            module_size: module,
        };
        let trial = vec![&finder_patterns[0], &finder_patterns[1], &synthetic];
        let mut fused = Vec::with_capacity(3);
        for p in trial {
            fused.push(FinderPattern {
                center: p.center,
                module_size: p.module_size,
            });
        }
        let decoded = if let Some(remaining) = remaining_attempts.as_deref_mut() {
            let (decoded, decode_tel) = pipeline::decode_groups_with_telemetry_limited(
                binary, gray, width, height, &fused, *remaining,
            );
            *remaining = remaining.saturating_sub(decode_tel.decode_attempts);
            if let Some(tel) = telemetry.as_deref_mut() {
                tel.merge_high_water_from(&decode_tel);
            }
            decoded
        } else {
            pipeline::decode_groups(binary, gray, width, height, &fused)
        };
        if !decoded.is_empty() {
            return decoded;
        }
    }

    Vec::new()
}

fn run_fast_path(gray: &[u8], width: usize, height: usize) -> Vec<QRCode> {
    // Fast path: one cheap global threshold pass only.
    let binary = otsu_binarize(gray, width, height);
    let finder_patterns = detect_finder_patterns(&binary, width, height);
    if finder_patterns.len() < 2 {
        return Vec::new();
    }
    pipeline::decode_groups(&binary, gray, width, height, &finder_patterns)
}

fn run_detection_with_phase4_fallbacks<F>(
    gray: &[u8],
    width: usize,
    height: usize,
    is_expired: F,
) -> Vec<QRCode>
where
    F: Fn() -> bool,
{
    let mut remaining = image_decode_attempt_budget();

    if is_overexposed(gray) {
        if is_expired() {
            return Vec::new();
        }
        let results = run_brightness_detection(gray, width, height, &is_expired, &mut remaining);
        if !results.is_empty() {
            return results;
        }
    }

    if is_expired() {
        return Vec::new();
    }
    let mut results = run_detection_strategies(gray, width, height, &is_expired);
    if !results.is_empty() {
        return results;
    }

    if is_expired() {
        return Vec::new();
    }
    let enhanced = contrast_stretch(gray);
    results = run_detection_strategies(&enhanced, width, height, &is_expired);
    if !results.is_empty() {
        return results;
    }

    if is_expired() {
        return Vec::new();
    }
    let rotated = rotate_gray_45(gray, width, height);
    run_detection_strategies(&rotated, width, height, &is_expired)
}

/// Detect QR codes from a validated image description.
///
/// This is the preferred public entry point. It accepts grayscale, RGB, and
/// RGBA data, including row padding, and rejects malformed layouts before any
/// conversion or detection code runs.
pub fn try_detect(input: ImageInput<'_>) -> Result<Vec<QRCode>, InputError> {
    let width = input.width;
    let height = input.height;
    let gray = input_to_grayscale(input)?;
    Ok(detect_grayscale_validated(&gray, width, height))
}

/// Detect QR codes with request-scoped options and optional diagnostics.
///
/// This entry point is the preferred library API for applications that need a
/// bounded request.  The deadline is cooperative: an individual inner image
/// operation is not preempted, so callers that require a hard wall-clock cap
/// should run the request in an isolated worker and discard late results.
pub fn try_detect_with_options(
    input: ImageInput<'_>,
    options: DecoderOptions,
) -> Result<DetectionResult, InputError> {
    let width = input.width;
    let height = input.height;
    let gray = input_to_grayscale(input)?;

    if !options.diagnostics_enabled() {
        let mut rgb = Vec::with_capacity(gray.len() * 3);
        for value in gray {
            rgb.extend_from_slice(&[value, value, value]);
        }
        let (codes, _) = detect_with_telemetry_budget(
            &rgb,
            width,
            height,
            Some(options.deadline()),
            Some(options.candidate_limit()),
            Some(options.erasure_attempt_limit()),
            options.cancellation(),
        );
        return Ok(DetectionResult {
            codes,
            diagnostics: RequestDiagnostics {
                failure_stage: None,
                telemetry: None,
            },
        });
    }

    // The telemetry path accepts RGB; expand normalized luminance once so all
    // image layouts share one diagnostics implementation.
    let mut rgb = Vec::with_capacity(gray.len() * 3);
    for value in gray {
        rgb.extend_from_slice(&[value, value, value]);
    }
    let started = std::time::Instant::now();
    let (codes, telemetry) = detect_with_telemetry_budget(
        &rgb,
        width,
        height,
        Some(options.deadline()),
        Some(options.candidate_limit()),
        Some(options.erasure_attempt_limit()),
        options.cancellation(),
    );
    let failure_stage = if codes.is_empty() {
        Some(if options.is_cancelled() {
            FailureStage::Cancelled
        } else if started.elapsed() >= options.deadline() {
            FailureStage::Timeout
        } else {
            classify_failure_stage(&telemetry)
        })
    } else {
        None
    };
    Ok(DetectionResult {
        codes,
        diagnostics: RequestDiagnostics {
            failure_stage,
            telemetry: Some(telemetry),
        },
    })
}

fn classify_failure_stage(telemetry: &DetectionTelemetry) -> FailureStage {
    if telemetry.finder_patterns_found < 2 {
        FailureStage::Detection
    } else if telemetry.transforms_built == 0 {
        FailureStage::Geometry
    } else if telemetry.unsupported_content > 0 {
        FailureStage::UnsupportedContent
    } else if telemetry.rs_decode_ok == 0 {
        FailureStage::ReedSolomon
    } else {
        FailureStage::Payload
    }
}

/// Detect QR codes in a tightly packed RGB image.
///
/// # Arguments
/// * `image` - Raw RGB bytes (3 bytes per pixel)
/// * `width` - Image width in pixels
/// * `height` - Image height in pixels
///
/// # Returns
/// Vector of detected QR codes
///
/// This compatibility wrapper returns an empty result for malformed input.
/// New callers should use [`try_detect`] to receive a structured error.
pub fn detect(image: &[u8], width: usize, height: usize) -> Vec<QRCode> {
    try_detect(ImageInput::new(image, width, height, PixelFormat::Rgb)).unwrap_or_default()
}

fn detect_grayscale_validated(image: &[u8], width: usize, height: usize) -> Vec<QRCode> {
    let start = std::time::Instant::now();
    let budget_ms = decoder::config::global_time_budget_ms();
    let is_expired = || start.elapsed().as_millis() as u64 >= budget_ms;

    if is_expired() {
        return Vec::new();
    }
    let fast = run_fast_path(image, width, height);
    if !fast.is_empty() {
        return fast;
    }

    run_detection_with_phase4_fallbacks(image, width, height, is_expired)
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
    detect_with_telemetry_budget(image, width, height, None, None, None, None)
}

/// Detect with telemetry while applying a caller-supplied cooperative budget.
///
/// Expensive pipeline and decoder loops observe the shared deadline, but this is not
/// preemptive cancellation: a single uninterruptible operation can return
/// after the requested duration. Callers must measure elapsed time to classify
/// that case as a timeout and discard its results.
pub fn detect_with_telemetry_timeout(
    image: &[u8],
    width: usize,
    height: usize,
    timeout: std::time::Duration,
) -> (Vec<QRCode>, DetectionTelemetry) {
    detect_with_telemetry_budget(image, width, height, Some(timeout), None, None, None)
}

fn detect_with_telemetry_budget(
    image: &[u8],
    width: usize,
    height: usize,
    requested_timeout: Option<std::time::Duration>,
    requested_candidate_limit: Option<usize>,
    requested_erasure_attempt_limit: Option<usize>,
    cancellation: Option<CancellationToken>,
) -> (Vec<QRCode>, DetectionTelemetry) {
    if validate_input(ImageInput::new(image, width, height, PixelFormat::Rgb)).is_err() {
        return (Vec::new(), DetectionTelemetry::default());
    }
    let mut tel = DetectionTelemetry::default();

    let start_tel = std::time::Instant::now();
    let budget_tel = requested_timeout.unwrap_or_else(|| {
        std::time::Duration::from_millis(decoder::config::global_time_budget_ms())
    });
    let deadline_tel = start_tel + budget_tel;
    let mut decode_context = DecodeRequestContext::with_deadline_and_cancellation(
        requested_erasure_attempt_limit.unwrap_or(0),
        deadline_tel,
        cancellation.clone(),
    );
    let is_expired_tel = || {
        cancellation
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
            || start_tel.elapsed() >= budget_tel
    };
    let gray = rgb_to_grayscale(image, width, height);
    if is_expired_tel() {
        return (Vec::new(), tel);
    }
    let request_candidate_limit =
        requested_candidate_limit.unwrap_or_else(image_decode_attempt_budget);
    let mut remaining_attempts = request_candidate_limit;

    if is_overexposed(&gray) {
        let results = run_brightness_detection(
            &gray,
            width,
            height,
            &is_expired_tel,
            &mut remaining_attempts,
        );
        if !results.is_empty() {
            tel.qr_codes_found = results.len();
            return (results, tel);
        }
    }

    // Step 2+: strict path first, then bounded fallback binarization ensemble on miss.
    let policies = phase9_binarization_sequence(width, height);
    let mut results = Vec::new();
    let mut prev_policy = policies[0];
    let mut best_finder_patterns: Vec<FinderPattern> = Vec::new();
    tel.binarize_ok = true;
    for (i, &policy) in policies.iter().enumerate() {
        // Do not begin another full-image binarization/finder pass after the
        // request has expired. Individual passes remain cooperative and may
        // finish late, but the scheduler must not compound that overrun.
        if is_expired_tel() {
            break;
        }
        if i > 0 {
            record_binarization_transition(&mut tel, prev_policy, policy);
            prev_policy = policy;
        }
        if remaining_attempts == 0 {
            tel.budget_skips += 1;
            break;
        }

        let binary = binarize_with_policy(&gray, width, height, policy);
        // Binarization itself is not preemptible; do not start a finder scan
        // if it consumed the remaining request budget.
        if is_expired_tel() {
            break;
        }
        let finder_patterns = if width >= 1600 && height >= 1600 {
            FinderDetector::detect_with_pyramid(&binary)
        } else {
            FinderDetector::detect(&binary)
        };
        if finder_patterns.len() > best_finder_patterns.len() {
            best_finder_patterns = finder_patterns.clone();
        }
        tel.finder_patterns_found = tel.finder_patterns_found.max(finder_patterns.len());

        // Early termination: if too many finder patterns, skip expensive decode_groups
        let too_many_patterns = finder_patterns.len() > FINDER_PATTERN_THRESHOLD;

        if finder_patterns.len() >= 3 && !too_many_patterns {
            let (decoded, decode_tel) = pipeline::decode_groups_with_telemetry_limited_in_context(
                &binary,
                &gray,
                width,
                height,
                &finder_patterns,
                remaining_attempts,
                &mut decode_context,
            );
            remaining_attempts = remaining_attempts.saturating_sub(decode_tel.decode_attempts);
            tel.merge_high_water_from(&decode_tel);
            if !decoded.is_empty() {
                if i > 0 {
                    tel.bin_fallback_successes += 1;
                }
                results = decoded;
                break;
            }
        } else if finder_patterns.len() == 2 && !too_many_patterns {
            tel.two_finder_attempts += 1;
            let decoded = decode_two_finder_fallback_limited(
                &binary,
                &gray,
                width,
                height,
                &finder_patterns,
                Some(&mut remaining_attempts),
                Some(&mut tel),
            );
            if !decoded.is_empty() {
                tel.two_finder_successes += 1;
                if i > 0 {
                    tel.bin_fallback_successes += 1;
                }
                results = decoded;
                break;
            }
        }

        // If too many finder patterns, skip directly to contour detection
        if too_many_patterns {
            if is_expired_tel() {
                break;
            }
            let contour_patterns = ContourDetector::detect(&binary);
            if contour_patterns.len() >= 3 {
                let (decoded, decode_tel) =
                    pipeline::decode_groups_with_telemetry_limited_in_context(
                        &binary,
                        &gray,
                        width,
                        height,
                        &contour_patterns,
                        remaining_attempts,
                        &mut decode_context,
                    );
                remaining_attempts = remaining_attempts.saturating_sub(decode_tel.decode_attempts);
                tel.merge_high_water_from(&decode_tel);
                if !decoded.is_empty() {
                    if i > 0 {
                        tel.bin_fallback_successes += 1;
                    }
                    results = decoded;
                    break;
                }
            }
        }
    }

    // Contour fallback: try when finder patterns exist but grouping/decode failed
    if results.is_empty() && !best_finder_patterns.is_empty() && remaining_attempts > 0 {
        for &policy in policies.iter().take(2) {
            if is_expired_tel() || remaining_attempts == 0 {
                break;
            }
            let binary = binarize_with_policy(&gray, width, height, policy);
            if is_expired_tel() {
                break;
            }
            let contour_patterns = ContourDetector::detect(&binary);
            if contour_patterns.len() >= 3 && contour_patterns.len() <= FINDER_PATTERN_THRESHOLD {
                let (decoded, decode_tel) =
                    pipeline::decode_groups_with_telemetry_limited_in_context(
                        &binary,
                        &gray,
                        width,
                        height,
                        &contour_patterns,
                        remaining_attempts,
                        &mut decode_context,
                    );
                remaining_attempts = remaining_attempts.saturating_sub(decode_tel.decode_attempts);
                tel.merge_high_water_from(&decode_tel);
                if !decoded.is_empty() {
                    results = decoded;
                    break;
                }
            }
        }
    }

    if results.is_empty() {
        let weak_contrast = grayscale_contrast_span(&gray) <= 90;
        if is_expired_tel() || remaining_attempts == 0 || !weak_contrast {
            tel.roi_norm_skipped += 1;
        } else if let Some(roi) = finder_roi_bounds(&best_finder_patterns, width, height) {
            if is_expired_tel() {
                tel.roi_norm_skipped += 1;
            } else {
                let normalized_gray = normalize_roi_local_contrast(&gray, width, height, roi);
                let norm_binary = adaptive_binarize(&normalized_gray, width, height, 31);
                if is_expired_tel() {
                    tel.roi_norm_skipped += 1;
                } else {
                    tel.roi_norm_attempts += 1;
                    let norm_patterns = if width >= 1600 && height >= 1600 {
                        FinderDetector::detect_with_pyramid(&norm_binary)
                    } else {
                        FinderDetector::detect(&norm_binary)
                    };
                    tel.finder_patterns_found = tel.finder_patterns_found.max(norm_patterns.len());
                    if norm_patterns.len() >= 3 {
                        let (decoded, decode_tel) =
                            pipeline::decode_groups_with_telemetry_limited_in_context(
                                &norm_binary,
                                &normalized_gray,
                                width,
                                height,
                                &norm_patterns,
                                remaining_attempts,
                                &mut decode_context,
                            );
                        tel.merge_high_water_from(&decode_tel);
                        if !decoded.is_empty() {
                            tel.roi_norm_successes += 1;
                            results = decoded;
                        }
                    } else {
                        tel.roi_norm_skipped += 1;
                    }
                }
            }
        } else {
            tel.roi_norm_skipped += 1;
        }
    }

    tel.qr_codes_found = results.len();
    let counters = decode_context.counters();
    tel.format_extracted = counters.format_bch_candidates;
    tel.format_bch_distance_hist = counters.format_bch_distance_hist;
    tel.rs_candidate_attempts = counters.rs_candidate_attempts;
    tel.nonzero_remainder_bit_rejections = counters.nonzero_remainder_bit_rejections;
    tel.rs_block_attempts = counters.rs_block_attempts;
    tel.rs_block_successes = counters.rs_block_successes;
    tel.rs_block_failures = counters.rs_block_failures;
    tel.deskew_attempts = counters.deskew_attempts;
    tel.deskew_successes = counters.deskew_successes;
    tel.high_version_precision_attempts = counters.high_version_precision_attempts;
    tel.recovery_mode_attempts = counters.recovery_mode_attempts;
    tel.scale_retry_attempts = counters.scale_retry_attempts;
    tel.scale_retry_successes = counters.scale_retry_successes;
    tel.scale_retry_skipped_by_budget = counters.scale_retry_skipped_by_budget;
    tel.hv_subpixel_attempts = counters.hv_subpixel_attempts;
    tel.hv_refine_attempts = counters.hv_refine_attempts;
    tel.hv_refine_successes = counters.hv_refine_successes;
    tel.rs_erasure_attempts = counters.rs_erasure_attempts;
    tel.rs_erasure_successes = counters.rs_erasure_successes;
    tel.rs_erasure_count_hist = counters.rs_erasure_count_hist;
    tel.phase11_time_budget_skips = counters.phase11_time_budget_skips;
    tel.unsupported_content = counters.unsupported_payloads;
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
    try_detect(ImageInput::new(
        image,
        width,
        height,
        PixelFormat::Grayscale,
    ))
    .unwrap_or_default()
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
    if validate_input(ImageInput::new(image, width, height, PixelFormat::Rgb)).is_err() {
        return Vec::new();
    }
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
        if fallback_patterns.len() >= 2 {
            finder_patterns = fallback_patterns;
            binary = if width >= 800 || height >= 800 {
                bin_otsu
            } else {
                bin_adaptive
            };
        }
    }

    // Early termination: if too many finder patterns, skip directly to contour detection
    let too_many_patterns = finder_patterns.len() > FINDER_PATTERN_THRESHOLD;

    // Step 4: Group and decode
    let mut results = if too_many_patterns {
        Vec::new()
    } else {
        decode_groups_with_module_aware_retry(binary, gray_buffer, width, height, &finder_patterns)
    };

    // If too many patterns or decode failed, try contour detection
    if results.is_empty() {
        let contour_patterns = ContourDetector::detect(binary);
        if contour_patterns.len() >= 2 {
            results =
                pipeline::decode_groups(binary, gray_buffer, width, height, &contour_patterns);
        }
    }

    // Sauvola fallback: adapts to local contrast (handles shadows/glare)
    if results.is_empty() && !too_many_patterns {
        let sauvola = sauvola_binarize(gray_buffer, width, height, 31, 0.2);
        let sauvola_patterns = detect_finder_patterns(&sauvola, width, height);
        if sauvola_patterns.len() >= 2 && sauvola_patterns.len() <= FINDER_PATTERN_THRESHOLD {
            results = decode_groups_with_module_aware_retry(
                &sauvola,
                gray_buffer,
                width,
                height,
                &sauvola_patterns,
            );
        }
    }

    if results.is_empty() && !too_many_patterns {
        let fallback_patterns = if width >= 800 || height >= 800 {
            detect_finder_patterns(bin_otsu, width, height)
        } else {
            detect_finder_patterns(bin_adaptive, width, height)
        };
        if fallback_patterns.len() >= 2 && fallback_patterns.len() <= FINDER_PATTERN_THRESHOLD {
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

    /// Detect QR codes from a checked image description.
    pub fn try_detect(&mut self, input: ImageInput<'_>) -> Result<Vec<QRCode>, InputError> {
        // Strided and non-RGB formats are normalized before detection. The pool
        // remains an optimization for the legacy tightly packed RGB entry point.
        try_detect(input)
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

    #[test]
    fn decoder_options_presets_are_immutable_and_ordered() {
        let fast = DecoderOptions::with_preset(DecoderPreset::Fast);
        let balanced = DecoderOptions::default();
        let exhaustive = DecoderOptions::with_preset(DecoderPreset::Exhaustive);
        assert!(fast.deadline() < balanced.deadline());
        assert!(balanced.deadline() < exhaustive.deadline());
        assert!(fast.candidate_limit() < balanced.candidate_limit());
        assert!(balanced.candidate_limit() < exhaustive.candidate_limit());
        assert!(fast.erasure_attempt_limit() < balanced.erasure_attempt_limit());
        assert!(balanced.erasure_attempt_limit() < exhaustive.erasure_attempt_limit());
        assert_eq!(fast.clone().with_candidate_limit(3).candidate_limit(), 3);
        assert_eq!(
            fast.clone()
                .with_erasure_attempt_limit(3)
                .erasure_attempt_limit(),
            3
        );
        assert_eq!(fast.candidate_limit(), 16);
        assert_eq!(fast.erasure_attempt_limit(), 4);
        assert!(!fast.diagnostics_enabled());
        assert!(fast.clone().with_diagnostics(true).diagnostics_enabled());
        assert!(!fast.diagnostics_enabled());
    }

    #[test]
    fn cancellation_is_request_scoped_and_reported() {
        let cancelled = CancellationToken::new();
        let independent = CancellationToken::new();
        cancelled.cancel();
        let image = vec![255u8; 32 * 32 * 3];

        let stopped = try_detect_with_options(
            ImageInput::new(&image, 32, 32, PixelFormat::Rgb),
            DecoderOptions::default()
                .with_diagnostics(true)
                .with_cancellation(cancelled),
        )
        .expect("valid image");
        assert!(stopped.codes.is_empty());
        assert_eq!(
            stopped.diagnostics.failure_stage,
            Some(FailureStage::Cancelled)
        );

        let live = try_detect_with_options(
            ImageInput::new(&image, 32, 32, PixelFormat::Rgb),
            DecoderOptions::default()
                .with_diagnostics(true)
                .with_cancellation(independent),
        )
        .expect("valid image");
        assert_eq!(
            live.diagnostics.failure_stage,
            Some(FailureStage::Detection)
        );
    }

    #[test]
    fn expired_deadline_does_not_schedule_binarization_or_finder() {
        let image = vec![255u8; 64 * 64 * 3];
        let (codes, telemetry) =
            detect_with_telemetry_timeout(&image, 64, 64, std::time::Duration::ZERO);

        assert!(codes.is_empty());
        assert!(!telemetry.binarize_ok);
        assert_eq!(telemetry.finder_patterns_found, 0);
        assert_eq!(telemetry.decode_attempts, 0);
    }

    #[test]
    fn concurrent_diagnostic_requests_keep_telemetry_request_scoped() {
        use std::sync::{Arc, Barrier};

        let barrier = Arc::new(Barrier::new(4));
        let mut workers = Vec::new();
        for candidate_limit in [0, 1, 16, 128] {
            let barrier = Arc::clone(&barrier);
            workers.push(std::thread::spawn(move || {
                let image = vec![255u8; 32 * 32 * 3];
                barrier.wait();
                let result = try_detect_with_options(
                    ImageInput::new(&image, 32, 32, PixelFormat::Rgb),
                    DecoderOptions::default()
                        .with_candidate_limit(candidate_limit)
                        .with_diagnostics(true),
                )
                .expect("valid RGB image");
                let telemetry = result.diagnostics.telemetry.expect("diagnostics requested");
                assert_eq!(
                    result.diagnostics.failure_stage,
                    Some(FailureStage::Detection)
                );
                assert_eq!(telemetry.decode_attempts, 0);
                assert_eq!(telemetry.rs_erasure_attempts, 0);
                telemetry
            }));
        }

        for worker in workers {
            let telemetry = worker.join().expect("worker should not panic");
            assert_eq!(telemetry.qr_codes_found, 0);
            assert_eq!(telemetry.finder_patterns_found, 0);
        }
    }

    #[test]
    fn request_diagnostics_classify_detection_miss() {
        let image = vec![255u8; 10 * 10 * 3];
        let result = try_detect_with_options(
            ImageInput::new(&image, 10, 10, PixelFormat::Rgb),
            DecoderOptions::default().with_diagnostics(true),
        )
        .expect("valid RGB image");
        assert!(result.codes.is_empty());
        assert_eq!(
            result.diagnostics.failure_stage,
            Some(FailureStage::Detection)
        );
        assert!(result.diagnostics.telemetry.is_some());
    }

    #[test]
    fn request_diagnostics_expose_unsupported_content() {
        let telemetry = DetectionTelemetry {
            finder_patterns_found: 3,
            transforms_built: 1,
            unsupported_content: 1,
            ..DetectionTelemetry::default()
        };
        assert_eq!(
            classify_failure_stage(&telemetry),
            FailureStage::UnsupportedContent
        );
    }

    #[test]
    fn request_without_diagnostics_avoids_telemetry() {
        let image = vec![255u8; 10 * 10 * 3];
        let result = try_detect_with_options(
            ImageInput::new(&image, 10, 10, PixelFormat::Rgb),
            DecoderOptions::default(),
        )
        .expect("valid RGB image");
        assert!(result.diagnostics.telemetry.is_none());
        assert!(result.diagnostics.failure_stage.is_none());
    }

    #[test]
    fn zero_candidate_limit_skips_work_without_diagnostics() {
        let image = vec![255u8; 32 * 32 * 3];
        let result = try_detect_with_options(
            ImageInput::new(&image, 32, 32, PixelFormat::Rgb),
            DecoderOptions::default().with_candidate_limit(0),
        )
        .expect("valid RGB image");
        assert!(result.codes.is_empty());
        assert!(result.diagnostics.telemetry.is_none());
    }

    #[test]
    fn padded_pixel_formats_normalize_equivalently() {
        let grayscale = [17, 47, 77, 107, 0, 0, 137, 167, 197, 227];
        let rgb = [
            18, 18, 18, 48, 48, 48, 78, 78, 78, 108, 108, 108, 255, 255, 255, 138, 138, 138, 168,
            168, 168, 198, 198, 198, 228, 228, 228,
        ];
        let rgba = [
            18, 18, 18, 1, 48, 48, 48, 2, 78, 78, 78, 3, 108, 108, 108, 4, 255, 255, 255, 255, 138,
            138, 138, 5, 168, 168, 168, 6, 198, 198, 198, 7, 228, 228, 228, 8,
        ];

        let expected = vec![17, 47, 77, 107, 137, 167, 197, 227];
        assert_eq!(
            input_to_grayscale(
                ImageInput::new(&grayscale, 4, 2, PixelFormat::Grayscale).with_stride(6)
            ),
            Ok(expected.clone())
        );
        assert_eq!(
            input_to_grayscale(ImageInput::new(&rgb, 4, 2, PixelFormat::Rgb).with_stride(15)),
            Ok(expected.clone())
        );
        assert_eq!(
            input_to_grayscale(ImageInput::new(&rgba, 4, 2, PixelFormat::Rgba).with_stride(20)),
            Ok(expected)
        );
    }

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
