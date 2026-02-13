use std::cmp::Ordering;
use std::time::Instant;

use crate::config::DetectConfig;
use crate::types::{DecodeCandidate, Hypothesis, Point, Proposal, QrCode};

use super::state::PipelineState;

const FALLBACK_MIN_HYPOTHESIS_SCORE: f32 = 0.55;
const FALLBACK_MIN_PROPOSAL_SCORE: f32 = 0.50;
const FALLBACK_MAX_BASE_DECODES: usize = 1;
const FALLBACK_MAX_PIXELS: usize = 16_000_000;
const FALLBACK_FULL_IMAGE_VARIANT_MAX_PIXELS: usize = 3_500_000;
const GRID_RESCUE_MAX_PIXELS: usize = 2_500_000;
const GRID_RESCUE_MAX_RESULTS: usize = 32;
const GRID_RESCUE_STEPS: usize = 3;
const GRID_RESCUE_WINDOW_SIZES: [usize; 2] = [512, 768];
const MAX_IDENTIFY_ATTEMPTS_PER_IMAGE: usize = 16;
const RESCUE_MIN_REMAINING_MS_GRID: f64 = 280.0;
const RESCUE_MIN_REMAINING_MS_CHANNEL: f64 = 220.0;
const RESCUE_MIN_REMAINING_MS_UPSCALE: f64 = 260.0;
const RESCUE_MIN_REMAINING_MS_FULL_VARIANTS: f64 = 180.0;
const CHANNEL_RESCUE_MAX_PIXELS: usize = 2_500_000;
const UPSCALE_RESCUE_MAX_PIXELS: usize = 2_000_000;
const UPSCALE_RESCUE_FACTOR: usize = 2;
const BASE_DECODE_MAX_PIXELS: usize = 3_000_000;
const BASE_DECODE_MAX_DIM: usize = 1800;
const BASE_DECODE_MAX_DIM_MEDIUM: usize = 1400;
const BASE_DECODE_MAX_DIM_TIGHT: usize = 1100;
const BASE_DECODE_MAX_DIM_CRITICAL: usize = 900;
const BASE_DECODE_FORCE_RESIZE_PIXELS_TIGHT: usize = 1_600_000;
const BASE_DECODE_REMAINING_MS_TIGHT: f64 = 260.0;
const BASE_DECODE_REMAINING_MS_CRITICAL: f64 = 140.0;
const LOCAL_DECODE_MAX_HYPOTHESES: usize = 8;
const LOCAL_DECODE_MIN_HYPOTHESIS_SCORE: f32 = 0.35;
const LOCAL_DECODE_MAX_PROPOSALS: usize = 16;
const LOCAL_DECODE_MIN_PROPOSAL_SCORE: f32 = 0.35;
const LOCAL_DECODE_MAX_RESULTS: usize = 64;
const LOCAL_DECODE_WINDOW_SIZES_STANDARD: [usize; 3] = [224, 320, 512];
const LOCAL_DECODE_WINDOW_SIZES_TIGHT: [usize; 2] = [224, 320];
const LOCAL_DECODE_WINDOW_SIZES_LARGE: [usize; 2] = [256, 384];

#[derive(Clone, Copy)]
struct FallbackPolicy {
    max_hypotheses: usize,
    max_proposals: usize,
    max_results: usize,
    windows: &'static [usize],
    allow_full_image_variants: bool,
}

struct DecodeGuard {
    fallback_deadline: Option<Instant>,
    attempts_remaining: usize,
}

impl DecodeGuard {
    fn new(fallback_deadline: Option<Instant>) -> Self {
        Self {
            fallback_deadline,
            attempts_remaining: MAX_IDENTIFY_ATTEMPTS_PER_IMAGE,
        }
    }

    fn deadline_reached(&self) -> bool {
        matches!(self.fallback_deadline, Some(deadline) if Instant::now() >= deadline)
    }

    fn remaining_ms(&self) -> Option<f64> {
        let deadline = self.fallback_deadline?;
        let now = Instant::now();
        if now >= deadline {
            Some(0.0)
        } else {
            Some((deadline - now).as_secs_f64() * 1_000.0)
        }
    }

