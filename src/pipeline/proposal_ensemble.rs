use std::cmp::Ordering;
use std::time::Instant;

use crate::config::DetectConfig;
use crate::telemetry::{ProposalEnsembleReport, ProposalEnsembleSummary, ProposalViewTelemetry};
use crate::types::{Proposal, ProposalView};

use super::state::PipelineState;

struct BinaryView {
    view: ProposalView,
    bits: Vec<u8>,
}

#[derive(Clone, Copy)]
struct RawCandidate {
    x: usize,
    y: usize,
    raw_score: f32,
    normalized_score: f32,
}

pub(crate) fn run(
    image: &[u8],
    state: &mut PipelineState,
    config: &DetectConfig,
) -> ProposalEnsembleReport {
    let start = Instant::now();
    let gray = rgb_to_grayscale(image, state.width, state.height);

    let mut views = vec![
        BinaryView {
            view: ProposalView::Otsu,
            bits: otsu_binarize(&gray, state.width, state.height),
        },
        BinaryView {
            view: ProposalView::Adaptive,
            bits: adaptive_mean_binarize(&gray, state.width, state.height),
        },
        BinaryView {
            view: ProposalView::Sauvola,
            bits: sauvola_binarize(&gray, state.width, state.height),
        },
    ];

    if config.enable_glare_suppression_view {
        let glare_suppressed = suppress_glare(&gray);
        views.push(BinaryView {
            view: ProposalView::GlareSuppressed,
            bits: otsu_binarize(&glare_suppressed, state.width, state.height),
        });
    }

    let mut per_view = Vec::with_capacity(views.len());
    let mut combined = Vec::new();
    let mut total_raw_candidates = 0usize;
    let mut next_id = 0usize;

    for view in views {
        let mut raw = extract_raw_candidates(&view.bits, state.width, state.height);
        let raw_count = raw.len();
        total_raw_candidates += raw_count;

        let (raw_min, raw_max, normalized_avg) = normalize_candidates(&mut raw);
        for c in raw {
            let weighted = c.normalized_score * view_weight(view.view);
            combined.push(Proposal {
                id: next_id,
                view: view.view,
                x: c.x,
                y: c.y,
                score: weighted,
                raw_score: c.raw_score,
            });
            next_id += 1;
        }

        per_view.push(ProposalViewTelemetry {
            view: view.view,
            raw_candidates: raw_count,
            kept_candidates: 0,
            raw_score_min: raw_min,
            raw_score_max: raw_max,
            normalized_score_avg: normalized_avg,
        });
    }

    normalize_global_scores(&mut combined);
    sort_proposals(&mut combined);
    combined.truncate(config.max_proposals);

    for (new_id, proposal) in combined.iter_mut().enumerate() {
        proposal.id = new_id;
    }

    for row in &mut per_view {
        row.kept_candidates = combined.iter().filter(|p| p.view == row.view).count();
    }

    let top_proposals = combined
        .iter()
        .take(16)
        .map(|p| ProposalEnsembleSummary {
            view: p.view,
            x: p.x,
            y: p.y,
            score: p.score,
        })
        .collect::<Vec<_>>();

    state.proposals = combined;

    let elapsed_ms = start.elapsed().as_secs_f64() * 1_000.0;
    ProposalEnsembleReport {
        elapsed_ms,
        budget_ms: config.proposal_ensemble_budget_ms,
        within_budget: elapsed_ms <= config.proposal_ensemble_budget_ms as f64,
        binary_views_built: per_view.len(),
        total_raw_candidates,
        total_kept_candidates: state.proposals.len(),
        views: per_view,
        top_proposals,
    }
}

fn view_weight(view: ProposalView) -> f32 {
    match view {
        ProposalView::Otsu => 1.00,
        ProposalView::Adaptive => 1.03,
        ProposalView::Sauvola => 1.06,
        ProposalView::GlareSuppressed => 0.98,
    }
}

fn rgb_to_grayscale(image: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height];
    for (idx, chunk) in image.chunks_exact(3).enumerate() {
        let r = chunk[0] as u32;
        let g = chunk[1] as u32;
        let b = chunk[2] as u32;
        let y = (299 * r + 587 * g + 114 * b + 500) / 1000;
        out[idx] = y as u8;
    }
    out
}

fn suppress_glare(gray: &[u8]) -> Vec<u8> {
    let mut out = gray.to_vec();
    for px in &mut out {
        if *px > 220 {
            let over = *px - 220;
            *px = 220 + over / 4;
        }
    }
    out
}