    fn has_time(&self, min_ms: f64) -> bool {
        match self.remaining_ms() {
            Some(ms) => ms >= min_ms,
            None => true,
        }
    }

    fn try_consume_attempt(&mut self) -> bool {
        if self.deadline_reached() || self.attempts_remaining == 0 {
            return false;
        }
        self.attempts_remaining -= 1;
        true
    }
}

#[allow(dead_code)]
pub(crate) fn run(image: &[u8], state: &mut PipelineState, config: &DetectConfig) {
    run_with_deadline(image, state, config, None);
}

pub(crate) fn run_with_deadline(
    image: &[u8],
    state: &mut PipelineState,
    config: &DetectConfig,
    fallback_deadline: Option<Instant>,
) {
    state.decode_candidates.clear();

    let max_candidates = config.max_decode_hypotheses;
    if max_candidates == 0 {
        return;
    }

    let expected_len = state.width.saturating_mul(state.height).saturating_mul(3);
    if image.len() != expected_len {
        return;
    }

    let mut ranked_hypotheses = state.refined_hypotheses.clone();
    ranked_hypotheses.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.id.cmp(&b.id)));

    let grayscale = rgb_to_grayscale(image);
    let mut guard = DecodeGuard::new(fallback_deadline);
    let mut decoded = decode_from_base_view(&grayscale, state.width, state.height, &mut guard);
    if should_run_decode_fallback(
        decoded.len(),
        &ranked_hypotheses,
        &state.proposals,
        state.width,
        state.height,
        max_candidates,
    ) && !guard.deadline_reached()
    {
        let fallback_policy = fallback_policy(state.width, state.height, &guard);
        if fallback_policy.max_hypotheses > 0 && !guard.deadline_reached() {
            decoded.extend(decode_from_hypothesis_crops(
                &grayscale,
                state.width,
                state.height,
                &ranked_hypotheses,
                &state.proposals,
                fallback_policy,
                &mut guard,
            ));
        }
        if !guard.deadline_reached() {
            decoded.extend(decode_from_proposal_crops(
                &grayscale,
                state.width,
                state.height,
                &state.proposals,
                fallback_policy,
                &mut guard,
            ));
        }

        if decoded.is_empty()
            && state.width.saturating_mul(state.height) <= GRID_RESCUE_MAX_PIXELS
            && guard.has_time(RESCUE_MIN_REMAINING_MS_GRID)
        {
            decoded.extend(decode_from_grid_crops(
                &grayscale,
                state.width,
                state.height,
                &mut guard,
            ));
        }
        if decoded.is_empty()
            && state.width.saturating_mul(state.height) <= CHANNEL_RESCUE_MAX_PIXELS
            && guard.has_time(RESCUE_MIN_REMAINING_MS_CHANNEL)
        {
            decoded.extend(decode_from_rgb_channels(
                image,
                state.width,
                state.height,
                &mut guard,
            ));
        }
        if decoded.is_empty()
            && state.width.saturating_mul(state.height) <= UPSCALE_RESCUE_MAX_PIXELS
            && guard.has_time(RESCUE_MIN_REMAINING_MS_UPSCALE)
        {
            decoded.extend(decode_from_upscaled_full_image(
                &grayscale,
                state.width,
                state.height,
                UPSCALE_RESCUE_FACTOR,
                &mut guard,
            ));
        }

        if fallback_policy.allow_full_image_variants
            && guard.has_time(RESCUE_MIN_REMAINING_MS_FULL_VARIANTS)
        {
            let contrast = contrast_stretch_grayscale(&grayscale);
            decoded.extend(decode_from_grayscale_with_guard(
                &contrast,
                state.width,
                state.height,
                &mut guard,
            ));

            if !guard.deadline_reached() {
                let inverted = invert_grayscale(&grayscale);
                decoded.extend(decode_from_grayscale_with_guard(
                    &inverted,
                    state.width,
                    state.height,
                    &mut guard,
                ));
            }
        }
    }

    decoded.sort_by(|a, b| {
        a.payload
            .cmp(&b.payload)
            .then_with(|| corners_cmp(&a.corners, &b.corners))
    });
    decoded.dedup_by(|left, right| {
        left.payload == right.payload
            && corners_cmp(&left.corners, &right.corners) == Ordering::Equal
    });

    let hypothesis_tail = ranked_hypotheses.last().copied();
    let mut candidates = decoded
        .into_iter()
        .take(max_candidates)
        .enumerate()
        .map(|(idx, decoded_qr)| {
            let hypothesis_score = ranked_hypotheses
                .get(idx)
                .copied()
                .or(hypothesis_tail)
                .map(|hypothesis| normalize_score(hypothesis.score))
                .unwrap_or(0.0);
            let score = calibrated_score(hypothesis_score);
            DecodeCandidate {
                score,
                qr: QrCode::new(decoded_qr.payload, score, decoded_qr.corners),
            }
        })
        .collect::<Vec<_>>();

    candidates.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.qr.payload.cmp(&b.qr.payload))
            .then_with(|| corners_cmp(&a.qr.corners, &b.qr.corners))
    });
    candidates.truncate(max_candidates);
    state.decode_candidates = candidates;
}

fn should_run_decode_fallback(
    decoded_count: usize,
    ranked_hypotheses: &[Hypothesis],
    proposals: &[Proposal],
    width: usize,
    height: usize,
    max_candidates: usize,
) -> bool {
    if decoded_count >= max_candidates || decoded_count > FALLBACK_MAX_BASE_DECODES {
        return false;
    }

    if width.saturating_mul(height) > FALLBACK_MAX_PIXELS {
        return false;
    }

    let has_strong_hypothesis = ranked_hypotheses
        .iter()
        .take(4)
        .any(|hypothesis| normalize_score(hypothesis.score) >= FALLBACK_MIN_HYPOTHESIS_SCORE);
    if has_strong_hypothesis {
        return true;
    }

    proposals
        .iter()
        .take(12)
        .any(|proposal| normalize_score(proposal.score) >= FALLBACK_MIN_PROPOSAL_SCORE)
}

fn fallback_policy(width: usize, height: usize, guard: &DecodeGuard) -> FallbackPolicy {
    let pixels = width.saturating_mul(height);
    let remaining_ms = guard.remaining_ms();

    if matches!(remaining_ms, Some(ms) if ms < RESCUE_MIN_REMAINING_MS_FULL_VARIANTS) {
        return FallbackPolicy {
            max_hypotheses: 2,
            max_proposals: 6,
            max_results: 24,
            windows: &LOCAL_DECODE_WINDOW_SIZES_TIGHT,
            allow_full_image_variants: false,
        };
    }

    if pixels > 8_000_000 {
        return FallbackPolicy {
            max_hypotheses: 0,
            max_proposals: 8,
            max_results: 32,
            windows: &LOCAL_DECODE_WINDOW_SIZES_LARGE,
            allow_full_image_variants: false,
        };
    }

    if pixels > FALLBACK_FULL_IMAGE_VARIANT_MAX_PIXELS {
        return FallbackPolicy {
            max_hypotheses: 4,
            max_proposals: 12,
            max_results: 48,
            windows: &LOCAL_DECODE_WINDOW_SIZES_STANDARD,
            allow_full_image_variants: false,
        };
    }

    FallbackPolicy {
        max_hypotheses: LOCAL_DECODE_MAX_HYPOTHESES,
        max_proposals: LOCAL_DECODE_MAX_PROPOSALS,
        max_results: LOCAL_DECODE_MAX_RESULTS,
        windows: &LOCAL_DECODE_WINDOW_SIZES_STANDARD,
        allow_full_image_variants: true,
    }
}

#[derive(Debug)]
struct DecodedQr {
    payload: String,
    corners: [Point; 4],
}

fn decode_code(code: quircs::Code) -> Option<DecodedQr> {
    let decoded = code.decode().ok()?;
    let payload = String::from_utf8_lossy(&decoded.payload)
        .trim_end_matches('\0')
        .to_string();
    if payload.is_empty() {
        return None;
    }

    Some(DecodedQr {
        payload,
        corners: map_corners(code.corners),
    })
}

fn map_corners(input: [quircs::Point; 4]) -> [Point; 4] {
    input.map(|corner| Point {
        x: corner.x as f32,
        y: corner.y as f32,
    })
}