fn otsu_binarize(gray: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut hist = [0usize; 256];
    for &v in gray {
        hist[v as usize] += 1;
    }

    let total = width * height;
    let mut sum_all = 0f64;
    for (t, &count) in hist.iter().enumerate() {
        sum_all += t as f64 * count as f64;
    }

    let mut sum_background = 0f64;
    let mut weight_background = 0usize;
    let mut best_threshold = 127usize;
    let mut best_between = f64::NEG_INFINITY;

    for (t, &count) in hist.iter().enumerate() {
        weight_background += count;
        if weight_background == 0 {
            continue;
        }
        let weight_foreground = total - weight_background;
        if weight_foreground == 0 {
            break;
        }
        sum_background += t as f64 * count as f64;
        let mean_background = sum_background / weight_background as f64;
        let mean_foreground = (sum_all - sum_background) / weight_foreground as f64;
        let between = (weight_background as f64)
            * (weight_foreground as f64)
            * (mean_background - mean_foreground).powi(2);
        if between > best_between {
            best_between = between;
            best_threshold = t;
        }
    }

    gray.iter()
        .map(|&v| if (v as usize) <= best_threshold { 1 } else { 0 })
        .collect()
}

fn adaptive_mean_binarize(gray: &[u8], width: usize, height: usize) -> Vec<u8> {
    let integral = integral_u8(gray, width, height);
    let mut out = vec![0u8; width * height];
    let mut window = (width.min(height) / 20).clamp(9, 41);
    if window % 2 == 0 {
        window += 1;
    }
    let radius = window / 2;

    for y in 0..height {
        let y0 = y.saturating_sub(radius);
        let y1 = (y + radius + 1).min(height);
        for x in 0..width {
            let x0 = x.saturating_sub(radius);
            let x1 = (x + radius + 1).min(width);
            let area = (x1 - x0) * (y1 - y0);
            let sum = rect_sum(&integral, width, x0, y0, x1, y1);
            let mean = sum as f32 / area as f32;
            let threshold = (mean - 5.0).max(0.0);
            let idx = y * width + x;
            out[idx] = if gray[idx] as f32 <= threshold { 1 } else { 0 };
        }
    }

    out
}

fn sauvola_binarize(gray: &[u8], width: usize, height: usize) -> Vec<u8> {
    let integral = integral_u8(gray, width, height);
    let integral_sq = integral_sq_u8(gray, width, height);
    let mut out = vec![0u8; width * height];
    let mut window = (width.min(height) / 16).clamp(11, 51);
    if window % 2 == 0 {
        window += 1;
    }
    let radius = window / 2;
    let k = 0.2f32;
    let r = 128.0f32;

    for y in 0..height {
        let y0 = y.saturating_sub(radius);
        let y1 = (y + radius + 1).min(height);
        for x in 0..width {
            let x0 = x.saturating_sub(radius);
            let x1 = (x + radius + 1).min(width);
            let area = (x1 - x0) * (y1 - y0);
            let sum = rect_sum(&integral, width, x0, y0, x1, y1) as f32;
            let sum_sq = rect_sum(&integral_sq, width, x0, y0, x1, y1) as f32;
            let mean = sum / area as f32;
            let mean_sq = (sum_sq / area as f32).max(mean * mean);
            let variance = (mean_sq - mean * mean).max(0.0);
            let std_dev = variance.sqrt();
            let threshold = mean * (1.0 + k * ((std_dev / r) - 1.0));
            let idx = y * width + x;
            out[idx] = if gray[idx] as f32 <= threshold { 1 } else { 0 };
        }
    }

    out
}

fn integral_u8(input: &[u8], width: usize, height: usize) -> Vec<u64> {
    let mut integral = vec![0u64; (width + 1) * (height + 1)];
    for y in 0..height {
        let mut row_sum = 0u64;
        for x in 0..width {
            row_sum += input[y * width + x] as u64;
            let idx = (y + 1) * (width + 1) + (x + 1);
            integral[idx] = integral[y * (width + 1) + (x + 1)] + row_sum;
        }
    }
    integral
}

fn integral_sq_u8(input: &[u8], width: usize, height: usize) -> Vec<u64> {
    let mut integral = vec![0u64; (width + 1) * (height + 1)];
    for y in 0..height {
        let mut row_sum = 0u64;
        for x in 0..width {
            let v = input[y * width + x] as u64;
            row_sum += v * v;
            let idx = (y + 1) * (width + 1) + (x + 1);
            integral[idx] = integral[y * (width + 1) + (x + 1)] + row_sum;
        }
    }
    integral
}

fn rect_sum(integral: &[u64], width: usize, x0: usize, y0: usize, x1: usize, y1: usize) -> u64 {
    let stride = width + 1;
    let a = integral[y0 * stride + x0];
    let b = integral[y0 * stride + x1];
    let c = integral[y1 * stride + x0];
    let d = integral[y1 * stride + x1];
    d + a - b - c
}