fn decode_from_grayscale(grayscale: &[u8], width: usize, height: usize) -> Vec<DecodedQr> {
    let mut decoder = quircs::Quirc::default();
    decoder
        .identify(width, height, grayscale)
        .filter_map(Result::ok)
        .filter_map(|code| decode_code(code))
        .collect()
}

fn decode_from_grayscale_with_guard(
    grayscale: &[u8],
    width: usize,
    height: usize,
    guard: &mut DecodeGuard,
) -> Vec<DecodedQr> {
    if !guard.try_consume_attempt() {
        return Vec::new();
    }
    decode_from_grayscale(grayscale, width, height)
}

fn base_decode_max_dim_for_budget(remaining_ms: Option<f64>) -> usize {
    match remaining_ms {
        Some(ms) if ms < BASE_DECODE_REMAINING_MS_CRITICAL => BASE_DECODE_MAX_DIM_CRITICAL,
        Some(ms) if ms < BASE_DECODE_REMAINING_MS_TIGHT => BASE_DECODE_MAX_DIM_TIGHT,
        Some(ms) if ms < RESCUE_MIN_REMAINING_MS_GRID => BASE_DECODE_MAX_DIM_MEDIUM,
        _ => BASE_DECODE_MAX_DIM,
    }
}

fn decode_from_base_view(
    grayscale: &[u8],
    width: usize,
    height: usize,
    guard: &mut DecodeGuard,
) -> Vec<DecodedQr> {
    if guard.deadline_reached() {
        return Vec::new();
    }
    let pixels = width.saturating_mul(height);
    let remaining_ms = guard.remaining_ms();
    let force_resize_for_budget = matches!(remaining_ms, Some(ms) if ms < BASE_DECODE_REMAINING_MS_TIGHT)
        && pixels > BASE_DECODE_FORCE_RESIZE_PIXELS_TIGHT;
    if pixels <= BASE_DECODE_MAX_PIXELS && !force_resize_for_budget {
        return decode_from_grayscale_with_guard(grayscale, width, height, guard);
    }

    let max_dim = base_decode_max_dim_for_budget(remaining_ms);

    let Some((scaled, scaled_width, scaled_height)) =
        resize_grayscale_to_max_dim(grayscale, width, height, max_dim)
    else {
        return decode_from_grayscale_with_guard(grayscale, width, height, guard);
    };

    let scale_x = width as f32 / scaled_width as f32;
    let scale_y = height as f32 / scaled_height as f32;
    decode_from_grayscale_with_guard(&scaled, scaled_width, scaled_height, guard)
        .into_iter()
        .map(|mut qr| {
            for corner in &mut qr.corners {
                corner.x *= scale_x;
                corner.y *= scale_y;
            }
            qr
        })
        .collect()
}

fn decode_from_hypothesis_crops(
    grayscale: &[u8],
    width: usize,
    height: usize,
    ranked_hypotheses: &[Hypothesis],
    proposals: &[Proposal],
    policy: FallbackPolicy,
    guard: &mut DecodeGuard,
) -> Vec<DecodedQr> {
    if ranked_hypotheses.is_empty() || proposals.is_empty() {
        return Vec::new();
    }

    let mut decoded = Vec::new();
    for (rank_idx, hypothesis) in ranked_hypotheses
        .iter()
        .take(policy.max_hypotheses)
        .enumerate()
    {
        if guard.deadline_reached() {
            return decoded;
        }
        if normalize_score(hypothesis.score) < LOCAL_DECODE_MIN_HYPOTHESIS_SCORE {
            continue;
        }

        let centers = hypothesis_center_candidates(proposals, hypothesis.id, rank_idx);
        if centers.is_empty() {
            continue;
        }

        for (center_x, center_y) in centers {
            for window_size in policy.windows {
                if guard.deadline_reached() {
                    return decoded;
                }
                let Some((crop, crop_width, crop_height, offset_x, offset_y)) =
                    crop_grayscale_square(
                        grayscale,
                        width,
                        height,
                        center_x,
                        center_y,
                        *window_size,
                    )
                else {
                    continue;
                };
                if crop_width < 24 || crop_height < 24 {
                    continue;
                }

                decoded.extend(
                    decode_from_grayscale_with_guard(&crop, crop_width, crop_height, guard)
                        .into_iter()
                        .map(|mut qr| {
                            for corner in &mut qr.corners {
                                corner.x += offset_x as f32;
                                corner.y += offset_y as f32;
                            }
                            qr
                        }),
                );
                if decoded.len() >= policy.max_results {
                    return decoded;
                }
            }
        }
    }

    decoded
}

fn hypothesis_center_candidates(
    proposals: &[Proposal],
    hypothesis_id: usize,
    rank_idx: usize,
) -> Vec<(usize, usize)> {
    let mut centers = Vec::with_capacity(2);
    if let Some(center) = proposal_center_by_id(proposals, hypothesis_id) {
        centers.push(center);
    }
    if let Some(proposal) = proposals.get(rank_idx) {
        let center = (proposal.x, proposal.y);
        if !centers.contains(&center) {
            centers.push(center);
        }
    }
    centers
}

fn proposal_center_by_id(proposals: &[Proposal], id: usize) -> Option<(usize, usize)> {
    proposals
        .iter()
        .find(|proposal| proposal.id == id)
        .map(|proposal| (proposal.x, proposal.y))
}

fn decode_from_proposal_crops(
    grayscale: &[u8],
    width: usize,
    height: usize,
    proposals: &[Proposal],
    policy: FallbackPolicy,
    guard: &mut DecodeGuard,
) -> Vec<DecodedQr> {
    if proposals.is_empty() {
        return Vec::new();
    }

    let mut ranked = proposals.to_vec();
    ranked.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.view.cmp(&b.view))
            .then_with(|| a.y.cmp(&b.y))
            .then_with(|| a.x.cmp(&b.x))
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut decoded = Vec::new();
    for proposal in ranked.into_iter().take(policy.max_proposals) {
        if guard.deadline_reached() {
            return decoded;
        }
        if normalize_score(proposal.score) < LOCAL_DECODE_MIN_PROPOSAL_SCORE {
            continue;
        }

        for window_size in policy.windows {
            if guard.deadline_reached() {
                return decoded;
            }
            let Some((crop, crop_width, crop_height, offset_x, offset_y)) = crop_grayscale_square(
                grayscale,
                width,
                height,
                proposal.x,
                proposal.y,
                *window_size,
            ) else {
                continue;
            };
            if crop_width < 24 || crop_height < 24 {
                continue;
            }

            decoded.extend(
                decode_from_grayscale_with_guard(&crop, crop_width, crop_height, guard)
                    .into_iter()
                    .map(|mut qr| {
                        for corner in &mut qr.corners {
                            corner.x += offset_x as f32;
                            corner.y += offset_y as f32;
                        }
                        qr
                    }),
            );
            if decoded.len() >= policy.max_results {
                return decoded;
            }
        }
    }

    decoded
}

fn decode_from_grid_crops(
    grayscale: &[u8],
    width: usize,
    height: usize,
    guard: &mut DecodeGuard,
) -> Vec<DecodedQr> {
    if width == 0 || height == 0 {
        return Vec::new();
    }

    let mut decoded = Vec::new();
    for step_y in 0..GRID_RESCUE_STEPS {
        if guard.deadline_reached() {
            return decoded;
        }
        let center_y = evenly_spaced_center(height, step_y, GRID_RESCUE_STEPS);
        for step_x in 0..GRID_RESCUE_STEPS {
            if guard.deadline_reached() {
                return decoded;
            }
            let center_x = evenly_spaced_center(width, step_x, GRID_RESCUE_STEPS);
            for window_size in GRID_RESCUE_WINDOW_SIZES {
                if guard.deadline_reached() {
                    return decoded;
                }
                let Some((crop, crop_width, crop_height, offset_x, offset_y)) =
                    crop_grayscale_square(
                        grayscale,
                        width,
                        height,
                        center_x,
                        center_y,
                        window_size,
                    )
                else {
                    continue;
                };
                if crop_width < 24 || crop_height < 24 {
                    continue;
                }

                decoded.extend(
                    decode_from_grayscale_with_guard(&crop, crop_width, crop_height, guard)
                        .into_iter()
                        .map(|mut qr| {
                            for corner in &mut qr.corners {
                                corner.x += offset_x as f32;
                                corner.y += offset_y as f32;
                            }
                            qr
                        }),
                );
                if decoded.len() >= GRID_RESCUE_MAX_RESULTS {
                    return decoded;
                }
            }
        }
    }

    decoded
}