fn extract_raw_candidates(bits: &[u8], width: usize, height: usize) -> Vec<RawCandidate> {
    if width < 5 || height < 5 {
        return Vec::new();
    }

    let cell = (width.min(height) / 18).clamp(8, 32);
    let mut out = Vec::new();

    for gy in (2..height.saturating_sub(2)).step_by(cell) {
        for gx in (2..width.saturating_sub(2)).step_by(cell) {
            let y_end = (gy + cell).min(height.saturating_sub(2));
            let x_end = (gx + cell).min(width.saturating_sub(2));

            let mut best: Option<RawCandidate> = None;
            for y in gy..y_end {
                for x in gx..x_end {
                    let score = local_edge_score(bits, width, x, y);
                    if score < 0.20 {
                        continue;
                    }
                    match best {
                        None => {
                            best = Some(RawCandidate {
                                x,
                                y,
                                raw_score: score,
                                normalized_score: 0.0,
                            })
                        }
                        Some(current) if score > current.raw_score => {
                            best = Some(RawCandidate {
                                x,
                                y,
                                raw_score: score,
                                normalized_score: 0.0,
                            })
                        }
                        _ => {}
                    }
                }
            }
            if let Some(candidate) = best {
                out.push(candidate);
            }
        }
    }

    out
}

fn local_edge_score(bits: &[u8], width: usize, x: usize, y: usize) -> f32 {
    let mut edges = 0usize;
    let mut checks = 0usize;

    for yy in (y - 2)..=(y + 2) {
        for xx in (x - 2)..=(x + 1) {
            let idx = yy * width + xx;
            let idx_right = yy * width + xx + 1;
            edges += usize::from(bits[idx] != bits[idx_right]);
            checks += 1;
        }
    }
    for yy in (y - 2)..=(y + 1) {
        for xx in (x - 2)..=(x + 2) {
            let idx = yy * width + xx;
            let idx_down = (yy + 1) * width + xx;
            edges += usize::from(bits[idx] != bits[idx_down]);
            checks += 1;
        }
    }

    let edge_density = if checks == 0 {
        0.0
    } else {
        edges as f32 / checks as f32
    };

    let mut black = 0usize;
    let mut total = 0usize;
    for yy in (y - 2)..=(y + 2) {
        for xx in (x - 2)..=(x + 2) {
            black += bits[yy * width + xx] as usize;
            total += 1;
        }
    }
    let black_ratio = if total == 0 {
        0.0
    } else {
        black as f32 / total as f32
    };
    let balance = (1.0 - (black_ratio - 0.5).abs() * 2.0).clamp(0.0, 1.0);
    (0.75 * edge_density + 0.25 * balance).clamp(0.0, 1.0)
}

fn normalize_candidates(candidates: &mut [RawCandidate]) -> (f32, f32, f32) {
    if candidates.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let mut min_score = f32::INFINITY;
    let mut max_score = f32::NEG_INFINITY;
    for c in candidates.iter() {
        min_score = min_score.min(c.raw_score);
        max_score = max_score.max(c.raw_score);
    }

    let denom = (max_score - min_score).abs();
    let mut sum_norm = 0.0f32;
    for c in candidates.iter_mut() {
        c.normalized_score = if denom < 1e-6 {
            0.5
        } else {
            ((c.raw_score - min_score) / (max_score - min_score)).clamp(0.0, 1.0)
        };
        sum_norm += c.normalized_score;
    }

    (min_score, max_score, sum_norm / candidates.len() as f32)
}

fn normalize_global_scores(proposals: &mut [Proposal]) {
    if proposals.is_empty() {
        return;
    }
    let mut min_score = f32::INFINITY;
    let mut max_score = f32::NEG_INFINITY;
    for proposal in proposals.iter() {
        min_score = min_score.min(proposal.score);
        max_score = max_score.max(proposal.score);
    }
    let denom = (max_score - min_score).abs();
    for proposal in proposals.iter_mut() {
        proposal.score = if denom < 1e-6 {
            0.5
        } else {
            ((proposal.score - min_score) / (max_score - min_score)).clamp(0.0, 1.0)
        };
    }
}

fn sort_proposals(proposals: &mut [Proposal]) {
    proposals.sort_by(|a, b| {
        let score_order = b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal);
        if score_order != Ordering::Equal {
            return score_order;
        }
        a.view
            .cmp(&b.view)
            .then_with(|| a.y.cmp(&b.y))
            .then_with(|| a.x.cmp(&b.x))
            .then_with(|| a.id.cmp(&b.id))
    });
}