fn evenly_spaced_center(length: usize, index: usize, steps: usize) -> usize {
    if length == 0 || steps == 0 {
        return 0;
    }
    let numerator = (index.saturating_mul(2))
        .saturating_add(1)
        .saturating_mul(length);
    let denominator = steps.saturating_mul(2);
    (numerator / denominator).min(length.saturating_sub(1))
}

fn decode_from_upscaled_full_image(
    grayscale: &[u8],
    width: usize,
    height: usize,
    factor: usize,
    guard: &mut DecodeGuard,
) -> Vec<DecodedQr> {
    if guard.deadline_reached() {
        return Vec::new();
    }
    let Some((upscaled, upscaled_width, upscaled_height)) =
        upscale_grayscale(grayscale, width, height, factor)
    else {
        return Vec::new();
    };
    let scale = factor.max(1) as f32;
    decode_from_grayscale_with_guard(&upscaled, upscaled_width, upscaled_height, guard)
        .into_iter()
        .map(|mut qr| {
            for corner in &mut qr.corners {
                corner.x /= scale;
                corner.y /= scale;
            }
            qr
        })
        .collect()
}

fn upscale_grayscale(
    grayscale: &[u8],
    width: usize,
    height: usize,
    factor: usize,
) -> Option<(Vec<u8>, usize, usize)> {
    if width == 0 || height == 0 || factor <= 1 {
        return None;
    }

    let out_width = width.saturating_mul(factor);
    let out_height = height.saturating_mul(factor);
    let mut out = vec![0u8; out_width.saturating_mul(out_height)];
    for y in 0..out_height {
        let src_y = y / factor;
        for x in 0..out_width {
            let src_x = x / factor;
            let dst = y.saturating_mul(out_width) + x;
            let src = src_y.saturating_mul(width) + src_x;
            out[dst] = *grayscale.get(src)?;
        }
    }

    Some((out, out_width, out_height))
}

fn crop_grayscale_square(
    grayscale: &[u8],
    width: usize,
    height: usize,
    center_x: usize,
    center_y: usize,
    window_size: usize,
) -> Option<(Vec<u8>, usize, usize, usize, usize)> {
    if width == 0 || height == 0 || window_size == 0 {
        return None;
    }

    let crop_width = window_size.min(width);
    let crop_height = window_size.min(height);
    let max_x0 = width.saturating_sub(crop_width);
    let max_y0 = height.saturating_sub(crop_height);
    let x0 = center_x.saturating_sub(crop_width / 2).min(max_x0);
    let y0 = center_y.saturating_sub(crop_height / 2).min(max_y0);

    let mut crop = Vec::with_capacity(crop_width.saturating_mul(crop_height));
    for row in 0..crop_height {
        let src_y = y0 + row;
        let row_start = src_y.saturating_mul(width) + x0;
        let row_end = row_start + crop_width;
        crop.extend_from_slice(grayscale.get(row_start..row_end)?);
    }

    Some((crop, crop_width, crop_height, x0, y0))
}

fn resize_grayscale_to_max_dim(
    grayscale: &[u8],
    width: usize,
    height: usize,
    max_dim: usize,
) -> Option<(Vec<u8>, usize, usize)> {
    if width == 0 || height == 0 || max_dim == 0 {
        return None;
    }
    if width <= max_dim && height <= max_dim {
        return Some((grayscale.to_vec(), width, height));
    }

    let source_max = width.max(height);
    let out_width = ((width.saturating_mul(max_dim)) / source_max).max(1);
    let out_height = ((height.saturating_mul(max_dim)) / source_max).max(1);
    let mut out = vec![0u8; out_width.saturating_mul(out_height)];

    for y in 0..out_height {
        let src_y = (y.saturating_mul(height)) / out_height;
        for x in 0..out_width {
            let src_x = (x.saturating_mul(width)) / out_width;
            let dst = y.saturating_mul(out_width) + x;
            let src = src_y.saturating_mul(width) + src_x;
            out[dst] = *grayscale.get(src)?;
        }
    }

    Some((out, out_width, out_height))
}

fn corners_cmp(left: &[Point; 4], right: &[Point; 4]) -> Ordering {
    for idx in 0..4 {
        let x_order = left[idx].x.total_cmp(&right[idx].x);
        if x_order != Ordering::Equal {
            return x_order;
        }

        let y_order = left[idx].y.total_cmp(&right[idx].y);
        if y_order != Ordering::Equal {
            return y_order;
        }
    }

    Ordering::Equal
}

fn rgb_to_grayscale(image: &[u8]) -> Vec<u8> {
    image
        .chunks_exact(3)
        .map(|chunk| {
            let r = chunk[0] as u32;
            let g = chunk[1] as u32;
            let b = chunk[2] as u32;
            ((299 * r + 587 * g + 114 * b + 500) / 1000) as u8
        })
        .collect()
}

fn decode_from_rgb_channels(
    image: &[u8],
    width: usize,
    height: usize,
    guard: &mut DecodeGuard,
) -> Vec<DecodedQr> {
    let mut decoded = Vec::new();
    for channel in 0..3usize {
        if guard.deadline_reached() {
            return decoded;
        }
        let plane = rgb_channel_plane(image, channel);
        decoded.extend(decode_from_grayscale_with_guard(
            &plane, width, height, guard,
        ));
    }
    decoded
}

fn rgb_channel_plane(image: &[u8], channel: usize) -> Vec<u8> {
    image
        .chunks_exact(3)
        .map(|chunk| chunk.get(channel).copied().unwrap_or(0))
        .collect()
}

fn contrast_stretch_grayscale(grayscale: &[u8]) -> Vec<u8> {
    let min = grayscale.iter().copied().min().unwrap_or(0);
    let max = grayscale.iter().copied().max().unwrap_or(0);
    if max <= min || max - min < 6 {
        return grayscale.to_vec();
    }

    let range = (max - min) as u32;
    grayscale
        .iter()
        .map(|value| (((value.saturating_sub(min) as u32) * 255) / range) as u8)
        .collect()
}

fn invert_grayscale(grayscale: &[u8]) -> Vec<u8> {
    grayscale
        .iter()
        .map(|value| 255u8.saturating_sub(*value))
        .collect()
}

fn normalize_score(score: f32) -> f32 {
    if score.is_finite() {
        score.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn calibrated_score(base: f32) -> f32 {
    (0.2 + 0.8 * base).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use crate::types::ProposalView;

    use super::{
        FALLBACK_MAX_BASE_DECODES, FALLBACK_MAX_PIXELS, GRID_RESCUE_STEPS, Hypothesis, Proposal,
        contrast_stretch_grayscale, crop_grayscale_square, evenly_spaced_center,
        hypothesis_center_candidates, invert_grayscale, resize_grayscale_to_max_dim,
        rgb_channel_plane, should_run_decode_fallback, upscale_grayscale,
    };

    #[test]
    fn fallback_runs_only_for_low_decode_count_and_strong_hypotheses() {
        let strong = vec![Hypothesis { id: 1, score: 0.9 }];
        assert!(should_run_decode_fallback(
            0,
            &strong,
            &[],
            512,
            512,
            FALLBACK_MAX_BASE_DECODES + 4
        ));

        assert!(!should_run_decode_fallback(
            FALLBACK_MAX_BASE_DECODES + 1,
            &strong,
            &[],
            512,
            512,
            FALLBACK_MAX_BASE_DECODES + 4
        ));

        let weak = vec![Hypothesis { id: 2, score: 0.1 }];
        assert!(!should_run_decode_fallback(
            0,
            &weak,
            &[],
            512,
            512,
            FALLBACK_MAX_BASE_DECODES + 4
        ));

        assert!(!should_run_decode_fallback(
            0,
            &strong,
            &[],
            FALLBACK_MAX_PIXELS + 1,
            1,
            FALLBACK_MAX_BASE_DECODES + 4
        ));
    }

    #[test]
    fn fallback_uses_proposal_signal_when_hypothesis_scores_are_weak() {
        let weak = vec![Hypothesis { id: 2, score: 0.1 }];
        let proposals = vec![Proposal {
            id: 9,
            view: ProposalView::Adaptive,
            x: 50,
            y: 50,
            score: 0.8,
            raw_score: 0.8,
        }];

        assert!(should_run_decode_fallback(
            0,
            &weak,
            &proposals,
            768,
            768,
            FALLBACK_MAX_BASE_DECODES + 4
        ));
    }

    #[test]
    fn grayscale_fallback_views_are_deterministic() {
        let source = vec![10, 20, 30, 40, 50, 60, 90, 110, 200];
        let contrast = contrast_stretch_grayscale(&source);
        let inverted = invert_grayscale(&source);

        assert_eq!(contrast_stretch_grayscale(&source), contrast);
        assert_eq!(invert_grayscale(&source), inverted);
        assert_eq!(inverted.len(), source.len());
    }

    #[test]
    fn crop_square_centers_and_bounds_are_stable() {
        let width = 6usize;
        let height = 5usize;
        let grayscale = (0u8..(width * height) as u8).collect::<Vec<_>>();

        let (crop, crop_w, crop_h, x0, y0) =
            crop_grayscale_square(&grayscale, width, height, 5, 4, 4).expect("crop should exist");
        assert_eq!((crop_w, crop_h), (4, 4));
        assert_eq!((x0, y0), (2, 1));
        assert_eq!(crop.len(), crop_w * crop_h);
        assert_eq!(crop[0], grayscale[y0 * width + x0]);
    }

    #[test]
    fn resize_grayscale_to_max_dim_is_bounded_and_deterministic() {
        let width = 6usize;
        let height = 4usize;
        let grayscale = (0u8..(width * height) as u8).collect::<Vec<_>>();

        let (resized, out_w, out_h) =
            resize_grayscale_to_max_dim(&grayscale, width, height, 3).expect("resize should work");
        assert_eq!((out_w, out_h), (3, 2));
        assert_eq!(resized.len(), out_w * out_h);
        assert_eq!(
            resize_grayscale_to_max_dim(&grayscale, width, height, 3)
                .expect("resize should be deterministic")
                .0,
            resized
        );
    }

    #[test]
    fn hypothesis_centers_include_anchor_id_and_rank_fallback() {
        let proposals = vec![
            Proposal {
                id: 0,
                view: ProposalView::Otsu,
                x: 10,
                y: 20,
                score: 0.8,
                raw_score: 0.8,
            },
            Proposal {
                id: 1,
                view: ProposalView::Adaptive,
                x: 40,
                y: 60,
                score: 0.7,
                raw_score: 0.7,
            },
        ];

        let centers = hypothesis_center_candidates(&proposals, 1, 0);
        assert!(centers.contains(&(40, 60)));
        assert!(centers.contains(&(10, 20)));
    }

    #[test]
    fn evenly_spaced_centers_cover_start_middle_end() {
        let centers = (0..GRID_RESCUE_STEPS)
            .map(|idx| evenly_spaced_center(1200, idx, GRID_RESCUE_STEPS))
            .collect::<Vec<_>>();
        assert_eq!(centers.len(), GRID_RESCUE_STEPS);
        assert!(centers[0] < centers[1] && centers[1] < centers[2]);
        assert!(centers[0] < 300);
        assert!(centers[2] > 800);
    }

    #[test]
    fn upscale_grayscale_is_bounded_and_deterministic() {
        let width = 3usize;
        let height = 2usize;
        let source = vec![1u8, 2, 3, 4, 5, 6];

        let (upscaled, out_w, out_h) =
            upscale_grayscale(&source, width, height, 2).expect("upscale should work");
        assert_eq!((out_w, out_h), (6, 4));
        assert_eq!(upscaled.len(), out_w * out_h);
        assert_eq!(
            upscale_grayscale(&source, width, height, 2)
                .expect("upscale should be deterministic")
                .0,
            upscaled
        );
    }

    #[test]
    fn rgb_channel_plane_extracts_expected_components() {
        let rgb = vec![10u8, 20, 30, 40, 50, 60];
        assert_eq!(rgb_channel_plane(&rgb, 0), vec![10, 40]);
        assert_eq!(rgb_channel_plane(&rgb, 1), vec![20, 50]);
        assert_eq!(rgb_channel_plane(&rgb, 2), vec![30, 60]);
    }
}
